use std::{error::Error, fmt, sync::Arc};

use crate::{
    ChallengeId, DeviceId, PairingId, PairingModelError, PairingRevision, PairingState,
    PairingTimestamp, RegistryStateError, SessionId, StateError,
};

/// Pairing Core result type.
pub type PairingResult<T> = Result<T, PairingError>;

/// Structured Pairing Core failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingError {
    /// Bridge State failure.
    State(StateError),
    /// Model validation failure.
    Model(PairingModelError),
    /// Pairing session was absent.
    PairingNotFound { pairing_id: PairingId },
    /// Pairing identifier already existed.
    DuplicatePairing { pairing_id: PairingId },
    /// Challenge identifier already existed.
    DuplicateChallenge { challenge_id: ChallengeId },
    /// Challenge lifetime violated local policy.
    InvalidChallengeLifetime { pairing_id: PairingId },
    /// Referenced Bridge session was absent.
    MissingSession { session_id: SessionId },
    /// Lifecycle transition was illegal.
    InvalidTransition { pairing_id: PairingId, previous: PairingState, requested: PairingState },
    /// Pairing session was terminal.
    TerminalPairing { pairing_id: PairingId, state: PairingState },
    /// Pairing revision was stale.
    StaleRevision { pairing_id: PairingId, expected: PairingRevision, actual: PairingRevision },
    /// Pairing timestamp regressed.
    TimestampRegression { pairing_id: PairingId, previous: PairingTimestamp, requested: PairingTimestamp },
    /// Challenge expired before response processing.
    ChallengeExpired { pairing_id: PairingId, challenge_id: ChallengeId },
    /// Expiration was requested before challenge expiry.
    ChallengeNotExpired { pairing_id: PairingId, challenge_id: ChallengeId },
    /// Challenge response was replayed.
    ReplayDetected { pairing_id: PairingId, challenge_id: ChallengeId },
    /// Response referenced another challenge.
    ChallengeMismatch { pairing_id: PairingId, expected: ChallengeId, actual: ChallengeId },
    /// Public key was rejected by the crypto provider.
    InvalidPublicKey { code: Arc<str>, message: Arc<str> },
    /// Ed25519 signature was invalid.
    InvalidSignature { code: Arc<str>, message: Arc<str> },
    /// Key-agreement confirmation was invalid.
    InvalidKeyConfirmation { code: Arc<str>, message: Arc<str> },
    /// Protocol version was unsupported.
    UnsupportedProtocolVersion,
    /// Peer selected a lower mutually supported version.
    ProtocolDowngrade,
    /// Required pairing capabilities were missing.
    MissingRequiredCapabilities,
    /// Device was trusted under another key.
    DuplicateDeviceIdentity { device_id: DeviceId },
    /// Identity key was trusted for another device.
    DuplicateIdentityKey { existing_device_id: DeviceId },
    /// Trusted peer was revoked.
    RevokedPeer { device_id: DeviceId },
    /// Trust decision explicitly rejected the peer.
    TrustRejected { device_id: DeviceId },
    /// Replacement was forbidden.
    TrustReplacementForbidden { device_id: DeviceId },
    /// Trust record was absent.
    TrustNotFound { device_id: DeviceId },
    /// Trust revision was stale.
    StaleTrustRevision { device_id: DeviceId, expected: PairingRevision, actual: PairingRevision },
    /// Trust timestamp regressed.
    TrustTimestampRegression { device_id: DeviceId, previous: PairingTimestamp, requested: PairingTimestamp },
    /// Pairing or trust revision exhausted.
    RevisionExhausted,
    /// Internal committed-state invariant failed.
    StateInvariant { message: Arc<str> },
}

impl PairingError {
    /// Creates an invalid-public-key error.
    #[must_use]
    pub fn invalid_public_key(code: impl Into<Arc<str>>, message: impl Into<Arc<str>>) -> Self {
        Self::InvalidPublicKey { code: code.into(), message: message.into() }
    }
    /// Creates an invalid-signature error.
    #[must_use]
    pub fn invalid_signature(code: impl Into<Arc<str>>, message: impl Into<Arc<str>>) -> Self {
        Self::InvalidSignature { code: code.into(), message: message.into() }
    }
    /// Creates an invalid-confirmation error.
    #[must_use]
    pub fn invalid_key_confirmation(code: impl Into<Arc<str>>, message: impl Into<Arc<str>>) -> Self {
        Self::InvalidKeyConfirmation { code: code.into(), message: message.into() }
    }
    pub(crate) fn state_invariant(message: impl Into<Arc<str>>) -> Self {
        Self::StateInvariant { message: message.into() }
    }
}

