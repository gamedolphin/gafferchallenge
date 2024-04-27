use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{net::UdpSocket, time};
use tokio_util::sync::CancellationToken;

pub async fn start_sender(
    local_addr: SocketAddr,
    server_addr: SocketAddr,
    frequency: u64,
    buffers: &[Vec<u8>],
    sent_counter: Arc<AtomicU64>,
    recv_counter: Arc<AtomicU64>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(local_addr).await?;

    socket.connect(&server_addr).await?;

    let mut interval = time::interval(Duration::from_millis(1000 / frequency));

    let mut index = 0;

    let mut buf = [0_u8; 100];

    loop {
        tokio::select! {
            _ = interval.tick() => {
                if index >= buffers.len() {
                    index = 0;
                }

                let buf = &buffers[index];
                tracing::debug!("sending {}", std::str::from_utf8(buf).unwrap());
                if let Err(e) = socket.send(buf).await {
                    tracing::debug!("failed to send {}", e);
                    continue;
                }

                index += 1;

                sent_counter.fetch_add(1, Ordering::Relaxed);

            }
            n = socket.recv_from(&mut buf) => {
                let Ok((n, _)) = n else {
                    continue;
                };

                recv_counter.fetch_add(1, Ordering::Relaxed);

                tracing::debug!("received hash: {}", std::str::from_utf8(&buf[0..n]).unwrap());
            }

            _ = cancel.cancelled() => {
                tracing::debug!("shutting down sender");
                return Ok(())
            }
        }
    }
}
