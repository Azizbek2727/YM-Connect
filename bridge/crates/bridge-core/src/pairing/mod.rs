//! Runtime-independent pairing and trust orchestration.
//!
//! Pairing Core enforces the security invariants documented in
//! `docs/PAIRING_SECURITY_INVARIANTS.md`. Bridge State is the only mutable source of truth. The
//! module stores public identity material, lifecycle records, signed responses, and immutable trust
//! records, but never stores private keys, shared secrets, derived keys, or transport credentials.

#[allow(clippy::missing_errors_doc)]
mod crypto;
#[allow(missing_docs)]
#[path = "error_v2.rs"]
mod error;
mod event;
#[allow(clippy::missing_errors_doc, clippy::needless_pass_by_value)]
#[path = "manager_v2.rs"]
mod manager;
#[allow(
    missing_docs,
    clippy::missing_errors_doc,
    clippy::too_many_arguments
)]
#[path = "model_v2.rs"]
mod model;
#[allow(clippy::missing_errors_doc)]
mod rust_crypto;

pub use crypto::PairingCryptoProvider;
pub use error::{PairingError, PairingResult};
pub use event::{PairingEvent, PairingEventKind};
pub use manager::{
    CreatePairingChallenge, CreatePairingSession, EstablishPairingTrust, PairingManager,
    PairingMutation, ReceivePairingResponse, RevokeTrustedPeer, TransitionPairing, TrustMutation,
    TrustStore, VerifyPairingIdentity,
};
pub use model::{
    BridgeId, BridgeIdentity, ChallengeId, PairingAlgorithmSuite, PairingCapabilities,
    PairingChallenge, PairingConfirmationTag, PairingId, PairingModelError, PairingNonce,
    PairingPolicy, PairingPublicKey, PairingRequest, PairingResponse, PairingRevision,
    PairingSession, PairingState, PairingTimestamp, TrustDecision, TrustMetadata,
    TrustMetadataKey, TrustMetadataValue, TrustedPeer,
};
pub use rust_crypto::{
    PairingEphemeralSecret, PairingEphemeralSecretSource, RustCryptoPairingProvider,
};

#[cfg(test)]
mod tests;
