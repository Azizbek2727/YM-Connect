use std::{error::Error, fmt, sync::Arc};

use crate::{
    DiscoveryModelError, DiscoveryPeerKey, DiscoveryRevision, DiscoverySource, DiscoveryState,
    DiscoveryTimestamp, StateError,
};

/// Result type used by Discovery Core interfaces and manager operations.
pub type DiscoveryResult<T> = Result<T, DiscoveryError>;

/// Provider operation associated with a concrete implementation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryOperation {
    /// Starting provider-specific discovery.
    Start,
    /// Receiving the next provider advertisement.
    Receive,
    /// Validating provider-specific advertisement authenticity.
    Validate,
    /// Stopping provider-specific discovery.
    Stop,
}

impl fmt::Display for DiscoveryOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Start => "start",
            Self::Receive => "receive",
            Self::Validate => "validate",
            Self::Stop => "stop",
        })
    }
}

/// Structured Discovery Core failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveryError {
    /// Bridge State rejected or failed the transaction.
    State(StateError),
    /// A Discovery Core model value failed validation.
    Model(DiscoveryModelError),
    /// A concrete provider failed one provider-owned operation.
    ProviderOperationFailed {
        /// Provider source identifier.
        source: DiscoverySource,
        /// Failed provider operation.
        operation: DiscoveryOperation,
        /// Stable provider-defined error code.
        code: Arc<str>,
        /// Human-readable diagnostic detail.
        message: Arc<str>,
    },
    /// The requested provider-specific peer does not exist.
    PeerNotFound {
        /// Missing peer key.
        peer_key: DiscoveryPeerKey,
    },
    /// Refreshing an existing peer requires its current optimistic revision.
    RevisionRequired {
        /// Existing peer key.
        peer_key: DiscoveryPeerKey,
    },
    /// A caller supplied a revision for a peer that has not yet been observed.
    UnexpectedRevision {
        /// New peer key.
        peer_key: DiscoveryPeerKey,
        /// Unexpected revision.
        revision: DiscoveryRevision,
    },
    /// The caller supplied an obsolete peer-local revision.
    StaleRevision {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Revision expected by the caller.
        expected: DiscoveryRevision,
        /// Current committed revision.
        actual: DiscoveryRevision,
    },
    /// A terminal peer cannot undergo another ordinary lifecycle transition or refresh.
    TerminalPeer {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Terminal lifecycle state.
        state: DiscoveryState,
    },
    /// The requested lifecycle transition is not part of the finite-state machine.
    InvalidTransition {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Current lifecycle state.
        previous: DiscoveryState,
        /// Requested lifecycle state.
        requested: DiscoveryState,
    },
    /// An operation timestamp moved backwards.
    TimestampRegression {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Last committed timestamp.
        previous: DiscoveryTimestamp,
        /// Rejected timestamp.
        requested: DiscoveryTimestamp,
    },
    /// An advertisement was already expired when observed or validated.
    AdvertisementExpired {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Advertisement expiration timestamp.
        expires_at: DiscoveryTimestamp,
        /// Runtime observation timestamp.
        observed_at: DiscoveryTimestamp,
    },
    /// An advertisement was dated beyond the configured future clock-skew allowance.
    AdvertisementFromFuture {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Signed discovery timestamp.
        discovered_at: DiscoveryTimestamp,
        /// Runtime observation timestamp.
        observed_at: DiscoveryTimestamp,
    },
    /// An advertisement lifetime exceeded policy.
    AdvertisementLifetimeExceeded {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Advertisement lifetime.
        lifetime_ms: u64,
        /// Maximum accepted lifetime.
        maximum_ms: u64,
    },
    /// Provider metadata exceeded policy limits.
    AdvertisementMetadataLimitExceeded {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Advertisement metadata entry count.
        entries: usize,
        /// Advertisement aggregate metadata byte size.
        bytes: u64,
    },
    /// A refreshed advertisement moved its signed discovery timestamp backwards.
    AdvertisementTimestampRegression {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Previously committed advertisement timestamp.
        previous: DiscoveryTimestamp,
        /// Rejected advertisement timestamp.
        requested: DiscoveryTimestamp,
    },
    /// Different advertisements reused the same composite key and signed timestamp.
    AdvertisementConflict {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Conflicting signed discovery timestamp.
        discovered_at: DiscoveryTimestamp,
    },
    /// No protocol version is supported by both policy and advertisement.
    NoCompatibleProtocolVersion {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
    },
    /// The advertisement omitted an application capability required by policy.
    MissingRequiredCapability {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Missing generated capability numeric value.
        capability: i32,
    },
    /// Expiration was requested before the advertisement expiration timestamp.
    ExpirationNotReached {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
        /// Advertisement expiration timestamp.
        expires_at: DiscoveryTimestamp,
        /// Rejected expiration timestamp.
        requested_at: DiscoveryTimestamp,
    },
    /// The peer-local revision reached `u64::MAX`.
    RevisionExhausted {
        /// Affected peer.
        peer_key: DiscoveryPeerKey,
    },
    /// A committed state update violated a Discovery Core invariant.
    StateInvariant {
        /// Stable diagnostic detail.
        message: Arc<str>,
    },
}

