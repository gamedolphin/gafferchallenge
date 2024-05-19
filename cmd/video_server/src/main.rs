use std::net::{Ipv4Addr, SocketAddr};

use clap::Parser;
use fnv::FnvHasher;
use std::hash::Hasher;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to setup tracing subscriber");
    let args = Args::parse();

    let port = args.port;
    let local_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), port);

    start_listener(local_addr).await?;

    Ok(())
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    port: u16,
}

pub async fn start_listener(local_addr: SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::UdpSocket::bind(local_addr).await?;

    tracing::info!("Server listening on : {}", listener.local_addr()?);

    let mut buf = Vec::with_capacity(100);

    loop {
        let (size, from) = listener.recv_from(&mut buf).await?;

        let hashed = hash_incoming(&buf[0..size]);

        listener.send_to(&hashed.to_le_bytes(), from).await?;
    }
}

fn hash_incoming(body: &[u8]) -> u64 {
    let mut hasher = FnvHasher::default();
    hasher.write(body);
    hasher.finish() // 64 byte
}
