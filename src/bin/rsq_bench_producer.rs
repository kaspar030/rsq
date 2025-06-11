#![warn(rust_2018_idioms)]

use anyhow::{Error, Result};
use std::net::SocketAddr;
use std::time::Duration;

use rsq::client::Rsq;
use rsq::messaging::channel::ChannelId;
use rsq::messaging::msg::Msg;
use rsq::messaging::peer::PeerId;

#[monoio::main(enable_timer = true)]
async fn main() -> Result<(), Error> {
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    // Configure a `tracing` subscriber
    tracing_subscriber::fmt()
        // Filter what traces are displayed based on the RUST_LOG environment
        // variable.
        //
        // Traces emitted by the example code will always be displayed. You
        // can set `RUST_LOG=tokio=trace` to enable additional traces emitted by
        // Tokio itself.
        .with_env_filter(EnvFilter::from_default_env().add_directive("rsq=info".parse()?))
        // Log events when `tracing` spans are created, entered, exited, or
        // closed. When Tokio's internal tracing support is enabled (as
        // described above), this can be used to track the lifecycle of spawned
        // tasks on the Tokio runtime.
        .with_span_events(FmtSpan::FULL)
        // Set this subscriber as the default, to collect all traces emitted by
        // the program.
        .init();

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
        //"aaaaaaa".into(), // 100 bytes
                          //"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
                          // 400 bytes
        //"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
                          // 1024 bytes
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
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
    let bytes = iterations * 1024;
    let elapsed = start.elapsed();
    let msgs_per_sec = iterations as u128 * 1000 as u128 / (elapsed.as_millis() + 1);
    let mb_per_sec = (bytes * 1000 / (elapsed.as_millis() + 1) as usize) / (1024 * 1024);
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
