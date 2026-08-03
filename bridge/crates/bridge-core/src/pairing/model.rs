use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

use ym_connect_protocol::v1::{Capability, CapabilitySet, ProtocolVersion};

use crate::{DeviceId, RegistryKind, SessionId, StateIdentifierError, StateRegistryValue};

const PUBLIC_KEY_LENGTH: usize = 32;
const CHALLENGE_NONCE_LENGTH: usize = 32;
const CONFIRMATION_TAG_LENGTH: usize = 16;

macro_rules! define_identifier {
    ($name:ident, $error:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Arc<str>);

        impl $name {
            /// Creates a validated identifier.
            ///
            /// # Errors
            ///
            /// Returns the corresponding model error when `value` is empty.
            pub fn new(value: impl Into<String>) -> Result<Self, PairingModelError> {
                let value = value.into();
                if value.is_empty() {
                    return Err(PairingModelError::$error);
                }
                Ok(Self(Arc::from(value)))
            }

            /// Returns the identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }
    };
}

define_identifier!(PairingId, EmptyPairingId, "Validated pairing-session identifier.");
define_identifier!(ChallengeId, EmptyChallengeId, "Validated pairing-challenge identifier.");
define_identifier!(BridgeId, EmptyBridgeId, "Validated Bridge identity identifier.");

/// Validation failure for a Pairing Core model value.
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
    /// An Ed25519 or X25519 public key had an invalid length.
    InvalidPublicKeyLength { actual: usize },
    /// A challenge nonce had an invalid length.
    InvalidChallengeNonceLength { actual: usize },
    /// A ChaCha20-Poly1305 confirmation tag had an invalid length.
    InvalidConfirmationTagLength { actual: usize },
    /// Challenge expiry did not follow creation time.
    InvalidChallengeWindow,
    /// A protocol version used major version zero.
    InvalidProtocolVersion,
    /// Pairing capabilities were malformed or omitted required security capabilities.
    InvalidCapabilities,
}

impl fmt::Display for PairingModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPairingId => formatter.write_str("pairing identifier must not be empty"),
            Self::EmptyChallengeId => formatter.write_str("challenge identifier must not be empty"),
            Self::EmptyBridgeId => formatter.write_str("Bridge identifier must not be empty"),
            Self::EmptyMetadataKey => formatter.write_str("trust metadata key must not be empty"),
            Self::InvalidPublicKeyLength { actual } => write!(
                formatter,
                "identity and ephemeral public keys must contain {PUBLIC_KEY_LENGTH} bytes, got {actual}"
            ),
            Self::InvalidChallengeNonceLength { actual } => write!(
                formatter,
                "pairing challenge nonce must contain {CHALLENGE_NONCE_LENGTH} bytes, got {actual}"
            ),
            Self::InvalidConfirmationTagLength { actual } => write!(
                formatter,
                "pairing confirmation tag must contain {CONFIRMATION_TAG_LENGTH} bytes, got {actual}"
            ),
            Self::InvalidChallengeWindow => {
                formatter.write_str("pairing challenge expiry must be later than creation")
            }
            Self::InvalidProtocolVersion => {
                formatter.write_str("pairing protocol major version must be non-zero")
            }
            Self::InvalidCapabilities => formatter.write_str(
                "pairing capabilities must be unique, valid, and include required security capabilities",
            ),
        }
    }
}

impl Error for PairingModelError {}

/// Milliseconds since the Unix epoch used by Pairing Core.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct PairingTimestamp(u64);

impl PairingTimestamp {
    /// Creates a timestamp from Unix milliseconds.
    #[must_use]
    pub const fn from_unix_millis(value: u64) -> Self {
        Self(value)
    }

    /// Returns Unix milliseconds.
    #[must_use]
    pub const fn as_unix_millis(self) -> u64 {
        self.0
    }
}

/// Monotonic revision of a pairing or trust record.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct PairingRevision(u64);

impl PairingRevision {
    /// Initial revision.
    pub const INITIAL: Self = Self(0);

