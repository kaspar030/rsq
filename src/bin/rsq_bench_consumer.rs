#![warn(rust_2018_idioms)]

use std::net::SocketAddr;

use anyhow::{Error, Result};

use rsq::client::Rsq;
use rsq::messaging::channel::ChannelId;
use rsq::messaging::msg::Msg;

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

    rsq.tx.send(Msg::channel_join(channel_id)).await?;

    let mut i = 0;
    let mut bytes = 0usize;
    let mut start = std::time::Instant::now();
    loop {
        match rsq.rx.recv().await {
            Some(Msg::ChannelMsg(msg)) => {
                let msg_content = msg.content();
                if msg_content.len() <= 5 {
                    let msg_str = std::str::from_utf8(msg.content()).unwrap();
                    match msg_str {
                        "start" => {
                            i = 0;
                            bytes = 0;
                            start = std::time::Instant::now();
                        }
                        "stop" => {
                            let elapsed = start.elapsed();
                            let msgs_per_sec = (i / elapsed.as_millis()) * 1000;
                            let mb_per_sec = (bytes / elapsed.as_secs() as usize) / (1024 * 1024);
                            println!("{i} msgs / {bytes} in {elapsed:?} ({msgs_per_sec}/s, {mb_per_sec}MB/s)");
                        }
                        _ => println!("unknown msg {msg_str}"),
                    }
                } else {
                    i += 1;
                    bytes += msg_content.len();
                }

                if i % 10000 == 0 {
                    println!("i={i}");
                }
            }
            Some(_) => {}
            None => break,
        }
    }

    Ok(())
}
