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

    while !cancel.is_cancelled() {
        interval.tick().await;
        let buf = &buffers[index];
        socket.send(buf).await?;

        index += 1;
        if index >= buffers.len() {
            index = 0;
        }
    }

    Ok(())
}
