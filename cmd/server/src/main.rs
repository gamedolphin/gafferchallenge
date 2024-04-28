use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Context;
use clap::Parser;
use tokio::{task::JoinHandle, time};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    port: i32,

    #[arg(short, long)]
    backend_addr: String,

    #[arg(short, long)]
    server_count: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .context("failed to setup tracing subscriber")?;

    let local_addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse()?;

    let (join_handle, cancel) = shutdown::setup_shutdown();

    // let backend_addr: SocketAddr = args.backend_addr.parse()?;
    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));

    let joins = (0..args.server_count)
        .map(|_| {
            let cancel_clone = cancel.clone();
            let counter_clone = sent_counter.clone();
            let recv_counter_clone = recv_counter.clone();
            tokio::spawn(async move {
                forwarder::start_forwarder(
                    local_addr,
                    counter_clone,
                    recv_counter_clone,
                    cancel_clone,
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
                tracing::info!("sent {}, received: {}", sent_count, recv_count);
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
