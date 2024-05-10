use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use fnv::FnvHasher;
use monoio::io::CancelHandle;
use monoio::net::udp::UdpSocket;
use socket2::{Domain, Protocol, Socket, Type};
use std::hash::Hasher;

pub async fn start_forwarder(
    local_addr: SocketAddr,
    sent_counter: Arc<AtomicU64>,
    recv_counter: Arc<AtomicU64>,
    canceller: CancelHandle,
    cancel_tag: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;
    socket.set_recv_buffer_size(12582912)?;
    socket.set_send_buffer_size(12582912)?;
    socket.bind(&local_addr.into())?;

    let socket = UdpSocket::from_std(socket.into())?;
    // let socket = UdpSocket::bind(&local_addr).await?;

    // let mut backend_stream = TcpStream::connect(backend_addr).await?;

    tracing::info!("Server listening on : {}", socket.local_addr()?);

    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut res;

    // let mut backend_response = [0_u8; 1024];

    loop {
        if cancel_tag.load(Ordering::SeqCst) {
            break Ok(());
        }

        (res, buf) = socket.cancelable_recv_from(buf, canceller.clone()).await;

        let (n, host) = match res {
            Ok(n) => n,
            Err(e) => break Err(e.into()),
        };

        recv_counter.fetch_add(1, Ordering::Relaxed);
        sent_counter.fetch_add(1, Ordering::Relaxed);

        let hash = hash_incoming(&buf[0..n]);
        let hash_bytes = Box::new(hash.to_le_bytes());

        let (res, _) = socket
            .cancelable_send_to(hash_bytes, host, canceller.clone())
            .await;
        if let Err(e) = res {
            tracing::error!("failed to return received packet: {}", e);
            continue;
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
