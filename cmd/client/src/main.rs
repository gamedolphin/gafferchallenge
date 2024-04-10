use std::net::SocketAddr;

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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    let server_addr: SocketAddr = args.server_addr.parse()?;

    let local_addr: SocketAddr = if server_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    }
    .parse()?;

    let (join_handle, cancel) = shutdown::setup_shutdown();

    let buffers = (0..100)
        .map(|_| {
            let rand_string: Vec<u8> = thread_rng().sample_iter(&Alphanumeric).take(100).collect();

            rand_string
        })
        .collect::<Vec<Vec<u8>>>();

    tracing::info!(
        "Starting client, connecting to {}, sending with frequency:{}",
        args.server_addr,
        args.frequency
    );

    sender::start_sender(local_addr, server_addr, args.frequency, buffers, cancel).await?;

    join_handle.await?;

    Ok(())
}
