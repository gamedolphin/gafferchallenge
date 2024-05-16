use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use clap::Parser;
use monoio::IoUringDriver;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    port: i32,

    #[arg(short, long)]
    backend_addr: String,

    #[arg(short, long)]
    server_count: u64,

    #[arg(short, long)]
    thread_count: u64,
}

fn main() {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to setup tracing subscriber");

    let local_addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse().unwrap();

    // let backend_addr: SocketAddr = args.backend_addr.parse()?;
    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));

    let cancel_tag = shutdown::setup_monoio_shutdown();

    let count_per_thread = args.server_count / args.thread_count;

    let sent_counter_clone = sent_counter.clone();
    let recv_counter_clone = recv_counter.clone();
    let cancel_tag_clone = cancel_tag.clone();

    let core_count = std::thread::available_parallelism().unwrap().get();

    let joins = (0..args.thread_count)
        .map(move |index| {
            let counter_clone = sent_counter_clone.clone();
            let recv_counter_clone = recv_counter_clone.clone();
            let cancel_tag_clone = cancel_tag_clone.clone();
            std::thread::spawn(move || {
                monoio::utils::bind_to_cpu_set(Some(index as usize % core_count));
                let counter_clone = counter_clone.clone();
                let recv_counter_clone = recv_counter_clone.clone();
                let mut rt = monoio::RuntimeBuilder::<IoUringDriver>::new()
                    .with_entries(32768)
                    .build()
                    .expect("failed to create runtime!");
                rt.block_on(async move {
                    let joins = (0..count_per_thread)
                        .map(move |_| {
                            let counter_clone = counter_clone.clone();
                            let recv_counter_clone = recv_counter_clone.clone();
                            let cancel_tag_clone = cancel_tag_clone.clone();
                            monoio::spawn(async move {
                                forwarder::start_forwarder(
                                    local_addr,
                                    counter_clone,
                                    recv_counter_clone,
                                    cancel_tag_clone,
                                )
                                .await
                            })
                        })
                        .collect::<Vec<monoio::task::JoinHandle<anyhow::Result<()>>>>();

                    for join in joins {
                        join.await?
                    }

                    Ok(())
                })
            })
        })
        .collect::<Vec<std::thread::JoinHandle<anyhow::Result<()>>>>();

    let mut rt = monoio::RuntimeBuilder::<IoUringDriver>::new()
        .with_entries(32768)
        .enable_timer()
        .build()
        .expect("failed to create runtime!");

    rt.block_on(async move {
        let mut ticker = monoio::time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let sent_count = sent_counter.swap(0, Ordering::Relaxed);
            let recv_count = recv_counter.swap(0, Ordering::Relaxed);
            tracing::info!("sent {}, received: {}", sent_count, recv_count);

            if cancel_tag.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    for join in joins {
        join.join()
            .expect("failed to join threads")
            .expect("failed to finish threads");
    }
}