impl fmt::Display for PairingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(source) => write!(f, "Bridge State operation failed: {source}"),
            Self::Model(source) => write!(f, "pairing model is invalid: {source}"),
            Self::PairingNotFound { pairing_id } => write!(f, "pairing session {pairing_id} does not exist"),
            Self::DuplicatePairing { pairing_id } => write!(f, "pairing session {pairing_id} already exists"),
            Self::DuplicateChallenge { challenge_id } => write!(f, "pairing challenge {challenge_id} already exists"),
            Self::InvalidChallengeLifetime { pairing_id } => write!(f, "pairing challenge for {pairing_id} violates local lifetime policy"),
            Self::MissingSession { session_id } => write!(f, "session {session_id} does not exist"),
            Self::InvalidTransition { pairing_id, previous, requested } => write!(f, "pairing session {pairing_id} cannot transition from {previous:?} to {requested:?}"),
            Self::TerminalPairing { pairing_id, state } => write!(f, "pairing session {pairing_id} is terminal in state {state:?}"),
            Self::StaleRevision { pairing_id, expected, actual } => write!(f, "pairing session {pairing_id} revision is stale: expected {}, actual {}", expected.get(), actual.get()),
            Self::TimestampRegression { pairing_id, previous, requested } => write!(f, "pairing session {pairing_id} timestamp regressed from {} to {}", previous.as_unix_millis(), requested.as_unix_millis()),
            Self::ChallengeExpired { pairing_id, challenge_id } => write!(f, "challenge {challenge_id} for pairing {pairing_id} expired"),
            Self::ChallengeNotExpired { pairing_id, challenge_id } => write!(f, "challenge {challenge_id} for pairing {pairing_id} has not expired"),
            Self::ReplayDetected { pairing_id, challenge_id } => write!(f, "challenge {challenge_id} for pairing {pairing_id} was replayed"),
            Self::ChallengeMismatch { pairing_id, expected, actual } => write!(f, "pairing {pairing_id} expected challenge {expected}, got {actual}"),
            Self::InvalidPublicKey { code, message } => write!(f, "public key rejected ({code}): {message}"),
            Self::InvalidSignature { code, message } => write!(f, "Ed25519 signature rejected ({code}): {message}"),
            Self::InvalidKeyConfirmation { code, message } => write!(f, "pairing confirmation rejected ({code}): {message}"),
            Self::UnsupportedProtocolVersion => f.write_str("peer protocol version is unsupported"),
            Self::ProtocolDowngrade => f.write_str("peer attempted protocol downgrade"),
            Self::MissingRequiredCapabilities => f.write_str("required pairing capabilities are missing"),
            Self::DuplicateDeviceIdentity { device_id } => write!(f, "device {device_id} is already trusted under another identity"),
            Self::DuplicateIdentityKey { existing_device_id } => write!(f, "identity key is already trusted for device {existing_device_id}"),
            Self::RevokedPeer { device_id } => write!(f, "trusted peer {device_id} is revoked"),
            Self::TrustRejected { device_id } => write!(f, "trust for device {device_id} was rejected"),
            Self::TrustReplacementForbidden { device_id } => write!(f, "trust replacement for device {device_id} is forbidden"),
            Self::TrustNotFound { device_id } => write!(f, "trusted peer {device_id} does not exist"),
            Self::StaleTrustRevision { device_id, expected, actual } => write!(f, "trusted peer {device_id} revision is stale: expected {}, actual {}", expected.get(), actual.get()),
            Self::TrustTimestampRegression { device_id, previous, requested } => write!(f, "trusted peer {device_id} timestamp regressed from {} to {}", previous.as_unix_millis(), requested.as_unix_millis()),
            Self::RevisionExhausted => f.write_str("pairing or trust revision is exhausted"),
            Self::StateInvariant { message } => write!(f, "Pairing Core state invariant failed: {message}"),
        }
    }
}

impl Error for PairingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::Model(source) => Some(source),
            _ => None,
        }
    }
}

impl From<StateError> for PairingError {
    fn from(source: StateError) -> Self { Self::State(source) }
}
impl From<RegistryStateError> for PairingError {
    fn from(source: RegistryStateError) -> Self { Self::State(StateError::from(source)) }
}
impl From<PairingModelError> for PairingError {
    fn from(source: PairingModelError) -> Self { Self::Model(source) }
}
