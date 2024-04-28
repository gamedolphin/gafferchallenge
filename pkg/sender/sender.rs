use socket2::{Domain, Protocol, Socket, Type};
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
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;
    socket.set_recv_buffer_size(12582912)?;
    socket.set_send_buffer_size(12582912)?;
    socket.bind(&local_addr.into())?;

    let socket = UdpSocket::from_std(socket.into())?;

    socket.connect(&server_addr).await?;

    let mut interval = time::interval(Duration::from_millis(1000 / frequency));

    let mut index = 0;

    let mut buf = [0_u8; 100];

    // let sent_clone = sent_counter.clone();
    // let cancel_clone = cancel.clone();
    // let socket_recv = &socket;
    // let joiner =
    //     tokio::spawn(
    //         async move { recv_another_thread(socket_recv, sent_clone, cancel_clone).await },
    //     );

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

async fn recv_another_thread(
    socket: &UdpSocket,
    recv_counter: Arc<AtomicU64>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let mut buf = [0_u8; 100];
    loop {
        tokio::select! {
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
