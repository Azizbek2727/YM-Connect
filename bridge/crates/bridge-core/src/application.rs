use std::sync::Arc;

use crate::{
    BridgeConfig, BridgeError, BridgeLifecycleState, BridgeStateStore, LogLevel, LogRecord, Logger,
    ShutdownSignal,
};

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
    state: BridgeStateStore,
    dependencies: BridgeDependencies,
}

impl BridgeApplication {
    /// Creates a Bridge lifecycle from validated configuration and injected dependencies.
    #[must_use]
    pub fn new(config: BridgeConfig, dependencies: BridgeDependencies) -> Self {
        let state = BridgeStateStore::new(config.clone());
        Self {
            config,
            state,
            dependencies,
        }
    }

    /// Returns a shared handle to the Bridge runtime state store.
    #[must_use]
    pub fn state(&self) -> BridgeStateStore {
        self.state.clone()
    }

    /// Runs until the injected shutdown signal resolves.
    ///
    /// # Errors
    ///
    /// Returns [`BridgeError`] when lifecycle state cannot be updated or the shutdown signal
    /// cannot be observed.
    pub async fn run(self) -> Result<(), BridgeError> {
        let worker_threads = self.config.runtime().worker_threads().to_string();
        self.set_lifecycle(BridgeLifecycleState::Running)?;

        self.dependencies.logger.log(LogRecord::new(
            LogLevel::Info,
            LOG_TARGET,
            &format!(
                "Bridge initialized; runtime_worker_threads={worker_threads}; awaiting shutdown"
            ),
        ));

        if let Err(source) = self.dependencies.shutdown_signal.wait().await {
            self.set_lifecycle(BridgeLifecycleState::Failed)?;
            return Err(source.into());
        }

        self.set_lifecycle(BridgeLifecycleState::Stopping)?;
        self.dependencies.logger.log(LogRecord::new(
            LogLevel::Info,
            LOG_TARGET,
            "Shutdown signal received; Bridge stopped cleanly",
        ));
        self.set_lifecycle(BridgeLifecycleState::Stopped)?;

        Ok(())
    }

    fn set_lifecycle(&self, lifecycle: BridgeLifecycleState) -> Result<(), BridgeError> {
        let _ = self.state.update(|draft| {
            let _ = draft.set_lifecycle(lifecycle);
            Ok(())
        })?;
        Ok(())
    }
}
