use monoio::io::CancelHandle;
use monoio::net::udp::UdpSocket;
use socket2::{Domain, Protocol, Socket, Type};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

pub async fn start_sender(
    local_addr: SocketAddr,
    server_addr: SocketAddr,
    frequency: u64,
    buffers: &'static [&'static [u8; 100]; 4],
    sent_counter: Arc<AtomicU64>,
    recv_counter: Arc<AtomicU64>,
    cancel_bool: Arc<AtomicBool>,
    cancel_handle: CancelHandle,
) -> anyhow::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;
    socket.set_recv_buffer_size(12582912)?;
    socket.set_send_buffer_size(12582912)?;
    socket.bind(&local_addr.into())?;

    let socket = UdpSocket::from_std(socket.into())?;

    socket.connect(server_addr).await?;

    let mut index = 0;

    // let mut buf = [0_u8; 100];

    // let sent_clone = sent_counter.clone();
    // let cancel_clone = cancel.clone();
    // let socket_recv = &socket;
    // let joiner =
    //     tokio::spawn(
    //         async move { recv_another_thread(socket_recv, sent_clone, cancel_clone).await },
    //     );

    // let _recv_addr = socket.local_addr()?;
    // let _recv_counter_clone = recv_counter.clone();
    // let _cancel_clone = cancel.clone();

    let recv_addr = socket.local_addr()?;

    let joined = monoio::spawn(recv_another_thread(
        recv_addr,
        recv_counter,
        cancel_bool.clone(),
        cancel_handle.clone(),
    ));
    let mut ticker = monoio::time::interval(Duration::from_millis(1000 / frequency));

    loop {
        if cancel_bool.load(Ordering::SeqCst) {
            break;
        }

        ticker.tick().await;

        if index >= buffers.len() {
            index = 0;
        }

        let buf = buffers[index];

        tracing::debug!("sending {}", std::str::from_utf8(buf).unwrap());
        let (res, _) = socket.cancelable_send(buf, cancel_handle.clone()).await;
        if let Err(e) = res {
            tracing::debug!("failed to send {}", e);
            continue;
        }

        index += 1;

        sent_counter.fetch_add(1, Ordering::Relaxed);
    }

    joined.await
}

async fn recv_another_thread(
    local_addr: SocketAddr,
    recv_counter: Arc<AtomicU64>,
    cancel_tag: Arc<AtomicBool>,
    canceller: CancelHandle,
) -> anyhow::Result<()> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_reuse_port(true)?;
    socket.set_recv_buffer_size(12582912)?;
    socket.set_send_buffer_size(12582912)?;
    socket.bind(&local_addr.into())?;

    let socket = UdpSocket::from_std(socket.into())?;
    let mut buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut res;

    loop {
        if cancel_tag.load(Ordering::Relaxed) {
            break;
        }

        (res, buf) = socket.cancelable_recv(buf, canceller.clone()).await;

        let Ok(n) = res else {
            break;
        };

        recv_counter.fetch_add(1, Ordering::Relaxed);

        tracing::debug!(
            "received hash: {}",
            std::str::from_utf8(&buf[0..n]).unwrap()
        );

        buf.clear();
    }

    tracing::debug!("shutting down sender");

    Ok(())
}
