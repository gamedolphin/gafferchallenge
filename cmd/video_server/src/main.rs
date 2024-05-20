use std::{
    io::Write,
    net::{Ipv4Addr, SocketAddr},
};

use clap::Parser;
use fnv::FnvHasher;
use monoio::IoUringDriver;
use socket2::{Domain, Protocol, Socket, Type};
use std::hash::Hasher;

fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to setup tracing subscriber");
    let args = Args::parse();

    let port = args.port;
    let local_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), port);

    let core_count: usize = std::thread::available_parallelism()?.into();
    let thread_count = core_count * 2;

    let threads = (0..thread_count)
        .map(|_| {
            std::thread::spawn(move || {
                let mut rt = monoio::RuntimeBuilder::<IoUringDriver>::new()
                    .with_entries(32768)
                    .build()
                    .expect("failed to start monoio runtime");

                rt.block_on(async move {
                    monoio::spawn(async move { start_listener(local_addr).await }).await
                })
            })
        })
        .collect::<Vec<std::thread::JoinHandle<anyhow::Result<()>>>>();

    // let threads = (0..args.server_count)
    //     .map(|_| tokio::spawn(async move { start_listener(local_addr).await }))
    //     .collect::<Vec<JoinHandle<anyhow::Result<()>>>>();

    for thread in threads {
        thread
            .join()
            .expect("failed to join")
            .expect("failed to finish task");
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
            continue;
        };

        let hashed = hash_incoming(&buf[0..size]);
        out_buf
            .write_all(&hashed.to_le_bytes())
            .expect("failed to write into vec");

        (out_res, out_buf) = listener.send_to(out_buf, from).await;

        if let Err(e) = out_res {
            tracing::error!("failed to respond: {e}");
        }

        out_buf.clear();
    }
}

fn hash_incoming(body: &[u8]) -> u64 {
    let mut hasher = FnvHasher::default();
    hasher.write(body);
    hasher.finish() // 64 byte
}
