use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    backend_port: i32,

    #[arg(short, long)]
    server_port: i32,

    #[arg(short, long)]
    frequency: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    let (join_handle, cancel) = shutdown::setup_shutdown();

    let local_backend_addr: SocketAddr = format!("0.0.0.0:{}", args.backend_port).parse()?;
    let backend_cancel = cancel.clone();
    let backend_thread =
        tokio::spawn(
            async move { hasher::start_backend(local_backend_addr, backend_cancel).await },
        );

    let backend_cancel2 = cancel.clone();
    let backend_thread2 =
        tokio::spawn(
            async move { hasher::start_backend(local_backend_addr, backend_cancel2).await },
        );

    let local_server_addr: SocketAddr = format!("0.0.0.0:{}", args.server_port).parse()?;
    let server_cancel = cancel.clone();

    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));

    let server_send_counter1 = sent_counter.clone();
    let server_recv_counter1 = recv_counter.clone();
    let server_thread = tokio::spawn(async move {
        forwarder::start_forwarder(
            local_server_addr,
            server_send_counter1,
            server_recv_counter1,
            server_cancel,
        )
        .await
    });

    let server_cancel2 = cancel.clone();
    let server_send_counter2 = sent_counter.clone();
    let server_recv_counter2 = recv_counter.clone();
    let server_thread2 = tokio::spawn(async move {
        forwarder::start_forwarder(
            local_server_addr,
            server_send_counter2,
            server_recv_counter2,
            server_cancel2,
        )
        .await
    });

    let buffers = (0..100)
        .map(|_| {
            let rand_string: Vec<u8> = thread_rng().sample_iter(&Alphanumeric).take(100).collect();

            rand_string
        })
        .collect::<Vec<Vec<u8>>>();

    let local_addr: SocketAddr = "0.0.0.0:0".parse()?;
    let client_cancel = cancel.clone();
    let sent_count = Arc::new(AtomicU64::new(0));
    let recv_count = Arc::new(AtomicU64::new(0));
    let buffer1 = buffers.clone();

    let sent_count1 = sent_count.clone();
    let recv_count1 = recv_count.clone();
    let client_thread = tokio::spawn(async move {
        sender::start_sender(
            local_addr,
            local_server_addr,
            args.frequency,
            &buffer1,
            sent_count1,
            recv_count1,
            client_cancel,
        )
        .await
    });

    let client_cancel2 = cancel.clone();
    let sent_count2 = sent_count.clone();
    let recv_count2 = recv_count.clone();
    let client_thread2 = tokio::spawn(async move {
        sender::start_sender(
            local_addr,
            local_server_addr,
            args.frequency,
            &buffers,
            sent_count2,
            recv_count2,
            client_cancel2,
        )
        .await
    });

    join_handle
        .await
        .context("failed to shutdown the shutdown watcher cleanly")?;
    client_thread
        .await
        .context("failed to shutdown client 1 thread ")?
        .context("failed to shutdown sender 1")?;
    client_thread2
        .await
        .context("failed to shutdown client 2 thread ")?
        .context("failed to shutdown sender 2")?;
    server_thread
        .await
        .context("failed to shutdown server 1 thread ")?
        .context("failed to shutdown forwarder 1")?;
    server_thread2
        .await
        .context("failed to shutdown server 2 thread ")?
        .context("failed to shutdown forwarder 2")?;
    backend_thread
        .await
        .context("failed to shutdown backend 1 thread ")?
        .context("failed to shutdown hasher 1")?;
    backend_thread2
        .await
        .context("failed to shutdown backend 2 thread ")?
        .context("failed to shutdown hasher 2")?;

    Ok(())
}
