use std::{error::Error, fmt, sync::Arc};

use super::RegistryStateError;

/// Internal synchronization primitive associated with a state error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateLock {
    /// State snapshot lock.
    State,
    /// Subscriber registry lock.
    Subscribers,
}

impl fmt::Display for StateLock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::State => "state",
            Self::Subscribers => "subscribers",
        })
    }
}

/// Structured Bridge state operation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateError {
    /// An internal synchronization lock was poisoned.
    LockPoisoned {
        /// Affected lock.
        lock: StateLock,
    },
    /// The monotonic state revision reached `u64::MAX`.
    RevisionExhausted,
    /// The monotonic subscription identifier reached `u64::MAX`.
    SubscriptionIdExhausted,
    /// A state update closure panicked while unwinding was enabled.
    UpdatePanicked,
    /// A registry operation failed.
    Registry(RegistryStateError),
    /// A caller rejected a transaction before commit.
    Rejected {
        /// Stable caller-defined rejection code.
        code: Arc<str>,
        /// Human-readable rejection detail.
        message: Arc<str>,
    },
}

impl StateError {
    /// Creates a structured caller-defined transaction rejection.
    #[must_use]
    pub fn rejected(code: impl Into<Arc<str>>, message: impl Into<Arc<str>>) -> Self {
        Self::Rejected {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for StateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned { lock } => write!(formatter, "Bridge {lock} lock is poisoned"),
            Self::RevisionExhausted => formatter.write_str("Bridge state revision is exhausted"),
            Self::SubscriptionIdExhausted => {
                formatter.write_str("Bridge state subscription identifier is exhausted")
            }
            Self::UpdatePanicked => formatter.write_str("Bridge state update panicked"),
            Self::Registry(source) => source.fmt(formatter),
            Self::Rejected { code, message } => {
                write!(
                    formatter,
                    "Bridge state update rejected ({code}): {message}"
                )
            }
        }
    }
}

impl Error for StateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Registry(source) => Some(source),
            Self::LockPoisoned { .. }
            | Self::RevisionExhausted
            | Self::SubscriptionIdExhausted
            | Self::UpdatePanicked
            | Self::Rejected { .. } => None,
        }
    }
}

impl From<RegistryStateError> for StateError {
    fn from(source: RegistryStateError) -> Self {
        Self::Registry(source)
    }
}

/// Non-blocking or blocking subscription receive failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StateReceiveError {
    /// No event is currently queued for a non-blocking receive.
    Empty,
    /// A timed receive reached its deadline.
    Timeout,
    /// The state store or subscription channel disconnected.
    Disconnected,
}

impl fmt::Display for StateReceiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "no Bridge state event is currently available",
            Self::Timeout => "timed out waiting for a Bridge state event",
            Self::Disconnected => "Bridge state subscription is disconnected",
        })
    }
}

impl Error for StateReceiveError {}
