use std::{error::Error, fmt};

use crate::{
    PairingId, PairingModelError, PairingRevision, PairingState, PairingTimestamp, StateError,
    StateIdentifierError, TrustId, TrustReplacementPolicy, TrustRevision,
};

/// Structured Pairing Core failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingError {
    /// Bridge State rejected or failed a transaction.
    State(StateError),
    /// A strongly typed state identifier failed validation.
    Identifier(StateIdentifierError),
    /// A strongly typed Pairing Core value failed validation.
    Model(PairingModelError),
    /// The requested pairing session does not exist.
    PairingNotFound {
        /// Missing pairing identifier.
        pairing_id: PairingId,
    },
    /// The requested trust record does not exist.
    TrustNotFound {
        /// Missing trust identifier.
        trust_id: TrustId,
    },
    /// A pairing identifier was already registered and cannot be replayed.
    DuplicatePairing {
        /// Duplicate pairing identifier.
        pairing_id: PairingId,
    },
    /// Another non-terminal pairing already owns this device identity.
    ConcurrentDevicePairing {
        /// Device identifier.
        device_id: String,
        /// Existing pairing identifier.
        existing_pairing_id: PairingId,
    },
    /// The pairing request did not match the signed offer.
    OfferMismatch,
    /// The signed offer expired before registration.
    OfferExpired {
        /// Offer expiration timestamp.
        expires_at: PairingTimestamp,
        /// Observation timestamp.
        observed_at: PairingTimestamp,
    },
    /// The bridge offer signature was invalid.
    InvalidOfferSignature,
    /// A bridge or peer Ed25519 public key was malformed or weak.
    InvalidIdentityPublicKey {
        /// Rejected identity field.
        identity: &'static str,
    },
    /// A signing key did not match the canonical identity descriptor.
    IdentitySigningKeyMismatch {
        /// Rejected identity field.
        identity: &'static str,
    },
    /// An X25519 public key was malformed.
    InvalidEphemeralPublicKey {
        /// Rejected public-key field.
        field: &'static str,
    },
    /// An X25519 public key was reused across pairing attempts.
    EphemeralKeyReuse {
        /// Reused public-key field.
        field: &'static str,
    },
    /// X25519 produced a non-contributory all-zero shared secret.
    NonContributoryKeyAgreement,
    /// HKDF or ChaCha20-Poly1305 orchestration failed.
    CryptographicOrchestration,
    /// No mutually supported protocol version exists.
    UnsupportedProtocolVersion,
    /// A pairing attempt selected an older version than existing trust permits.
    ProtocolDowngrade {
        /// Device identifier.
        device_id: String,
    },
    /// A security suite other than the approved suite was requested.
    UnsupportedSecuritySuite,
    /// An active or revoked duplicate identity is disallowed by policy.
    DuplicateDeviceIdentity {
        /// Device identifier.
        device_id: String,
        /// Existing trust identifier.
        trust_id: TrustId,
    },
    /// A revoked device attempted to pair without explicit repair permission.
    RevokedPeer {
        /// Device identifier.
        device_id: String,
        /// Revoked trust identifier.
        trust_id: TrustId,
    },
    /// A trust identifier is already in use.
    DuplicateTrust {
        /// Duplicate trust identifier.
        trust_id: TrustId,
    },
    /// Explicit replacement did not target the current device trust record.
    TrustReplacementTargetMismatch {
        /// Expected trust record.
        expected: TrustId,
        /// Requested trust record.
        requested: TrustId,
    },
    /// Trust replacement was forbidden by policy.
    TrustReplacementDenied {
        /// Active replacement policy.
        policy: TrustReplacementPolicy,
    },
    /// The requested trust replacement would reuse the existing trust identifier.
    TrustIdentifierReuse {
        /// Reused trust identifier.
        trust_id: TrustId,
    },
    /// The peer response signature was malformed or invalid.
    InvalidResponseSignature,
    /// A move-only challenge context did not match the authoritative session.
    KeyAgreementContextMismatch {
        /// Pairing identifier.
        pairing_id: PairingId,
    },
    /// The pairing challenge expired.
    ChallengeExpired {
        /// Pairing identifier.
        pairing_id: PairingId,
        /// Challenge expiration timestamp.
        expires_at: PairingTimestamp,
        /// Observation timestamp.
        observed_at: PairingTimestamp,
    },
    /// A pairing expiration was requested before its active deadline.
    PairingStillFresh {
        /// Pairing identifier.
        pairing_id: PairingId,
        /// Current expiration deadline.
        expires_at: PairingTimestamp,
        /// Observation timestamp.
        observed_at: PairingTimestamp,
    },
    /// A mutation observed a stale pairing revision.
    StalePairingRevision {
        /// Pairing identifier.
        pairing_id: PairingId,
        /// Expected revision supplied by the caller.
        expected: PairingRevision,
        /// Current committed revision.
        actual: PairingRevision,
    },
    /// A mutation observed a stale trust revision.
    StaleTrustRevision {
        /// Trust identifier.
        trust_id: TrustId,
        /// Expected revision supplied by the caller.
        expected: TrustRevision,
        /// Current committed revision.
        actual: TrustRevision,
    },
    /// A timestamp moved backward relative to the current record.
    TimestampRegression,
    /// A pairing lifecycle transition was illegal.
    IllegalTransition {
        /// Pairing identifier.
        pairing_id: PairingId,
        /// Current state.
        current: PairingState,
        /// Requested state.
        requested: PairingState,
    },
    /// A terminal state was asked to transition.
    TerminalState {
        /// Pairing identifier.
        pairing_id: PairingId,
        /// Terminal state.
        state: PairingState,
    },
    /// A monotonic pairing revision was exhausted.
    PairingRevisionExhausted {
        /// Pairing identifier.
        pairing_id: PairingId,
    },
    /// A monotonic trust revision was exhausted.
    TrustRevisionExhausted {
        /// Trust identifier.
        trust_id: TrustId,
    },
    /// Required pairing material was absent from the current state.
    MissingPairingMaterial {
        /// Pairing identifier.
        pairing_id: PairingId,
        /// Missing field description.
        field: &'static str,
    },
}

