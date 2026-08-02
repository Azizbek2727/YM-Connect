use std::{error::Error, fmt};

use crate::ShutdownError;

/// A failure while running the Bridge lifecycle.
#[derive(Debug)]
pub enum BridgeError {
    /// The injected shutdown source failed.
    Shutdown(ShutdownError),
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shutdown(source) => write!(formatter, "Bridge shutdown failed: {source}"),
        }
    }
}

impl Error for BridgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Shutdown(source) => Some(source),
        }
    }
}

impl From<ShutdownError> for BridgeError {
    fn from(source: ShutdownError) -> Self {
        Self::Shutdown(source)
    }
}
