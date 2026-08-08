use std::{collections::BTreeSet, error::Error, fmt, num::NonZeroU64, sync::Arc};

use crate::{
    ConnectionId, RegistryKind, SessionId, StateIdentifierError, StateRegistryValue, TransportId,
};

/// Validation failure for a transport model value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportModelError {
    /// Endpoint addresses must not be empty.
    EmptyEndpointAddress,
    /// Maximum envelope sizes must be greater than zero.
    ZeroMaximumEnvelopeSize,
}

impl fmt::Display for TransportModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyEndpointAddress => "transport endpoint address must not be empty",
            Self::ZeroMaximumEnvelopeSize => {
                "transport maximum envelope size must be greater than zero"
            }
        })
    }
}

impl Error for TransportModelError {}

/// Milliseconds since the Unix epoch supplied by a runtime owner.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TransportTimestamp(u64);

impl TransportTimestamp {
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
}

/// Monotonic revision of one transport connection record.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TransportRevision(u64);

impl TransportRevision {
    /// Initial revision assigned to a newly created connection.
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

/// Lifecycle state of a transport connection.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TransportState {
    /// The connection record exists but no connection attempt has started.
    Created,
    /// A concrete transport is establishing connectivity.
    Connecting,
    /// Connectivity exists but transport authentication is incomplete.
    Connected,
    /// The transport connection completed its authentication boundary.
    Authenticated,
    /// Graceful connection shutdown is in progress.
    Closing,
    /// Connection shutdown completed and no further transitions are allowed.
    Closed,
}

impl TransportState {
    /// Returns whether no further lifecycle transition is permitted.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Closed)
    }

    /// Returns whether this state may transition directly to `next`.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Created, Self::Connecting | Self::Closing)
                | (Self::Connecting, Self::Connected | Self::Closing)
                | (Self::Connected, Self::Authenticated | Self::Closing)
                | (Self::Authenticated, Self::Closing)
                | (Self::Closing, Self::Closed)
        )
    }
}

/// Directional role represented by a transport endpoint.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TransportEndpointRole {
    /// Endpoint initiates connections.
    Dialer,
    /// Endpoint accepts connections.
    Listener,
    /// Endpoint may participate in either direction.
    Peer,
}

/// Validated opaque address interpreted only by its concrete transport factory.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TransportEndpointAddress(Arc<str>);

impl TransportEndpointAddress {
    /// Creates an endpoint address.
    ///
    /// # Errors
    ///
    /// Returns [`TransportModelError::EmptyEndpointAddress`] when `value` is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, TransportModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TransportModelError::EmptyEndpointAddress);
        }
        Ok(Self(Arc::from(value)))
    }

    /// Returns the opaque address text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TransportEndpointAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Transport-independent endpoint descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportEndpoint {
    transport_id: TransportId,
    address: TransportEndpointAddress,
    role: TransportEndpointRole,
}

impl TransportEndpoint {
    /// Creates an endpoint descriptor.
    #[must_use]
    pub const fn new(
        transport_id: TransportId,
        address: TransportEndpointAddress,
        role: TransportEndpointRole,
    ) -> Self {
        Self {
            transport_id,
            address,
            role,
        }
    }

    /// Returns the transport implementation identifier.
    #[must_use]
    pub const fn transport_id(&self) -> &TransportId {
        &self.transport_id
    }

    /// Returns the opaque transport-specific address.
    #[must_use]
    pub const fn address(&self) -> &TransportEndpointAddress {
        &self.address
    }

    /// Returns the endpoint role.
    #[must_use]
    pub const fn role(&self) -> TransportEndpointRole {
        self.role
    }
}

/// Feature advertised by a concrete transport implementation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TransportFeature {
    /// Envelopes are delivered reliably while the connection remains established.
    ReliableDelivery,
    /// Envelopes preserve send order.
    OrderedDelivery,
    /// Both peers may send envelopes.
    Bidirectional,
    /// The transport is intended to remain connected for extended periods.
    Persistent,
    /// Multiple logical sessions may share the underlying transport implementation.
    Multiplexed,
    /// The transport is restricted to the local device or local host boundary.
    LocalOnly,
    /// The concrete transport provides an authenticated secure channel.
    SecureChannel,
}

/// Immutable capability declaration for a transport implementation or connection.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TransportCapabilities {
    features: BTreeSet<TransportFeature>,
    maximum_envelope_size: Option<NonZeroU64>,
}

