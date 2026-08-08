use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    sync::Arc,
};

use ym_connect_protocol::v1::{Capability, CapabilitySet, ProtocolVersion};

use crate::{
    BridgeId, RegistryKind, StateIdentifierError, StateRegistryValue, StateRevision, TransportId,
};

/// Default maximum accepted advertisement lifetime.
pub const DEFAULT_MAXIMUM_ADVERTISEMENT_LIFETIME_MS: u64 = 120_000;
/// Default accepted future clock skew for signed discovery timestamps.
pub const DEFAULT_MAXIMUM_FUTURE_CLOCK_SKEW_MS: u64 = 5_000;
/// Default maximum number of provider metadata entries.
pub const DEFAULT_MAXIMUM_METADATA_ENTRIES: usize = 64;
/// Default maximum aggregate provider metadata size.
pub const DEFAULT_MAXIMUM_METADATA_BYTES: u64 = 16 * 1024;

/// Discovery model validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveryModelError {
    /// Discovery source identifier was empty.
    EmptyDiscoverySource,
    /// Bridge version was empty.
    EmptyBridgeVersion,
    /// No supported protocol version was supplied.
    EmptyProtocolVersions,
    /// A protocol version had major version zero.
    InvalidProtocolVersion,
    /// A protocol version appeared more than once.
    DuplicateProtocolVersion,
    /// Capability declarations were malformed.
    InvalidCapabilities,
    /// Expiration did not follow discovery time.
    InvalidAdvertisementWindow,
    /// Signed metadata contained an empty algorithm.
    EmptySignatureAlgorithm,
    /// Signed metadata contained an empty key identifier.
    EmptySignatureKeyId,
    /// Signed metadata contained an empty signature.
    EmptySignature,
    /// Provider metadata contained an empty key.
    EmptyMetadataKey,
    /// Policy advertisement lifetime was zero.
    ZeroMaximumAdvertisementLifetime,
    /// Policy metadata-entry limit was zero.
    ZeroMaximumMetadataEntries,
    /// Policy metadata-byte limit was zero.
    ZeroMaximumMetadataBytes,
}

impl fmt::Display for DiscoveryModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyDiscoverySource => "discovery source must not be empty",
            Self::EmptyBridgeVersion => "Bridge version must not be empty",
            Self::EmptyProtocolVersions => "at least one protocol version is required",
            Self::InvalidProtocolVersion => "protocol major version must be non-zero",
            Self::DuplicateProtocolVersion => "protocol versions must be unique",
            Self::InvalidCapabilities => "discovery capabilities are invalid",
            Self::InvalidAdvertisementWindow => {
                "advertisement expiration must follow discovery time"
            }
            Self::EmptySignatureAlgorithm => "signature algorithm must not be empty",
            Self::EmptySignatureKeyId => "signature key identifier must not be empty",
            Self::EmptySignature => "signature bytes must not be empty",
            Self::EmptyMetadataKey => "provider metadata key must not be empty",
            Self::ZeroMaximumAdvertisementLifetime => {
                "maximum advertisement lifetime must be greater than zero"
            }
            Self::ZeroMaximumMetadataEntries => {
                "maximum provider metadata entries must be greater than zero"
            }
            Self::ZeroMaximumMetadataBytes => {
                "maximum provider metadata bytes must be greater than zero"
            }
        })
    }
}

impl Error for DiscoveryModelError {}

/// Milliseconds since the Unix epoch supplied by a runtime owner.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiscoveryTimestamp(u64);

impl DiscoveryTimestamp {
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

/// Monotonic revision of one discovered-peer record.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct DiscoveryRevision(u64);

impl DiscoveryRevision {
    /// Initial revision assigned to a newly discovered peer.
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

/// Stable identifier of one discovery provider or provider instance.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiscoverySource(Arc<str>);

impl DiscoverySource {
    /// Creates a validated discovery source identifier.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryModelError::EmptyDiscoverySource`] when `value` is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, DiscoveryModelError> {
        let value = value.into();
        if value.is_empty() {
            return Err(DiscoveryModelError::EmptyDiscoverySource);
        }
        Ok(Self(Arc::from(value)))
    }

