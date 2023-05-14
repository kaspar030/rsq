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

    let mut rsq = Rsq::new(&addr).await;

    let mut args: Vec<String> = std::env::args().collect();
    let channel_id = if args.len() >= 2 {
        ChannelId(args.remove(1))
    } else {
        ChannelId("test_channel".into())
    };

    rsq.tx.send(Msg::channel_join(channel_id.clone())).await?;

    let start = std::time::Instant::now();
    let N = 100000;
    for _ in 0..N {
        rsq.tx
            .send(Msg::new_channel_msg(
                PeerId::new("sender"),
                channel_id.clone(),
                "ping".into(),
            ))
            .await
            .unwrap();
        match rsq.rx.recv().await {
            Some(Msg::ChannelMsg(_msg)) => {}
            Some(_) => {
                println!("other");
            }
            None => break,
        }
    }
    let elapsed = start.elapsed();
    let ns = elapsed.as_nanos();
    println!("{N} send/reply in {elapsed:.2?}, {}ns/op", ns / N);
    rsq.finish().await?;

    Ok(())
}
