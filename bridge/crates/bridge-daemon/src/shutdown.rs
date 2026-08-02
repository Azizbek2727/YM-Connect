use ym_connect_bridge_core::{ShutdownError, ShutdownFuture, ShutdownSignal};

#[derive(Debug)]
pub(crate) struct OperatingSystemShutdown;

impl ShutdownSignal for OperatingSystemShutdown {
    fn wait(&self) -> ShutdownFuture<'_> {
        Box::pin(async {
            tokio::signal::ctrl_c().await.map_err(|source| {
                ShutdownError::new("failed to observe the operating-system shutdown signal", source)
            })
        })
    }
}
