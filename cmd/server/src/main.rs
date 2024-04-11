use std::net::SocketAddr;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    port: i32,

    #[arg(short, long)]
    backend_addr: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    let local_addr: SocketAddr = format!("0.0.0.0:{}", args.port).parse()?;

    let (join_handle, cancel) = shutdown::setup_shutdown();

    let backend_addr: SocketAddr = args.backend_addr.parse()?;

    forwarder::start_forwarder(local_addr, backend_addr, cancel).await?;

    join_handle.await?;

    Ok(())
}
