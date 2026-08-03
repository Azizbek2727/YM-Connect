use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

use ym_connect_protocol::v1::{Capability, CapabilitySet, ProtocolVersion};

use crate::{DeviceId, RegistryKind, SessionId, StateIdentifierError, StateRegistryValue};

macro_rules! identifier {
    ($name:ident, $variant:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Arc<str>);
        impl $name {
            /// Creates a validated identifier.
            pub fn new(value: impl Into<String>) -> Result<Self, PairingModelError> {
                let value = value.into();
                if value.is_empty() { return Err(PairingModelError::$variant); }
                Ok(Self(Arc::from(value)))
            }
            /// Returns identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.as_str()) }
        }
    };
}

identifier!(PairingId, EmptyPairingId, "Validated pairing-session identifier.");
identifier!(ChallengeId, EmptyChallengeId, "Validated pairing-challenge identifier.");
identifier!(BridgeId, EmptyBridgeId, "Validated Bridge identity identifier.");

/// Pairing model validation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingModelError {
    /// Pairing identifier was empty.
    EmptyPairingId,
    /// Challenge identifier was empty.
    EmptyChallengeId,
    /// Bridge identifier was empty.
    EmptyBridgeId,
    /// Metadata key was empty.
    EmptyMetadataKey,
    /// Public key was not 32 bytes.
    InvalidPublicKeyLength { actual: usize },
    /// Challenge nonce was not 32 bytes.
    InvalidNonceLength { actual: usize },
    /// Confirmation tag was not 16 bytes.
    InvalidConfirmationTagLength { actual: usize },
    /// Challenge expiry was invalid.
    InvalidChallengeWindow,
    /// Protocol version was invalid.
    InvalidProtocolVersion,
    /// Pairing capabilities were invalid.
    InvalidCapabilities,
}

impl fmt::Display for PairingModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPairingId => f.write_str("pairing identifier must not be empty"),
            Self::EmptyChallengeId => f.write_str("challenge identifier must not be empty"),
            Self::EmptyBridgeId => f.write_str("Bridge identifier must not be empty"),
            Self::EmptyMetadataKey => f.write_str("trust metadata key must not be empty"),
            Self::InvalidPublicKeyLength { actual } => write!(f, "public key must contain 32 bytes, got {actual}"),
            Self::InvalidNonceLength { actual } => write!(f, "challenge nonce must contain 32 bytes, got {actual}"),
            Self::InvalidConfirmationTagLength { actual } => write!(f, "confirmation tag must contain 16 bytes, got {actual}"),
            Self::InvalidChallengeWindow => f.write_str("challenge expiry must follow creation"),
            Self::InvalidProtocolVersion => f.write_str("protocol major version must be non-zero"),
            Self::InvalidCapabilities => f.write_str("pairing capabilities are invalid"),
        }
    }
}
impl Error for PairingModelError {}

/// Unix-millisecond timestamp.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct PairingTimestamp(u64);
impl PairingTimestamp {
    /// Creates a timestamp.
    #[must_use]
    pub const fn from_unix_millis(value: u64) -> Self { Self(value) }
    /// Returns Unix milliseconds.
    #[must_use]
    pub const fn as_unix_millis(self) -> u64 { self.0 }
}

/// Monotonic pairing/trust revision.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct PairingRevision(u64);
impl PairingRevision {
    /// Initial revision.
    pub const INITIAL: Self = Self(0);
    /// Creates a revision.
    #[must_use]
    pub const fn new(value: u64) -> Self { Self(value) }
    /// Returns numeric value.
    #[must_use]
    pub const fn get(self) -> u64 { self.0 }
    pub(crate) const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) { Some(value) => Some(Self(value)), None => None }
    }
}

/// Fixed approved algorithm suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingAlgorithmSuite;
impl PairingAlgorithmSuite {
    /// Key agreement algorithm.
    pub const KEY_AGREEMENT: &'static str = "X25519";
    /// Signature algorithm.
    pub const SIGNATURE: &'static str = "Ed25519";
    /// Key derivation algorithm.
    pub const KEY_DERIVATION: &'static str = "HKDF-SHA-256";
    /// Confirmation algorithm.
    pub const CONFIRMATION: &'static str = "ChaCha20-Poly1305";
}

