use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::task::JoinHandle;
use tracing_subscriber::util::SubscriberInitExt;

static BUFFER1: [u8; 100] = [
    43, 20, 193, 151, 203, 27, 136, 87, 216, 82, 131, 147, 1, 55, 252, 8, 148, 181, 244, 139, 13,
    221, 95, 240, 225, 196, 121, 104, 250, 37, 96, 199, 202, 189, 37, 21, 38, 191, 143, 70, 5, 216,
    158, 166, 157, 90, 174, 206, 83, 233, 103, 2, 196, 72, 222, 56, 103, 189, 62, 182, 103, 108,
    249, 243, 6, 149, 13, 197, 50, 69, 99, 55, 38, 165, 163, 23, 13, 200, 12, 98, 26, 128, 194, 47,
    144, 149, 15, 212, 13, 64, 147, 2, 211, 20, 151, 117, 35, 99, 55, 190,
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to setup tracing subscriber");

    let args = Args::parse();

    let count = args.client_count;
    let frequency = args.frequency;
    let server_addr = args.server_addr.parse()?;

    tracing::info!(
        "Starting client count: {}, connecting to {}, sending with frequency:{}",
        args.client_count,
        server_addr,
        args.frequency
    );

    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));

    let threads = (0..count)
        .map(|_| {
            let sent_counter = sent_counter.clone();
            let recv_counter = recv_counter.clone();
            tokio::spawn(async move {
                start_client(frequency, server_addr, sent_counter, recv_counter).await
            })
        })
        .collect::<Vec<JoinHandle<anyhow::Result<()>>>>();

    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let sent_count = sent_counter.swap(0, Ordering::Relaxed);
        let recv_count = recv_counter.swap(0, Ordering::Relaxed);
        tracing::info!("Sent: {}, Received: {}", sent_count, recv_count);
    }
}

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

pub async fn start_many_clients() -> anyhow::Result<()> {
    Ok(())
}

pub async fn start_client(
    frequency: u64,
    server_addr: SocketAddr,
    sent_count: Arc<AtomicU64>,
    recv_count: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let local_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 0);
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;

    socket.set_recv_buffer_size(1024 * 1024 * 1024);
    socket.set_send_buffer_size(1024 * 1024 * 1024);

    socket.bind(&local_addr.into())?;

    let sender = tokio::net::UdpSocket::from_std(socket.into())?;

    let mut interval = tokio::time::interval(Duration::from_millis(1000 / frequency));

    let mut buf = Vec::with_capacity(100);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                sender.send_to(&BUFFER1, server_addr).await?;
                sent_count.fetch_add(1, Ordering::Relaxed);
            },

            _ = sender.recv_from(&mut buf) => {
                recv_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
