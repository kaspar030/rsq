use std::{net::SocketAddr, sync::Arc};

use rsq::client::Rsq;
use rsq::messaging::channel::ChannelId;
use rsq::messaging::msg::Msg;

use argh::FromArgs;
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

/// rsq benchmark consumer
#[derive(FromArgs, Clone)]
struct Args {
    /// number of consumer threads
    #[argh(option, default = "1")]
    threads: usize,

    /// channel_id
    #[argh(option, default = "String::from(\"test_channel\")")]
    channel_id: String,
}

fn main() -> std::io::Result<()> {
    // Configure a `tracing` subscriber
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("rsq=info".parse().unwrap()))
        .with_span_events(FmtSpan::FULL)
        .init();

    let args: Args = argh::from_env();

    let thread_count = args.threads;

    let threads: Vec<_> = (0..thread_count)
        .map(|thread| start_consumer_thread(thread, args.clone()))
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    Ok(())
}

fn start_consumer_thread(thread: usize, args: Args) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
            .with_entries(32768)
            .build()
            .unwrap();

        rt.block_on(consumer(thread, args));
    })
}

async fn consumer(thread: usize, args: Args) {
    tracing::info!("thread {thread}: started");
    let addr = "127.0.0.1:6142".to_string();
    let addr = addr.parse::<SocketAddr>().unwrap();

    let channel_id = ChannelId(args.channel_id.replace("{thread}", &format!("{thread}")));

    tracing::info!("thread {thread}: connecting...");

    let rsq = Rsq::new(&addr).await;

    rsq.tx
        .send_async(Arc::new(Msg::channel_join(channel_id)))
        .await
        .unwrap();

    let mut i = 0;
    let mut bytes = 0usize;
    let mut start = std::time::Instant::now();
    let mut expected = 0;
    loop {
        match rsq.rx.recv_async().await {
            Ok(msg) => match &*msg {
                Msg::ChannelMsg(msg) => {
                    let msg_content = msg.content();
                    if msg_content.len() <= 5 {
                        let msg_str = std::str::from_utf8(msg.content()).unwrap();
                        match msg_str {
                            "start" => {
                                if expected == 0 {
                                    i = 0;
                                    bytes = 0;
                                    start = std::time::Instant::now();
                                }
                                expected += 1;
                            }
                            "stop" => {
                                expected -= 1;
                                if expected == 0 {
                                    let elapsed = start.elapsed();
                                    let msgs_per_sec = i * 1000000 / (elapsed.as_micros() + 1);
                                    let mb_per_sec = (bytes * 1000000
                                        / (elapsed.as_micros() as usize + 1))
                                        / (1024 * 1024);
                                    tracing::info!("thread {thread}: {i} msgs / {bytes} in {elapsed:?} ({msgs_per_sec}/s, {mb_per_sec}MB/s)");
                                }
                            }
                            _ => println!("unknown msg {msg_str}"),
                        }
                    } else {
                        i += 1;
                        bytes += msg_content.len();
                    }

                    if i % 100000 == 0 && i > 0 {
                        tracing::info!("thread {thread}: i={i}");
                    }
                }
                _ => {}
            },
            Err(_) => break,
        }
    }
}