    /// Creates a revision.
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

/// Fixed cryptographic algorithm suite permitted by Pairing Core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairingAlgorithmSuite;

impl PairingAlgorithmSuite {
    /// X25519 key agreement identifier.
    pub const KEY_AGREEMENT: &'static str = "X25519";
    /// Ed25519 signature identifier.
    pub const SIGNATURE: &'static str = "Ed25519";
    /// HKDF-SHA-256 key derivation identifier.
    pub const KEY_DERIVATION: &'static str = "HKDF-SHA-256";
    /// ChaCha20-Poly1305 confirmation identifier.
    pub const CONFIRMATION: &'static str = "ChaCha20-Poly1305";
}

/// Validated immutable 32-byte public key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PairingPublicKey(Arc<[u8; PUBLIC_KEY_LENGTH]>);

impl PairingPublicKey {
    /// Creates a validated public key.
    ///
    /// # Errors
    ///
    /// Returns [`PairingModelError::InvalidPublicKeyLength`] for non-32-byte input.
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, PairingModelError> {
        let bytes = bytes.as_ref();
        let array = <[u8; PUBLIC_KEY_LENGTH]>::try_from(bytes).map_err(|_| {
            PairingModelError::InvalidPublicKeyLength {
                actual: bytes.len(),
            }
        })?;
        Ok(Self(Arc::new(array)))
    }

    /// Returns public-key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; PUBLIC_KEY_LENGTH] {
        &self.0
    }
}

/// Immutable challenge nonce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingNonce(Arc<[u8; CHALLENGE_NONCE_LENGTH]>);

impl PairingNonce {
    /// Creates a validated challenge nonce.
    ///
    /// # Errors
    ///
    /// Returns a model error for non-32-byte input.
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, PairingModelError> {
        let bytes = bytes.as_ref();
        let array = <[u8; CHALLENGE_NONCE_LENGTH]>::try_from(bytes).map_err(|_| {
            PairingModelError::InvalidChallengeNonceLength {
                actual: bytes.len(),
            }
        })?;
        Ok(Self(Arc::new(array)))
    }

    /// Returns nonce bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; CHALLENGE_NONCE_LENGTH] {
        &self.0
    }
}

/// Immutable ChaCha20-Poly1305 authentication tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingConfirmationTag(Arc<[u8; CONFIRMATION_TAG_LENGTH]>);

impl PairingConfirmationTag {
    /// Creates a validated confirmation tag.
    ///
    /// # Errors
    ///
    /// Returns a model error for non-16-byte input.
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, PairingModelError> {
        let bytes = bytes.as_ref();
        let array = <[u8; CONFIRMATION_TAG_LENGTH]>::try_from(bytes).map_err(|_| {
            PairingModelError::InvalidConfirmationTagLength {
                actual: bytes.len(),
            }
        })?;
        Ok(Self(Arc::new(array)))
    }

    /// Returns tag bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; CONFIRMATION_TAG_LENGTH] {
        &self.0
    }
}

/// Immutable bridge identity used by pairing transcripts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeIdentity {
    id: BridgeId,
    identity_key: PairingPublicKey,
}

impl BridgeIdentity {
    /// Creates a bridge identity.
    #[must_use]
    pub const fn new(id: BridgeId, identity_key: PairingPublicKey) -> Self {
        Self { id, identity_key }
    }

    /// Returns the bridge identifier.
    #[must_use]
    pub const fn id(&self) -> &BridgeId {
        &self.id
    }

    /// Returns the Ed25519 identity key.
    #[must_use]
    pub const fn identity_key(&self) -> &PairingPublicKey {
        &self.identity_key
    }
}

/// Pairing-specific immutable capability declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingCapabilities {
    canonical: CapabilitySet,
}

