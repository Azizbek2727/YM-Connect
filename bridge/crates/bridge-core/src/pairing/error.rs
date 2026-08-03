use std::{error::Error, fmt, sync::Arc};

use crate::{
    ChallengeId, DeviceId, PairingId, PairingModelError, PairingRevision, PairingState,
    PairingTimestamp, SessionId, StateError,
};

/// Result type used by Pairing Core.
pub type PairingResult<T> = Result<T, PairingError>;

/// Structured Pairing Core failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingError {
    /// Bridge State rejected or failed a transaction.
    State(StateError),
    /// A pairing model value failed validation.
    Model(PairingModelError),
    /// A pairing session was not found.
    PairingNotFound { pairing_id: PairingId },
    /// A duplicate pairing identifier was supplied.
    DuplicatePairing { pairing_id: PairingId },
    /// A duplicate challenge identifier was supplied.
    DuplicateChallenge { challenge_id: ChallengeId },
    /// A referenced session does not exist.
    MissingSession { session_id: SessionId },
    /// The requested lifecycle transition is illegal.
    InvalidTransition {
        pairing_id: PairingId,
        previous: PairingState,
        requested: PairingState,
    },
    /// A terminal pairing session cannot be changed.
    TerminalPairing {
        pairing_id: PairingId,
        state: PairingState,
    },
    /// The expected pairing revision was stale.
    StaleRevision {
        pairing_id: PairingId,
        expected: PairingRevision,
        actual: PairingRevision,
    },
    /// An operation timestamp regressed.
    TimestampRegression {
        pairing_id: PairingId,
        previous: PairingTimestamp,
        requested: PairingTimestamp,
    },
    /// The challenge expired.
    ChallengeExpired {
        pairing_id: PairingId,
        challenge_id: ChallengeId,
    },
    /// A challenge response was replayed.
    ReplayDetected {
        pairing_id: PairingId,
        challenge_id: ChallengeId,
    },
    /// The response referred to another challenge.
    ChallengeMismatch {
        pairing_id: PairingId,
        expected: ChallengeId,
        actual: ChallengeId,
    },
    /// A cryptographic public key was rejected.
    InvalidPublicKey { code: Arc<str>, message: Arc<str> },
    /// Ed25519 signature verification failed.
    InvalidSignature { code: Arc<str>, message: Arc<str> },
    /// X25519/HKDF/ChaCha20-Poly1305 confirmation failed.
    InvalidKeyConfirmation { code: Arc<str>, message: Arc<str> },
    /// The peer proposed an unsupported protocol version.
    UnsupportedProtocolVersion,
    /// The peer attempted protocol downgrade.
    ProtocolDowngrade,
    /// Required pairing capabilities could not be negotiated.
    MissingRequiredCapabilities,
    /// The peer is already trusted under another identity.
    DuplicateDeviceIdentity { device_id: DeviceId },
    /// The identity key is already trusted for another device.
    DuplicateIdentityKey { existing_device_id: DeviceId },
    /// The trusted peer is revoked.
    RevokedPeer { device_id: DeviceId },
    /// Trust replacement was not allowed by policy or decision.
    TrustReplacementForbidden { device_id: DeviceId },
    /// Trust record was not found.
    TrustNotFound { device_id: DeviceId },
    /// A monotonic pairing or trust revision was exhausted.
    RevisionExhausted,
    /// A committed state update violated a Pairing Core invariant.
    StateInvariant { message: Arc<str> },
}

impl PairingError {
    /// Creates a structured invalid-key error from a crypto provider.
    #[must_use]
    pub fn invalid_public_key(code: impl Into<Arc<str>>, message: impl Into<Arc<str>>) -> Self {
        Self::InvalidPublicKey { code: code.into(), message: message.into() }
    }

    /// Creates a structured invalid-signature error from a crypto provider.
    #[must_use]
    pub fn invalid_signature(code: impl Into<Arc<str>>, message: impl Into<Arc<str>>) -> Self {
        Self::InvalidSignature { code: code.into(), message: message.into() }
    }

