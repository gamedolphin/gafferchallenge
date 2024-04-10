use std::net::SocketAddr;

use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

pub async fn start_forwarder(
    local_addr: SocketAddr,
    cancelled: CancellationToken,
) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(local_addr).await?;

    tracing::info!("Server listening on : {}", socket.local_addr()?);

    let mut buf = [0_u8; 100];

    loop {
        tokio::select! {
            _ = cancelled.cancelled() => {
                tracing::info!("shutting down forwarder!");
                return Ok(())
            }
            _ = socket.recv_from(&mut buf) => {
                tracing::trace!("receiving {}", std::str::from_utf8(&buf).unwrap());
            }
        }
    }
}