impl PairingCapabilities {
    /// Validates a canonical protocol capability set for pairing.
    ///
    /// # Errors
    ///
    /// Returns [`PairingModelError::InvalidCapabilities`] when values are invalid, duplicated, or
    /// required values are absent from supported values.
    pub fn new(mut canonical: CapabilitySet) -> Result<Self, PairingModelError> {
        canonical.supported.sort_unstable();
        canonical.required.sort_unstable();
        if canonical.supported.windows(2).any(|pair| pair[0] == pair[1])
            || canonical.required.windows(2).any(|pair| pair[0] == pair[1])
            || canonical.supported.iter().any(|value| Capability::try_from(*value).is_err())
            || canonical.required.iter().any(|value| Capability::try_from(*value).is_err())
            || canonical.supported.contains(&(Capability::CapabilityUnspecified as i32))
            || canonical.required.contains(&(Capability::CapabilityUnspecified as i32))
            || canonical
                .required
                .iter()
                .any(|value| canonical.supported.binary_search(value).is_err())
        {
            return Err(PairingModelError::InvalidCapabilities);
        }
        Ok(Self { canonical })
    }

    /// Returns the canonical generated capability set.
    #[must_use]
    pub const fn canonical(&self) -> &CapabilitySet {
        &self.canonical
    }

    /// Returns whether a capability is supported.
    #[must_use]
    pub fn supports(&self, capability: Capability) -> bool {
        self.canonical
            .supported
            .binary_search(&(capability as i32))
            .is_ok()
    }
}

/// Pairing policy governing freshness, versions, replacement, and required capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingPolicy {
    protocol_version: ProtocolVersion,
    capabilities: PairingCapabilities,
    challenge_lifetime_ms: u64,
    allow_trust_replacement: bool,
    allow_revoked_replacement: bool,
}

impl PairingPolicy {
    /// Creates a validated policy.
    ///
    /// # Errors
    ///
    /// Returns a model error for invalid version or zero challenge lifetime.
    pub fn new(
        protocol_version: ProtocolVersion,
        capabilities: PairingCapabilities,
        challenge_lifetime_ms: u64,
        allow_trust_replacement: bool,
        allow_revoked_replacement: bool,
    ) -> Result<Self, PairingModelError> {
        if protocol_version.major == 0 || challenge_lifetime_ms == 0 {
            return Err(PairingModelError::InvalidProtocolVersion);
        }
        Ok(Self {
            protocol_version,
            capabilities,
            challenge_lifetime_ms,
            allow_trust_replacement,
            allow_revoked_replacement,
        })
    }

    /// Returns the supported protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> &ProtocolVersion {
        &self.protocol_version
    }

    /// Returns pairing capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &PairingCapabilities {
        &self.capabilities
    }

    /// Returns challenge lifetime in milliseconds.
    #[must_use]
    pub const fn challenge_lifetime_ms(&self) -> u64 {
        self.challenge_lifetime_ms
    }

    /// Returns whether active trust replacement is permitted.
    #[must_use]
    pub const fn allow_trust_replacement(&self) -> bool {
        self.allow_trust_replacement
    }

    /// Returns whether revoked trust replacement is permitted.
    #[must_use]
    pub const fn allow_revoked_replacement(&self) -> bool {
        self.allow_revoked_replacement
    }
}

/// Pairing challenge bound to Bridge identity and an ephemeral X25519 key.
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
    ///
    /// # Errors
    ///
    /// Returns a model error unless expiry follows creation.
    pub fn new(
        id: ChallengeId,
        nonce: PairingNonce,
        bridge_ephemeral_key: PairingPublicKey,
        created_at: PairingTimestamp,
        expires_at: PairingTimestamp,
    ) -> Result<Self, PairingModelError> {
        if expires_at <= created_at {
            return Err(PairingModelError::InvalidChallengeWindow);
        }
        Ok(Self {
            id,
            nonce,
            bridge_ephemeral_key,
            created_at,
            expires_at,
        })
    }

    /// Returns the challenge identifier.
    #[must_use]
    pub const fn id(&self) -> &ChallengeId { &self.id }
    /// Returns the nonce.
    #[must_use]
    pub const fn nonce(&self) -> &PairingNonce { &self.nonce }
    /// Returns the bridge X25519 public key.
    #[must_use]
    pub const fn bridge_ephemeral_key(&self) -> &PairingPublicKey { &self.bridge_ephemeral_key }
    /// Returns creation time.
    #[must_use]
    pub const fn created_at(&self) -> PairingTimestamp { self.created_at }
    /// Returns expiry time.
    #[must_use]
    pub const fn expires_at(&self) -> PairingTimestamp { self.expires_at }
    /// Returns whether the challenge is expired at `observed_at`.
    #[must_use]
    pub fn is_expired(&self, observed_at: PairingTimestamp) -> bool { observed_at >= self.expires_at }
}

