use std::net::SocketAddr;

use anyhow::{Error, Result};

use monoio::{
    io::{sink::Sink, stream::Stream, Splitable},
    net::TcpStream,
};
use monoio_codec::{FramedRead, FramedWrite};

use crate::{messaging::msg::Msg, monoio_bincode::BincodeCodec};

type Rx = flume::Receiver<Msg>;
type Tx = flume::Sender<Msg>;
type OnshotRx = local_sync::oneshot::Receiver<()>;
type OneshotTx = local_sync::oneshot::Sender<()>;

pub struct Rsq {
    pub tx: Tx,
    pub rx: Rx,
    pub done: OnshotRx,
}

impl Rsq {
    pub async fn new(addr: &SocketAddr) -> Rsq {
        let (out_tx, out_rx) = flume::bounded(1000);
        let (in_tx, in_rx) = flume::bounded(1000);
        let (done_tx, done) = local_sync::oneshot::channel();

        monoio::spawn(Self::connect(*addr, in_tx, out_rx, done_tx));

        Rsq {
            tx: out_tx,
            rx: in_rx,
            done,
        }
    }

    pub async fn connect(addr: SocketAddr, rx: Tx, tx: Rx, done: OneshotTx) -> Result<(), Error> {
        use crate::messaging::msg::*;

        rx.send_async(Msg::new_status(StatusMsg::Connecting))
            .await?;

        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        let (stream_in, stream_out) = stream.into_split();
        let mut msgs_in = FramedRead::new(stream_in, BincodeCodec::<Msg>::new());
        let mut msgs_out = FramedWrite::new(stream_out, BincodeCodec::<Msg>::new());

        rx.send_async(Msg::new_status(crate::messaging::msg::StatusMsg::Connected))
            .await?;

        monoio::select! {
            _ = async {
                loop {
                    match tx.recv_async().await {
                        Ok(msg) => {
                            msgs_out.send(msg).await?;
                            if tx.is_empty() {
                                msgs_out.flush().await?;
                            }
                        },
                        // The stream has been exhausted.
                        Err(_) => break,
                    }
                }
                Ok::<(), anyhow::Error>(())
            } => {
            }
            // A message was received from the server.
            _ = async {
                loop {
                    match msgs_in.next().await {
                    // A message was received from the current connection.
                    // pass it on to the application.
                    Some(Ok(msg)) => {
                        rx.send_async(msg).await?;
                    }
                    // An error occurred.
                    Some(Err(e)) => {
                        tracing::error!(
                            "an error occurred while processing messages error = {:?}",
                            e
                        );
                    }
                   // The stream has been exhausted.
                    None => break,
                    }
                }
                Ok::<(), anyhow::Error>(())
            } => {
            }
        }

        // close stream so it flushes
        msgs_out.close().await?;

        rx.send_async(Msg::new_status(
            crate::messaging::msg::StatusMsg::Disconnected,
        ))
        .await?;

        done.send(()).unwrap();

        Ok(())
    }

    pub async fn finish(self) -> Result<(), Error> {
        drop(self.tx);
        self.done.await?;
        Ok(())
    }
}