    /// Returns the source identifier text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DiscoverySource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Composite identity of one provider-specific discovered peer.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiscoveryPeerKey {
    bridge_id: BridgeId,
    source: DiscoverySource,
    transport_id: TransportId,
}

impl DiscoveryPeerKey {
    /// Creates a provider-specific peer key.
    #[must_use]
    pub const fn new(
        bridge_id: BridgeId,
        source: DiscoverySource,
        transport_id: TransportId,
    ) -> Self {
        Self {
            bridge_id,
            source,
            transport_id,
        }
    }

    /// Returns the advertised Bridge identifier.
    #[must_use]
    pub const fn bridge_id(&self) -> &BridgeId {
        &self.bridge_id
    }

    /// Returns the discovery source.
    #[must_use]
    pub const fn source(&self) -> &DiscoverySource {
        &self.source
    }

    /// Returns the advertised transport identifier.
    #[must_use]
    pub const fn transport_id(&self) -> &TransportId {
        &self.transport_id
    }
}

impl fmt::Display for DiscoveryPeerKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}@{}:{}",
            self.bridge_id, self.source, self.transport_id
        )
    }
}

/// Discovery lifecycle state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiscoveryState {
    /// Discovery is inactive while a prior peer record remains retained.
    Idle,
    /// A provider is actively rediscovering a retained peer.
    Discovering,
    /// A structurally and policy-valid advertisement has been observed.
    AdvertisementReceived,
    /// Provider-specific authenticity validation succeeded.
    Validated,
    /// The peer is currently available for subsequent connection orchestration.
    Available,
    /// The peer is retained but currently unavailable.
    Unavailable,
    /// The advertisement expired and no ordinary transition is permitted.
    Expired,
    /// The record was administratively removed from the registry.
    Removed,
}

impl DiscoveryState {
    /// Returns whether the state permits no further lifecycle transition.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Expired | Self::Removed)
    }

    /// Returns whether the state may transition directly to `next`.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Idle, Self::Discovering | Self::AdvertisementReceived)
                | (
                    Self::Discovering | Self::Available,
                    Self::AdvertisementReceived | Self::Unavailable | Self::Expired
                )
                | (
                    Self::AdvertisementReceived,
                    Self::Validated | Self::Unavailable | Self::Expired
                )
                | (
                    Self::Validated,
                    Self::AdvertisementReceived
                        | Self::Available
                        | Self::Unavailable
                        | Self::Expired
                )
                | (
                    Self::Unavailable,
                    Self::Idle | Self::Discovering | Self::AdvertisementReceived | Self::Expired
                )
        )
    }
}

/// Signature metadata attached to an immutable advertisement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdvertisementSignatureMetadata {
    /// The provider supplies authenticity through a non-signature mechanism.
    Unsigned,
    /// The advertisement carries opaque signature bytes and their interpretation metadata.
    Signed {
        /// Provider-defined signature algorithm identifier.
        algorithm: Arc<str>,
        /// Provider-defined verification-key identifier.
        key_id: Arc<str>,
        /// Opaque signature bytes.
        signature: Arc<[u8]>,
    },
}

impl AdvertisementSignatureMetadata {
    /// Creates signed advertisement metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured validation error when any required value is empty.
    pub fn signed(
        algorithm: impl Into<String>,
        key_id: impl Into<String>,
        signature: impl Into<Arc<[u8]>>,
    ) -> Result<Self, DiscoveryModelError> {
        let algorithm = algorithm.into();
        if algorithm.is_empty() {
            return Err(DiscoveryModelError::EmptySignatureAlgorithm);
        }
        let key_id = key_id.into();
        if key_id.is_empty() {
            return Err(DiscoveryModelError::EmptySignatureKeyId);
        }
        let signature = signature.into();
        if signature.is_empty() {
            return Err(DiscoveryModelError::EmptySignature);
        }
        Ok(Self::Signed {
            algorithm: Arc::from(algorithm),
            key_id: Arc::from(key_id),
            signature,
        })
    }

