use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
// use rand::distributions::Alphanumeric;
// use rand::{thread_rng, Rng};

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

static BUFFER1: [u8; 100] = [
    43, 20, 193, 151, 203, 27, 136, 87, 216, 82, 131, 147, 1, 55, 252, 8, 148, 181, 244, 139, 13,
    221, 95, 240, 225, 196, 121, 104, 250, 37, 96, 199, 202, 189, 37, 21, 38, 191, 143, 70, 5, 216,
    158, 166, 157, 90, 174, 206, 83, 233, 103, 2, 196, 72, 222, 56, 103, 189, 62, 182, 103, 108,
    249, 243, 6, 149, 13, 197, 50, 69, 99, 55, 38, 165, 163, 23, 13, 200, 12, 98, 26, 128, 194, 47,
    144, 149, 15, 212, 13, 64, 147, 2, 211, 20, 151, 117, 35, 99, 55, 190,
];
static BUFFER2: [u8; 100] = [
    27, 94, 176, 86, 163, 253, 229, 85, 137, 72, 97, 184, 211, 242, 77, 174, 120, 22, 203, 44, 85,
    92, 116, 82, 41, 6, 103, 67, 176, 239, 54, 251, 228, 123, 150, 32, 178, 8, 41, 229, 183, 91,
    201, 100, 233, 229, 200, 134, 53, 176, 45, 233, 230, 132, 130, 122, 1, 150, 35, 74, 12, 228,
    216, 133, 184, 47, 193, 241, 103, 189, 189, 243, 171, 221, 241, 106, 220, 147, 38, 0, 136, 192,
    146, 7, 156, 214, 2, 0, 66, 23, 176, 150, 191, 216, 23, 166, 243, 58, 206, 166,
];
static BUFFER3: [u8; 100] = [
    30, 242, 84, 106, 32, 165, 69, 14, 65, 140, 213, 143, 130, 25, 117, 106, 192, 142, 18, 206,
    125, 104, 184, 51, 217, 22, 197, 160, 19, 77, 188, 134, 121, 53, 192, 203, 192, 246, 166, 166,
    171, 151, 180, 101, 17, 142, 134, 98, 1, 157, 111, 231, 122, 169, 255, 151, 236, 68, 31, 195,
    30, 202, 232, 12, 2, 82, 107, 203, 172, 38, 94, 70, 16, 86, 240, 86, 44, 66, 98, 152, 23, 11,
    147, 162, 101, 241, 221, 221, 85, 205, 96, 52, 106, 87, 219, 36, 185, 158, 24, 227,
];
static BUFFER4: [u8; 100] = [
    79, 160, 134, 47, 159, 103, 34, 162, 74, 33, 148, 212, 252, 24, 169, 36, 229, 65, 84, 163, 156,
    104, 178, 185, 9, 150, 147, 139, 31, 137, 19, 169, 3, 20, 175, 97, 173, 97, 55, 215, 11, 2,
    120, 114, 41, 70, 89, 132, 17, 134, 199, 135, 110, 80, 105, 208, 203, 230, 28, 143, 36, 229,
    200, 25, 226, 79, 117, 38, 155, 202, 160, 208, 3, 10, 255, 96, 20, 230, 194, 106, 173, 6, 235,
    39, 109, 21, 180, 55, 150, 20, 130, 152, 55, 63, 247, 115, 91, 67, 74, 165,
];
static BUFFERS: [&[u8; 100]; 4] = [&BUFFER1, &BUFFER2, &BUFFER3, &BUFFER4];

#[monoio::main(timer_enabled = true)]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    let cancel = shutdown::setup_monoio_shutdown();

    let local_server_addr: SocketAddr = format!("0.0.0.0:{}", args.server_port).parse()?;
    let server_cancel = cancel.clone();

    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));

    let server_send_counter1 = sent_counter.clone();
    let server_recv_counter1 = recv_counter.clone();
    let server_thread = monoio::spawn(async move {
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
    let server_thread2 = monoio::spawn(async move {
        forwarder::start_forwarder(
            local_server_addr,
            server_send_counter2,
            server_recv_counter2,
            server_cancel2,
        )
        .await
    });

    let local_addr: SocketAddr = "0.0.0.0:0".parse()?;
    let client_cancel = cancel.clone();
    let sent_count = Arc::new(AtomicU64::new(0));
    let recv_count = Arc::new(AtomicU64::new(0));

    let sent_count1 = sent_count.clone();
    let recv_count1 = recv_count.clone();
    let client_thread = monoio::spawn(async move {
        sender::start_sender(
            local_addr,
            local_server_addr,
            args.frequency,
            &BUFFERS,
            sent_count1,
            recv_count1,
            client_cancel,
        )
        .await
    });

    let client_cancel2 = cancel.clone();
    let sent_count2 = sent_count.clone();
    let recv_count2 = recv_count.clone();
    let client_thread2 = monoio::spawn(async move {
        sender::start_sender(
            local_addr,
            local_server_addr,
            args.frequency,
            &BUFFERS,
            sent_count2,
            recv_count2,
            client_cancel2,
        )
        .await
    });

    tracing::info!("started everything!");

    client_thread
        .await
        .context("failed to shutdown client 1 thread ")?;
    client_thread2
        .await
        .context("failed to shutdown client 2 thread ")?;
    server_thread
        .await
        .context("failed to shutdown server 1 thread ")?;
    server_thread2
        .await
        .context("failed to shutdown server 2 thread ")?;
    // backend_thread
    //     .await
    //     .context("failed to shutdown backend 1 thread ")?
    //     .context("failed to shutdown hasher 1")?;
    // backend_thread2
    //     .await
    //     .context("failed to shutdown backend 2 thread ")?
    //     .context("failed to shutdown hasher 2")?;

    Ok(())
}
