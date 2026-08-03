use std::fmt;

use crate::{
    PairingConfirmationTag, PairingPublicKey, PairingResult,
};

/// Runtime-independent cryptographic orchestration contract for the fixed pairing algorithm suite.
///
/// Implementations must use X25519 for key agreement, Ed25519 for identity verification,
/// HKDF-SHA-256 for pairing-key derivation, and ChaCha20-Poly1305 for confirmation validation.
/// Private keys and derived secrets remain entirely inside the implementation.
pub trait PairingCryptoProvider: fmt::Debug + Send + Sync + 'static {
    /// Validates an Ed25519 public verification key.
    fn validate_ed25519_public_key(&self, public_key: &PairingPublicKey) -> PairingResult<()>;

    /// Validates an X25519 public key.
    fn validate_x25519_public_key(&self, public_key: &PairingPublicKey) -> PairingResult<()>;

    /// Verifies an Ed25519 signature over the canonical transcript.
    fn verify_ed25519(
        &self,
        public_key: &PairingPublicKey,
        transcript: &[u8],
        signature: &[u8],
    ) -> PairingResult<()>;

    /// Performs X25519 agreement, HKDF-SHA-256 derivation, and ChaCha20-Poly1305 confirmation.
    ///
    /// The provider owns all private and derived key material and must discard it before returning.
    fn verify_key_agreement_confirmation(
        &self,
        bridge_ephemeral_public_key: &PairingPublicKey,
        peer_ephemeral_public_key: &PairingPublicKey,
        transcript: &[u8],
        confirmation_tag: &PairingConfirmationTag,
    ) -> PairingResult<()>;
}
