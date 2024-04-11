use std::net::SocketAddr;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UdpSocket},
};
use tokio_util::sync::CancellationToken;

pub async fn start_forwarder(
    local_addr: SocketAddr,
    backend_addr: SocketAddr,
    cancelled: CancellationToken,
) -> anyhow::Result<()> {
    let socket = UdpSocket::bind(local_addr).await?;

    let mut backend_stream = TcpStream::connect(backend_addr).await?;

    tracing::info!("Server listening on : {}", socket.local_addr()?);

    let mut buf = [0_u8; 100];

    let mut backend_response = [0_u8; 1024];

    loop {
        tokio::select! {
            _ = cancelled.cancelled() => {
                tracing::info!("shutting down forwarder!");
                return Ok(())
            }
            n = socket.recv_from(&mut buf) => {
                let Ok((n, host)) = n else {
                    continue;
                };

                tracing::info!("receiving and forwarding {}", std::str::from_utf8(&buf[0..n]).unwrap());

                let request = create_request(host, &buf[0..n]);
                backend_stream.write_all(request.as_bytes()).await?;
            }
            resp = backend_stream.read(&mut backend_response) => {
                let Ok(n) = resp else {
                    continue;
                };

                if n == 0 {
                    continue
                }

                let (from, body) = parse_response(&backend_response[0..n]);

                tracing::info!("received hash, forwarding: {} for {}", body, from);

                socket.send_to(body.as_bytes(), from).await?;
            }
        }
    }
}

fn parse_response(buf: &[u8]) -> (SocketAddr, &str) {
    let buf = std::str::from_utf8(buf).unwrap();
    let mut lines = buf.lines();

    let _ = lines.next().unwrap(); // HTTP/1.1 200 OK
    let host = lines.next().unwrap(); // Host: <addr>
    let mut host = host.split(' ');
    let _ = host.next().unwrap(); // Host:
    let addr: SocketAddr = host.next().unwrap().parse().unwrap();

    let _ = lines.next().unwrap(); // empty line
    let body = lines.next().unwrap();

    (addr, body)
}

fn create_request(host: SocketAddr, data: &[u8]) -> String {
    format!(
        "GET / HTTP/1.1\r\nHost: {}\r\n\r\n{}",
        host,
        std::str::from_utf8(data).unwrap()
    )
}
