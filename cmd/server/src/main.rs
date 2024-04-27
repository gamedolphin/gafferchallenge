use std::net::SocketAddr;

use anyhow::Context;
use clap::Parser;
use tokio::task::JoinHandle;

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

    let joins = (0..args.server_count)
        .map(|_| {
            let cancel_clone = cancel.clone();
            tokio::spawn(async move { forwarder::start_forwarder(local_addr, cancel_clone).await })
        })
        .collect::<Vec<JoinHandle<anyhow::Result<()>>>>();

    join_handle.await?;

    for join in joins {
        join.await??;
    }

    Ok(())
}