    /// Returns the signature algorithm when the advertisement is signed.
    #[must_use]
    pub fn algorithm(&self) -> Option<&str> {
        match self {
            Self::Unsigned => None,
            Self::Signed { algorithm, .. } => Some(algorithm),
        }
    }

    /// Returns the verification-key identifier when the advertisement is signed.
    #[must_use]
    pub fn key_id(&self) -> Option<&str> {
        match self {
            Self::Unsigned => None,
            Self::Signed { key_id, .. } => Some(key_id),
        }
    }

    /// Returns opaque signature bytes when the advertisement is signed.
    #[must_use]
    pub fn signature(&self) -> Option<&[u8]> {
        match self {
            Self::Unsigned => None,
            Self::Signed { signature, .. } => Some(signature),
        }
    }
}

/// Validated canonical application capability set advertised through discovery.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveryCapabilities(CapabilitySet);

impl DiscoveryCapabilities {
    /// Creates validated capabilities while preserving generated Protocol Buffer ownership.
    ///
    /// # Errors
    ///
    /// Returns [`DiscoveryModelError::InvalidCapabilities`] for unknown values, duplicates,
    /// unspecified values, or required capabilities absent from the supported set.
    pub fn new(mut value: CapabilitySet) -> Result<Self, DiscoveryModelError> {
        value.supported.sort_unstable();
        value.required.sort_unstable();
        let invalid = value.supported.windows(2).any(|items| items[0] == items[1])
            || value.required.windows(2).any(|items| items[0] == items[1])
            || value
                .supported
                .iter()
                .any(|item| *item == 0 || Capability::try_from(*item).is_err())
            || value
                .required
                .iter()
                .any(|item| *item == 0 || Capability::try_from(*item).is_err())
            || value
                .required
                .iter()
                .any(|item| value.supported.binary_search(item).is_err());
        if invalid {
            return Err(DiscoveryModelError::InvalidCapabilities);
        }
        Ok(Self(value))
    }

    /// Returns the generated canonical capability set.
    #[must_use]
    pub const fn canonical(&self) -> &CapabilitySet {
        &self.0
    }

    /// Returns whether a raw generated capability value is supported.
    #[must_use]
    pub fn supports_raw(&self, capability: i32) -> bool {
        self.0.supported.binary_search(&capability).is_ok()
    }

    /// Iterates supported raw generated capability values in deterministic order.
    #[must_use]
    pub fn supported(&self) -> impl ExactSizeIterator<Item = i32> + DoubleEndedIterator + '_ {
        self.0.supported.iter().copied()
    }
}

/// Immutable provider advertisement.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveryAdvertisement {
    bridge_id: BridgeId,
    transport_id: TransportId,
    supported_protocol_versions: Arc<[ProtocolVersion]>,
    capabilities: DiscoveryCapabilities,
    bridge_version: Arc<str>,
    discovered_at: DiscoveryTimestamp,
    expires_at: DiscoveryTimestamp,
    signature: AdvertisementSignatureMetadata,
    provider_metadata: BTreeMap<Arc<str>, Arc<[u8]>>,
}

