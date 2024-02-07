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

    let msg = Msg::new_channel_msg(
        PeerId::new("sender"),
        channel_id.clone(),
        // 100 bytes
        //"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
        // 400 bytes
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
    );

    println!("sending...");
    rsq.tx
        .send_async(Msg::new_channel_msg(
            PeerId::new("sender"),
            channel_id.clone(),
            "start".into(),
        ))
        .await
        .unwrap();

    let start = std::time::Instant::now();
    let iterations = 1000000usize;
    for n in 0..iterations {
        if n % 10000 == 0 {
            println!("{n}");
        }
        rsq.tx.send_async(msg.clone()).await.unwrap();
    }
    let bytes = iterations;
    let elapsed = start.elapsed();
    let msgs_per_sec = (iterations as u128 / elapsed.as_millis()) * 1000;
    let mb_per_sec = (bytes / elapsed.as_secs() as usize) / (1024 * 1024);
    println!("{iterations} msgs / {bytes} in {elapsed:?} ({msgs_per_sec}/s, {mb_per_sec}MB/s)");

    rsq.tx
        .send_async(Msg::new_channel_msg(
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
