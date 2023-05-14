use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Error, Result};

use futures::SinkExt;
use tokio::net::TcpStream;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use crate::messaging::msg::Msg;

type Rx = tokio::sync::mpsc::Receiver<Msg>;
type Tx = tokio::sync::mpsc::Sender<Msg>;
type OnshotRx = tokio::sync::oneshot::Receiver<()>;
type OneshotTx = tokio::sync::oneshot::Sender<()>;

type MsgCodec = tokio_serde::formats::SymmetricalBincode<Msg>;

pub struct Rsq {
    pub tx: Tx,
    pub rx: Rx,
    pub done: OnshotRx,
}

impl Rsq {
    pub async fn new(addr: &SocketAddr) -> Rsq {
        let (out_tx, out_rx) = tokio::sync::mpsc::channel(1000);
        let (in_tx, in_rx) = tokio::sync::mpsc::channel(1000);
        let (done_tx, done) = tokio::sync::oneshot::channel();

        tokio::spawn(Self::connect(*addr, in_tx, out_rx, done_tx));

        Rsq {
            tx: out_tx,
            rx: in_rx,
            done,
        }
    }

    pub async fn connect(addr: SocketAddr, rx: Tx, tx: Rx, done: OneshotTx) -> Result<(), Error> {
        use crate::messaging::msg::*;

        rx.send(Msg::new_status(StatusMsg::Connecting)).await?;

        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;
        stream.set_linger(Some(Duration::from_secs(10)))?;

        rx.send(Msg::new_status(crate::messaging::msg::StatusMsg::Connected))
            .await?;

        let length_delimited = Framed::new(stream, tokio_util::codec::LengthDelimitedCodec::new());
        let mut serialized =
            tokio_serde::SymmetricallyFramed::new(length_delimited, MsgCodec::default());

        let mut tx = ReceiverStream::new(tx);

        loop {
            tokio::select! {
                // A message was received from the server.
                result = serialized.next() => match result {
                    // A message was received from the current connection.
                    // pass it on to the application.
                    Some(Ok(msg)) => {
                        rx.send(msg).await?;
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
                result = tx.next() => match result {
                    Some(msg) => {
                        serialized.send(msg).await?;
                    }
                    // The stream has been exhausted.
                    None => break,
                }
            }
        }

        rx.send(Msg::new_status(
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