impl DiscoveryAdvertisement {
    /// Creates an immutable advertisement.
    ///
    /// # Errors
    ///
    /// Returns a structured model error when required fields, protocol versions, timestamps, or
    /// provider metadata are invalid.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bridge_id: BridgeId,
        transport_id: TransportId,
        mut supported_protocol_versions: Vec<ProtocolVersion>,
        capabilities: DiscoveryCapabilities,
        bridge_version: impl Into<String>,
        discovered_at: DiscoveryTimestamp,
        expires_at: DiscoveryTimestamp,
        signature: AdvertisementSignatureMetadata,
        provider_metadata: BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, DiscoveryModelError> {
        let bridge_version = bridge_version.into();
        if bridge_version.is_empty() {
            return Err(DiscoveryModelError::EmptyBridgeVersion);
        }
        normalize_protocol_versions(&mut supported_protocol_versions)?;
        if expires_at <= discovered_at {
            return Err(DiscoveryModelError::InvalidAdvertisementWindow);
        }
        let provider_metadata = provider_metadata
            .into_iter()
            .map(|(key, value)| {
                if key.is_empty() {
                    Err(DiscoveryModelError::EmptyMetadataKey)
                } else {
                    Ok((Arc::from(key), Arc::from(value)))
                }
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        Ok(Self {
            bridge_id,
            transport_id,
            supported_protocol_versions: supported_protocol_versions.into(),
            capabilities,
            bridge_version: Arc::from(bridge_version),
            discovered_at,
            expires_at,
            signature,
            provider_metadata,
        })
    }

    /// Returns the advertised Bridge identifier.
    #[must_use]
    pub const fn bridge_id(&self) -> &BridgeId {
        &self.bridge_id
    }

    /// Returns the advertised transport identifier.
    #[must_use]
    pub const fn transport_id(&self) -> &TransportId {
        &self.transport_id
    }

    /// Returns supported generated protocol versions in deterministic ascending order.
    #[must_use]
    pub fn supported_protocol_versions(&self) -> &[ProtocolVersion] {
        &self.supported_protocol_versions
    }

    /// Returns the advertised generated application capabilities.
    #[must_use]
    pub fn capabilities(&self) -> &DiscoveryCapabilities {
        &self.capabilities
    }

    /// Returns the advertised Bridge version.
    #[must_use]
    pub fn bridge_version(&self) -> &str {
        &self.bridge_version
    }

    /// Returns the signed discovery timestamp.
    #[must_use]
    pub const fn discovered_at(&self) -> DiscoveryTimestamp {
        self.discovered_at
    }

    /// Returns the expiration timestamp.
    #[must_use]
    pub const fn expires_at(&self) -> DiscoveryTimestamp {
        self.expires_at
    }

    /// Returns signature interpretation metadata.
    #[must_use]
    pub const fn signature_metadata(&self) -> &AdvertisementSignatureMetadata {
        &self.signature
    }

    /// Returns provider-specific opaque metadata in deterministic key order.
    #[must_use]
    pub const fn provider_metadata(&self) -> &BTreeMap<Arc<str>, Arc<[u8]>> {
        &self.provider_metadata
    }

    /// Returns the total advertisement lifetime in milliseconds.
    #[must_use]
    pub const fn lifetime_ms(&self) -> u64 {
        self.expires_at
            .as_unix_millis()
            .saturating_sub(self.discovered_at.as_unix_millis())
    }

    /// Returns whether the advertisement is expired at `timestamp`.
    #[must_use]
    pub const fn is_expired_at(&self, timestamp: DiscoveryTimestamp) -> bool {
        timestamp.as_unix_millis() >= self.expires_at.as_unix_millis()
    }

    pub(crate) fn metadata_size_bytes(&self) -> u64 {
        self.provider_metadata
            .iter()
            .fold(0_u64, |total, (key, value)| {
                let key_bytes = u64::try_from(key.len()).unwrap_or(u64::MAX);
                let value_bytes = u64::try_from(value.len()).unwrap_or(u64::MAX);
                total.saturating_add(key_bytes).saturating_add(value_bytes)
            })
    }
}

/// Immutable discovery acceptance and retention policy.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveryPolicy {
    supported_protocol_versions: Arc<[ProtocolVersion]>,
    required_capabilities: Arc<[i32]>,
    maximum_advertisement_lifetime_ms: u64,
    maximum_future_clock_skew_ms: u64,
    maximum_metadata_entries: usize,
    maximum_metadata_bytes: u64,
}

impl DiscoveryPolicy {
    /// Creates a validated discovery policy.
    ///
    /// # Errors
    ///
    /// Returns a structured validation error for invalid versions, capabilities, or zero limits.
    pub fn new(
        mut supported_protocol_versions: Vec<ProtocolVersion>,
        mut required_capabilities: Vec<i32>,
        maximum_advertisement_lifetime_ms: u64,
        maximum_future_clock_skew_ms: u64,
        maximum_metadata_entries: usize,
        maximum_metadata_bytes: u64,
    ) -> Result<Self, DiscoveryModelError> {
        normalize_protocol_versions(&mut supported_protocol_versions)?;
        required_capabilities.sort_unstable();
        let invalid_capabilities = required_capabilities
            .windows(2)
            .any(|items| items[0] == items[1])
            || required_capabilities
                .iter()
                .any(|item| *item == 0 || Capability::try_from(*item).is_err());
        if invalid_capabilities {
            return Err(DiscoveryModelError::InvalidCapabilities);
        }
        if maximum_advertisement_lifetime_ms == 0 {
            return Err(DiscoveryModelError::ZeroMaximumAdvertisementLifetime);
        }
        if maximum_metadata_entries == 0 {
            return Err(DiscoveryModelError::ZeroMaximumMetadataEntries);
        }
        if maximum_metadata_bytes == 0 {
            return Err(DiscoveryModelError::ZeroMaximumMetadataBytes);
        }
        Ok(Self {
            supported_protocol_versions: supported_protocol_versions.into(),
            required_capabilities: required_capabilities.into(),
            maximum_advertisement_lifetime_ms,
            maximum_future_clock_skew_ms,
            maximum_metadata_entries,
            maximum_metadata_bytes,
        })
    }

