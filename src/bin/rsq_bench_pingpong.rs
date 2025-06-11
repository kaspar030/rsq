#![warn(rust_2018_idioms)]

use std::net::SocketAddr;

use anyhow::{Error, Result};

use rsq::client::Rsq;
use rsq::messaging::channel::ChannelId;
use rsq::messaging::msg::Msg;
use rsq::messaging::peer::PeerId;

#[monoio::main(enable_timer = true)]
async fn main() -> Result<(), Error> {
    let addr = "127.0.0.1:6142".to_string();
    let addr = addr.parse::<SocketAddr>()?;

    let rsq = Rsq::new(&addr).await;

    let mut args: Vec<String> = std::env::args().collect();
    let channel_id = if args.len() >= 2 {
        ChannelId(args.remove(1))
    } else {
        ChannelId("pingpong_test_channel".into())
    };

    rsq.tx
        .send_async(Msg::channel_join(channel_id.clone()))
        .await?;

    let _ = rsq.rx.recv_async().await;
    let _ = rsq.rx.recv_async().await;

    let msg = Msg::new_channel_msg(PeerId::new("sender"), channel_id.clone(), 
        "pingaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
    let start = std::time::Instant::now();
    let n = 100000;
    for i in 0..n {
        rsq.tx.send_async(msg.clone()).await.unwrap();
        match rsq.rx.recv_async().await {
            Ok(Msg::ChannelMsg(_msg)) => {
                if i % 1000 == 0 {
                    println!("{i}");
                }
            }
            Ok(msg) => {
                println!("other {msg:#?}");
            }
            _ => break,
        }
    }
    let elapsed = start.elapsed();
    let us = elapsed.as_micros();
    println!(
        "{n} send/reply in {elapsed:.2?}, {}/s, {}us/op",
        n * 1000 / elapsed.as_millis() + 1,
        us / n as u128
    );
    rsq.finish().await?;

    Ok(())
}
