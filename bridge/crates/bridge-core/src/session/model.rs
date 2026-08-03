use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    num::NonZeroU64,
    sync::Arc,
};

use ym_connect_protocol::v1::{CapabilitySet, ProtocolVersion};

use crate::{
    ConnectorId, DeviceId, RegistryKind, SessionId, StateIdentifierError, StateRegistryValue,
};

/// Validation failure for a strongly typed session model value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionModelError {
    /// Session durations must be greater than zero.
    ZeroDuration,
    /// Metadata keys must not be empty.
    EmptyMetadataKey,
}

impl fmt::Display for SessionModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ZeroDuration => "session duration must be greater than zero",
            Self::EmptyMetadataKey => "session metadata key must not be empty",
        })
    }
}

impl Error for SessionModelError {}

/// Milliseconds since the Unix epoch used by the runtime-independent session model.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SessionTimestamp(u64);

impl SessionTimestamp {
    /// Creates a timestamp from Unix milliseconds.
    #[must_use]
    pub const fn from_unix_millis(value: u64) -> Self {
        Self(value)
    }

    /// Returns the represented Unix milliseconds.
    #[must_use]
    pub const fn as_unix_millis(self) -> u64 {
        self.0
    }

    /// Returns elapsed milliseconds when `self` is not earlier than `earlier`.
    #[must_use]
    pub const fn checked_duration_since(self, earlier: Self) -> Option<u64> {
        self.0.checked_sub(earlier.0)
    }
}

/// Non-zero session duration represented in milliseconds.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SessionDuration(NonZeroU64);

impl SessionDuration {
    /// Creates a non-zero duration.
    ///
    /// # Errors
    ///
    /// Returns [`SessionModelError::ZeroDuration`] when `milliseconds` is zero.
    pub const fn from_millis(milliseconds: u64) -> Result<Self, SessionModelError> {
        match NonZeroU64::new(milliseconds) {
            Some(value) => Ok(Self(value)),
            None => Err(SessionModelError::ZeroDuration),
        }
    }

    /// Returns the duration in milliseconds.
    #[must_use]
    pub const fn as_millis(self) -> u64 {
        self.0.get()
    }
}

/// Monotonic revision of one session record.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct SessionRevision(u64);

impl SessionRevision {
    /// Initial revision assigned to a newly created session.
    pub const INITIAL: Self = Self(0);

    /// Creates a revision from its numeric representation.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric revision.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Lifecycle state of a Bridge session.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SessionLifecycleState {
    /// Session identity and associations were recorded.
    Created,
    /// Protocol and capability negotiation is in progress.
    Negotiating,
    /// Session is available for normal use.
    Active,
    /// Session is temporarily unavailable but may resume.
    Suspended,
    /// Session shutdown is in progress.
    Closing,
    /// Session shutdown completed and no further transitions are allowed.
    Closed,
}

impl SessionLifecycleState {
    /// Returns whether no further lifecycle transition is allowed.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Closed)
    }

    /// Returns whether the session still occupies its device/connector association.
    #[must_use]
    pub const fn is_live(self) -> bool {
        !self.is_terminal()
    }

    /// Returns whether this state may transition directly to `next`.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Negotiating | Self::Closing)
                | (Self::Negotiating | Self::Suspended, Self::Active | Self::Closing)
                | (Self::Active, Self::Suspended | Self::Closing)
                | (Self::Closing, Self::Closed)
        )
    }
}

/// Validated key in a deterministic session metadata container.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SessionMetadataKey(Arc<str>);

impl SessionMetadataKey {
    /// Creates a metadata key.
    ///
    /// # Errors
    ///
    /// Returns [`SessionModelError::EmptyMetadataKey`] when `value` is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, SessionModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SessionModelError::EmptyMetadataKey);
        }
        Ok(Self(Arc::from(value)))
    }

    /// Returns the metadata key text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionMetadataKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Immutable value stored in session metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionMetadataValue(Arc<str>);

impl SessionMetadataValue {
    /// Creates a metadata value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(Arc::from(value.into()))
    }

    /// Returns the metadata value text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionMetadataValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Deterministically ordered session metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionMetadata {
    entries: BTreeMap<SessionMetadataKey, SessionMetadataValue>,
}

impl SessionMetadata {
    /// Creates an empty metadata container.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Returns the number of metadata entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the container is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the value associated with `key`.
    #[must_use]
    pub fn get(&self, key: &SessionMetadataKey) -> Option<&SessionMetadataValue> {
        self.entries.get(key)
    }

    /// Inserts or replaces one metadata entry.
    #[must_use]
    pub fn insert(
        &mut self,
        key: SessionMetadataKey,
        value: SessionMetadataValue,
    ) -> Option<SessionMetadataValue> {
        self.entries.insert(key, value)
    }

    /// Removes one metadata entry.
    #[must_use]
    pub fn remove(&mut self, key: &SessionMetadataKey) -> Option<SessionMetadataValue> {
        self.entries.remove(key)
    }

