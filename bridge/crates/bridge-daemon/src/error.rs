use std::{
    error::Error,
    fmt,
    io::{self, Write},
};

use ym_connect_bridge_core::{BridgeError, ConfigError};

#[derive(Debug)]
pub(crate) enum DaemonError {
    Config(ConfigError),
    Runtime(io::Error),
    Bridge(BridgeError),
}

impl fmt::Display for DaemonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(source) => write!(formatter, "Bridge configuration failed: {source}"),
            Self::Runtime(source) => write!(formatter, "asynchronous runtime failed: {source}"),
            Self::Bridge(source) => write!(formatter, "Bridge lifecycle failed: {source}"),
        }
    }
}

impl Error for DaemonError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Config(source) => Some(source),
            Self::Runtime(source) => Some(source),
            Self::Bridge(source) => Some(source),
        }
    }
}

impl From<ConfigError> for DaemonError {
    fn from(source: ConfigError) -> Self {
        Self::Config(source)
    }
}

impl From<BridgeError> for DaemonError {
    fn from(source: BridgeError) -> Self {
        Self::Bridge(source)
    }
}

pub(crate) fn report_fatal(error: &DaemonError) {
    let mut stderr = io::stderr().lock();
    let _ = writeln!(stderr, "ym-connect-bridge: {error}");

    let mut source = error.source();
    while let Some(current) = source {
        let _ = writeln!(stderr, "caused by: {current}");
        source = current.source();
    }
}
