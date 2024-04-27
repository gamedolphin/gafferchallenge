use tokio::{signal, task::JoinHandle};
use tokio_util::sync::CancellationToken;

pub fn setup_shutdown() -> (JoinHandle<()>, CancellationToken) {
    let cancel = CancellationToken::new();
    let cloned_cancel = cancel.clone();

    let join_handle = tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                tracing::info!("shutting down");
                cloned_cancel.cancel();
            }
            Err(e) => {
                tracing::error!("failed to listen for exit signal : {}", e);
                std::process::exit(1);
            }
        }
    });

    (join_handle, cancel)
}
