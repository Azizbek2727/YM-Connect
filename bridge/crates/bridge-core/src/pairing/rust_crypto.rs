use std::{fmt, sync::Arc};

use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{AeadInPlace, KeyInit},
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
        verifying_key
            .verify_strict(transcript, &signature)
            .map_err(|source| {
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
        if !bool::from(
            expected_public
                .as_bytes()
                .ct_eq(bridge_ephemeral_public_key.as_bytes()),
        ) {
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
        hkdf.expand(PAIRING_KEY_INFO, key_bytes.as_mut())
            .map_err(|_| {
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

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        error::Error,
        fmt,
        sync::{Arc, Mutex},
    };

    use super::*;

    type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

    const TRANSCRIPT: &[u8] = b"ym-connect pairing known-answer transcript v1";
    const ED25519_PUBLIC: [u8; 32] = [
        0x79, 0xb5, 0x56, 0x2e, 0x8f, 0xe6, 0x54, 0xf9, 0x40, 0x78, 0xb1, 0x12, 0xe8, 0xa9, 0x8b,
        0xa7, 0x90, 0x1f, 0x85, 0x3a, 0xe6, 0x95, 0xbe, 0xd7, 0xe0, 0xe3, 0x91, 0x0b, 0xad, 0x04,
        0x96, 0x64,
    ];
    const ED25519_SIGNATURE: [u8; 64] = [
        0x97, 0xe6, 0xdd, 0x35, 0xe8, 0xd0, 0x04, 0xef, 0x54, 0x44, 0x53, 0x68, 0x30, 0xef, 0xcc,
        0xbc, 0x37, 0x67, 0x93, 0xf8, 0xc1, 0x66, 0xc3, 0xe9, 0x17, 0x5f, 0xe3, 0x61, 0x14, 0x02,
        0x89, 0x95, 0x1e, 0x6e, 0x23, 0x04, 0x59, 0xba, 0x85, 0x51, 0x37, 0xf3, 0x8b, 0x58, 0xfc,
        0xc2, 0x1e, 0xed, 0xa2, 0x31, 0x03, 0xfd, 0x9a, 0xb9, 0x36, 0x74, 0x3c, 0x60, 0x8f, 0xab,
        0xee, 0xe3, 0x41, 0x06,
    ];
    const BRIDGE_SECRET: [u8; 32] = [
        0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e,
        0x2f, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d,
        0x3e, 0x3f,
    ];
    const BRIDGE_PUBLIC: [u8; 32] = [
        0x35, 0x80, 0x72, 0xd6, 0x36, 0x58, 0x80, 0xd1, 0xae, 0xea, 0x32, 0x9a, 0xdf, 0x91, 0x21,
        0x38, 0x38, 0x51, 0xed, 0x21, 0xa2, 0x8e, 0x3b, 0x75, 0xe9, 0x65, 0xd0, 0xd2, 0xcd, 0x16,
        0x62, 0x54,
    ];
    const PEER_PUBLIC: [u8; 32] = [
        0x79, 0xa6, 0x31, 0xee, 0xde, 0x1b, 0xf9, 0xc9, 0x8f, 0x12, 0x03, 0x2c, 0xde, 0xad, 0xd0,
        0xe7, 0xa0, 0x79, 0x39, 0x8f, 0xc7, 0x86, 0xb8, 0x8c, 0xc8, 0x46, 0xec, 0x89, 0xaf, 0x85,
        0xa5, 0x1a,
    ];
    const CONFIRMATION_TAG: [u8; 16] = [
        0x30, 0x5d, 0x1a, 0xde, 0xb1, 0x70, 0x4b, 0x95, 0x13, 0x3e, 0xf1, 0x7a, 0x41, 0x06, 0xef,
        0xd9,
    ];

    struct DeterministicSecretSource {
        secrets: Mutex<BTreeMap<PairingPublicKey, [u8; 32]>>,
    }

    impl DeterministicSecretSource {
        fn new(entries: impl IntoIterator<Item = (PairingPublicKey, [u8; 32])>) -> Self {
            Self {
                secrets: Mutex::new(entries.into_iter().collect()),
            }
        }
    }

    impl fmt::Debug for DeterministicSecretSource {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("DeterministicSecretSource([REDACTED])")
        }
    }

    impl PairingEphemeralSecretSource for DeterministicSecretSource {
        fn take_secret(
            &self,
            public_key: &PairingPublicKey,
        ) -> PairingResult<PairingEphemeralSecret> {
            let secret = self
                .secrets
                .lock()
                .map_err(|_| {
                    PairingError::invalid_key_confirmation(
                        "ephemeral-secret-lock-poisoned",
                        "deterministic secret source lock was poisoned",
                    )
                })?
                .remove(public_key)
                .ok_or_else(|| {
                    PairingError::invalid_key_confirmation(
                        "ephemeral-secret-unavailable",
                        "ephemeral secret was absent or already consumed",
                    )
                })?;
            Ok(PairingEphemeralSecret::new(secret))
        }
    }

    fn public_key(bytes: [u8; 32]) -> TestResult<PairingPublicKey> {
        Ok(PairingPublicKey::new(bytes)?)
    }

    fn tag(bytes: [u8; 16]) -> TestResult<PairingConfirmationTag> {
        Ok(PairingConfirmationTag::new(bytes)?)
    }

    fn provider(secret: [u8; 32]) -> TestResult<RustCryptoPairingProvider> {
        let bridge_public = public_key(BRIDGE_PUBLIC)?;
        Ok(RustCryptoPairingProvider::new(Arc::new(
            DeterministicSecretSource::new([(bridge_public, secret)]),
        )))
    }

    #[test]
    fn ed25519_known_answer_vector_is_strict() -> TestResult {
        let provider = RustCryptoPairingProvider::new(Arc::new(DeterministicSecretSource::new([])));
        let public_key = public_key(ED25519_PUBLIC)?;
        provider.validate_ed25519_public_key(&public_key)?;
        provider.verify_ed25519(&public_key, TRANSCRIPT, &ED25519_SIGNATURE)?;

        let mut invalid = ED25519_SIGNATURE;
        invalid[0] ^= 1;
        assert!(matches!(
            provider.verify_ed25519(&public_key, TRANSCRIPT, &invalid),
            Err(PairingError::InvalidSignature { .. })
        ));
        Ok(())
    }

    #[test]
    fn x25519_hkdf_chacha_known_answer_consumes_secret_once() -> TestResult {
        let provider = provider(BRIDGE_SECRET)?;
        let bridge_public = public_key(BRIDGE_PUBLIC)?;
        let peer_public = public_key(PEER_PUBLIC)?;
        let confirmation_tag = tag(CONFIRMATION_TAG)?;
        provider.verify_key_agreement_confirmation(
            &bridge_public,
            &peer_public,
            TRANSCRIPT,
            &confirmation_tag,
        )?;
        assert!(matches!(
            provider.verify_key_agreement_confirmation(
                &bridge_public,
                &peer_public,
                TRANSCRIPT,
                &confirmation_tag,
            ),
            Err(PairingError::InvalidKeyConfirmation { .. })
        ));
        Ok(())
    }

    #[test]
    fn invalid_confirmation_consumes_the_ephemeral_secret() -> TestResult {
        let provider = provider(BRIDGE_SECRET)?;
        let bridge_public = public_key(BRIDGE_PUBLIC)?;
        let peer_public = public_key(PEER_PUBLIC)?;
        let invalid_tag = tag([0; 16])?;
        assert!(matches!(
            provider.verify_key_agreement_confirmation(
                &bridge_public,
                &peer_public,
                TRANSCRIPT,
                &invalid_tag,
            ),
            Err(PairingError::InvalidKeyConfirmation { .. })
        ));
        assert!(matches!(
            provider.verify_key_agreement_confirmation(
                &bridge_public,
                &peer_public,
                TRANSCRIPT,
                &tag(CONFIRMATION_TAG)?,
            ),
            Err(PairingError::InvalidKeyConfirmation { .. })
        ));
        Ok(())
    }

    #[test]
    fn mismatched_private_key_is_consumed_and_rejected() -> TestResult {
        let provider = provider([1; 32])?;
        let bridge_public = public_key(BRIDGE_PUBLIC)?;
        let peer_public = public_key(PEER_PUBLIC)?;
        let confirmation_tag = tag(CONFIRMATION_TAG)?;
        assert!(matches!(
            provider.verify_key_agreement_confirmation(
                &bridge_public,
                &peer_public,
                TRANSCRIPT,
                &confirmation_tag,
            ),
            Err(PairingError::InvalidPublicKey { .. })
        ));
        assert!(matches!(
            provider.verify_key_agreement_confirmation(
                &bridge_public,
                &peer_public,
                TRANSCRIPT,
                &confirmation_tag,
            ),
            Err(PairingError::InvalidKeyConfirmation { .. })
        ));
        Ok(())
    }

    #[test]
    fn invalid_peer_key_is_rejected_before_secret_consumption() -> TestResult {
        let provider = provider(BRIDGE_SECRET)?;
        let bridge_public = public_key(BRIDGE_PUBLIC)?;
        let invalid_peer = public_key([0; 32])?;
        let confirmation_tag = tag(CONFIRMATION_TAG)?;
        assert!(matches!(
            provider.verify_key_agreement_confirmation(
                &bridge_public,
                &invalid_peer,
                TRANSCRIPT,
                &confirmation_tag,
            ),
            Err(PairingError::InvalidPublicKey { .. })
        ));
        provider.verify_key_agreement_confirmation(
            &bridge_public,
            &public_key(PEER_PUBLIC)?,
            TRANSCRIPT,
            &confirmation_tag,
        )?;
        Ok(())
    }
}