/// Peer pairing request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingRequest {
    device_id: DeviceId,
    identity_key: PairingPublicKey,
    ephemeral_key: PairingPublicKey,
    protocol_version: ProtocolVersion,
    capabilities: PairingCapabilities,
}

impl PairingRequest {
    /// Creates a pairing request.
    #[must_use]
    pub const fn new(
        device_id: DeviceId,
        identity_key: PairingPublicKey,
        ephemeral_key: PairingPublicKey,
        protocol_version: ProtocolVersion,
        capabilities: PairingCapabilities,
    ) -> Self {
        Self { device_id, identity_key, ephemeral_key, protocol_version, capabilities }
    }
    /// Returns the peer device identifier.
    #[must_use]
    pub const fn device_id(&self) -> &DeviceId { &self.device_id }
    /// Returns the Ed25519 public identity key.
    #[must_use]
    pub const fn identity_key(&self) -> &PairingPublicKey { &self.identity_key }
    /// Returns the X25519 ephemeral public key.
    #[must_use]
    pub const fn ephemeral_key(&self) -> &PairingPublicKey { &self.ephemeral_key }
    /// Returns the proposed protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> &ProtocolVersion { &self.protocol_version }
    /// Returns proposed pairing capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &PairingCapabilities { &self.capabilities }
}

/// Signed peer response and pairing-confirmation tag.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingResponse {
    challenge_id: ChallengeId,
    request: PairingRequest,
    signature: Arc<[u8]>,
    confirmation_tag: PairingConfirmationTag,
}

impl PairingResponse {
    /// Creates a response.
    #[must_use]
    pub fn new(
        challenge_id: ChallengeId,
        request: PairingRequest,
        signature: impl Into<Arc<[u8]>>,
        confirmation_tag: PairingConfirmationTag,
    ) -> Self {
        Self { challenge_id, request, signature: signature.into(), confirmation_tag }
    }
    /// Returns the challenge identifier.
    #[must_use]
    pub const fn challenge_id(&self) -> &ChallengeId { &self.challenge_id }
    /// Returns the request.
    #[must_use]
    pub const fn request(&self) -> &PairingRequest { &self.request }
    /// Returns the Ed25519 signature.
    #[must_use]
    pub fn signature(&self) -> &[u8] { &self.signature }
    /// Returns the confirmation tag.
    #[must_use]
    pub const fn confirmation_tag(&self) -> &PairingConfirmationTag { &self.confirmation_tag }
}

/// Pairing lifecycle state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PairingState {
    /// Pairing record exists before challenge creation.
    Idle,
    /// Challenge was created.
    ChallengeCreated,
    /// Challenge was exposed to the peer.
    ChallengeSent,
    /// A response was accepted for verification.
    ResponseReceived,
    /// Peer identity and confirmation were verified.
    IdentityVerified,
    /// Trust record was committed.
    TrustEstablished,
    /// Pairing completed successfully.
    Completed,
    /// Pairing was explicitly rejected.
    Rejected,
    /// Pairing challenge expired.
    Expired,
    /// Pairing or peer trust was revoked.
    Revoked,
    /// Pairing was cancelled.
    Cancelled,
}

