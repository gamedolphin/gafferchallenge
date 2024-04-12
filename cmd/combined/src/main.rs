use std::net::SocketAddr;

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

    let local_server_addr: SocketAddr = format!("0.0.0.0:{}", args.server_port).parse()?;
    let server_cancel = cancel.clone();
    let server_thread = tokio::spawn(async move {
        forwarder::start_forwarder(local_server_addr, local_backend_addr, server_cancel).await
    });

    let buffers = (0..100)
        .map(|_| {
            let rand_string: Vec<u8> = thread_rng().sample_iter(&Alphanumeric).take(100).collect();

            rand_string
        })
        .collect::<Vec<Vec<u8>>>();

    let local_addr: SocketAddr = "0.0.0.0:0".parse()?;
    let client_cancel = cancel.clone();
    let client_thread = tokio::spawn(async move {
        sender::start_sender(
            local_addr,
            local_server_addr,
            args.frequency,
            buffers,
            client_cancel,
        )
        .await
    });

    join_handle.await?;
    server_thread.await??;
    client_thread.await??;
    backend_thread.await??;

    Ok(())
}