    /// Iterates metadata in stable key order.
    #[must_use]
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&SessionMetadataKey, &SessionMetadataValue)>
           + ExactSizeIterator {
        self.entries.iter()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SessionRecordParts {
    pub(crate) session_id: SessionId,
    pub(crate) created_at: SessionTimestamp,
    pub(crate) last_activity_at: SessionTimestamp,
    pub(crate) lifecycle: SessionLifecycleState,
    pub(crate) device_id: DeviceId,
    pub(crate) connector_id: ConnectorId,
    pub(crate) capabilities: CapabilitySet,
    pub(crate) protocol_version: ProtocolVersion,
    pub(crate) revision: SessionRevision,
    pub(crate) metadata: SessionMetadata,
}

/// Immutable runtime representation of one Bridge session.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeSession {
    session_id: SessionId,
    created_at: SessionTimestamp,
    last_activity_at: SessionTimestamp,
    lifecycle: SessionLifecycleState,
    device_id: DeviceId,
    connector_id: ConnectorId,
    capabilities: CapabilitySet,
    protocol_version: ProtocolVersion,
    revision: SessionRevision,
    metadata: SessionMetadata,
}

impl BridgeSession {
    pub(crate) fn from_parts(parts: SessionRecordParts) -> Self {
        Self {
            session_id: parts.session_id,
            created_at: parts.created_at,
            last_activity_at: parts.last_activity_at,
            lifecycle: parts.lifecycle,
            device_id: parts.device_id,
            connector_id: parts.connector_id,
            capabilities: parts.capabilities,
            protocol_version: parts.protocol_version,
            revision: parts.revision,
            metadata: parts.metadata,
        }
    }

    pub(crate) fn to_parts(&self) -> SessionRecordParts {
        SessionRecordParts {
            session_id: self.session_id.clone(),
            created_at: self.created_at,
            last_activity_at: self.last_activity_at,
            lifecycle: self.lifecycle,
            device_id: self.device_id.clone(),
            connector_id: self.connector_id.clone(),
            capabilities: self.capabilities.clone(),
            protocol_version: self.protocol_version.clone(),
            revision: self.revision,
            metadata: self.metadata.clone(),
        }
    }

    /// Returns the stable session identifier.
    #[must_use]
    pub const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Returns the creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> SessionTimestamp {
        self.created_at
    }

    /// Returns the last activity timestamp.
    #[must_use]
    pub const fn last_activity_at(&self) -> SessionTimestamp {
        self.last_activity_at
    }

    /// Returns the lifecycle state.
    #[must_use]
    pub const fn lifecycle(&self) -> SessionLifecycleState {
        self.lifecycle
    }

    /// Returns the associated device identifier.
    #[must_use]
    pub const fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// Returns the associated connector identifier.
    #[must_use]
    pub const fn connector_id(&self) -> &ConnectorId {
        &self.connector_id
    }

    /// Returns the canonical negotiated capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    /// Returns the canonical negotiated protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> &ProtocolVersion {
        &self.protocol_version
    }

    /// Returns the session-local revision.
    #[must_use]
    pub const fn revision(&self) -> SessionRevision {
        self.revision
    }

    /// Returns the deterministic metadata container.
    #[must_use]
    pub const fn metadata(&self) -> &SessionMetadata {
        &self.metadata
    }
}

impl StateRegistryValue for BridgeSession {
    type Key = SessionId;

    const REGISTRY_KIND: RegistryKind = RegistryKind::Sessions;

    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> {
        Ok(self.session_id.clone())
    }
}

/// Typed lifecycle transition attached to a committed Bridge State event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionStateTransition {
    session_id: SessionId,
    previous: Option<SessionLifecycleState>,
    current: Option<SessionLifecycleState>,
    session_revision: SessionRevision,
    timestamp: SessionTimestamp,
}

impl SessionStateTransition {
    pub(crate) fn new(
        session_id: SessionId,
        previous: Option<SessionLifecycleState>,
        current: Option<SessionLifecycleState>,
        session_revision: SessionRevision,
        timestamp: SessionTimestamp,
    ) -> Self {
        Self {
            session_id,
            previous,
            current,
            session_revision,
            timestamp,
        }
    }

    /// Returns the transitioned session identifier.
    #[must_use]
    pub const fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Returns the lifecycle state before the transaction.
    #[must_use]
    pub const fn previous(&self) -> Option<SessionLifecycleState> {
        self.previous
    }

    /// Returns the lifecycle state after the transaction.
    #[must_use]
    pub const fn current(&self) -> Option<SessionLifecycleState> {
        self.current
    }

    /// Returns the session-local revision associated with the transition.
    #[must_use]
    pub const fn session_revision(&self) -> SessionRevision {
        self.session_revision
    }

    /// Returns the operation timestamp supplied by the Session Manager.
    #[must_use]
    pub const fn timestamp(&self) -> SessionTimestamp {
        self.timestamp
    }
}
