#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use monoio::IoUringDriver;
use socket2::{Domain, Protocol, Socket, Type};

static BUFFER1: [u8; 100] = [
    43, 20, 193, 151, 203, 27, 136, 87, 216, 82, 131, 147, 1, 55, 252, 8, 148, 181, 244, 139, 13,
    221, 95, 240, 225, 196, 121, 104, 250, 37, 96, 199, 202, 189, 37, 21, 38, 191, 143, 70, 5, 216,
    158, 166, 157, 90, 174, 206, 83, 233, 103, 2, 196, 72, 222, 56, 103, 189, 62, 182, 103, 108,
    249, 243, 6, 149, 13, 197, 50, 69, 99, 55, 38, 165, 163, 23, 13, 200, 12, 98, 26, 128, 194, 47,
    144, 149, 15, 212, 13, 64, 147, 2, 211, 20, 151, 117, 35, 99, 55, 190,
];

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    server_addr: String,

    #[arg(short, long)]
    frequency: u64,

    #[arg(short, long)]
    client_count: usize,

    #[arg(short, long)]
    thread_count: usize,
}

pub async fn start_many_clients() -> anyhow::Result<()> {
    Ok(())
}

fn main() -> anyhow::Result<()> {
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

    let core_count: usize = std::thread::available_parallelism()?.into();
    let count_per_thread = count / args.thread_count;

    let threads = (0..args.thread_count)
        .map(|index| {
            let sent_counter = sent_counter.clone();
            let recv_counter = recv_counter.clone();
            let local_ip = Ipv4Addr::new(0, 0, 0, 0).into();
            std::thread::spawn(move || {
                let current_core = index % core_count;
                monoio::utils::bind_to_cpu_set(Some(current_core)).expect("failed to bind to cpu");
                let mut rt = monoio::RuntimeBuilder::<IoUringDriver>::new()
                    .with_entries(32768)
                    .enable_timer()
                    .build()
                    .expect("failed to start monoio runtime");

                rt.block_on(async move {
                    let joins = (0..count_per_thread)
                        .map(move |index| {
                            let sent_counter = sent_counter.clone();
                            let recv_counter = recv_counter.clone();
                            let local_addr = SocketAddr::new(local_ip, 0).into();
                            monoio::spawn(async move {
                                start_client(
                                    frequency,
                                    local_addr,
                                    server_addr,
                                    sent_counter,
                                    recv_counter,
                                )
                                .await
                            })
                        })
                        .collect::<Vec<monoio::task::JoinHandle<anyhow::Result<()>>>>();

                    for join in joins {
                        join.await?;
                    }

                    Ok(())
                })
            })
        })
        .collect::<Vec<std::thread::JoinHandle<anyhow::Result<()>>>>();

    let mut rt = monoio::RuntimeBuilder::<IoUringDriver>::new()
        .enable_timer()
        .with_entries(32768)
        .build()
        .expect("failed to start monoio runtime");

    rt.block_on(async move {
        let mut interval = monoio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let sent_count = sent_counter.swap(0, Ordering::Relaxed);
            let recv_count = recv_counter.swap(0, Ordering::Relaxed);
            tracing::info!("Sent: {}, Received: {}", sent_count, recv_count);
        }
    });

    for thread in threads {
        thread
            .join()
            .expect("failed to join")
            .expect("failed to finish task");
    }

    Ok(())
}

pub async fn start_receiver(
    local_addr: SocketAddr,
    server_addr: SocketAddr,
    recv_count: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    // socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;

    socket.set_recv_buffer_size(1024 * 1024 * 1024)?;
    socket.set_send_buffer_size(1024 * 1024 * 1024)?;

    if let Err(e) = socket.bind(&local_addr.into()) {
        tracing::info!("failed to bind socket to {local_addr} : {e}");
        return Err(e.into());
    };

    let recv = monoio::net::udp::UdpSocket::from_std(socket.into())?;
    recv.connect(server_addr).await?;

    let mut buf = Vec::with_capacity(8 * 1024);
    let mut res;

    loop {
        (res, buf) = recv.recv(buf).await;
        if res.is_err() {
            continue;
        }

        recv_count.fetch_add(1, Ordering::Relaxed);
    }
}

pub async fn start_client(
    frequency: u64,
    local_addr: SocketAddr,
    server_addr: SocketAddr,
    sent_count: Arc<AtomicU64>,
    recv_count: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    // socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;

    socket.set_recv_buffer_size(1024 * 1024 * 1024)?;
    socket.set_send_buffer_size(1024 * 1024 * 1024)?;

    socket.bind(&local_addr.into())?;

    let sender = monoio::net::udp::UdpSocket::from_std(socket.into())?;

    sender.connect(server_addr).await?;

    let bound_addr = sender.local_addr()?;

    monoio::spawn(start_receiver(bound_addr, server_addr, recv_count));

    let mut interval = monoio::time::interval(Duration::from_millis(1000 / frequency));

    loop {
        interval.tick().await;
        (0..10).for_each(|_| {
            let (res, _) = sender.send(&BUFFER1).await;
            if res.is_err() {
                continue;
            }
        });

        sent_count.fetch_add(10, Ordering::Relaxed);
    }
}
