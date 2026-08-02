use std::sync::Arc;

use crate::{BridgeConfig, BridgeError, LogLevel, LogRecord, Logger, ShutdownSignal};

const LOG_TARGET: &str = "bridge.lifecycle";

/// Runtime-independent dependencies required by the Bridge lifecycle.
#[derive(Debug)]
pub struct BridgeDependencies {
    logger: Arc<dyn Logger>,
    shutdown_signal: Arc<dyn ShutdownSignal>,
}

impl BridgeDependencies {
    /// Creates a dependency set from production or test implementations.
    #[must_use]
    pub fn new(logger: Arc<dyn Logger>, shutdown_signal: Arc<dyn ShutdownSignal>) -> Self {
        Self {
            logger,
            shutdown_signal,
        }
    }
}

/// Coordinates the Bridge lifecycle without implementing any transport behavior.
#[derive(Debug)]
pub struct BridgeApplication {
    config: BridgeConfig,
    dependencies: BridgeDependencies,
}

impl BridgeApplication {
    /// Creates a Bridge lifecycle from validated configuration and injected dependencies.
    #[must_use]
    pub fn new(config: BridgeConfig, dependencies: BridgeDependencies) -> Self {
        Self {
            config,
            dependencies,
        }
    }

    /// Runs until the injected shutdown signal resolves.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeError`] when the shutdown signal cannot be observed.
    pub async fn run(self) -> Result<(), BridgeError> {
        let worker_threads = self
            .config
            .runtime()
            .worker_threads()
            .map_or_else(|| "automatic".to_owned(), |value| value.get().to_string());

        self.dependencies.logger.log(LogRecord::new(
            LogLevel::Info,
            LOG_TARGET,
            &format!(
                "Bridge skeleton initialized; runtime_worker_threads={worker_threads}; awaiting shutdown"
            ),
        ));

        self.dependencies.shutdown_signal.wait().await?;

        self.dependencies.logger.log(LogRecord::new(
            LogLevel::Info,
            LOG_TARGET,
            "Shutdown signal received; Bridge skeleton stopped cleanly",
        ));

        Ok(())
    }
}
