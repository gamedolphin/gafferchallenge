use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use clap::Parser;

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

#[monoio::main(timer_enabled = true, entries = 4294967295, worker_threads = 12)]
async fn main() {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to setup tracing subscriber");

    let local_addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse().unwrap();

    // let backend_addr: SocketAddr = args.backend_addr.parse()?;
    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));

    let (join_handle, cancel, cancel_tag) = shutdown::setup_monoio_shutdown();

    let joins = (0..args.server_count)
        .map(|_| {
            let counter_clone = sent_counter.clone();
            let recv_counter_clone = recv_counter.clone();
            let cancel_channel_clone = cancel_tag.clone();
            let client_cancel = cancel.clone();
            monoio::spawn(async move {
                forwarder::start_forwarder(
                    local_addr,
                    counter_clone,
                    recv_counter_clone,
                    client_cancel,
                    cancel_channel_clone,
                )
                .await
            })
        })
        .collect::<Vec<monoio::task::JoinHandle<anyhow::Result<()>>>>();

    loop {
        monoio::time::sleep(Duration::from_secs(1)).await;
        let sent_count = sent_counter.swap(0, Ordering::Relaxed);
        let recv_count = recv_counter.swap(0, Ordering::Relaxed);
        tracing::info!("sent {}, received: {}", sent_count, recv_count);

        if cancel_tag.load(Ordering::SeqCst) {
            break;
        }
    }

    join_handle.await;

    for join in joins {
        join.await.expect("failed to join threads");
    }
}
