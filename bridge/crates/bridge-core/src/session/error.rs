use std::{error::Error, fmt, sync::Arc};

use crate::{
    ConnectorId, DeviceId, SessionDuration, SessionId, SessionLifecycleState, SessionRevision,
    SessionTimestamp, StateError,
};

/// Capability list associated with a structured validation error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionCapabilityList {
    /// Negotiated supported capabilities.
    Supported,
    /// Negotiated required capabilities.
    Required,
}

impl fmt::Display for SessionCapabilityList {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Supported => "supported",
            Self::Required => "required",
        })
    }
}

/// Structured Session Manager operation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionManagerError {
    /// Bridge State rejected or failed the transaction.
    State(StateError),
    /// The requested session does not exist.
    SessionNotFound {
        /// Missing session identifier.
        session_id: SessionId,
    },
    /// The requested session identifier already exists.
    DuplicateSession {
        /// Duplicate session identifier.
        session_id: SessionId,
    },
    /// A live session already owns the same device/connector association.
    DuplicateLiveAssociation {
        /// Session that was being created or restored.
        session_id: SessionId,
        /// Existing conflicting session.
        conflicting_session_id: SessionId,
        /// Associated device.
        device_id: DeviceId,
        /// Associated connector.
        connector_id: ConnectorId,
    },
    /// The associated device is absent from Bridge State.
    MissingDevice {
        /// Missing device identifier.
        device_id: DeviceId,
    },
    /// The associated connector is absent from Bridge State.
    MissingConnector {
        /// Missing connector identifier.
        connector_id: ConnectorId,
    },
    /// The requested lifecycle transition is not part of the finite state machine.
    InvalidTransition {
        /// Affected session.
        session_id: SessionId,
        /// Current lifecycle state.
        previous: SessionLifecycleState,
        /// Requested lifecycle state.
        requested: SessionLifecycleState,
    },
    /// A terminal session cannot transition again.
    TerminalSession {
        /// Affected session.
        session_id: SessionId,
        /// Terminal lifecycle state.
        state: SessionLifecycleState,
    },
    /// The caller supplied an obsolete session-local revision.
    StaleRevision {
        /// Affected session.
        session_id: SessionId,
        /// Revision expected by the caller.
        expected: SessionRevision,
        /// Current committed revision.
        actual: SessionRevision,
    },
    /// The operation targeted an expired session.
    ExpiredSession {
        /// Affected session.
        session_id: SessionId,
        /// Last recorded activity.
        last_activity_at: SessionTimestamp,
        /// Timestamp used for expiration evaluation.
        observed_at: SessionTimestamp,
        /// Configured inactivity timeout.
        timeout: SessionDuration,
    },
    /// An operation timestamp moved backwards.
    TimestampRegression {
        /// Affected session.
        session_id: SessionId,
        /// Last committed timestamp.
        previous: SessionTimestamp,
        /// Rejected timestamp.
        requested: SessionTimestamp,
    },
    /// Restored timestamps are not ordered as creation <= activity <= restoration.
    InvalidRestoreTimestamps {
        /// Restored session identifier.
        session_id: SessionId,
        /// Creation timestamp.
        created_at: SessionTimestamp,
        /// Last activity timestamp.
        last_activity_at: SessionTimestamp,
        /// Restoration timestamp.
        restored_at: SessionTimestamp,
    },
    /// Protocol version zero is not a valid negotiated version.
    InvalidProtocolVersion {
        /// Protocol major version.
        major: u32,
        /// Protocol minor version.
        minor: u32,
        /// Protocol patch version.
        patch: u32,
    },
    /// A capability numeric value is unknown or unspecified.
    InvalidCapability {
        /// Rejected list.
        list: SessionCapabilityList,
        /// Rejected numeric capability value.
        value: i32,
    },
    /// A capability appears more than once in one canonical list.
    DuplicateCapability {
        /// Rejected list.
        list: SessionCapabilityList,
        /// Duplicate numeric capability value.
        value: i32,
    },
    /// A required capability is absent from the supported capability list.
    MissingRequiredCapability {
        /// Missing numeric capability value.
        value: i32,
    },
    /// The session-local revision reached `u64::MAX`.
    RevisionExhausted {
        /// Affected session.
        session_id: SessionId,
    },
    /// A committed state update violated an internal Session Manager invariant.
    StateInvariant {
        /// Stable diagnostic detail.
        message: Arc<str>,
    },
}

impl SessionManagerError {
    pub(crate) fn state_invariant(message: impl Into<Arc<str>>) -> Self {
        Self::StateInvariant {
            message: message.into(),
        }
    }

