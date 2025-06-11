use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Error, Result};

use monoio::{
    io::{sink::Sink, stream::Stream},
    net::TcpStream,
};
use monoio_codec::Framed;

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
        //        stream.set_linger(Some(Duration::from_secs(10)))?;

        rx.send_async(Msg::new_status(crate::messaging::msg::StatusMsg::Connected))
            .await?;

        let mut serialized = Framed::new(stream, BincodeCodec::new());

        loop {
            monoio::select! {
                result = tx.recv_async() => match result {
                    Ok(msg) => {
                        serialized.send(msg).await?;
                        if tx.is_empty() {
                            serialized.flush().await?;
                        }
                    }
                    // The stream has been exhausted.
                    Err(_) => break,
                },
                // A message was received from the server.
                result = serialized.next() => match result {
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
                },
            }
        }

        // close stream so it flushes
        serialized.close().await?;

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
