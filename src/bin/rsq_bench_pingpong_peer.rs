#![warn(rust_2018_idioms)]

use std::net::SocketAddr;

use anyhow::{Error, Result};

use rsq::client::Rsq;
use rsq::messaging::channel::ChannelId;
use rsq::messaging::msg::Msg;
use rsq::messaging::peer::PeerId;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let addr = "127.0.0.1:6142".to_string();
    let addr = addr.parse::<SocketAddr>()?;

    let rsq = Rsq::new(&addr).await;

    let mut args: Vec<String> = std::env::args().collect();
    let channel_id = if args.len() >= 2 {
        ChannelId(args.remove(1))
    } else {
        ChannelId("test_channel".into())
    };
    let msg = Msg::new_channel_msg(PeerId::new("receiver"), channel_id.clone(), "pong".into());
    rsq.tx
        .send_async(Msg::channel_join(channel_id.clone()))
        .await?;

    loop {
        match rsq.rx.recv_async().await {
            Ok(Msg::ChannelMsg(_msg)) => {
                rsq.tx.send_async(msg.clone()).await.unwrap();
            }
            Ok(_) => {}
            _ => break,
        }
    }

    Ok(())
}
