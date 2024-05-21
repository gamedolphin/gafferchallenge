#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use std::{
    io::Write,
    net::{Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use clap::Parser;
use fnv::FnvHasher;
use monoio::IoUringDriver;
use socket2::{Domain, Protocol, Socket, Type};
use std::hash::Hasher;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    port: u16,

    #[arg(short, long)]
    server_count: usize,

    #[arg(short, long)]
    thread_count: usize,
}

fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to setup tracing subscriber");
    let args = Args::parse();

    let port = args.port;
    let local_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), port);

    let core_count: usize = std::thread::available_parallelism()?.into();
    let count_per_thread = args.server_count / args.thread_count;

    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));

    let threads = (0..args.thread_count)
        .map(|index| {
            let sent_counter = sent_counter.clone();
            let recv_counter = recv_counter.clone();
            std::thread::spawn(move || {
                let current_core = index % core_count;
                monoio::utils::bind_to_cpu_set(Some(current_core)).expect("failed to bind to cpu");
                let mut rt = monoio::RuntimeBuilder::<IoUringDriver>::new()
                    .with_entries(32768)
                    .build()
                    .expect("failed to start monoio runtime");

                rt.block_on(async move {
                    let joins = (0..count_per_thread)
                        .map(move |_| {
                            let sent_counter = sent_counter.clone();
                            let recv_counter = recv_counter.clone();
                            monoio::spawn(async move {
                                start_listener(local_addr, current_core, recv_counter, sent_counter)
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

pub async fn start_listener(
    local_addr: SocketAddr,
    current_core: usize,
    recv_count: Arc<AtomicU64>,
    send_count: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    // socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;
    socket.set_cpu_affinity(current_core)?;

    socket.set_recv_buffer_size(1024 * 1024 * 1024)?;
    socket.set_send_buffer_size(1024 * 1024 * 1024)?;

    socket.bind(&local_addr.into())?;

    let listener = monoio::net::udp::UdpSocket::from_std(socket.into())?;

    tracing::info!("Server listening on : {}", listener.local_addr()?);

    let mut buf = Vec::with_capacity(100);
    let mut res;

    let mut out_buf = Vec::with_capacity(8);
    let mut out_res;

    loop {
        (res, buf) = listener.recv_from(buf).await;

        let Ok((size, from)) = res else {
            tracing::error!("failed to recv");
            continue;
        };

        recv_count.fetch_add(1, Ordering::Relaxed);

        let hashed = hash_incoming(&buf[0..size]);
        out_buf
            .write_all(&hashed.to_le_bytes())
            .expect("failed to write into vec");

        (out_res, out_buf) = listener.send_to(out_buf, from).await;

        if let Err(e) = out_res {
            tracing::error!("failed to respond: {e}");
            continue;
        }

        send_count.fetch_add(1, Ordering::Relaxed);

        out_buf.clear();
    }
}

fn hash_incoming(body: &[u8]) -> u64 {
    let mut hasher = FnvHasher::default();
    hasher.write(body);
    hasher.finish() // 64 byte
}