/// Validated immutable 32-byte public key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PairingPublicKey(Arc<[u8; 32]>);
impl PairingPublicKey {
    /// Creates a key.
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, PairingModelError> {
        let bytes = bytes.as_ref();
        Ok(Self(Arc::new(<[u8; 32]>::try_from(bytes).map_err(|_| PairingModelError::InvalidPublicKeyLength { actual: bytes.len() })?)))
    }
    /// Returns key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

/// Validated immutable 32-byte nonce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingNonce(Arc<[u8; 32]>);
impl PairingNonce {
    /// Creates a nonce.
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, PairingModelError> {
        let bytes = bytes.as_ref();
        Ok(Self(Arc::new(<[u8; 32]>::try_from(bytes).map_err(|_| PairingModelError::InvalidNonceLength { actual: bytes.len() })?)))
    }
    /// Returns nonce bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] { &self.0 }
}

/// Validated immutable 16-byte confirmation tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingConfirmationTag(Arc<[u8; 16]>);
impl PairingConfirmationTag {
    /// Creates a tag.
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, PairingModelError> {
        let bytes = bytes.as_ref();
        Ok(Self(Arc::new(<[u8; 16]>::try_from(bytes).map_err(|_| PairingModelError::InvalidConfirmationTagLength { actual: bytes.len() })?)))
    }
    /// Returns tag bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 16] { &self.0 }
}

/// Immutable Bridge identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeIdentity { id: BridgeId, identity_key: PairingPublicKey }
impl BridgeIdentity {
    /// Creates a Bridge identity.
    #[must_use]
    pub const fn new(id: BridgeId, identity_key: PairingPublicKey) -> Self { Self { id, identity_key } }
    /// Returns identifier.
    #[must_use]
    pub const fn id(&self) -> &BridgeId { &self.id }
    /// Returns Ed25519 public key.
    #[must_use]
    pub const fn identity_key(&self) -> &PairingPublicKey { &self.identity_key }
}

/// Validated canonical pairing capability set.
#[derive(Clone, Debug, PartialEq)]
pub struct PairingCapabilities(CapabilitySet);
impl PairingCapabilities {
    /// Creates validated capabilities.
    pub fn new(mut value: CapabilitySet) -> Result<Self, PairingModelError> {
        value.supported.sort_unstable();
        value.required.sort_unstable();
        let invalid = value.supported.windows(2).any(|v| v[0] == v[1])
            || value.required.windows(2).any(|v| v[0] == v[1])
            || value.supported.iter().any(|v| Capability::try_from(*v).is_err() || *v == 0)
            || value.required.iter().any(|v| Capability::try_from(*v).is_err() || *v == 0)
            || value.required.iter().any(|v| value.supported.binary_search(v).is_err());
        if invalid { return Err(PairingModelError::InvalidCapabilities); }
        Ok(Self(value))
    }
    /// Returns generated canonical value.
    #[must_use]
    pub const fn canonical(&self) -> &CapabilitySet { &self.0 }
}

/// Pairing policy.
#[derive(Clone, Debug, PartialEq)]
pub struct PairingPolicy {
    protocol_version: ProtocolVersion,
    capabilities: PairingCapabilities,
    challenge_lifetime_ms: u64,
    allow_trust_replacement: bool,
    allow_revoked_replacement: bool,
}
impl PairingPolicy {
    /// Creates a policy.
    pub fn new(protocol_version: ProtocolVersion, capabilities: PairingCapabilities, challenge_lifetime_ms: u64, allow_trust_replacement: bool, allow_revoked_replacement: bool) -> Result<Self, PairingModelError> {
        if protocol_version.major == 0 || challenge_lifetime_ms == 0 { return Err(PairingModelError::InvalidProtocolVersion); }
        Ok(Self { protocol_version, capabilities, challenge_lifetime_ms, allow_trust_replacement, allow_revoked_replacement })
    }
    /// Returns protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> &ProtocolVersion { &self.protocol_version }
    /// Returns capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &PairingCapabilities { &self.capabilities }
    /// Returns challenge lifetime.
    #[must_use]
    pub const fn challenge_lifetime_ms(&self) -> u64 { self.challenge_lifetime_ms }
    /// Returns active replacement policy.
    #[must_use]
    pub const fn allow_trust_replacement(&self) -> bool { self.allow_trust_replacement }
    /// Returns revoked replacement policy.
    #[must_use]
    pub const fn allow_revoked_replacement(&self) -> bool { self.allow_revoked_replacement }
}

/// Pairing challenge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingChallenge {
    id: ChallengeId,
    nonce: PairingNonce,
    bridge_ephemeral_key: PairingPublicKey,
    created_at: PairingTimestamp,
    expires_at: PairingTimestamp,
}
impl PairingChallenge {
    /// Creates a challenge.
    pub fn new(id: ChallengeId, nonce: PairingNonce, bridge_ephemeral_key: PairingPublicKey, created_at: PairingTimestamp, expires_at: PairingTimestamp) -> Result<Self, PairingModelError> {
        if expires_at <= created_at { return Err(PairingModelError::InvalidChallengeWindow); }
        Ok(Self { id, nonce, bridge_ephemeral_key, created_at, expires_at })
    }
    /// Returns identifier.
    #[must_use]
    pub const fn id(&self) -> &ChallengeId { &self.id }
    /// Returns nonce.
    #[must_use]
    pub const fn nonce(&self) -> &PairingNonce { &self.nonce }
    /// Returns Bridge X25519 key.
    #[must_use]
    pub const fn bridge_ephemeral_key(&self) -> &PairingPublicKey { &self.bridge_ephemeral_key }
    /// Returns creation time.
    #[must_use]
    pub const fn created_at(&self) -> PairingTimestamp { self.created_at }
    /// Returns expiry time.
    #[must_use]
    pub const fn expires_at(&self) -> PairingTimestamp { self.expires_at }
    /// Returns expiry status.
    #[must_use]
    pub fn is_expired(&self, observed_at: PairingTimestamp) -> bool { observed_at >= self.expires_at }
}

