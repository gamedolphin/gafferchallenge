use fnv::FnvHasher;
use std::hash::Hasher;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpSocket, TcpStream};

use tokio_util::sync::CancellationToken;

pub async fn start_backend(
    local_addr: SocketAddr,
    cancelled: CancellationToken,
) -> anyhow::Result<()> {
    let socket = TcpSocket::new_v4()?;
    socket.set_reuseport(true)?;
    socket.bind(local_addr)?;

    let listener = socket.listen(1024)?;

    loop {
        tokio::select! {
            _ = cancelled.cancelled() => {
                tracing::info!("shutting down backend!");
                return Ok(())
            }
            res = listener.accept() => {
                let cancel_clone = cancelled.clone();
                let (socket, incoming) = res?;
                tracing::info!("incoming connection from : {}", incoming);
                handle_connection(socket, cancel_clone).await?;
            }
        }
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    cancelled: CancellationToken,
) -> anyhow::Result<()> {
    tokio::spawn(async move {
        let mut buf = vec![0; 1024];

        loop {
            tokio::select! {
                _ = cancelled.cancelled() => {
                    return
                }
                res = socket.read(&mut buf) => {
                    let Ok(n) = res else {
                        continue
                    };

                    if n == 0 {
                        continue
                    }

                    let (from, body) = parse_request(&buf[0..n]);

                    tracing::info!("received {} from {}", body, from);

                    let mut hasher = FnvHasher::default();
                    hasher.write(body.as_bytes());
                    let hash = hasher.finish(); // 64 bytes

                    let response = generate_response(from, hash);

                    tracing::info!("responding with {}", hash);

                    socket
                    .write_all(response.as_bytes())
                    .await.unwrap();
                }
            }
        }
    });

    Ok(())
}

fn parse_request(buf: &[u8]) -> (&str, &str) {
    let req = std::str::from_utf8(buf).unwrap();
    let mut lines = req.lines();

    let _ = lines.next().unwrap(); // GET <path> HTTP/1.1

    let host = lines.next().unwrap(); // Host: someplace:someport

    let _ = lines.next().unwrap(); // empty line

    let body = lines.next().unwrap();

    (host, body)
}

fn generate_response(from: &str, hash: u64) -> String {
    format!("HTTP/1.1 200 OK\r\n{}\r\n\r\n{}", from, hash)
}
