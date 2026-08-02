use std::{error::Error, fmt};

use crate::{ShutdownError, StateError};

/// A failure while running the Bridge lifecycle.
#[derive(Debug)]
pub enum BridgeError {
    /// Runtime state could not be read or updated.
    State(StateError),
    /// The injected shutdown source failed.
    Shutdown(ShutdownError),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(source) => write!(formatter, "Bridge state failed: {source}"),
            Self::Shutdown(source) => write!(formatter, "Bridge shutdown failed: {source}"),
        }
    }
}

impl Error for BridgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::Shutdown(source) => Some(source),
        }
    }
}

impl From<StateError> for BridgeError {
    fn from(source: StateError) -> Self {
        Self::State(source)
    }
}

impl From<ShutdownError> for BridgeError {
    fn from(source: ShutdownError) -> Self {
        Self::Shutdown(source)
    }
}
