use anyhow::Context;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time;

use clap::Parser;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    server_addr: String,

    #[arg(short, long)]
    frequency: u64,

    #[arg(short, long)]
    client_count: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .context("failed to setup tracing subscriber")?;

    let server_addr: SocketAddr = args
        .server_addr
        .parse()
        .context("failed to parse server address")?;

    let local_addr: SocketAddr = if server_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    }
    .parse()
    .context("failed to parse local address")?;

    let (join_handle, cancel) = shutdown::setup_shutdown();

    let buffers = (0..100)
        .map(|_| {
            let rand_string: Vec<u8> = thread_rng().sample_iter(&Alphanumeric).take(100).collect();

            rand_string
        })
        .collect::<Vec<Vec<u8>>>();

    tracing::info!(
        "Starting client count: {}, connecting to {}, sending with frequency:{}",
        args.client_count,
        server_addr,
        args.frequency
    );

    let client_count = args.client_count;
    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));

    let joins = (0..client_count)
        .map(|_| {
            let buffers = buffers.clone();
            let client_cancel = cancel.clone();
            let counter_clone = sent_counter.clone();
            let recv_counter_clone = recv_counter.clone();
            tokio::spawn(async move {
                sender::start_sender(
                    local_addr,
                    server_addr,
                    args.frequency,
                    &buffers,
                    counter_clone,
                    recv_counter_clone,
                    client_cancel,
                )
                .await
            })
        })
        .collect::<Vec<JoinHandle<anyhow::Result<()>>>>();

    let mut interval = time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let sent_count = sent_counter.swap(0, Ordering::Relaxed);
                let recv_count = recv_counter.swap(0, Ordering::Relaxed);
                // 100 bytes sent, 64 bytes returned
                let total_mb = (sent_count*100 + recv_count*64)/(1024*1024);
                tracing::info!("sent {}, received: {}, total bandwidth: {} mbs/s", sent_count, recv_count, total_mb);
            }

            _ = cancel.cancelled() => {
                break;
            }
        }
    }

    join_handle.await?;

    for join in joins {
        join.await??;
    }

    Ok(())
}