impl fmt::Display for PairingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State(source) => source.fmt(formatter),
            Self::Identifier(source) => source.fmt(formatter),
            Self::Model(source) => source.fmt(formatter),
            Self::PairingNotFound { pairing_id } => {
                write!(formatter, "pairing session {pairing_id} does not exist")
            }
            Self::TrustNotFound { trust_id } => {
                write!(formatter, "trust record {trust_id} does not exist")
            }
            Self::DuplicatePairing { pairing_id } => write!(
                formatter,
                "pairing identifier {pairing_id} was already registered and cannot be replayed"
            ),
            Self::ConcurrentDevicePairing {
                device_id,
                existing_pairing_id,
            } => write!(
                formatter,
                "device {device_id:?} already has non-terminal pairing {existing_pairing_id}"
            ),
            Self::OfferMismatch => formatter.write_str(
                "pairing request does not match the signed bridge offer and pairing identifier",
            ),
            Self::OfferExpired {
                expires_at,
                observed_at,
            } => write!(
                formatter,
                "pairing offer expired at {} before observation at {}",
                expires_at.as_unix_millis(),
                observed_at.as_unix_millis()
            ),
            Self::InvalidOfferSignature => {
                formatter.write_str("bridge pairing offer signature is invalid")
            }
            Self::InvalidIdentityPublicKey { identity } => {
                write!(formatter, "{identity} Ed25519 public key is malformed or weak")
            }
            Self::IdentitySigningKeyMismatch { identity } => {
                write!(formatter, "{identity} signing key does not match its identity descriptor")
            }
            Self::InvalidEphemeralPublicKey { field } => {
                write!(formatter, "{field} X25519 public key is malformed")
            }
            Self::EphemeralKeyReuse { field } => {
                write!(formatter, "{field} was already used by another pairing attempt")
            }
            Self::NonContributoryKeyAgreement => formatter.write_str(
                "X25519 key agreement produced a non-contributory all-zero shared secret",
            ),
            Self::CryptographicOrchestration => {
                formatter.write_str("pairing cryptographic orchestration failed")
            }
            Self::UnsupportedProtocolVersion => {
                formatter.write_str("pairing offer has no mutually supported protocol version")
            }
            Self::ProtocolDowngrade { device_id } => write!(
                formatter,
                "pairing device {device_id:?} attempted a protocol downgrade"
            ),
            Self::UnsupportedSecuritySuite => formatter.write_str(
                "pairing supports only X25519/Ed25519/HKDF-SHA-256/ChaCha20-Poly1305",
            ),
            Self::DuplicateDeviceIdentity {
                device_id,
                trust_id,
            } => write!(
                formatter,
                "device {device_id:?} already has trust record {trust_id}"
            ),
            Self::RevokedPeer {
                device_id,
                trust_id,
            } => write!(
                formatter,
                "revoked device {device_id:?} cannot pair through trust record {trust_id}"
            ),
            Self::DuplicateTrust { trust_id } => {
                write!(formatter, "trust identifier {trust_id} already exists")
            }
            Self::TrustReplacementTargetMismatch {
                expected,
                requested,
            } => write!(
                formatter,
                "trust replacement must target {expected}, not {requested}"
            ),
            Self::TrustReplacementDenied { policy } => {
                write!(formatter, "trust replacement is denied by policy {policy:?}")
            }
            Self::TrustIdentifierReuse { trust_id } => write!(
                formatter,
                "replacement trust identifier {trust_id} must differ from the revoked record"
            ),
            Self::InvalidResponseSignature => {
                formatter.write_str("peer challenge response signature is invalid")
            }
            Self::KeyAgreementContextMismatch { pairing_id } => write!(
                formatter,
                "one-time key agreement context does not match pairing {pairing_id}"
            ),
            Self::ChallengeExpired {
                pairing_id,
                expires_at,
                observed_at,
            } => write!(
                formatter,
                "pairing {pairing_id} challenge expired at {} before observation at {}",
                expires_at.as_unix_millis(),
                observed_at.as_unix_millis()
            ),
            Self::PairingStillFresh {
                pairing_id,
                expires_at,
                observed_at,
            } => write!(
                formatter,
                "pairing {pairing_id} remains valid until {}; observed at {}",
                expires_at.as_unix_millis(),
                observed_at.as_unix_millis()
            ),
            Self::StalePairingRevision {
                pairing_id,
                expected,
                actual,
            } => write!(
                formatter,
                "pairing {pairing_id} revision is stale: expected {}, actual {}",
                expected.get(),
                actual.get()
            ),
            Self::StaleTrustRevision {
                trust_id,
                expected,
                actual,
            } => write!(
                formatter,
                "trust {trust_id} revision is stale: expected {}, actual {}",
                expected.get(),
                actual.get()
            ),
            Self::TimestampRegression => {
                formatter.write_str("pairing or trust timestamp moved backward")
            }
            Self::IllegalTransition {
                pairing_id,
                current,
                requested,
            } => write!(
                formatter,
                "pairing {pairing_id} cannot transition from {current:?} to {requested:?}"
            ),
            Self::TerminalState { pairing_id, state } => write!(
                formatter,
                "pairing {pairing_id} is terminal in state {state:?}"
            ),
            Self::PairingRevisionExhausted { pairing_id } => {
                write!(formatter, "pairing {pairing_id} revision is exhausted")
            }
            Self::TrustRevisionExhausted { trust_id } => {
                write!(formatter, "trust {trust_id} revision is exhausted")
            }
            Self::MissingPairingMaterial { pairing_id, field } => {
                write!(formatter, "pairing {pairing_id} is missing required {field}")
            }
        }
    }
}

impl Error for PairingError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::State(source) => Some(source),
            Self::Identifier(source) => Some(source),
            Self::Model(source) => Some(source),
            _ => None,
        }
    }
}

impl From<StateError> for PairingError {
    fn from(source: StateError) -> Self {
        Self::State(source)
    }
}

impl From<StateIdentifierError> for PairingError {
    fn from(source: StateIdentifierError) -> Self {
        Self::Identifier(source)
    }
}

impl From<PairingModelError> for PairingError {
    fn from(source: PairingModelError) -> Self {
        Self::Model(source)
    }
}
