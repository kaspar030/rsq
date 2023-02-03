//! A message broker in Rust.
//!

use fdlimit::raise_fd_limit;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio_serde_cbor::Codec;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use futures::SinkExt;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rsq::messaging::msg::{ControlMsg, Msg};
use rsq::messaging::peer::{Peer, PeerHandle, PeerId};
use rsq::messaging::router::Router;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

static OPEN_CONNECTIONS: AtomicU64 = AtomicU64::new(0);

//// counting allocator
// use std::alloc::{GlobalAlloc, Layout, System};
// use std::sync::atomic::{AtomicUsize, Ordering};

// struct AllocCounter {
//     count: AtomicUsize,
// }

// unsafe impl GlobalAlloc for AllocCounter {
//     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
//         self.count.fetch_add(layout.size(), Ordering::Relaxed);
//         System.alloc(layout)
//     }

//     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
//         self.count.fetch_sub(layout.size(), Ordering::Relaxed);
//         System.dealloc(ptr, layout)
//     }

//     unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
//         self.count.fetch_add(layout.size(), Ordering::Relaxed);
//         System.alloc_zeroed(layout)
//     }
// }
// use std::time::Duration;

// #[global_allocator]
// static A: AllocCounter = AllocCounter {
//     count: AtomicUsize::new(0),
// };

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    // Configure a `tracing` subscriber
    tracing_subscriber::fmt()
        // Filter what traces are displayed based on the RUST_LOG environment
        // variable.
        //
        // Traces emitted by the example code will always be displayed. You
        // can set `RUST_LOG=tokio=trace` to enable additional traces emitted by
        // Tokio itself.
        .with_env_filter(EnvFilter::from_default_env().add_directive("rsq=info".parse()?))
        // Log events when `tracing` spans are created, entered, exited, or
        // closed. When Tokio's internal tracing support is enabled (as
        // described above), this can be used to track the lifecycle of spawned
        // tasks on the Tokio runtime.
        .with_span_events(FmtSpan::FULL)
        // Set this subscriber as the default, to collect all traces emitted by
        // the program.
        .init();

    // Create the shared state.
    //
    // The server task will hold a handle to this. For every new client, the
    // `state` handle is cloned and passed into the task that processes the
    // client connection.
    let state = Arc::new(Mutex::new(Shared::new()));

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:6142".to_string());

    // figure out possible number of connections
    let max_connections = if let Some(max) = raise_fd_limit() {
        max - 64
    } else {
        512
    };

    // Bind a TCP listener to the socket address.
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!("server running on {}", addr);
    tracing::info!("server accepting up to {} connections", max_connections);

    // std::thread::spawn(|| loop {
    //     std::thread::sleep(Duration::from_secs(1));
    //     println!("Current memory: {}", A.count.load(Ordering::Relaxed));
    // });
    loop {
        while OPEN_CONNECTIONS.load(Ordering::Relaxed) >= max_connections {
            use tokio::time::{sleep, Duration};

            tracing::info!("connection limit ({}) reached", max_connections);
            sleep(Duration::from_secs(1)).await;
        }

        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;

        // Clone a handle to the `Shared` state for the new connection.
        let state = Arc::clone(&state);

        // Spawn our handler to be run asynchronously.
        tokio::spawn(async move {
            tracing::debug!("accepted connection");
            OPEN_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
            if let Err(e) = process(state, stream, addr).await {
                tracing::info!("an error occurred; error = {:?}", e);
            }
            OPEN_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

/// Shorthand for the transmit half of the message channel.
type Tx = mpsc::UnboundedSender<Msg>;

/// Shorthand for the receive half of the message channel.
type Rx = mpsc::UnboundedReceiver<Msg>;

/// Data that is shared between all client connections
struct Shared {
    connections: HashMap<SocketAddr, Tx>,
    router: Router,
}

/// The state for each connected client.
struct Connection {
    /// The TCP socket wrapped with the cbor codec
    msgs: Framed<TcpStream, Codec<Msg, Msg>>,

    /// Receive half of the message channel.
    ///
    /// This is used to receive messages from peers. When a message is received
    /// off of this `Rx`, it will be written to the socket.
    rx: Rx,

    /// Sender message channel.
    ///
    /// Messages sent here will be sent to the Rx half.
    tx: Tx,

    peer_id: PeerId,
}

impl Shared {
    /// Create a new, empty, instance of `Shared`.
    fn new() -> Self {
        Shared {
            connections: HashMap::new(),
            router: Router::new(),
        }
    }
}

impl Connection {
    /// Create a new instance of `Connection`.
    async fn new(
        state: &Arc<Mutex<Shared>>,
        msgs: Framed<TcpStream, Codec<Msg, Msg>>,
    ) -> io::Result<Connection> {
        // Get the client socket address
        let addr = msgs.get_ref().peer_addr()?;

        // Generate peer id. using the socket address for now.
        let peer_id = PeerId::new(&addr.to_string());

        // Create a channel for this peer
        let (tx, rx) = mpsc::unbounded_channel();

        // Add an entry for this `Connection` in the shared state map.
        {
            let mut state = state.lock().await;
            state.connections.insert(addr, tx.clone());
        }

        Ok(Connection {
            msgs,
            tx,
            rx,
            peer_id,
        })
    }
}

impl Peer for Connection {
    fn get_id(&self) -> &PeerId {
        &self.peer_id
    }

    fn get_sink(&self) -> &PeerHandle {
        &self.tx
    }
}

/// Process an individual chat client
async fn process(
    state: Arc<Mutex<Shared>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let msgs = Framed::new(stream, Codec::new());

    // Register our peer with state which internally sets up some channels.
    let mut connection = Connection::new(&state, msgs).await?;

    let peer_name = addr.to_string();
    let msg = format!("new connection from {}", &peer_name);
    tracing::info!("{}", msg);

    // A client has connected
    {
        let mut state = state.lock().await;
        state.router.peer_add(&connection);
    }

    // Process incoming messages until our stream is exhausted by a disconnect.
    loop {
        tokio::select! {
            // A message was received for the peer. Send it to the framed TCP
            // stream.
            Some(msg) = connection.rx.recv() => {
                connection.msgs.send(msg).await.map_err(|e| {
                    connection.rx.close();
                    e
                })?;
            }
            result = connection.msgs.next() => match result {
                // A message was received from the peer's framed TCP stream.
                Some(Ok(msg)) => {
                    match msg {
                        Msg::ChannelMsg(msg) => {
                            let mut state = state.lock().await;
                            state.router.send(msg).unwrap();
                        },
                        Msg::ControlMsg(controlmsg) => {
                            match controlmsg {
                                ControlMsg::ChannelJoin(channel) => {
                                    let mut state = state.lock().await;
                                    state.router.attach(&channel, &connection)?;
                                }
                                ControlMsg::ChannelLeave(channel) => {
                                    let mut state = state.lock().await;
                                    state.router.detach(&channel, &connection)?;
                                }
                            }
                        }
                        _ => {
                            tracing::error!("unhandled message: {:?}", msg);
                        }
                    }
                }
                // An error occurred.
                Some(Err(e)) => {
                    tracing::error!(
                        "an error occurred while processing messages for {}; error = {:?}",
                        &peer_name,
                        e
                    );
                }
                // The stream has been exhausted.
                None => break,
            },
        }
    }

    // If this section is reached it means that the client was disconnected!
    {
        let mut state = state.lock().await;
        state.connections.remove(&addr);
        state.router.peer_remove(&connection);
    }

    let msg = format!("{} disconnected", &peer_name);
    tracing::info!("{}", msg);

    Ok(())
}