impl PairingState {
    /// Returns whether the state is terminal.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Rejected | Self::Expired | Self::Revoked | Self::Cancelled)
    }

    /// Returns whether a direct transition is legal.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Idle, Self::ChallengeCreated | Self::Cancelled)
                | (Self::ChallengeCreated, Self::ChallengeSent | Self::Expired | Self::Cancelled)
                | (Self::ChallengeSent, Self::ResponseReceived | Self::Rejected | Self::Expired | Self::Cancelled)
                | (Self::ResponseReceived, Self::IdentityVerified | Self::Rejected | Self::Expired | Self::Cancelled)
                | (Self::IdentityVerified, Self::TrustEstablished | Self::Rejected | Self::Revoked | Self::Cancelled)
                | (Self::TrustEstablished, Self::Completed | Self::Revoked)
        )
    }
}

/// Explicit trust action supplied by the caller after user or policy authorization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustDecision {
    /// Create trust only when no record exists.
    Trust,
    /// Replace an existing identity under policy.
    Replace,
    /// Reject trust establishment.
    Reject,
}

/// Validated trust metadata key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TrustMetadataKey(Arc<str>);

impl TrustMetadataKey {
    /// Creates a metadata key.
    ///
    /// # Errors
    ///
    /// Returns a model error when empty.
    pub fn new(value: impl Into<String>) -> Result<Self, PairingModelError> {
        let value = value.into();
        if value.is_empty() { return Err(PairingModelError::EmptyMetadataKey); }
        Ok(Self(Arc::from(value)))
    }
    /// Returns key text.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Immutable trust metadata value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustMetadataValue(Arc<str>);

impl TrustMetadataValue {
    /// Creates a metadata value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self { Self(Arc::from(value.into())) }
    /// Returns value text.
    #[must_use]
    pub fn as_str(&self) -> &str { &self.0 }
}

/// Deterministically ordered trust metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrustMetadata(BTreeMap<TrustMetadataKey, TrustMetadataValue>);

impl TrustMetadata {
    /// Creates empty metadata.
    #[must_use]
    pub const fn new() -> Self { Self(BTreeMap::new()) }
    /// Inserts or replaces an entry.
    #[must_use]
    pub fn insert(&mut self, key: TrustMetadataKey, value: TrustMetadataValue) -> Option<TrustMetadataValue> {
        self.0.insert(key, value)
    }
    /// Iterates entries in key order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&TrustMetadataKey, &TrustMetadataValue)> {
        self.0.iter()
    }
}

/// Immutable trusted-peer record.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// Creates an immutable trust record.
    #[must_use]
    pub const fn new(
        bridge_identity: BridgeIdentity,
        device_id: DeviceId,
        peer_identity_key: PairingPublicKey,
        capabilities: PairingCapabilities,
        protocol_version: ProtocolVersion,
        trusted_at: PairingTimestamp,
        metadata: TrustMetadata,
        revision: PairingRevision,
    ) -> Self {
        Self { bridge_identity, device_id, peer_identity_key, capabilities, protocol_version, trusted_at, last_verified_at: trusted_at, revoked_at: None, metadata, revision }
    }
    /// Returns the Bridge identity.
    #[must_use]
    pub const fn bridge_identity(&self) -> &BridgeIdentity { &self.bridge_identity }
    /// Returns the peer device identifier.
    #[must_use]
    pub const fn device_id(&self) -> &DeviceId { &self.device_id }
    /// Returns the peer identity key.
    #[must_use]
    pub const fn peer_identity_key(&self) -> &PairingPublicKey { &self.peer_identity_key }
    /// Returns capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &PairingCapabilities { &self.capabilities }
    /// Returns protocol version.
    #[must_use]
    pub const fn protocol_version(&self) -> &ProtocolVersion { &self.protocol_version }
    /// Returns initial trust timestamp.
    #[must_use]
    pub const fn trusted_at(&self) -> PairingTimestamp { self.trusted_at }
    /// Returns last verification timestamp.
    #[must_use]
    pub const fn last_verified_at(&self) -> PairingTimestamp { self.last_verified_at }
    /// Returns revocation timestamp.
    #[must_use]
    pub const fn revoked_at(&self) -> Option<PairingTimestamp> { self.revoked_at }
    /// Returns metadata.
    #[must_use]
    pub const fn metadata(&self) -> &TrustMetadata { &self.metadata }
    /// Returns record revision.
    #[must_use]
    pub const fn revision(&self) -> PairingRevision { self.revision }
    /// Returns whether trust is revoked.
    #[must_use]
    pub const fn is_revoked(&self) -> bool { self.revoked_at.is_some() }

    pub(crate) fn revoked(&self, timestamp: PairingTimestamp) -> Option<Self> {
        let revision = self.revision.checked_next()?;
        let mut next = self.clone();
        next.revoked_at = Some(timestamp);
        next.last_verified_at = timestamp;
        next.revision = revision;
        Some(next)
    }
}

