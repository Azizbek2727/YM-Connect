//! Runtime-independent lifecycle primitives for the YM Connect desktop Bridge.
//!
//! This crate owns configuration, logging contracts, dependency injection, and the
//! application lifecycle. Platform and asynchronous-runtime integrations belong in
//! executable crates so the core remains browser-, transport-, and runtime-agnostic.

mod application;
mod config;
mod error;
mod logging;
mod shutdown;

pub use application::{BridgeApplication, BridgeDependencies};
pub use config::{
    BridgeConfig, BridgeConfigLayer, BridgeConfigLoader, ConfigError, ConfigErrorKind,
    ConfigField, ConfigSource, DEFAULT_LOG_LEVEL, DEFAULT_RUNTIME_WORKER_THREADS, LOG_LEVEL_ENV,
    LoggingConfig, RUNTIME_WORKER_THREADS_ENV, RuntimeConfig, RuntimeWorkerThreads,
};
pub use error::BridgeError;
pub use logging::{LogLevel, LogRecord, Logger, StderrLogger};
pub use shutdown::{ShutdownError, ShutdownFuture, ShutdownSignal};