/// Peer pairing request.
#[derive(Clone, Debug, PartialEq)]
pub struct PairingRequest {
    device_id: DeviceId,
    identity_key: PairingPublicKey,
    ephemeral_key: PairingPublicKey,
    protocol_version: ProtocolVersion,
    capabilities: PairingCapabilities,
}
impl PairingRequest {
    /// Creates a request.
    #[must_use]
    pub const fn new(device_id: DeviceId, identity_key: PairingPublicKey, ephemeral_key: PairingPublicKey, protocol_version: ProtocolVersion, capabilities: PairingCapabilities) -> Self {
        Self { device_id, identity_key, ephemeral_key, protocol_version, capabilities }
    }
    /// Returns device identifier.
    #[must_use]
    pub const fn device_id(&self) -> &DeviceId { &self.device_id }
    /// Returns Ed25519 key.
    #[must_use]
    pub const fn identity_key(&self) -> &PairingPublicKey { &self.identity_key }
    /// Returns X25519 key.
    #[must_use]
    pub const fn ephemeral_key(&self) -> &PairingPublicKey { &self.ephemeral_key }
    /// Returns protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> &ProtocolVersion { &self.protocol_version }
    /// Returns capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &PairingCapabilities { &self.capabilities }
}

/// Signed peer response.
#[derive(Clone, Debug, PartialEq)]
pub struct PairingResponse {
    challenge_id: ChallengeId,
    request: PairingRequest,
    signature: Arc<[u8]>,
    confirmation_tag: PairingConfirmationTag,
}
impl PairingResponse {
    /// Creates a response.
    #[must_use]
    pub fn new(challenge_id: ChallengeId, request: PairingRequest, signature: impl Into<Arc<[u8]>>, confirmation_tag: PairingConfirmationTag) -> Self {
        Self { challenge_id, request, signature: signature.into(), confirmation_tag }
    }
    /// Returns challenge identifier.
    #[must_use]
    pub const fn challenge_id(&self) -> &ChallengeId { &self.challenge_id }
    /// Returns request.
    #[must_use]
    pub const fn request(&self) -> &PairingRequest { &self.request }
    /// Returns signature bytes.
    #[must_use]
    pub fn signature(&self) -> &[u8] { &self.signature }
    /// Returns confirmation tag.
    #[must_use]
    pub const fn confirmation_tag(&self) -> &PairingConfirmationTag { &self.confirmation_tag }
}