impl StateRegistryValue for TrustedPeer {
    type Key = DeviceId;
    const REGISTRY_KIND: RegistryKind = RegistryKind::TrustedPeers;
    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> { Ok(self.device_id.clone()) }
}

/// Immutable pairing-session record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingSession {
    id: PairingId,
    bridge_identity: BridgeIdentity,
    session_id: Option<SessionId>,
    challenge: Option<PairingChallenge>,
    request: Option<PairingRequest>,
    state: PairingState,
    challenge_consumed: bool,
    revision: PairingRevision,
    created_at: PairingTimestamp,
    updated_at: PairingTimestamp,
}

impl PairingSession {
    /// Creates an idle pairing session.
    #[must_use]
    pub const fn new(
        id: PairingId,
        bridge_identity: BridgeIdentity,
        session_id: Option<SessionId>,
        created_at: PairingTimestamp,
    ) -> Self {
        Self { id, bridge_identity, session_id, challenge: None, request: None, state: PairingState::Idle, challenge_consumed: false, revision: PairingRevision::INITIAL, created_at, updated_at: created_at }
    }
    /// Returns the pairing identifier.
    #[must_use]
    pub const fn id(&self) -> &PairingId { &self.id }
    /// Returns Bridge identity.
    #[must_use]
    pub const fn bridge_identity(&self) -> &BridgeIdentity { &self.bridge_identity }
    /// Returns optional bound session.
    #[must_use]
    pub const fn session_id(&self) -> Option<&SessionId> { self.session_id.as_ref() }
    /// Returns challenge.
    #[must_use]
    pub const fn challenge(&self) -> Option<&PairingChallenge> { self.challenge.as_ref() }
    /// Returns peer request.
    #[must_use]
    pub const fn request(&self) -> Option<&PairingRequest> { self.request.as_ref() }
    /// Returns lifecycle state.
    #[must_use]
    pub const fn state(&self) -> PairingState { self.state }
    /// Returns whether challenge was consumed.
    #[must_use]
    pub const fn challenge_consumed(&self) -> bool { self.challenge_consumed }
    /// Returns record revision.
    #[must_use]
    pub const fn revision(&self) -> PairingRevision { self.revision }
    /// Returns creation timestamp.
    #[must_use]
    pub const fn created_at(&self) -> PairingTimestamp { self.created_at }
    /// Returns last update timestamp.
    #[must_use]
    pub const fn updated_at(&self) -> PairingTimestamp { self.updated_at }

    pub(crate) fn next(
        &self,
        state: PairingState,
        timestamp: PairingTimestamp,
        challenge: Option<PairingChallenge>,
        request: Option<PairingRequest>,
        consume: bool,
    ) -> Option<Self> {
        let revision = self.revision.checked_next()?;
        let mut next = self.clone();
        next.state = state;
        next.updated_at = timestamp;
        if let Some(challenge) = challenge { next.challenge = Some(challenge); }
        if let Some(request) = request { next.request = Some(request); }
        next.challenge_consumed |= consume;
        next.revision = revision;
        Some(next)
    }
}

impl StateRegistryValue for PairingSession {
    type Key = PairingId;
    const REGISTRY_KIND: RegistryKind = RegistryKind::PairingSessions;
    fn registry_key(&self) -> Result<Self::Key, StateIdentifierError> { Ok(self.id.clone()) }
}
