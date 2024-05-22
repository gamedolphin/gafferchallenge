use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use monoio::IoUringDriver;
use tokio::task::JoinHandle;

// use rand::distributions::Alphanumeric;
// use rand::{thread_rng, Rng};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    server_addr: String,

    #[arg(short, long)]
    frequency: u64,

    #[arg(short, long)]
    client_count: u64,

    #[arg(short, long)]
    thread_count: u64,
}

// const fn generate_data() -> [[u8; 100]; 100] {
//     let out = (0..100)
//         .map(|_| {
//             let rand_string: Vec<u8> = thread_rng()
//                 .sample_iter(&Alphanumeric)
//                 .take(100)
//                 .collect::<Vec<u8>>();

//             let rand_string: [u8; 100] = rand_string.try_into().expect("failed");

//             // let rand_string = match rand_string {
//             //     Ok(r) => r,
//             //     Err(_) => panic!("rand string generation failed"),
//             // };

//             rand_string
//         })
//         .collect::<Vec<[u8; 100]>>()
//         .try_into()
//         .unwrap();

//     match out {
//         Ok(o) => return o,
//         Err(_) => panic!("failed"),
//     }
// }

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

fn main() {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)
        .expect("failed to setup tracing subscriber");

    let server_addr: SocketAddr = args
        .server_addr
        .parse()
        .expect("failed to parse server address");

    let local_addr: SocketAddr = if server_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    }
    .parse()
    .expect("failed to parse local address");

    let cancel_tag = shutdown::setup_monoio_shutdown();

    // let buffers = (0..100)
    //     .map(|_| {
    //         let rand_string: Vec<u8> = thread_rng().sample_iter(&Alphanumeric).take(100).collect();

    //         rand_string
    //     })
    //     .collect::<Vec<Vec<u8>>>();

    tracing::info!(
        "Starting client count: {}, connecting to {}, sending with frequency:{}",
        args.client_count,
        server_addr,
        args.frequency
    );

    let sent_counter = Arc::new(AtomicU64::new(0));
    let recv_counter = Arc::new(AtomicU64::new(0));
    let frequency = args.frequency;

    let sent_counter_clone = sent_counter.clone();
    let recv_counter_clone = recv_counter.clone();
    let cancel_tag_clone = cancel_tag.clone();

    let count_per_thread = args.client_count / args.thread_count;

    let core_count = std::thread::available_parallelism().unwrap().get();

    let threads = (0..args.thread_count)
        .map(move |index| {
            let sent_counter_clone = sent_counter_clone.clone();
            let recv_counter_clone = recv_counter_clone.clone();
            let cancel_tag_clone = cancel_tag_clone.clone();

            std::thread::spawn(move || {
                monoio::utils::bind_to_cpu_set(Some(index as usize % core_count));

                let sent_counter_clone = sent_counter_clone.clone();
                let recv_counter_clone = recv_counter_clone.clone();
                let cancel_tag_clone = cancel_tag_clone.clone();
                let mut rt = monoio::RuntimeBuilder::<IoUringDriver>::new()
                    .with_entries(32768)
                    .enable_timer()
                    .build()
                    .expect("failed to create runtime!");

                rt.block_on(async move {
                    let joins = (0..count_per_thread)
                        .map(move |_| {
                            let counter_clone = sent_counter_clone.clone();
                            let recv_counter_clone = recv_counter_clone.clone();
                            let cancel_chan = cancel_tag_clone.clone();
                            monoio::spawn(async move {
                                sender::start_sender(
                                    local_addr,
                                    server_addr,
                                    frequency,
                                    &BUFFERS,
                                    counter_clone,
                                    recv_counter_clone,
                                    cancel_chan,
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
            // 100 bytes sent, 64 bytes returned
            let total_mb = (sent_count * 100 + recv_count * 64) / (1024 * 1024);
            tracing::info!(
                "sent {}, received: {}, total bandwidth: {} mbs/s",
                sent_count,
                recv_count,
                total_mb
            );

            if cancel_tag.load(Ordering::SeqCst) {
                break;
            }
        }
    });

    for task in threads {
        task.join()
            .expect("failed to join all threads")
            .expect("error from joined thread");
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct ClientArgs {
    #[arg(short, long)]
    server_addr: String,

    #[arg(short, long)]
    frequency: u64,

    #[arg(short, long)]
    client_count: u64,
}

pub async fn start_many_clients() -> anyhow::Result<()> {
    let args = ClientArgs::parse();

    let count = args.client_count;
    let frequency = args.frequency;
    let server_addr = args.server_addr.parse()?;

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

    Ok(())
}

pub async fn start_client(
    frequency: u64,
    server_addr: SocketAddr,
    sent_count: Arc<AtomicU64>,
    recv_count: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let local_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 0);
    let sender = tokio::net::UdpSocket::bind(local_addr).await?;

    let mut interval = tokio::time::interval(Duration::from_millis(1000 / frequency));

    let mut buf = Vec::with_capacity(100);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                sender.send_to(&BUFFER1, server_addr).await?;
                sent_count.fetch_add(1, Ordering::Relaxed);
            }

            _ = sender.recv_from(&mut buf) => {
                recv_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
