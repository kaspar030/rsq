#![warn(rust_2018_idioms)]

use std::env;
use std::error::Error;
use std::net::SocketAddr;
use tokio_serde_cbor::Codec;

use rsq::messaging::msg::Msg;

type MsgCodec = Codec<Msg, Msg>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let peer_name = env::args().nth(1).unwrap();

    let addr = "127.0.0.1:6142".to_string();
    let addr = addr.parse::<SocketAddr>()?;

    tcp::connect(&addr, &peer_name).await?;

    Ok(())
}

mod tcp {
    use futures::{SinkExt, StreamExt};
    use rsq::messaging::channel::ChannelId;
    use rsq::messaging::msg::Msg;
    use rsq::messaging::peer::PeerId;
    use std::{error::Error, net::SocketAddr};
    use tokio::net::TcpStream;
    use tokio_util::codec::{FramedRead, FramedWrite};

    use crate::MsgCodec;

    pub async fn connect(addr: &SocketAddr, peer_name: &String) -> Result<(), Box<dyn Error>> {
        let mut stream = TcpStream::connect(addr).await?;
        let (r, w) = stream.split();
        //let mut sink = FramedWrite::new(w, MsgCodec::new());
        let mut reader = FramedRead::new(r, MsgCodec::new());
        let mut writer = FramedWrite::new(w, MsgCodec::new());

        {
            writer
                .send(Msg::channel_join(ChannelId(peer_name.clone())))
                .await?;
        }

        {
            if peer_name == "foo" {
                let msg = Msg::new_channel_msg(
                    PeerId::new(peer_name),
                    ChannelId("other".into()),
                    "012345678900123456789001234567890012345678900123456789001234567890012345678900123456789001234567890012345678900123456789001234567890"
                        .into(),
                );
                for _ in 0..10_000_000 {
                    writer.send(msg.clone()).await?;
                }
                return Ok(());
            }
        }
        let mut count = 0;
        loop {
            tokio::select! {
                // A message was received from the server.
                result = reader.next() => match result {
                    // A message was received from the current connection.
                    // pass it on to the router.
                    Some(Ok(_msg)) => {
                        count += 1;
                        if count % 1000 == 0 {
                            println!("{}", count);
                        }
                        // if count == 10_000_000 {
                        //     break
                        // }
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

        Ok(())
    }
}
