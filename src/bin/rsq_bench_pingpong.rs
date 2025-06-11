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
    let n = 10000;
    for i in 0..n {
        rsq.tx.send_async(msg.clone()).await.unwrap();
        match rsq.rx.recv_async().await {
            Ok(Msg::ChannelMsg(_msg)) => {
                if i % 10 == 0 {
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
    let ns = elapsed.as_nanos();
    println!("{n} send/reply in {elapsed:.2?}, {}ns/op", ns / n);
    rsq.finish().await?;

    Ok(())
}
