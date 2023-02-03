#![warn(rust_2018_idioms)]

use anyhow::{Error, Result};
use std::net::SocketAddr;

use rsq::client::Rsq;
use rsq::messaging::channel::ChannelId;
use rsq::messaging::msg::Msg;
use rsq::messaging::peer::PeerId;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let addr = "127.0.0.1:6142".to_string();
    let addr = addr.parse::<SocketAddr>()?;

    let mut args: Vec<String> = std::env::args().collect();
    let channel_id = if args.len() >= 2 {
        ChannelId(args.remove(1))
    } else {
        ChannelId("test_channel".into())
    };

    println!("connecting...");
    let rsq = Rsq::new(&addr).await;

    println!("sending...");
    rsq.tx
        .send(Msg::new_channel_msg(
            PeerId::new("sender"),
            channel_id.clone(),
            "start".into(),
        ))
        .await
        .unwrap();

    for n in 0..1000000 {
        if n % 10000 == 0 {
            println!("{n}");
        }
        rsq.tx
            .send(Msg::new_channel_msg(
                PeerId::new("sender"),
                channel_id.clone(),
                // 100 bytes
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
            ))
            .await
            .unwrap();
    }

    rsq.tx
        .send(Msg::new_channel_msg(
            PeerId::new("sender"),
            channel_id,
            "stop".into(),
        ))
        .await
        .unwrap();

    println!("finishing...");
    rsq.finish().await?;
    println!("done");

    Ok(())
}
