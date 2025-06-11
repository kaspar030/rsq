//! A message broker in Rust.
//!

use fdlimit::{raise_fd_limit, Outcome};

use flume::Receiver;
use monoio::io::sink::Sink;
use monoio::io::stream::Stream;
use monoio::io::{OwnedReadHalf, OwnedWriteHalf, Splitable};
use monoio::net::{TcpListener, TcpStream};

use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Context;

use rsq::messaging::msg::{ControlMsg, Msg};
use rsq::messaging::peer::{Peer, PeerHandle, PeerId};
use rsq::messaging::router::Router;

// Codec
use monoio_codec::{Framed, FramedRead, FramedWrite};
use rsq::monoio_bincode::BincodeCodec;

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

#[monoio::main(enable_timer = true)]
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
    let state = Rc::new(RefCell::new(Shared::new()));

    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:6142".to_string());

    // figure out possible number of connections
    let max_connections = if let Outcome::LimitRaised { from: _, to } = raise_fd_limit()? {
        to - 64
    } else {
        512
    };

    // Bind a TCP listener to the socket address.
    let listener = TcpListener::bind(&addr)?;

    tracing::info!("server running on {}", addr);
    tracing::info!("server accepting up to {} connections", max_connections);

    // std::thread::spawn(|| loop {
    //     std::thread::sleep(Duration::from_secs(1));
    //     println!("Current memory: {}", A.count.load(Ordering::Relaxed));
    // });
    loop {
        while OPEN_CONNECTIONS.load(Ordering::Relaxed) >= max_connections {
            use monoio::time::{sleep, Duration};

            tracing::info!("connection limit ({}) reached", max_connections);
            sleep(Duration::from_secs(1)).await;
        }

        // Asynchronously wait for an inbound TcpStream.
        let (stream, addr) = listener.accept().await?;
        stream.set_nodelay(true)?;

        // Clone a handle to the `Shared` state for the new connection.
        let state = Rc::clone(&state);

        // Spawn our handler to be run asynchronously.
        monoio::spawn(async move {
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
type PeerTx = flume::Sender<Arc<Msg>>;

/// Data that is shared between all client connections
struct Shared {
    connections: HashMap<SocketAddr, PeerTx>,
    router: Router,
}

/// `Peer` handle for TCP connections.
struct ConnectionPeer {
    /// Sender message channel.
    ///
    /// Messages sent here will be sent to the Rx half.
    tx: PeerTx,

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

impl ConnectionPeer {
    /// Create a new instance of `Connection`.
    fn new(peer_addr: SocketAddr) -> (Receiver<Arc<Msg>>, ConnectionPeer) {
        // Generate peer id. using the socket address for now.
        let peer_id = PeerId::new(&peer_addr.to_string());

        // Create a channel for this peer
        let (tx, rx) = flume::unbounded();

        (rx, ConnectionPeer { tx, peer_id })
    }
}

impl Peer for ConnectionPeer {
    fn get_id(&self) -> &PeerId {
        &self.peer_id
    }

    fn get_sink(&self) -> &PeerHandle {
        &self.tx
    }
}

/// Process an individual chat client
async fn process(
    state: Rc<RefCell<Shared>>,
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let peer_addr = stream.peer_addr()?;
    let (stream_in, stream_out) = stream.into_split();
    let msgs_in = FramedRead::new(stream_in, BincodeCodec::<Msg>::new());
    let msgs_out = FramedWrite::new(stream_out, BincodeCodec::<Msg>::new());

    // Register our peer with state which internally sets up some channels.
    let (rx, peer) = ConnectionPeer::new(peer_addr);

    let peer_name = addr.to_string();
    let msg = format!("new connection from {}", &peer_name);
    tracing::info!("{}", msg);

    // A client has connected
    {
        let mut state = state.borrow_mut();
        state.connections.insert(peer_addr, peer.tx.clone());
        state.router.peer_add(&peer);
    }

    let peer_id = peer.get_id().clone();

    let from_client_handle = monoio::spawn(from_client(state.clone(), msgs_in, peer));
    let to_client_handle = monoio::spawn(to_client(rx, msgs_out));

    //
    monoio::select!(
        _ = from_client_handle => {
        },
        _ = to_client_handle => {
        }
    );

    // If this section is reached it means that the client was disconnected!
    {
        let mut state = state.borrow_mut();
        state.connections.remove(&addr);
        state.router.peer_remove(&peer_id);
    }

    tracing::info!("{peer_name} disconnected");

    Ok(())
}

async fn from_client(
    state: Rc<RefCell<Shared>>,
    mut msgs_in: FramedRead<OwnedReadHalf<TcpStream>, BincodeCodec<Msg>>,
    peer: ConnectionPeer,
) -> Result<(), anyhow::Error> {
    loop {
        match msgs_in.next().await {
            // A message was received from the peer's framed TCP stream.
            Some(Ok(msg)) => match msg {
                Msg::ChannelMsg(_) => {
                    let msg = Arc::new(msg);
                    let mut state = state.borrow_mut();
                    let _count = state.router.forward(msg, peer.get_id());
                }
                Msg::ControlMsg(controlmsg) => match controlmsg {
                    ControlMsg::ChannelJoin(channel) => {
                        let mut state = state.borrow_mut();
                        state.router.attach(&channel, &peer)?;
                    }
                    ControlMsg::ChannelLeave(channel) => {
                        let mut state = state.borrow_mut();
                        state.router.detach(&channel, &peer)?;
                    }
                },
                _ => {
                    tracing::error!("unhandled message: {:?}", msg);
                }
            },
            // An error occurred.
            Some(Err(e)) => {
                tracing::error!(
                    "an error occurred while processing messages for {:?}; error = {:?}",
                    peer.get_id(),
                    e
                );
                return Err(e.into());
            }
            // The stream has been exhausted.
            None => break,
        }
    }

    Ok(())
}

async fn to_client(
    rx: Receiver<Arc<Msg>>,
    mut msgs_out: FramedWrite<OwnedWriteHalf<TcpStream>, BincodeCodec<Msg>>,
) -> Result<(), anyhow::Error> {
    // A message was received for the peer. Send it to the framed TCP
    // stream.
    loop {
        match rx.recv_async().await {
            Ok(msg) => {
                let msg = (*msg).clone();
                msgs_out
                    .send(msg)
                    .await
                    .inspect_err(|_e| {
                        //connection.rx.close().await;
                    })
                    .context("while sending a message")?;
                if rx.is_empty() {
                    msgs_out.flush().await?;
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}