impl DiscoveryError {
    /// Creates a structured provider-operation failure.
    #[must_use]
    pub fn provider_operation_failed(
        source: DiscoverySource,
        operation: DiscoveryOperation,
        code: impl Into<Arc<str>>,
        message: impl Into<Arc<str>>,
    ) -> Self {
        Self::ProviderOperationFailed {
            source,
            operation,
            code: code.into(),
            message: message.into(),
        }
    }

    pub(crate) fn state_invariant(message: impl Into<Arc<str>>) -> Self {
        Self::StateInvariant {
            message: message.into(),
        }
    }
}

impl DiscoveryError {
    fn format_operation_and_peer(&self, formatter: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
        match self {
            Self::State(source) => {
                Some(write!(formatter, "Bridge State operation failed: {source}"))
            }
            Self::Model(source) => Some(write!(formatter, "discovery model is invalid: {source}")),
            Self::ProviderOperationFailed {
                source,
                operation,
                code,
                message,
            } => Some(write!(
                formatter,
                "discovery provider {source} failed to {operation} ({code}): {message}"
            )),
            Self::PeerNotFound { peer_key } => Some(write!(
                formatter,
                "discovered peer {peer_key} does not exist"
            )),
            Self::RevisionRequired { peer_key } => Some(write!(
                formatter,
                "refreshing discovered peer {peer_key} requires its current revision"
            )),
            Self::UnexpectedRevision { peer_key, revision } => Some(write!(
                formatter,
                "new discovered peer {peer_key} cannot use revision {}",
                revision.get()
            )),
            Self::StaleRevision {
                peer_key,
                expected,
                actual,
            } => Some(write!(
                formatter,
                "discovered peer {peer_key} revision is stale: expected {}, actual {}",
                expected.get(),
                actual.get()
            )),
            Self::TerminalPeer { peer_key, state } => Some(write!(
                formatter,
                "discovered peer {peer_key} is terminal in state {state:?}"
            )),
            Self::InvalidTransition {
                peer_key,
                previous,
                requested,
            } => Some(write!(
                formatter,
                "discovered peer {peer_key} cannot transition from {previous:?} to {requested:?}"
            )),
            Self::TimestampRegression {
                peer_key,
                previous,
                requested,
            } => Some(write!(
                formatter,
                "discovered peer {peer_key} timestamp regressed from {} to {}",
                previous.as_unix_millis(),
                requested.as_unix_millis()
            )),
            _ => None,
        }
    }

