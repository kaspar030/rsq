#![warn(rust_2018_idioms)]

use std::error::Error;
use std::mem::swap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use argh::FromArgs;
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

use rsq::client::Rsq;
use rsq::messaging::channel::ChannelId;
use rsq::messaging::msg::Msg;
use rsq::messaging::peer::PeerId;

/// rsq benchmark producer
#[derive(FromArgs, Clone)]
struct Args {
    /// number of producer threads
    #[argh(option, default = "1")]
    threads: usize,

    /// message size
    #[argh(option, default = "100")]
    msg_size: usize,

    /// message count (per thread)
    #[argh(option, default = "10_000_000")]
    msg_count: usize,

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
        .map(|thread| start_producer_thread(thread, args.clone()))
        .collect();

    let mut all_stats = Vec::new();
    for thread in threads {
        match thread.join() {
            Ok(stats) => all_stats.push(stats),
            Err(e) => tracing::error!("thread errored: {e:?}"),
        }
    }

    Stats::average(&all_stats).log("  avg: ");
    Stats::total(&all_stats).log("total: ");

    Ok(())
}

fn start_producer_thread(thread: usize, args: Args) -> std::thread::JoinHandle<Stats> {
    std::thread::spawn(move || {
        let mut rt = monoio::RuntimeBuilder::<monoio::FusionDriver>::new()
            .with_entries(4096)
            .enable_timer()
            .build()
            .unwrap();

        let res = rt.block_on(producer(thread, args));
        res.inspect_err(|e| tracing::error!("thread errored: {e:?}"))
            .unwrap_or_default()
    })
}

async fn producer(thread: usize, args: Args) -> Result<Stats, Box<dyn Error>> {
    tracing::info!("thread {thread}: started");
    let addr = "127.0.0.1:6142".to_string();
    let addr = addr.parse::<SocketAddr>().unwrap();

    let channel_id = ChannelId(args.channel_id.replace("{thread}", &format!("{thread}")));

    tracing::info!("thread {thread}: connecting...");

    let rsq = Rsq::new(&addr).await;

    let msg = Arc::new(Msg::new_channel_msg(
        PeerId::new("sender"),
        channel_id.clone(),
        "a".repeat(args.msg_size).into(),
    ));

    tracing::info!("thread {thread}: starting...");

    rsq.tx
        .send_async(Arc::new(Msg::new_channel_msg(
            PeerId::new("sender"),
            channel_id.clone(),
            "start".into(),
        )))
        .await?;

    let start = std::time::Instant::now();
    let iterations = (args.msg_count / args.threads).max(1);

    let mut stats = Stats::default();
    for n in 0..iterations {
        rsq.tx.send_async(msg.clone()).await?;
        let elapsed = start.elapsed();
        // if elapsed.as_millis() >= 1000 {
        // stats
        //     .update(n * args.msg_size, n, elapsed)
        //     .log(&format!("thread {thread}: "));
        // }
    }
    tracing::info!(".");
    rsq.tx
        .send_async(Arc::new(Msg::new_channel_msg(
            PeerId::new("sender"),
            channel_id,
            "stop".into(),
        )))
        .await
        .unwrap();

    rsq.finish().await?;

    let elapsed = start.elapsed();
    let bytes = iterations * args.msg_size;
    let stats = Stats {
        bytes,
        elapsed,
        msgs: iterations,
    };

    stats.log(&format!("thread {thread}: done. "));

    Ok(stats)
}

#[derive(Default)]
struct Stats {
    bytes: usize,
    msgs: usize,
    elapsed: Duration,
}

impl Stats {
    pub fn new(bytes: usize, msgs: usize, elapsed: Duration) -> Self {
        Self {
            bytes,
            msgs,
            elapsed,
        }
    }

    pub fn msgs_per_sec(&self) -> u64 {
        (self.msgs * 1_000_000) as u64 / self.elapsed.as_micros().max(1) as u64
    }

    pub fn bytes_per_sec(&self) -> u64 {
        (self.bytes * 1_000_000) as u64 / self.elapsed.as_micros().max(1) as u64
    }

    fn sum(vector: &Vec<Stats>) -> Stats {
        let mut res = Stats::default();
        for stat in vector {
            res.bytes += stat.bytes;
            res.msgs += stat.msgs;
            res.elapsed += stat.elapsed;
        }
        res
    }

    pub fn average(vector: &Vec<Stats>) -> Stats {
        let mut res = Self::sum(vector);

        res.bytes /= vector.len();
        res.msgs /= vector.len();
        res.elapsed /= vector.len() as u32;

        res
    }

    pub fn total(vector: &Vec<Stats>) -> Stats {
        let mut res = Self::sum(vector);

        res.elapsed /= vector.len() as u32;

        res
    }

    pub fn update(&mut self, bytes: usize, msgs: usize, elapsed: Duration) -> Stats {
        let mut new = Stats::new(bytes, msgs, elapsed);
        let diff = Stats::new(bytes - self.bytes, msgs - self.msgs, elapsed - self.elapsed);

        swap(self, &mut new);

        diff
    }

    fn log(&self, prefix: &str) {
        let msgs_per_sec = self.msgs_per_sec();
        let mb_per_sec = self.bytes_per_sec() / (1024 * 1024);
        let msgs = self.msgs;
        let bytes = self.bytes;
        let elapsed = self.elapsed;

        tracing::info!(
            "{prefix}{msgs} msgs / {bytes}b in {elapsed:?} ({msgs_per_sec}/s, {mb_per_sec}MB/s)"
        );
    }
}
