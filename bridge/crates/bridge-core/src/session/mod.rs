//! Runtime-independent Bridge session lifecycle orchestration.
//!
//! The Session Manager owns no runtime state. Every mutation is executed as an atomic
//! [`crate::BridgeStateStore`] transaction, while canonical protocol values remain generated
//! Protocol Buffer models.

mod error;
mod manager;
mod model;

pub use error::{SessionCapabilityList, SessionManagerError};
pub use manager::{
    CloseSession, CreateSession, DEFAULT_SESSION_INACTIVITY_TIMEOUT_MS, ExpiredSessions,
    RemoveExpiredSessions, RestoreSession, ResumeSession, SessionManager, SessionMutation,
    SessionPolicy, SuspendSession, UpdateSession,
};
pub use model::{
    BridgeSession, SessionDuration, SessionLifecycleState, SessionMetadata, SessionMetadataKey,
    SessionMetadataValue, SessionModelError, SessionRevision, SessionStateTransition,
    SessionTimestamp,
};

pub(crate) use model::SessionRecordParts;

#[cfg(test)]
mod tests;
