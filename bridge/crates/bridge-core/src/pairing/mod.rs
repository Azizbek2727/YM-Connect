//! Runtime-independent pairing, identity verification, and immutable trust orchestration.
//!
//! Pairing Core owns no transport, runtime, operating-system persistence, UI, discovery, TLS, or
//! networking behavior. [`PairingManager`] holds only a cloneable [`crate::BridgeStateStore`]
//! handle and immutable [`PairingPolicy`]. Every pairing and trust mutation is one Bridge State
//! transaction, and [`crate::BridgeStateEvent`] remains the only event model.
//!
//! # Security invariants
//!
//! - **Device identity:** a peer is identified by its canonical generated
//!   [`ym_connect_protocol::v1::DeviceDescriptor`] and a valid, non-weak 32-byte Ed25519 public
//!   key. The device identifier and identity key are bound into signed, domain-separated
//!   transcripts. An existing identifier cannot silently change keys.
//! - **Bridge identity:** every pairing request must reference a bridge offer signed by the
//!   Ed25519 key in the canonical generated [`ym_connect_protocol::v1::BridgeDescriptor`]. Offer
//!   signing rejects a private key that does not match that descriptor.
//! - **Trust establishment:** cryptographic identity verification is necessary but not sufficient.
//!   Trust is committed only after an explicit [`TrustDecision`] in the `IdentityVerified` state.
//! - **Replay protection:** pairing identifiers are never reusable, signed-offer digests bind each
//!   request to one offer, challenge fingerprints remain in terminal session history, and repeated
//!   responses are rejected by the finite-state machine and optimistic revisions.
//! - **Session binding:** the signed offer, request, selected protocol version, security suite,
//!   encrypted challenge, peer proof, and trust identifier remain bound to one [`PairingId`] and
//!   one deterministic transcript digest.
//! - **Challenge freshness:** callers supply explicit observation timestamps. Challenges have a
//!   non-zero policy lifetime, expire at a committed absolute timestamp, and cannot be accepted at
//!   or after expiry.
//! - **Trust persistence:** [`TrustedPeer`] records are immutable values in deterministic Bridge
//!   State registries. [`TrustStore`] is a read abstraction over that authoritative state; future
//!   persistence adapters may restore records but cannot become a parallel mutable authority.
//! - **Key agreement lifecycle:** X25519 private material is caller-supplied per attempt, used once,
//!   and destroyed before challenge creation returns. HKDF-SHA-256 derives one challenge key;
//!   ChaCha20-Poly1305 encrypts only the pairing challenge. The remaining move-only proof context
//!   is excluded from Bridge State and zeroized on consumption or drop.
//! - **Key rotation:** each pairing requires a new X25519 secret and nonce. Ed25519 identity-key
//!   rotation is denied by default and requires both an explicit replacement decision and the
//!   [`TrustReplacementPolicy::ExplicitIdentityRotation`] policy.
//! - **Revocation:** revocation creates a new immutable trust-record revision, removes active
//!   capability ownership, terminates related non-terminal pairings, and blocks repair unless
//!   policy explicitly permits it. Terminal pairing states remain terminal.
//! - **Downgrade resistance:** the exact approved security suite and selected protocol version are
//!   signed into transcripts. A device with existing trust cannot pair below its highest trusted
//!   protocol version.
//! - **Duplicate devices:** one device identifier may have at most one non-terminal pairing and one
//!   current trust lineage. Duplicate active identities, ambiguous replacements, trust-id reuse,
//!   and unauthorized identity-key changes are rejected transactionally.

mod crypto;
mod error;
mod manager;
mod model;
mod trust_store;

pub use error::PairingError;
pub use manager::{
    BeginPairing, PairingChallengeCreation, PairingManager, PairingMutation,
    PairingTrustMutation, ReceivePairingResponse, TrustMutation,
};
pub use model::{
    CHACHA20_POLY1305_NONCE_LENGTH, ED25519_PUBLIC_KEY_LENGTH, ED25519_SIGNATURE_LENGTH,
    PAIRING_CHALLENGE_LENGTH, SHA256_DIGEST_LENGTH, X25519_KEY_LENGTH, PairingCapabilities,
    PairingDuration, PairingEntropy, PairingKeyAgreement, PairingModelError, PairingPolicy,
    PairingRevision, PairingSession, PairingState, PairingStateTransition, PairingTimestamp,
    TrustDecision, TrustMetadata, TrustMetadataKey, TrustMetadataValue, TrustReplacementPolicy,
    TrustRevocationState, TrustRevision, TrustedPeer,
};
pub use trust_store::TrustStore;

pub use ym_connect_protocol::v1::{
    PairingChallenge, PairingOffer, PairingProof as PairingResponse, PairingRequest, PairingResult,
};

pub(crate) use model::{PairingSessionParts, TrustedPeerParts};

#[cfg(test)]
mod tests;
