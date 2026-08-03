use std::{fmt, sync::Arc};

use chacha20poly1305::{
    aead::{AeadInPlace, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use ed25519_dalek::{Signature, VerifyingKey};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

use crate::{
    PairingConfirmationTag, PairingCryptoProvider, PairingError, PairingPublicKey, PairingResult,
};

const PAIRING_KEY_INFO: &[u8] = b"ym-connect/pairing/v1/key-confirmation";
const CONFIRMATION_NONCE_INFO: &[u8] = b"ym-connect/pairing/v1/confirmation-nonce";

/// Single-use X25519 private key used only while verifying one pairing transcript.
///
/// The bytes are zeroized on drop and are intentionally omitted from `Debug` output.
pub struct PairingEphemeralSecret(Zeroizing<[u8; 32]>);

impl PairingEphemeralSecret {
    /// Creates a secret from exactly 32 bytes.
    #[must_use]
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for PairingEphemeralSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PairingEphemeralSecret([REDACTED])")
    }
}

/// Runtime-independent source of single-use Bridge X25519 private keys.
///
/// Implementations must atomically remove the key associated with `public_key` before returning
/// it. Returning the same secret twice would violate Pairing Core replay and key-lifecycle
/// invariants.
pub trait PairingEphemeralSecretSource: fmt::Debug + Send + Sync + 'static {
    /// Takes the private key corresponding to a Bridge ephemeral public key.
    fn take_secret(&self, public_key: &PairingPublicKey) -> PairingResult<PairingEphemeralSecret>;
}

/// Production cryptographic implementation of the approved pairing suite.
///
/// This type uses X25519, Ed25519, HKDF-SHA-256, and ChaCha20-Poly1305 exclusively. It is
/// executor-independent and stores no key material itself.
#[derive(Clone, Debug)]
pub struct RustCryptoPairingProvider {
    secrets: Arc<dyn PairingEphemeralSecretSource>,
}

impl RustCryptoPairingProvider {
    /// Creates a provider backed by a single-use ephemeral-secret source.
    #[must_use]
    pub fn new(secrets: Arc<dyn PairingEphemeralSecretSource>) -> Self {
        Self { secrets }
    }
}

impl PairingCryptoProvider for RustCryptoPairingProvider {
    fn validate_ed25519_public_key(&self, public_key: &PairingPublicKey) -> PairingResult<()> {
        VerifyingKey::from_bytes(public_key.as_bytes()).map_err(|source| {
            PairingError::invalid_public_key("invalid-ed25519-key", source.to_string())
        })?;
        Ok(())
    }

    fn validate_x25519_public_key(&self, public_key: &PairingPublicKey) -> PairingResult<()> {
        if bool::from(public_key.as_bytes().ct_eq(&[0_u8; 32])) {
            return Err(PairingError::invalid_public_key(
                "invalid-x25519-key",
                "X25519 public key must not be all zero",
            ));
        }
        Ok(())
    }

    fn verify_ed25519(
        &self,
        public_key: &PairingPublicKey,
        transcript: &[u8],
        signature: &[u8],
    ) -> PairingResult<()> {
        let verifying_key = VerifyingKey::from_bytes(public_key.as_bytes()).map_err(|source| {
            PairingError::invalid_public_key("invalid-ed25519-key", source.to_string())
        })?;
        let signature = Signature::try_from(signature).map_err(|source| {
            PairingError::invalid_signature("invalid-ed25519-signature", source.to_string())
        })?;
        verifying_key.verify_strict(transcript, &signature).map_err(|source| {
            PairingError::invalid_signature("ed25519-verification-failed", source.to_string())
        })
    }

    fn verify_key_agreement_confirmation(
        &self,
        bridge_ephemeral_public_key: &PairingPublicKey,
        peer_ephemeral_public_key: &PairingPublicKey,
        transcript: &[u8],
        confirmation_tag: &PairingConfirmationTag,
    ) -> PairingResult<()> {
        self.validate_x25519_public_key(bridge_ephemeral_public_key)?;
        self.validate_x25519_public_key(peer_ephemeral_public_key)?;

        let mut secret = self.secrets.take_secret(bridge_ephemeral_public_key)?;
        let static_secret = StaticSecret::from(*secret.as_bytes());
        let expected_public = PublicKey::from(&static_secret);
        if !bool::from(expected_public.as_bytes().ct_eq(bridge_ephemeral_public_key.as_bytes())) {
            secret.0.zeroize();
            return Err(PairingError::invalid_public_key(
                "x25519-secret-mismatch",
                "ephemeral private key does not match the recorded Bridge public key",
            ));
        }

        let peer_public = PublicKey::from(*peer_ephemeral_public_key.as_bytes());
        let shared = static_secret.diffie_hellman(&peer_public);
        if bool::from(shared.as_bytes().ct_eq(&[0_u8; 32])) {
            secret.0.zeroize();
            return Err(PairingError::invalid_key_confirmation(
                "x25519-low-order-point",
                "X25519 agreement produced the all-zero shared secret",
            ));
        }

        let transcript_digest = Sha256::digest(transcript);
        let hkdf = Hkdf::<Sha256>::new(Some(&transcript_digest), shared.as_bytes());
        let mut key_bytes = Zeroizing::new([0_u8; 32]);
        hkdf.expand(PAIRING_KEY_INFO, key_bytes.as_mut()).map_err(|_| {
            PairingError::invalid_key_confirmation(
                "hkdf-expand-failed",
                "HKDF-SHA-256 could not derive the pairing confirmation key",
            )
        })?;
        let mut nonce_digest = Sha256::new();
        nonce_digest.update(CONFIRMATION_NONCE_INFO);
        nonce_digest.update(transcript_digest);
        let nonce_digest = nonce_digest.finalize();
        let nonce = Nonce::from_slice(&nonce_digest[..12]);
        let cipher = ChaCha20Poly1305::new(Key::from_slice(key_bytes.as_ref()));
        let mut empty = [];
        let expected_tag = cipher
            .encrypt_in_place_detached(nonce, transcript, &mut empty)
            .map_err(|_| {
                PairingError::invalid_key_confirmation(
                    "chacha20-poly1305-failed",
                    "ChaCha20-Poly1305 could not compute the confirmation tag",
                )
            })?;

        if !bool::from(expected_tag.as_slice().ct_eq(confirmation_tag.as_bytes())) {
            return Err(PairingError::invalid_key_confirmation(
                "confirmation-tag-mismatch",
                "ChaCha20-Poly1305 confirmation tag did not match the pairing transcript",
            ));
        }
        Ok(())
    }
}