    /// Creates a structured confirmation error from a crypto provider.
    #[must_use]
    pub fn invalid_key_confirmation(
        code: impl Into<Arc<str>>,
        message: impl Into<Arc<str>>,
    ) -> Self {
        Self::InvalidKeyConfirmation { code: code.into(), message: message.into() }
    }

    pub(crate) fn state_invariant(message: impl Into<Arc<str>>) -> Self {
        Self::StateInvariant { message: message.into() }
    }
}

impl fmt::Display for PairingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(source) => write!(formatter, "Bridge State operation failed: {source}"),
            Self::Model(source) => write!(formatter, "pairing model is invalid: {source}"),
            Self::PairingNotFound { pairing_id } => write!(formatter, "pairing session {pairing_id} does not exist"),
            Self::DuplicatePairing { pairing_id } => write!(formatter, "pairing session {pairing_id} already exists"),
            Self::DuplicateChallenge { challenge_id } => write!(formatter, "pairing challenge {challenge_id} already exists"),
            Self::MissingSession { session_id } => write!(formatter, "session {session_id} does not exist"),
            Self::InvalidTransition { pairing_id, previous, requested } => write!(formatter, "pairing session {pairing_id} cannot transition from {previous:?} to {requested:?}"),
            Self::TerminalPairing { pairing_id, state } => write!(formatter, "pairing session {pairing_id} is terminal in state {state:?}"),
            Self::StaleRevision { pairing_id, expected, actual } => write!(formatter, "pairing session {pairing_id} revision is stale: expected {}, actual {}", expected.get(), actual.get()),
            Self::TimestampRegression { pairing_id, previous, requested } => write!(formatter, "pairing session {pairing_id} timestamp regressed from {} to {}", previous.as_unix_millis(), requested.as_unix_millis()),
            Self::ChallengeExpired { pairing_id, challenge_id } => write!(formatter, "pairing challenge {challenge_id} for session {pairing_id} expired"),
            Self::ReplayDetected { pairing_id, challenge_id } => write!(formatter, "pairing challenge {challenge_id} for session {pairing_id} was replayed"),
            Self::ChallengeMismatch { pairing_id, expected, actual } => write!(formatter, "pairing session {pairing_id} expected challenge {expected}, got {actual}"),
            Self::InvalidPublicKey { code, message } => write!(formatter, "public key rejected ({code}): {message}"),
            Self::InvalidSignature { code, message } => write!(formatter, "Ed25519 signature rejected ({code}): {message}"),
            Self::InvalidKeyConfirmation { code, message } => write!(formatter, "pairing key confirmation rejected ({code}): {message}"),
            Self::UnsupportedProtocolVersion => formatter.write_str("peer protocol version is unsupported"),
            Self::ProtocolDowngrade => formatter.write_str("peer attempted a protocol downgrade"),
            Self::MissingRequiredCapabilities => formatter.write_str("required pairing capabilities are missing"),
            Self::DuplicateDeviceIdentity { device_id } => write!(formatter, "device {device_id} is already trusted under another identity"),
            Self::DuplicateIdentityKey { existing_device_id } => write!(formatter, "identity key is already trusted for device {existing_device_id}"),
            Self::RevokedPeer { device_id } => write!(formatter, "trusted peer {device_id} is revoked"),
            Self::TrustReplacementForbidden { device_id } => write!(formatter, "trust replacement for device {device_id} is forbidden"),
            Self::TrustNotFound { device_id } => write!(formatter, "trusted peer {device_id} does not exist"),
            Self::RevisionExhausted => formatter.write_str("pairing or trust revision is exhausted"),
            Self::StateInvariant { message } => write!(formatter, "Pairing Core state invariant failed: {message}"),
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

impl From<PairingModelError> for PairingError {
    fn from(source: PairingModelError) -> Self { Self::Model(source) }
}