    /// Returns locally supported generated protocol versions.
    #[must_use]
    pub fn supported_protocol_versions(&self) -> &[ProtocolVersion] {
        &self.supported_protocol_versions
    }

    /// Returns required raw generated capability values.
    #[must_use]
    pub fn required_capabilities(&self) -> &[i32] {
        &self.required_capabilities
    }

    /// Returns the maximum accepted advertisement lifetime.
    #[must_use]
    pub const fn maximum_advertisement_lifetime_ms(&self) -> u64 {
        self.maximum_advertisement_lifetime_ms
    }

    /// Returns the maximum accepted future clock skew.
    #[must_use]
    pub const fn maximum_future_clock_skew_ms(&self) -> u64 {
        self.maximum_future_clock_skew_ms
    }

    /// Returns the maximum provider metadata entry count.
    #[must_use]
    pub const fn maximum_metadata_entries(&self) -> usize {
        self.maximum_metadata_entries
    }

    /// Returns the maximum aggregate provider metadata size.
    #[must_use]
    pub const fn maximum_metadata_bytes(&self) -> u64 {
        self.maximum_metadata_bytes
    }
}

impl Default for DiscoveryPolicy {
    fn default() -> Self {
        Self {
            supported_protocol_versions: vec![ProtocolVersion {
                major: 1,
                minor: 0,
                patch: 0,
            }]
            .into(),
            required_capabilities: Arc::from([]),
            maximum_advertisement_lifetime_ms: DEFAULT_MAXIMUM_ADVERTISEMENT_LIFETIME_MS,
            maximum_future_clock_skew_ms: DEFAULT_MAXIMUM_FUTURE_CLOCK_SKEW_MS,
            maximum_metadata_entries: DEFAULT_MAXIMUM_METADATA_ENTRIES,
            maximum_metadata_bytes: DEFAULT_MAXIMUM_METADATA_BYTES,
        }
    }
}

/// Deterministic typed discovered-peer filter.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryFilter {
    bridge_ids: BTreeSet<BridgeId>,
    transport_ids: BTreeSet<TransportId>,
    sources: BTreeSet<DiscoverySource>,
    required_capabilities: BTreeSet<i32>,
    protocol_versions: BTreeSet<(u32, u32, u32)>,
}

