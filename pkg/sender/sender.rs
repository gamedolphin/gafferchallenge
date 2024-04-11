use std::{net::SocketAddr, time::Duration};

use tokio::{net::UdpSocket, time};
use tokio_util::sync::CancellationToken;

pub async fn start_sender(
    local_addr: SocketAddr,
    server_addr: SocketAddr,
    frequency: u64,
    buffers: Vec<Vec<u8>>,
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
                tracing::info!("sending {}", std::str::from_utf8(buf).unwrap());
                socket.send(buf).await?;

                index += 1;
            }
            _ = cancel.cancelled() => {
                return Ok(())
            }
            n = socket.recv_from(&mut buf) => {
                let Ok((n, _)) = n else {
                    continue;
                };

                tracing::info!("received hash: {}", std::str::from_utf8(&buf[0..n]).unwrap());
            }
        }
    }
}