impl TransportCapabilities {
    /// Creates an empty capability declaration.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            features: BTreeSet::new(),
            maximum_envelope_size: None,
        }
    }

    /// Adds one advertised feature.
    #[must_use]
    pub fn with_feature(mut self, feature: TransportFeature) -> Self {
        self.features.insert(feature);
        self
    }

    /// Sets the maximum accepted envelope size in bytes.
    ///
    /// # Errors
    ///
    /// Returns [`TransportModelError::ZeroMaximumEnvelopeSize`] when `bytes` is zero.
    pub fn with_maximum_envelope_size(mut self, bytes: u64) -> Result<Self, TransportModelError> {
        self.maximum_envelope_size =
            Some(NonZeroU64::new(bytes).ok_or(TransportModelError::ZeroMaximumEnvelopeSize)?);
        Ok(self)
    }

    /// Returns whether a feature is advertised.
    #[must_use]
    pub fn supports(&self, feature: TransportFeature) -> bool {
        self.features.contains(&feature)
    }

    /// Iterates advertised features in deterministic order.
    #[must_use]
    pub fn features(
        &self,
    ) -> impl DoubleEndedIterator<Item = TransportFeature> + ExactSizeIterator + '_ {
        self.features.iter().copied()
    }

    /// Returns the maximum envelope size in bytes when one is declared.
    #[must_use]
    pub fn maximum_envelope_size(&self) -> Option<u64> {
        self.maximum_envelope_size.map(NonZeroU64::get)
    }
}

/// Opaque application message carried by any concrete transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportMessageEnvelope {
    session_id: Option<SessionId>,
    payload: Arc<[u8]>,
}

impl TransportMessageEnvelope {
    /// Creates an unbound envelope containing opaque bytes.
    #[must_use]
    pub fn new(payload: impl Into<Arc<[u8]>>) -> Self {
        Self {
            session_id: None,
            payload: payload.into(),
        }
    }

    /// Associates the envelope with a session without changing its opaque payload.
    #[must_use]
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// Returns the optional session association.
    #[must_use]
    pub const fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    /// Returns the opaque bytes supplied by the protocol layer.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// Monotonic counters reported by a concrete transport connection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TransportStatistics {
    messages_sent: u64,
    messages_received: u64,
    bytes_sent: u64,
    bytes_received: u64,
}

impl TransportStatistics {
    /// Creates a statistics snapshot.
    #[must_use]
    pub const fn new(
        messages_sent: u64,
        messages_received: u64,
        bytes_sent: u64,
        bytes_received: u64,
    ) -> Self {
        Self {
            messages_sent,
            messages_received,
            bytes_sent,
            bytes_received,
        }
    }

    /// Returns sent envelope count.
    #[must_use]
    pub const fn messages_sent(self) -> u64 {
        self.messages_sent
    }

    /// Returns received envelope count.
    #[must_use]
    pub const fn messages_received(self) -> u64 {
        self.messages_received
    }

    /// Returns sent byte count.
    #[must_use]
    pub const fn bytes_sent(self) -> u64 {
        self.bytes_sent
    }

    /// Returns received byte count.
    #[must_use]
    pub const fn bytes_received(self) -> u64 {
        self.bytes_received
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TransportConnectionParts {
    pub(crate) connection_id: ConnectionId,
    pub(crate) transport_id: TransportId,
    pub(crate) endpoint: TransportEndpoint,
    pub(crate) capabilities: TransportCapabilities,
    pub(crate) state: TransportState,
    pub(crate) session_id: Option<SessionId>,
    pub(crate) revision: TransportRevision,
    pub(crate) created_at: TransportTimestamp,
    pub(crate) updated_at: TransportTimestamp,
}

/// Immutable Bridge State representation of one transport connection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportConnectionSnapshot {
    connection_id: ConnectionId,
    transport_id: TransportId,
    endpoint: TransportEndpoint,
    capabilities: TransportCapabilities,
    state: TransportState,
    session_id: Option<SessionId>,
    revision: TransportRevision,
    created_at: TransportTimestamp,
    updated_at: TransportTimestamp,
}

impl TransportConnectionSnapshot {
    pub(crate) fn from_parts(parts: TransportConnectionParts) -> Self {
        Self {
            connection_id: parts.connection_id,
            transport_id: parts.transport_id,
            endpoint: parts.endpoint,
            capabilities: parts.capabilities,
            state: parts.state,
            session_id: parts.session_id,
            revision: parts.revision,
            created_at: parts.created_at,
            updated_at: parts.updated_at,
        }
    }