impl DiscoveryFilter {
    /// Creates a filter that accepts every discovered peer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bridge_ids: BTreeSet::new(),
            transport_ids: BTreeSet::new(),
            sources: BTreeSet::new(),
            required_capabilities: BTreeSet::new(),
            protocol_versions: BTreeSet::new(),
        }
    }

    /// Restricts results to one Bridge identifier.
    #[must_use]
    pub fn with_bridge_id(mut self, bridge_id: BridgeId) -> Self {
        self.bridge_ids.insert(bridge_id);
        self
    }

    /// Restricts results to one transport identifier.
    #[must_use]
    pub fn with_transport_id(mut self, transport_id: TransportId) -> Self {
        self.transport_ids.insert(transport_id);
        self
    }

    /// Restricts results to one discovery source.
    #[must_use]
    pub fn with_source(mut self, source: DiscoverySource) -> Self {
        self.sources.insert(source);
        self
    }

    /// Requires one raw generated application capability.
    #[must_use]
    pub fn requiring_capability(mut self, capability: i32) -> Self {
        self.required_capabilities.insert(capability);
        self
    }

    /// Restricts results to one selected generated protocol version.
    #[must_use]
    pub fn with_protocol_version(mut self, version: &ProtocolVersion) -> Self {
        self.protocol_versions.insert(version_key(version));
        self
    }

    /// Returns whether `peer` satisfies every configured criterion.
    #[must_use]
    pub fn matches(&self, peer: &DiscoveredPeer) -> bool {
        (self.bridge_ids.is_empty() || self.bridge_ids.contains(peer.bridge_id()))
            && (self.transport_ids.is_empty() || self.transport_ids.contains(peer.transport_id()))
            && (self.sources.is_empty() || self.sources.contains(peer.source()))
            && self
                .required_capabilities
                .iter()
                .all(|capability| peer.capabilities().supports_raw(*capability))
            && (self.protocol_versions.is_empty()
                || self
                    .protocol_versions
                    .contains(&version_key(peer.protocol_version())))
    }
}

/// Immutable discovered-peer record stored by Bridge State.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveredPeer {
    key: DiscoveryPeerKey,
    state: DiscoveryState,
    advertisement: Arc<DiscoveryAdvertisement>,
    last_observed_at: DiscoveryTimestamp,
    protocol_version: ProtocolVersion,
    revision: DiscoveryRevision,
}

impl DiscoveredPeer {
    pub(crate) fn new(
        key: DiscoveryPeerKey,
        advertisement: DiscoveryAdvertisement,
        observed_at: DiscoveryTimestamp,
        protocol_version: ProtocolVersion,
    ) -> Self {
        Self {
            key,
            state: DiscoveryState::AdvertisementReceived,
            advertisement: Arc::new(advertisement),
            last_observed_at: observed_at,
            protocol_version,
            revision: DiscoveryRevision::INITIAL,
        }
    }

    pub(crate) fn refreshed(
        &self,
        advertisement: DiscoveryAdvertisement,
        observed_at: DiscoveryTimestamp,
        protocol_version: ProtocolVersion,
    ) -> Option<Self> {
        Some(Self {
            key: self.key.clone(),
            state: DiscoveryState::AdvertisementReceived,
            advertisement: Arc::new(advertisement),
            last_observed_at: observed_at,
            protocol_version,
            revision: self.revision.checked_next()?,
        })
    }

    pub(crate) fn transitioned(
        &self,
        state: DiscoveryState,
        timestamp: DiscoveryTimestamp,
    ) -> Option<Self> {
        Some(Self {
            key: self.key.clone(),
            state,
            advertisement: Arc::clone(&self.advertisement),
            last_observed_at: timestamp,
            protocol_version: self.protocol_version.clone(),
            revision: self.revision.checked_next()?,
        })
    }

    /// Returns the provider-specific registry key.
    #[must_use]
    pub const fn key(&self) -> &DiscoveryPeerKey {
        &self.key
    }

    /// Returns the Bridge identifier.
    #[must_use]
    pub const fn bridge_id(&self) -> &BridgeId {
        self.key.bridge_id()
    }