    fn format_advertisement(&self, formatter: &mut fmt::Formatter<'_>) -> Option<fmt::Result> {
        match self {
            Self::AdvertisementExpired {
                peer_key,
                expires_at,
                observed_at,
            } => Some(write!(
                formatter,
                "advertisement for {peer_key} expired at {} before observation at {}",
                expires_at.as_unix_millis(),
                observed_at.as_unix_millis()
            )),
            Self::AdvertisementFromFuture {
                peer_key,
                discovered_at,
                observed_at,
            } => Some(write!(
                formatter,
                "advertisement for {peer_key} is future-dated at {} relative to observation at {}",
                discovered_at.as_unix_millis(),
                observed_at.as_unix_millis()
            )),
            Self::AdvertisementLifetimeExceeded {
                peer_key,
                lifetime_ms,
                maximum_ms,
            } => Some(write!(
                formatter,
                "advertisement for {peer_key} has lifetime {lifetime_ms} ms exceeding policy maximum {maximum_ms} ms"
            )),
            Self::AdvertisementMetadataLimitExceeded {
                peer_key,
                entries,
                bytes,
            } => Some(write!(
                formatter,
                "advertisement for {peer_key} contains {entries} metadata entries and {bytes} bytes beyond policy limits"
            )),
            Self::AdvertisementTimestampRegression {
                peer_key,
                previous,
                requested,
            } => Some(write!(
                formatter,
                "advertisement for {peer_key} regressed from signed timestamp {} to {}",
                previous.as_unix_millis(),
                requested.as_unix_millis()
            )),
            Self::AdvertisementConflict {
                peer_key,
                discovered_at,
            } => Some(write!(
                formatter,
                "advertisement for {peer_key} conflicts at signed timestamp {}",
                discovered_at.as_unix_millis()
            )),
            Self::NoCompatibleProtocolVersion { peer_key } => Some(write!(
                formatter,
                "advertisement for {peer_key} has no compatible protocol version"
            )),
            Self::MissingRequiredCapability {
                peer_key,
                capability,
            } => Some(write!(
                formatter,
                "advertisement for {peer_key} is missing required capability {capability}"
            )),
            Self::ExpirationNotReached {
                peer_key,
                expires_at,
                requested_at,
            } => Some(write!(
                formatter,
                "discovered peer {peer_key} cannot expire at {} before advertisement expiry {}",
                requested_at.as_unix_millis(),
                expires_at.as_unix_millis()
            )),
            _ => None,
        }
    }
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(result) = self.format_operation_and_peer(formatter) {
            return result;
        }
        if let Some(result) = self.format_advertisement(formatter) {
            return result;
        }
        match self {
            Self::RevisionExhausted { peer_key } => {
                write!(
                    formatter,
                    "discovered peer {peer_key} revision is exhausted"
                )
            }
            Self::StateInvariant { message } => {
                write!(
                    formatter,
                    "Discovery Core state invariant failed: {message}"
                )
            }
            _ => formatter.write_str("Discovery Core error formatting invariant failed"),
        }
    }
}

impl Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::Model(source) => Some(source),
            Self::ProviderOperationFailed { .. }
            | Self::PeerNotFound { .. }
            | Self::RevisionRequired { .. }
            | Self::UnexpectedRevision { .. }
            | Self::StaleRevision { .. }
            | Self::TerminalPeer { .. }
            | Self::InvalidTransition { .. }
            | Self::TimestampRegression { .. }
            | Self::AdvertisementExpired { .. }
            | Self::AdvertisementFromFuture { .. }
            | Self::AdvertisementLifetimeExceeded { .. }
            | Self::AdvertisementMetadataLimitExceeded { .. }
            | Self::AdvertisementTimestampRegression { .. }
            | Self::AdvertisementConflict { .. }
            | Self::NoCompatibleProtocolVersion { .. }
            | Self::MissingRequiredCapability { .. }
            | Self::ExpirationNotReached { .. }
            | Self::RevisionExhausted { .. }
            | Self::StateInvariant { .. } => None,
        }
    }
}

impl From<StateError> for DiscoveryError {
    fn from(source: StateError) -> Self {
        Self::State(source)
    }
}

impl From<DiscoveryModelError> for DiscoveryError {
    fn from(source: DiscoveryModelError) -> Self {
        Self::Model(source)
    }
}
