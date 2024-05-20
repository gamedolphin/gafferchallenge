use std::net::{Ipv4Addr, SocketAddr};

use clap::Parser;
use fnv::FnvHasher;
use socket2::{Domain, Protocol, Socket, Type};
use std::hash::Hasher;
use tokio::task::JoinHandle;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to setup tracing subscriber");
    let args = Args::parse();

    let port = args.port;
    let local_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), port);

    let threads = (0..args.server_count)
        .map(|_| tokio::spawn(async move { start_listener(local_addr).await }))
        .collect::<Vec<JoinHandle<anyhow::Result<()>>>>();

    for thread in threads {
        thread.await?;
    }

    Ok(())
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    port: u16,

    #[arg(short, long)]
    server_count: u64,
}

pub async fn start_listener(local_addr: SocketAddr) -> anyhow::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;

    socket.set_recv_buffer_size(1024 * 1024 * 1024);
    socket.set_send_buffer_size(1024 * 1024 * 1024);

    socket.bind(&local_addr.into())?;

    let listener = tokio::net::UdpSocket::from_std(socket.into())?;

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
