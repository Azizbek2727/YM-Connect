use std::sync::Arc;

use tokio::runtime::{Builder, Runtime};
use ym_connect_bridge_core::{
    BridgeApplication, BridgeConfig, BridgeDependencies, Logger, RuntimeWorkerThreads,
    StderrLogger,
};

use crate::{error::DaemonError, shutdown::OperatingSystemShutdown};

pub(crate) fn run() -> Result<(), DaemonError> {
    let config = BridgeConfig::load()?;
    let runtime = build_runtime(&config)?;
    let logger: Arc<dyn Logger> = Arc::new(StderrLogger::new(config.logging().level()));
    let dependencies = BridgeDependencies::new(logger, Arc::new(OperatingSystemShutdown));
    let application = BridgeApplication::new(config, dependencies);

    runtime.block_on(application.run())?;
    Ok(())
}

fn build_runtime(config: &BridgeConfig) -> Result<Runtime, DaemonError> {
    let mut builder = Builder::new_multi_thread();
    builder.enable_all().thread_name("ym-connect-bridge");

    if let RuntimeWorkerThreads::Fixed(worker_threads) = config.runtime().worker_threads() {
        builder.worker_threads(worker_threads.get());
    }

    builder.build().map_err(DaemonError::Runtime)
}