    fn format_identity_and_lifecycle(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> Option<fmt::Result> {
        match self {
            Self::State(source) => {
                Some(write!(formatter, "Bridge State operation failed: {source}"))
            }
            Self::SessionNotFound { session_id } => {
                Some(write!(formatter, "session {session_id} does not exist"))
            }
            Self::DuplicateSession { session_id } => {
                Some(write!(formatter, "session {session_id} already exists"))
            }
            Self::DuplicateLiveAssociation {
                session_id,
                conflicting_session_id,
                device_id,
                connector_id,
            } => Some(write!(
                formatter,
                "session {session_id} conflicts with live session {conflicting_session_id} for device {device_id} and connector {connector_id}"
            )),
            Self::MissingDevice { device_id } => {
                Some(write!(formatter, "device {device_id} does not exist"))
            }
            Self::MissingConnector { connector_id } => {
                Some(write!(formatter, "connector {connector_id} does not exist"))
            }
            Self::InvalidTransition {
                session_id,
                previous,
                requested,
            } => Some(write!(
                formatter,
                "session {session_id} cannot transition from {previous:?} to {requested:?}"
            )),
            Self::TerminalSession { session_id, state } => Some(write!(
                formatter,
                "session {session_id} is terminal in state {state:?}"
            )),
            Self::StaleRevision {
                session_id,
                expected,
                actual,
            } => Some(write!(
                formatter,
                "session {session_id} revision is stale: expected {}, actual {}",
                expected.get(),
                actual.get()
            )),
            Self::ExpiredSession { .. }
            | Self::TimestampRegression { .. }
            | Self::InvalidRestoreTimestamps { .. }
            | Self::InvalidProtocolVersion { .. }
            | Self::InvalidCapability { .. }
            | Self::DuplicateCapability { .. }
            | Self::MissingRequiredCapability { .. }
            | Self::RevisionExhausted { .. }
            | Self::StateInvariant { .. } => None,
        }
    }

    fn format_temporal_and_negotiation(
        &self,
        formatter: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self {
            Self::ExpiredSession {
                session_id,
                last_activity_at,
                observed_at,
                timeout,
            } => write!(
                formatter,
                "session {session_id} expired: last activity {}, observed {}, timeout {} ms",
                last_activity_at.as_unix_millis(),
                observed_at.as_unix_millis(),
                timeout.as_millis()
            ),
            Self::TimestampRegression {
                session_id,
                previous,
                requested,
            } => write!(
                formatter,
                "session {session_id} timestamp regressed from {} to {}",
                previous.as_unix_millis(),
                requested.as_unix_millis()
            ),
            Self::InvalidRestoreTimestamps {
                session_id,
                created_at,
                last_activity_at,
                restored_at,
            } => write!(
                formatter,
                "session {session_id} restore timestamps are invalid: created {}, activity {}, restored {}",
                created_at.as_unix_millis(),
                last_activity_at.as_unix_millis(),
                restored_at.as_unix_millis()
            ),
            Self::InvalidProtocolVersion {
                major,
                minor,
                patch,
            } => write!(
                formatter,
                "protocol version {major}.{minor}.{patch} is invalid"
            ),
            Self::InvalidCapability { list, value } => {
                write!(formatter, "{list} capability value {value} is invalid")
            }
            Self::DuplicateCapability { list, value } => {
                write!(formatter, "{list} capability value {value} is duplicated")
            }
            Self::MissingRequiredCapability { value } => write!(
                formatter,
                "required capability value {value} is not supported"
            ),
            Self::RevisionExhausted { session_id } => {
                write!(formatter, "session {session_id} revision is exhausted")
            }
            Self::StateInvariant { message } => {
                write!(formatter, "Session Manager state invariant failed: {message}")
            }
            Self::State(_)
            | Self::SessionNotFound { .. }
            | Self::DuplicateSession { .. }
            | Self::DuplicateLiveAssociation { .. }
            | Self::MissingDevice { .. }
            | Self::MissingConnector { .. }
            | Self::InvalidTransition { .. }
            | Self::TerminalSession { .. }
            | Self::StaleRevision { .. } => Ok(()),
        }
    }
}

impl fmt::Display for SessionManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format_identity_and_lifecycle(formatter)
            .unwrap_or_else(|| self.format_temporal_and_negotiation(formatter))
    }
}

impl Error for SessionManagerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::SessionNotFound { .. }
            | Self::DuplicateSession { .. }
            | Self::DuplicateLiveAssociation { .. }
            | Self::MissingDevice { .. }
            | Self::MissingConnector { .. }
            | Self::InvalidTransition { .. }
            | Self::TerminalSession { .. }
            | Self::StaleRevision { .. }
            | Self::ExpiredSession { .. }
            | Self::TimestampRegression { .. }
            | Self::InvalidRestoreTimestamps { .. }
            | Self::InvalidProtocolVersion { .. }
            | Self::InvalidCapability { .. }
            | Self::DuplicateCapability { .. }
            | Self::MissingRequiredCapability { .. }
            | Self::RevisionExhausted { .. }
            | Self::StateInvariant { .. } => None,
        }
    }
}

impl From<StateError> for SessionManagerError {
    fn from(source: StateError) -> Self {
        Self::State(source)
    }
}