    pub(crate) fn to_parts(&self) -> TransportConnectionParts {
        TransportConnectionParts {
            connection_id: self.connection_id.clone(),
            transport_id: self.transport_id.clone(),
            endpoint: self.endpoint.clone(),
            capabilities: self.capabilities.clone(),
            state: self.state,
            session_id: self.session_id.clone(),
            revision: self.revision,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    /// Returns the stable connection identifier.
    #[must_use]
    pub const fn connection_id(&self) -> &ConnectionId {
        &self.connection_id
    }

    /// Returns the transport implementation identifier.
    #[must_use]
    pub const fn transport_id(&self) -> &TransportId {
        &self.transport_id
    }

    /// Returns the endpoint descriptor.
    #[must_use]
    pub const fn endpoint(&self) -> &TransportEndpoint {
        &self.endpoint
    }

    /// Returns the immutable transport capability declaration.
    #[must_use]
    pub const fn capabilities(&self) -> &TransportCapabilities {
        &self.capabilities
    }

    /// Returns the lifecycle state.
    #[must_use]
    pub const fn state(&self) -> TransportState {
        self.state
    }

    /// Returns the optional bound session identifier.
    #[must_use]
    pub const fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    /// Returns the connection-local revision.
    #[must_use]
    pub const fn revision(&self) -> TransportRevision {
        self.revision
    }

    /// Returns the creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> TransportTimestamp {
        self.created_at
    }

    /// Returns the most recent lifecycle or binding timestamp.
    #[must_use]
    pub const fn updated_at(&self) -> TransportTimestamp {
        self.updated_at
    }
}

impl StateRegistryValue for TransportConnectionSnapshot {
    type Key = ConnectionId;

    const REGISTRY_KIND: RegistryKind = RegistryKind::Connections;

    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> {
        Ok(self.connection_id.clone())
    }
}

/// Typed transport change attached to a committed Bridge State event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportEvent {
    connection_id: ConnectionId,
    transport_id: TransportId,
    kind: TransportEventKind,
    connection_revision: TransportRevision,
    timestamp: TransportTimestamp,
}

impl TransportEvent {
    pub(crate) fn lifecycle(
        connection_id: ConnectionId,
        transport_id: TransportId,
        previous: Option<TransportState>,
        current: Option<TransportState>,
        connection_revision: TransportRevision,
        timestamp: TransportTimestamp,
    ) -> Self {
        Self {
            connection_id,
            transport_id,
            kind: TransportEventKind::Lifecycle { previous, current },
            connection_revision,
            timestamp,
        }
    }

    pub(crate) fn session_binding(
        connection_id: ConnectionId,
        transport_id: TransportId,
        previous: Option<SessionId>,
        current: Option<SessionId>,
        connection_revision: TransportRevision,
        timestamp: TransportTimestamp,
    ) -> Self {
        Self {
            connection_id,
            transport_id,
            kind: TransportEventKind::SessionBinding { previous, current },
            connection_revision,
            timestamp,
        }
    }

    /// Returns the affected connection identifier.
    #[must_use]
    pub const fn connection_id(&self) -> &ConnectionId {
        &self.connection_id
    }

    /// Returns the transport implementation identifier.
    #[must_use]
    pub const fn transport_id(&self) -> &TransportId {
        &self.transport_id
    }

    /// Returns the typed event kind.
    #[must_use]
    pub const fn kind(&self) -> &TransportEventKind {
        &self.kind
    }

    /// Returns the connection-local revision associated with the event.
    #[must_use]
    pub const fn connection_revision(&self) -> TransportRevision {
        self.connection_revision
    }

    /// Returns the operation timestamp supplied by the caller.
    #[must_use]
    pub const fn timestamp(&self) -> TransportTimestamp {
        self.timestamp
    }

    pub(crate) const fn sort_rank(&self) -> u8 {
        match &self.kind {
            TransportEventKind::Lifecycle { .. } => 0,
            TransportEventKind::SessionBinding { .. } => 1,
        }
    }
}

/// Specific transport change represented by [`TransportEvent`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportEventKind {
    /// Connection lifecycle changed.
    Lifecycle {
        /// Lifecycle state before the operation. Creation uses `None`.
        previous: Option<TransportState>,
        /// Lifecycle state after the operation.
        current: Option<TransportState>,
    },
    /// Session binding changed.
    SessionBinding {
        /// Session bound before the operation.
        previous: Option<SessionId>,
        /// Session bound after the operation.
        current: Option<SessionId>,
    },
}
