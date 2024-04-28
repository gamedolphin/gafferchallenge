use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

// use anyhow::Context;
use fnv::FnvHasher;
use socket2::{Domain, Protocol, Socket, Type};
use std::hash::Hasher;
use tokio::{
    // io::{AsyncReadExt, AsyncWriteExt},
    net::UdpSocket,
};
use tokio_util::sync::CancellationToken;

pub async fn start_forwarder(
    local_addr: SocketAddr,
    sent_counter: Arc<AtomicU64>,
    recv_counter: Arc<AtomicU64>,
    cancelled: CancellationToken,
) -> anyhow::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;
    socket.bind(&local_addr.into())?;

    let socket = UdpSocket::from_std(socket.into())?;
    // let socket = UdpSocket::bind(&local_addr).await?;

    // let mut backend_stream = TcpStream::connect(backend_addr).await?;

    tracing::info!("Server listening on : {}", socket.local_addr()?);

    let mut buf = [0_u8; 100];

    // let mut backend_response = [0_u8; 1024];

    loop {
        tokio::select! {
            n = socket.recv_from(&mut buf) => {
                let (n, host) = match n {
                    Ok((n, host)) => (n, host),
                    Err(e)=> {
                        tracing::debug!("failed to recv on udp: {}", e);
                        continue;
                    }
                };

                recv_counter.fetch_add(1, Ordering::Relaxed);

                let hash = hash_incoming(&buf[0..n]);

                // let request = create_request(host, &buf[0..n]);

                // tracing::trace!("receiving from {} and forwarding {}", host, request);

                if let Err(e) = socket.send_to(&hash.to_be_bytes(), host).await {
                    tracing::error!("failed to return received packet: {}", e);
                    continue;
                }

                sent_counter.fetch_add(1, Ordering::Relaxed);

                // if let Err(e) = backend_stream.write_all(request.as_bytes()).await {
                //     tracing::error!("failed to forward received packet: {}", e);
                //     continue;
                // }
            }
            // resp = backend_stream.read(&mut backend_response) => {
            //     let n = match resp {
            //         Ok(n) => n,
            //         Err(e) => {
            //             tracing::error!("failed to receive backend response: {}", e);
            //             continue;
            //         }
            //     };

            //     if n == 0 {
            //         tracing::debug!("received empty backend response, closing connection");
            //         return Ok(());
            //     }

            //     let (from, body) = match parse_response(&backend_response[0..n]) {
            //         Ok((from, body)) => (from, body),
            //         Err(e)=> {
            //             tracing::error!("failed to parse incoming response: {}", e);
            //             continue;
            //         }
            //     };

            //     tracing::debug!("received hash, forwarding: {} for {}", body, from);

            // }
            _ = cancelled.cancelled() => {
                tracing::info!("shutting down forwarder!");
                return Ok(())
            }
        }
    }
}

fn hash_incoming(body: &[u8]) -> u64 {
    let mut hasher = FnvHasher::default();
    hasher.write(body);
    hasher.finish() // 64 byte
}

// fn parse_response(buf: &[u8]) -> anyhow::Result<(SocketAddr, &str)> {
//     let buf = std::str::from_utf8(buf).context("failed to parse bytes into utf8 string")?;
//     tracing::trace!("received from backend: {}", buf,);

//     let mut lines = buf.lines();

//     let _ = lines
//         .next()
//         .context("failed to parse http response header")?; // HTTP/1.1 200 OK
//     let host = lines.next().context("failed to parse host")?; // Host: <addr>
//     let mut host = host.split(' ');
//     let _ = host.next().context("failed to get host header part")?; // Host:
//     let addr: SocketAddr = host
//         .next()
//         .context(format!("failed to get host part, {}", buf))?
//         .parse()
//         .context("failed to parse host addr")?;

//     let _ = lines.next().unwrap(); // empty line
//     let body = lines.next().unwrap();

//     Ok((addr, body))
// }

// fn create_request(host: SocketAddr, data: &[u8]) -> String {
//     format!(
//         "GET / HTTP/1.1\r\nHost: {}\r\n\r\n{}",
//         host,
//         std::str::from_utf8(data).unwrap()
//     )
// }