    /// Returns the discovery source.
    #[must_use]
    pub const fn source(&self) -> &DiscoverySource {
        self.key.source()
    }

    /// Returns the transport identifier.
    #[must_use]
    pub const fn transport_id(&self) -> &TransportId {
        self.key.transport_id()
    }

    /// Returns the lifecycle state.
    #[must_use]
    pub const fn state(&self) -> DiscoveryState {
        self.state
    }

    /// Returns the current immutable advertisement.
    #[must_use]
    pub fn current_advertisement(&self) -> &DiscoveryAdvertisement {
        &self.advertisement
    }

    /// Returns the last observation or lifecycle timestamp.
    #[must_use]
    pub const fn last_observed_at(&self) -> DiscoveryTimestamp {
        self.last_observed_at
    }

    /// Returns the selected generated protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> &ProtocolVersion {
        &self.protocol_version
    }

    /// Returns the peer-local revision.
    #[must_use]
    pub const fn revision(&self) -> DiscoveryRevision {
        self.revision
    }

    /// Returns the current generated application capabilities.
    #[must_use]
    pub fn capabilities(&self) -> &DiscoveryCapabilities {
        self.advertisement.capabilities()
    }

    /// Returns current provider-specific metadata.
    #[must_use]
    pub fn metadata(&self) -> &BTreeMap<Arc<str>, Arc<[u8]>> {
        self.advertisement.provider_metadata()
    }
}

impl StateRegistryValue for DiscoveredPeer {
    type Key = DiscoveryPeerKey;

    const REGISTRY_KIND: RegistryKind = RegistryKind::Discoveries;

    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> {
        Ok(self.key.clone())
    }
}

/// Immutable aggregate discovery snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoverySnapshot {
    state_revision: StateRevision,
    peers: Arc<[Arc<DiscoveredPeer>]>,
}

impl DiscoverySnapshot {
    pub(crate) fn new(state_revision: StateRevision, peers: Vec<Arc<DiscoveredPeer>>) -> Self {
        Self {
            state_revision,
            peers: peers.into(),
        }
    }

    /// Returns the Bridge State revision represented by this snapshot.
    #[must_use]
    pub const fn state_revision(&self) -> StateRevision {
        self.state_revision
    }

    /// Returns peers in deterministic composite-key order.
    #[must_use]
    pub fn peers(&self) -> &[Arc<DiscoveredPeer>] {
        &self.peers
    }

    /// Returns matching peers in deterministic snapshot order.
    #[must_use]
    pub fn filtered(&self, filter: &DiscoveryFilter) -> Vec<Arc<DiscoveredPeer>> {
        self.filtered_with(filter, |_| true)
    }

    /// Returns matching peers and applies one caller-supplied custom predicate in deterministic
    /// snapshot order.
    #[must_use]
    pub fn filtered_with(
        &self,
        filter: &DiscoveryFilter,
        predicate: impl Fn(&DiscoveredPeer) -> bool,
    ) -> Vec<Arc<DiscoveredPeer>> {
        self.peers
            .iter()
            .filter(|peer| filter.matches(peer) && predicate(peer))
            .cloned()
            .collect()
    }
}

pub(crate) const fn version_key(version: &ProtocolVersion) -> (u32, u32, u32) {
    (version.major, version.minor, version.patch)
}

fn normalize_protocol_versions(
    versions: &mut [ProtocolVersion],
) -> Result<(), DiscoveryModelError> {
    if versions.is_empty() {
        return Err(DiscoveryModelError::EmptyProtocolVersions);
    }
    if versions.iter().any(|version| version.major == 0) {
        return Err(DiscoveryModelError::InvalidProtocolVersion);
    }
    versions.sort_by_key(version_key);
    if versions
        .windows(2)
        .any(|items| version_key(&items[0]) == version_key(&items[1]))
    {
        return Err(DiscoveryModelError::DuplicateProtocolVersion);
    }
    Ok(())
}