/// Pairing lifecycle state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PairingState { Idle, ChallengeCreated, ChallengeSent, ResponseReceived, IdentityVerified, TrustEstablished, Completed, Rejected, Expired, Revoked, Cancelled }
impl PairingState {
    /// Returns terminal status.
    #[must_use]
    pub const fn is_terminal(self) -> bool { matches!(self, Self::Completed | Self::Rejected | Self::Expired | Self::Revoked | Self::Cancelled) }
    /// Returns transition validity.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!((self, next),
            (Self::Idle, Self::ChallengeCreated | Self::Cancelled)
            | (Self::ChallengeCreated, Self::ChallengeSent | Self::Expired | Self::Cancelled)
            | (Self::ChallengeSent, Self::ResponseReceived | Self::Rejected | Self::Expired | Self::Cancelled)
            | (Self::ResponseReceived, Self::IdentityVerified | Self::Rejected | Self::Expired | Self::Cancelled)
            | (Self::IdentityVerified, Self::TrustEstablished | Self::Rejected | Self::Revoked | Self::Cancelled)
            | (Self::TrustEstablished, Self::Completed | Self::Revoked))
    }
}

/// Explicit trust decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustDecision { Trust, Replace, Reject }

/// Trust metadata key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TrustMetadataKey(Arc<str>);
impl TrustMetadataKey {
    /// Creates a key.
    pub fn new(value: impl Into<String>) -> Result<Self, PairingModelError> {
        let value = value.into();
        if value.is_empty() { return Err(PairingModelError::EmptyMetadataKey); }
        Ok(Self(Arc::from(value)))
    }
    /// Returns text.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Trust metadata value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustMetadataValue(Arc<str>);
impl TrustMetadataValue {
    /// Creates a value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self { Self(Arc::from(value.into())) }
    /// Returns text.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Deterministic immutable trust metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrustMetadata(BTreeMap<TrustMetadataKey, TrustMetadataValue>);
impl TrustMetadata {
    /// Creates empty metadata.
    #[must_use]
    pub const fn new() -> Self { Self(BTreeMap::new()) }
    /// Inserts an entry.
    #[must_use]
    pub fn insert(&mut self, key: TrustMetadataKey, value: TrustMetadataValue) -> Option<TrustMetadataValue> { self.0.insert(key, value) }
    /// Iterates in deterministic order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&TrustMetadataKey, &TrustMetadataValue)> { self.0.iter() }
}

/// Immutable trusted-peer record.
#[derive(Clone, Debug, PartialEq)]
pub struct TrustedPeer {
    bridge_identity: BridgeIdentity,
    device_id: DeviceId,
    peer_identity_key: PairingPublicKey,
    capabilities: PairingCapabilities,
    protocol_version: ProtocolVersion,
    trusted_at: PairingTimestamp,
    last_verified_at: PairingTimestamp,
    revoked_at: Option<PairingTimestamp>,
    metadata: TrustMetadata,
    revision: PairingRevision,
}
impl TrustedPeer {
    /// Creates a trust record.
    #[must_use]
    pub const fn new(bridge_identity: BridgeIdentity, device_id: DeviceId, peer_identity_key: PairingPublicKey, capabilities: PairingCapabilities, protocol_version: ProtocolVersion, trusted_at: PairingTimestamp, metadata: TrustMetadata, revision: PairingRevision) -> Self {
        Self { bridge_identity, device_id, peer_identity_key, capabilities, protocol_version, trusted_at, last_verified_at: trusted_at, revoked_at: None, metadata, revision }
    }
    /// Returns Bridge identity.
    #[must_use]
    pub const fn bridge_identity(&self) -> &BridgeIdentity { &self.bridge_identity }
    /// Returns device identifier.
    #[must_use]
    pub const fn device_id(&self) -> &DeviceId { &self.device_id }
    /// Returns peer key.
    #[must_use]
    pub const fn peer_identity_key(&self) -> &PairingPublicKey { &self.peer_identity_key }
    /// Returns capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &PairingCapabilities { &self.capabilities }
    /// Returns version.
    #[must_use]
    pub const fn protocol_version(&self) -> &ProtocolVersion { &self.protocol_version }
    /// Returns trust time.
    #[must_use]
    pub const fn trusted_at(&self) -> PairingTimestamp { self.trusted_at }
    /// Returns last verification time.
    #[must_use]
    pub const fn last_verified_at(&self) -> PairingTimestamp { self.last_verified_at }
    /// Returns revocation time.
    #[must_use]
    pub const fn revoked_at(&self) -> Option<PairingTimestamp> { self.revoked_at }
    /// Returns metadata.
    #[must_use]
    pub const fn metadata(&self) -> &TrustMetadata { &self.metadata }
    /// Returns revision.
    #[must_use]
    pub const fn revision(&self) -> PairingRevision { self.revision }
    /// Returns revocation status.
    #[must_use]
    pub const fn is_revoked(&self) -> bool { self.revoked_at.is_some() }
    pub(crate) fn revoked(&self, at: PairingTimestamp) -> Option<Self> {
        let mut next = self.clone();
        next.revision = self.revision.checked_next()?;
        next.last_verified_at = at;
        next.revoked_at = Some(at);
        Some(next)
    }
}
impl StateRegistryValue for TrustedPeer {
    type Key = DeviceId;
    const REGISTRY_KIND: RegistryKind = RegistryKind::TrustedPeers;
    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> { Ok(self.device_id.clone()) }
}

/// Immutable pairing aggregate.
#[derive(Clone, Debug, PartialEq)]
pub struct PairingSession {
    id: PairingId,
    bridge_identity: BridgeIdentity,
    session_id: Option<SessionId>,
    challenge: Option<PairingChallenge>,
    response: Option<PairingResponse>,
    state: PairingState,
    challenge_consumed: bool,
    revision: PairingRevision,
    created_at: PairingTimestamp,
    updated_at: PairingTimestamp,
}
impl PairingSession {
    /// Creates an idle pairing aggregate.
    #[must_use]
    pub const fn new(id: PairingId, bridge_identity: BridgeIdentity, session_id: Option<SessionId>, created_at: PairingTimestamp) -> Self {
        Self { id, bridge_identity, session_id, challenge: None, response: None, state: PairingState::Idle, challenge_consumed: false, revision: PairingRevision::INITIAL, created_at, updated_at: created_at }
    }
    /// Returns identifier.
    #[must_use]
    pub const fn id(&self) -> &PairingId { &self.id }
    /// Returns Bridge identity.
    #[must_use]
    pub const fn bridge_identity(&self) -> &BridgeIdentity { &self.bridge_identity }
    /// Returns optional session.
    #[must_use]
    pub const fn session_id(&self) -> Option<&SessionId> { self.session_id.as_ref() }
    /// Returns challenge.
    #[must_use]
    pub const fn challenge(&self) -> Option<&PairingChallenge> { self.challenge.as_ref() }
    /// Returns response.
    #[must_use]
    pub const fn response(&self) -> Option<&PairingResponse> { self.response.as_ref() }
    /// Returns request.
    #[must_use]
    pub fn request(&self) -> Option<&PairingRequest> { self.response.as_ref().map(PairingResponse::request) }
    /// Returns state.
    #[must_use]
    pub const fn state(&self) -> PairingState { self.state }
    /// Returns consumption status.
    #[must_use]
    pub const fn challenge_consumed(&self) -> bool { self.challenge_consumed }
    /// Returns revision.
    #[must_use]
    pub const fn revision(&self) -> PairingRevision { self.revision }
    /// Returns creation time.
    #[must_use]
    pub const fn created_at(&self) -> PairingTimestamp { self.created_at }
    /// Returns update time.
    #[must_use]
    pub const fn updated_at(&self) -> PairingTimestamp { self.updated_at }
    pub(crate) fn next(&self, state: PairingState, at: PairingTimestamp, challenge: Option<PairingChallenge>, response: Option<PairingResponse>, consume: bool) -> Option<Self> {
        let mut next = self.clone();
        next.revision = self.revision.checked_next()?;
        next.state = state;
        next.updated_at = at;
        if let Some(value) = challenge { next.challenge = Some(value); }
        if let Some(value) = response { next.response = Some(value); }
        next.challenge_consumed |= consume;
        Some(next)
    }
}
impl StateRegistryValue for PairingSession {
    type Key = PairingId;
    const REGISTRY_KIND: RegistryKind = RegistryKind::PairingSessions;
    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> { Ok(self.id.clone()) }
}
