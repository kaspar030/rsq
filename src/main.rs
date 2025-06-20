//! A message broker in Rust.
//!

use argh::FromArgs;
use bytes::Bytes;
use fdlimit::{raise_fd_limit, Outcome};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use flume::Receiver;
use monoio::io::{
    AsyncWriteRent, AsyncWriteRentExt, BufWriter, OwnedReadHalf, OwnedWriteHalf, Splitable,
};
use monoio::net::{TcpListener, TcpStream};
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

use rsq::messaging::msg::{ControlMsg, Msg};
use rsq::messaging::peer::{Peer, PeerHandle, PeerId};
use rsq::messaging::router::Router;

// Codec
use monoio_codec::FramedWrite;
use rsq::monoio_bincode::BincodeCodec;
use rsq::msg_stream::FrameDecoder;

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

/// rsq server
#[derive(FromArgs, Clone)]
struct Args {
    /// listen address
    #[argh(option, default = "String::from(\"0.0.0.0:6142\")")]
    addr: String,
}

fn main() -> std::result::Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("rsq=info".parse()?))
        .with_span_events(FmtSpan::FULL)
        .init();

    let args: Args = argh::from_env();

    let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
        .with_entries(32768)
        .build()
        .unwrap();

    rt.block_on(server(args))
}

async fn server(args: Args) -> Result<(), Box<dyn Error>> {
    // figure out possible number of connections
    let max_connections = if let Outcome::LimitRaised { from: _, to } = raise_fd_limit()? {
        to - 64
    } else {
        512
    };

    tracing::info!("server listening on {}", args.addr);
    tracing::info!("server accepting up to {} connections", max_connections);

    // Create the shared state.
    //
    // The server task will hold a handle to this. For every new client, the
    // `state` handle is cloned and passed into the task that processes the
    // client connection.
    let state = Rc::new(RefCell::new(Shared::new()));

    // Bind a TCP listener to the socket address.
    let listener = TcpListener::bind(&args.addr)?;

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
type PeerTx = flume::Sender<Bytes>;

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
    fn new(peer_addr: SocketAddr) -> (Receiver<Bytes>, ConnectionPeer) {
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
    //let msgs_in = FramedRead::new(stream_in, BincodeCodec::<Msg>::new());
    let msgs_in = FrameDecoder::new(stream_in);

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
    let to_client_handle = monoio::spawn(to_client(rx, stream_out));

    //
    monoio::select!(
        e = from_client_handle => {
            if let Err(e) = e {
                tracing::error!("{peer_name}: {e}");
            }
        },
        e = to_client_handle => {
            if let Err(e) = e {
                tracing::error!("{peer_name}: {e}");
            }
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
    mut msgs_in: FrameDecoder<OwnedReadHalf<TcpStream>>,
    peer: ConnectionPeer,
) -> Result<(), anyhow::Error> {
    loop {
        match msgs_in.next().await {
            Some(Ok(bytes)) => {
                // tracing::info!(
                //     "4. from_client: len:{} cap:{}",
                //     bytes.len(),
                //     bytes.capacity()
                // );
                //tracing::info!("header_size:{:?}", header_size);
                // Deserialize message header
                let msg_slice = &bytes[4..];
                let (msg, _hdr_len) = bincode::decode_from_slice_with_context::<bool, Msg, _>(
                    msg_slice,
                    bincode::config::standard(),
                    true,
                )
                .unwrap();

                // handle message
                match msg {
                    Msg::ChannelMsg(msg) => {
                        //tracing::info!("msg len: {}", msg.content().len());
                        let mut state = state.borrow_mut();
                        let _count =
                            state
                                .router
                                .forward(bytes.freeze(), msg.channel(), peer.get_id());
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
                };
            }
            Some(Err(e)) => {
                tracing::info!("from_client error: {e}");
                break;
            }
            None => break,
        }
    }

    Ok(())
}

async fn to_client(
    rx: Receiver<Bytes>,
    writer: OwnedWriteHalf<TcpStream>,
) -> Result<(), anyhow::Error> {
    // A message was received for the peer. Send it to the framed TCP
    // stream.
    let mut writer = BufWriter::new(writer);
    loop {
        match rx.recv_async().await {
            Ok(payload) => {
                let (res, _buf) = writer.write_all(payload).await;
                let _ = res?;
                if rx.is_empty() {
                    writer.flush().await?;
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}
