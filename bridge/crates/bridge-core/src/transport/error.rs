use std::{error::Error, fmt, sync::Arc};

use crate::{
    ConnectionId, SessionId, StateError, TransportModelError, TransportRevision, TransportState,
    TransportTimestamp,
};

/// Result type used by Transport Core interfaces and lifecycle operations.
pub type TransportResult<T> = Result<T, TransportError>;

/// Structured Transport Core failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportError {
    /// Bridge State rejected or failed the transaction.
    State(StateError),
    /// A transport model value failed validation.
    Model(TransportModelError),
    /// The requested connection does not exist.
    ConnectionNotFound {
        /// Missing connection identifier.
        connection_id: ConnectionId,
    },
    /// The requested connection identifier already exists.
    DuplicateConnection {
        /// Duplicate connection identifier.
        connection_id: ConnectionId,
    },
    /// The requested lifecycle transition is not part of the finite state machine.
    InvalidTransition {
        /// Affected connection.
        connection_id: ConnectionId,
        /// Current lifecycle state.
        previous: TransportState,
        /// Requested lifecycle state.
        requested: TransportState,
    },
    /// A terminal connection cannot transition or change binding.
    TerminalConnection {
        /// Affected connection.
        connection_id: ConnectionId,
        /// Terminal lifecycle state.
        state: TransportState,
    },
    /// The caller supplied an obsolete connection-local revision.
    StaleRevision {
        /// Affected connection.
        connection_id: ConnectionId,
        /// Revision expected by the caller.
        expected: TransportRevision,
        /// Current committed revision.
        actual: TransportRevision,
    },
    /// An operation timestamp moved backwards.
    TimestampRegression {
        /// Affected connection.
        connection_id: ConnectionId,
        /// Last committed timestamp.
        previous: TransportTimestamp,
        /// Rejected timestamp.
        requested: TransportTimestamp,
    },
    /// Session binding requires an authenticated connection.
    BindingRequiresAuthenticated {
        /// Affected connection.
        connection_id: ConnectionId,
        /// Current lifecycle state.
        state: TransportState,
    },
    /// The requested session is absent from Bridge State.
    MissingSession {
        /// Missing session identifier.
        session_id: SessionId,
    },
    /// The connection already has a session binding.
    SessionAlreadyBound {
        /// Affected connection.
        connection_id: ConnectionId,
        /// Existing session binding.
        session_id: SessionId,
    },
    /// The connection has no session binding to remove.
    SessionNotBound {
        /// Affected connection.
        connection_id: ConnectionId,
    },
    /// The connection-local revision reached `u64::MAX`.
    RevisionExhausted {
        /// Affected connection.
        connection_id: ConnectionId,
    },
    /// A committed state update violated a Transport Core invariant.
    StateInvariant {
        /// Stable diagnostic detail.
        message: Arc<str>,
    },
}

impl TransportError {
    pub(crate) fn state_invariant(message: impl Into<Arc<str>>) -> Self {
        Self::StateInvariant {
            message: message.into(),
        }
    }

    fn format_connection(&self, formatter: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
        match self {
            Self::ConnectionNotFound { connection_id } => {
                Some(write!(formatter, "transport connection {connection_id} does not exist"))
            }
            Self::DuplicateConnection { connection_id } => Some(write!(
                formatter,
                "transport connection {connection_id} already exists"
            )),
            Self::InvalidTransition {
                connection_id,
                previous,
                requested,
            } => Some(write!(
                formatter,
                "transport connection {connection_id} cannot transition from {previous:?} to {requested:?}"
            )),
            Self::TerminalConnection {
                connection_id,
                state,
            } => Some(write!(
                formatter,
                "transport connection {connection_id} is terminal in state {state:?}"
            )),
            Self::StaleRevision {
                connection_id,
                expected,
                actual,
            } => Some(write!(
                formatter,
                "transport connection {connection_id} revision is stale: expected {}, actual {}",
                expected.get(),
                actual.get()
            )),
            Self::TimestampRegression {
                connection_id,
                previous,
                requested,
            } => Some(write!(
                formatter,
                "transport connection {connection_id} timestamp regressed from {} to {}",
                previous.as_unix_millis(),
                requested.as_unix_millis()
            )),
            Self::State(_)
            | Self::Model(_)
            | Self::BindingRequiresAuthenticated { .. }
            | Self::MissingSession { .. }
            | Self::SessionAlreadyBound { .. }
            | Self::SessionNotBound { .. }
            | Self::RevisionExhausted { .. }
            | Self::StateInvariant { .. } => None,
        }
    }

    fn format_binding_and_source(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(source) => write!(formatter, "Bridge State operation failed: {source}"),
            Self::Model(source) => write!(formatter, "transport model is invalid: {source}"),
            Self::BindingRequiresAuthenticated {
                connection_id,
                state,
            } => write!(
                formatter,
                "transport connection {connection_id} cannot bind a session while in state {state:?}"
            ),
            Self::MissingSession { session_id } => {
                write!(formatter, "session {session_id} does not exist")
            }
            Self::SessionAlreadyBound {
                connection_id,
                session_id,
            } => write!(
                formatter,
                "transport connection {connection_id} is already bound to session {session_id}"
            ),
            Self::SessionNotBound { connection_id } => write!(
                formatter,
                "transport connection {connection_id} has no session binding"
            ),
            Self::RevisionExhausted { connection_id } => write!(
                formatter,
                "transport connection {connection_id} revision is exhausted"
            ),
            Self::StateInvariant { message } => {
                write!(formatter, "Transport Core state invariant failed: {message}")
            }
            Self::ConnectionNotFound { .. }
            | Self::DuplicateConnection { .. }
            | Self::InvalidTransition { .. }
            | Self::TerminalConnection { .. }
            | Self::StaleRevision { .. }
            | Self::TimestampRegression { .. } => Ok(()),
        }
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format_connection(formatter)
            .unwrap_or_else(|| self.format_binding_and_source(formatter))
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::Model(source) => Some(source),
            Self::ConnectionNotFound { .. }
            | Self::DuplicateConnection { .. }
            | Self::InvalidTransition { .. }
            | Self::TerminalConnection { .. }
            | Self::StaleRevision { .. }
            | Self::TimestampRegression { .. }
            | Self::BindingRequiresAuthenticated { .. }
            | Self::MissingSession { .. }
            | Self::SessionAlreadyBound { .. }
            | Self::SessionNotBound { .. }
            | Self::RevisionExhausted { .. }
            | Self::StateInvariant { .. } => None,
        }
    }
}

impl From<StateError> for TransportError {
    fn from(source: StateError) -> Self {
        Self::State(source)
    }
}

impl From<TransportModelError> for TransportError {
    fn from(source: TransportModelError) -> Self {
        Self::Model(source)
    }
}
