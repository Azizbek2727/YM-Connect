use std::{
    collections::BTreeMap,
    error::Error,
    io,
    sync::Arc,
    thread,
};

use ym_connect_protocol::v1::{Capability, CapabilitySet, ProtocolVersion};

use crate::*;

const BRIDGE_ID: &str = "bridge-1";
const CHALLENGE_CREATED_AT: u64 = 110;
const CHALLENGE_EXPIRES_AT: u64 = 1_110;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug)]
struct TestCrypto;

impl PairingCryptoProvider for TestCrypto {
    fn validate_ed25519_public_key(&self, public_key: &PairingPublicKey) -> PairingResult<()> {
        (public_key.as_bytes()[0] != 0).then_some(()).ok_or_else(|| {
            PairingError::invalid_public_key("ed25519", "rejected deterministic test key")
        })
    }

    fn validate_x25519_public_key(&self, public_key: &PairingPublicKey) -> PairingResult<()> {
        (public_key.as_bytes()[0] != 0).then_some(()).ok_or_else(|| {
            PairingError::invalid_public_key("x25519", "rejected deterministic test key")
        })
    }

    fn verify_ed25519(
        &self,
        _public_key: &PairingPublicKey,
        transcript: &[u8],
        signature: &[u8],
    ) -> PairingResult<()> {
        (!transcript.is_empty() && signature == b"valid-signature")
            .then_some(())
            .ok_or_else(|| PairingError::invalid_signature("verify", "invalid test signature"))
    }

    fn verify_key_agreement_confirmation(
        &self,
        _bridge_ephemeral_public_key: &PairingPublicKey,
        _peer_ephemeral_public_key: &PairingPublicKey,
        transcript: &[u8],
        confirmation_tag: &PairingConfirmationTag,
    ) -> PairingResult<()> {
        (!transcript.is_empty() && confirmation_tag.as_bytes()[0] == 7)
            .then_some(())
            .ok_or_else(|| PairingError::invalid_key_confirmation("confirm", "invalid test tag"))
    }
}

fn capabilities() -> TestResult<PairingCapabilities> {
    let mut parameters = BTreeMap::new();
    parameters.insert("pairing-mode".to_owned(), "native-peer-auth".to_owned());
    Ok(PairingCapabilities::new(CapabilitySet {
        supported: vec![
            Capability::CapabilityTrustManagement as i32,
            Capability::CapabilityClientRevocation as i32,
            Capability::CapabilityReplayProtection as i32,
            Capability::CapabilityNativePeerAuth as i32,
        ],
        required: vec![
            Capability::CapabilityReplayProtection as i32,
            Capability::CapabilityNativePeerAuth as i32,
        ],
        parameters,
    })?)
}

fn policy(
    allow_trust_replacement: bool,
    allow_revoked_replacement: bool,
) -> TestResult<PairingPolicy> {
    Ok(PairingPolicy::new(
        ProtocolVersion {
            major: 1,
            minor: 2,
            patch: 0,
        },
        capabilities()?,
        1_000,
        allow_trust_replacement,
        allow_revoked_replacement,
    )?)
}

fn new_manager(
    allow_trust_replacement: bool,
    allow_revoked_replacement: bool,
) -> TestResult<(BridgeStateStore, PairingManager)> {
    let state = BridgeStateStore::default();
    let manager = PairingManager::new(
        state.clone(),
        policy(allow_trust_replacement, allow_revoked_replacement)?,
        Arc::new(TestCrypto),
    );
    Ok((state, manager))
}

fn key(byte: u8) -> TestResult<PairingPublicKey> {
    Ok(PairingPublicKey::new([byte; 32])?)
}

fn pairing_id(value: &str) -> TestResult<PairingId> {
    Ok(PairingId::new(value)?)
}

fn challenge_id(value: &str) -> TestResult<ChallengeId> {
    Ok(ChallengeId::new(value)?)
}

fn bridge_identity() -> TestResult<BridgeIdentity> {
    Ok(BridgeIdentity::new(BridgeId::new(BRIDGE_ID)?, key(1)?))
}

fn challenge(value: &str) -> TestResult<PairingChallenge> {
    Ok(PairingChallenge::new(
        challenge_id(value)?,
        PairingNonce::new([3; 32])?,
        key(4)?,
        PairingTimestamp::from_unix_millis(CHALLENGE_CREATED_AT),
        PairingTimestamp::from_unix_millis(CHALLENGE_EXPIRES_AT),
    )?)
}

fn request(
    device_id: &str,
    identity_byte: u8,
    protocol_version: ProtocolVersion,
) -> TestResult<PairingRequest> {
    Ok(PairingRequest::new(
        DeviceId::new(device_id)?,
        key(identity_byte)?,
        key(6)?,
        protocol_version,
        capabilities()?,
    ))
}

fn response(
    challenge: &str,
    device_id: &str,
    identity_byte: u8,
) -> TestResult<PairingResponse> {
    response_with(
        challenge,
        request(
            device_id,
            identity_byte,
            ProtocolVersion {
                major: 1,
                minor: 2,
                patch: 0,
            },
        )?,
        b"valid-signature",
        [7; 16],
    )
}

fn response_with(
    challenge: &str,
    request: PairingRequest,
    signature: &[u8],
    tag: [u8; 16],
) -> TestResult<PairingResponse> {
    Ok(PairingResponse::new(
        challenge_id(challenge)?,
        request,
        Arc::<[u8]>::from(signature),
        PairingConfirmationTag::new(tag)?,
    ))
}

fn create_and_send(
    manager: &PairingManager,
    pairing: &PairingId,
    challenge: &str,
) -> TestResult {
    manager.create_session(CreatePairingSession {
        pairing_id: pairing.clone(),
        bridge_identity: bridge_identity()?,
        session_id: None,
        created_at: PairingTimestamp::from_unix_millis(100),
    })?;
    manager.create_challenge(CreatePairingChallenge {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::INITIAL,
        challenge: self::challenge(challenge)?,
    })?;
    manager.transition(TransitionPairing {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::new(1),
        state: PairingState::ChallengeSent,
        timestamp: PairingTimestamp::from_unix_millis(120),
    })?;
    Ok(())
}

fn verify(
    manager: &PairingManager,
    pairing: &PairingId,
    challenge: &str,
    device_id: &str,
    identity_byte: u8,
) -> TestResult {
    create_and_send(manager, pairing, challenge)?;
    manager.receive_response(ReceivePairingResponse {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::new(2),
        response: response(challenge, device_id, identity_byte)?,
        received_at: PairingTimestamp::from_unix_millis(130),
    })?;
    manager.verify_identity(VerifyPairingIdentity {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::new(3),
        verified_at: PairingTimestamp::from_unix_millis(140),
    })?;
    Ok(())
}

fn establish(
    manager: &PairingManager,
    pairing: PairingId,
    expected_trust_revision: Option<PairingRevision>,
    decision: TrustDecision,
    timestamp: u64,
) -> PairingResult<TrustMutation> {
    manager.establish_trust(EstablishPairingTrust {
        pairing_id: pairing,
        expected_revision: PairingRevision::new(4),
        expected_trust_revision,
        decision,
        metadata: TrustMetadata::new(),
        trusted_at: PairingTimestamp::from_unix_millis(timestamp),
    })
}

fn seed_trust(
    manager: &PairingManager,
    pairing_name: &str,
    challenge_name: &str,
    device_id: &str,
    identity_byte: u8,
    timestamp: u64,
) -> TestResult<TrustMutation> {
    let pairing = pairing_id(pairing_name)?;
    verify(
        manager,
        &pairing,
        challenge_name,
        device_id,
        identity_byte,
    )?;
    Ok(establish(
        manager,
        pairing,
        None,
        TrustDecision::Trust,
        timestamp,
    )?)
}

fn required_peer(
    store: &BridgeStateStore,
    device_id: &DeviceId,
) -> TestResult<Arc<TrustedPeer>> {
    TrustStore::lookup_trusted_peer(store, device_id)?
        .ok_or_else(|| io::Error::other("trusted peer missing").into())
}

#[test]
fn approved_algorithms_and_model_validation_are_fixed() {
    assert!(PairingPublicKey::new([1; 31]).is_err());
    assert!(PairingNonce::new([1; 31]).is_err());
    assert!(PairingConfirmationTag::new([1; 15]).is_err());
    assert!(PairingId::new("").is_err());
    assert!(ChallengeId::new("").is_err());
    assert_eq!(PairingAlgorithmSuite::KEY_AGREEMENT, "X25519");
    assert_eq!(PairingAlgorithmSuite::SIGNATURE, "Ed25519");
    assert_eq!(PairingAlgorithmSuite::KEY_DERIVATION, "HKDF-SHA-256");
    assert_eq!(
        PairingAlgorithmSuite::CONFIRMATION,
        "ChaCha20-Poly1305"
    );
}

#[test]
fn lifecycle_matrix_covers_every_legal_and_illegal_transition() {
    let states = [
        PairingState::Idle,
        PairingState::ChallengeCreated,
        PairingState::ChallengeSent,
        PairingState::ResponseReceived,
        PairingState::IdentityVerified,
        PairingState::TrustEstablished,
        PairingState::Completed,
        PairingState::Rejected,
        PairingState::Expired,
        PairingState::Revoked,
        PairingState::Cancelled,
    ];
    for previous in states {
        for next in states {
            let legal = matches!(
                (previous, next),
                (
                    PairingState::Idle,
                    PairingState::ChallengeCreated | PairingState::Cancelled
                ) | (
                    PairingState::ChallengeCreated,
                    PairingState::ChallengeSent
                        | PairingState::Expired
                        | PairingState::Cancelled
                ) | (
                    PairingState::ChallengeSent,
                    PairingState::ResponseReceived
                        | PairingState::Rejected
                        | PairingState::Expired
                        | PairingState::Cancelled
                ) | (
                    PairingState::ResponseReceived,
                    PairingState::IdentityVerified
                        | PairingState::Rejected
                        | PairingState::Expired
                        | PairingState::Cancelled
                ) | (
                    PairingState::IdentityVerified,
                    PairingState::TrustEstablished
                        | PairingState::Rejected
                        | PairingState::Revoked
                        | PairingState::Cancelled
                ) | (
                    PairingState::TrustEstablished,
                    PairingState::Completed | PairingState::Revoked
                )
            );
            assert_eq!(
                previous.can_transition_to(next),
                legal,
                "{previous:?} -> {next:?}"
            );
        }
    }
}

#[test]
fn replay_stale_revision_and_duplicate_identifiers_roll_back() -> TestResult {
    let (store, manager) = new_manager(false, false)?;
    let pairing = pairing_id("pairing-replay")?;
    create_and_send(&manager, &pairing, "challenge-replay")?;
    let before_duplicate = store.snapshot()?;
    let duplicate = manager.create_session(CreatePairingSession {
        pairing_id: pairing.clone(),
        bridge_identity: bridge_identity()?,
        session_id: None,
        created_at: PairingTimestamp::from_unix_millis(121),
    });
    assert!(matches!(
        duplicate,
        Err(PairingError::DuplicatePairing { .. })
    ));
    assert_eq!(store.snapshot()?, before_duplicate);

    let first_response = response("challenge-replay", "device-replay", 8)?;
    manager.receive_response(ReceivePairingResponse {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::new(2),
        response: first_response.clone(),
        received_at: PairingTimestamp::from_unix_millis(130),
    })?;
    let before_replay = store.snapshot()?;
    let replay = manager.receive_response(ReceivePairingResponse {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::new(3),
        response: first_response,
        received_at: PairingTimestamp::from_unix_millis(131),
    });
    assert!(matches!(
        replay,
        Err(PairingError::ReplayDetected { .. })
    ));
    assert_eq!(store.snapshot()?, before_replay);

    let stale = manager.transition(TransitionPairing {
        pairing_id: pairing,
        expected_revision: PairingRevision::new(2),
        state: PairingState::Cancelled,
        timestamp: PairingTimestamp::from_unix_millis(132),
    });
    assert!(matches!(stale, Err(PairingError::StaleRevision { .. })));
    assert_eq!(store.snapshot()?, before_replay);
    Ok(())
}

#[test]
fn challenge_freshness_expiration_and_terminal_states_are_enforced() -> TestResult {
    let (store, manager) = new_manager(false, false)?;
    let pairing = pairing_id("pairing-expiry")?;
    create_and_send(&manager, &pairing, "challenge-expiry")?;
    let before = store.snapshot()?;
    let early = manager.transition(TransitionPairing {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::new(2),
        state: PairingState::Expired,
        timestamp: PairingTimestamp::from_unix_millis(500),
    });
    assert!(matches!(
        early,
        Err(PairingError::ChallengeNotExpired { .. })
    ));
    assert_eq!(store.snapshot()?, before);

    let stale_response = manager.receive_response(ReceivePairingResponse {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::new(2),
        response: response("challenge-expiry", "device-expiry", 8)?,
        received_at: PairingTimestamp::from_unix_millis(CHALLENGE_EXPIRES_AT),
    });
    assert!(matches!(
        stale_response,
        Err(PairingError::ChallengeExpired { .. })
    ));
    assert_eq!(store.snapshot()?, before);

    manager.transition(TransitionPairing {
        pairing_id: pairing.clone(),
        expected_revision: PairingRevision::new(2),
        state: PairingState::Expired,
        timestamp: PairingTimestamp::from_unix_millis(CHALLENGE_EXPIRES_AT),
    })?;
    let terminal = manager.transition(TransitionPairing {
        pairing_id: pairing,
        expected_revision: PairingRevision::new(3),
        state: PairingState::Cancelled,
        timestamp: PairingTimestamp::from_unix_millis(CHALLENGE_EXPIRES_AT + 1),
    });
    assert!(matches!(
        terminal,
        Err(PairingError::TerminalPairing { .. })
    ));
    Ok(())
}

#[test]
fn invalid_keys_signatures_versions_and_downgrades_are_rejected() -> TestResult {
    let (_store, manager) = new_manager(false, false)?;

    let downgrade_pairing = pairing_id("pairing-downgrade")?;
    create_and_send(&manager, &downgrade_pairing, "challenge-downgrade")?;
    let downgrade = manager.receive_response(ReceivePairingResponse {
        pairing_id: downgrade_pairing,
        expected_revision: PairingRevision::new(2),
        response: response_with(
            "challenge-downgrade",
            request(
                "device-downgrade",
                8,
                ProtocolVersion {
                    major: 1,
                    minor: 1,
                    patch: 9,
                },
            )?,
            b"valid-signature",
            [7; 16],
        )?,
        received_at: PairingTimestamp::from_unix_millis(130),
    });
    assert!(matches!(
        downgrade,
        Err(PairingError::ProtocolDowngrade)
    ));

    let unsupported_pairing = pairing_id("pairing-unsupported")?;
    create_and_send(&manager, &unsupported_pairing, "challenge-unsupported")?;
    let unsupported = manager.receive_response(ReceivePairingResponse {
        pairing_id: unsupported_pairing,
        expected_revision: PairingRevision::new(2),
        response: response_with(
            "challenge-unsupported",
            request(
                "device-unsupported",
                8,
                ProtocolVersion {
                    major: 2,
                    minor: 0,
                    patch: 0,
                },
            )?,
            b"valid-signature",
            [7; 16],
        )?,
        received_at: PairingTimestamp::from_unix_millis(130),
    });
    assert!(matches!(
        unsupported,
        Err(PairingError::UnsupportedProtocolVersion)
    ));

    let invalid_key_pairing = pairing_id("pairing-invalid-key")?;
    create_and_send(&manager, &invalid_key_pairing, "challenge-invalid-key")?;
    manager.receive_response(ReceivePairingResponse {
        pairing_id: invalid_key_pairing.clone(),
        expected_revision: PairingRevision::new(2),
        response: response("challenge-invalid-key", "device-invalid-key", 0)?,
        received_at: PairingTimestamp::from_unix_millis(130),
    })?;
    let invalid_key = manager.verify_identity(VerifyPairingIdentity {
        pairing_id: invalid_key_pairing,
        expected_revision: PairingRevision::new(3),
        verified_at: PairingTimestamp::from_unix_millis(140),
    });
    assert!(matches!(
        invalid_key,
        Err(PairingError::InvalidPublicKey { .. })
    ));

    let invalid_signature_pairing = pairing_id("pairing-invalid-signature")?;
    create_and_send(
        &manager,
        &invalid_signature_pairing,
        "challenge-invalid-signature",
    )?;
    manager.receive_response(ReceivePairingResponse {
        pairing_id: invalid_signature_pairing.clone(),
        expected_revision: PairingRevision::new(2),
        response: response_with(
            "challenge-invalid-signature",
            request(
                "device-invalid-signature",
                8,
                ProtocolVersion {
                    major: 1,
                    minor: 2,
                    patch: 0,
                },
            )?,
            b"invalid-signature",
            [7; 16],
        )?,
        received_at: PairingTimestamp::from_unix_millis(130),
    })?;
    let invalid_signature = manager.verify_identity(VerifyPairingIdentity {
        pairing_id: invalid_signature_pairing,
        expected_revision: PairingRevision::new(3),
        verified_at: PairingTimestamp::from_unix_millis(140),
    });
    assert!(matches!(
        invalid_signature,
        Err(PairingError::InvalidSignature { .. })
    ));
    Ok(())
}

#[test]
fn trust_insertion_lookup_ordering_and_revocation_are_consistent() -> TestResult {
    let (store, manager) = new_manager(false, false)?;
    let first = seed_trust(
        &manager,
        "pairing-trust-b",
        "challenge-trust-b",
        "device-b",
        8,
        150,
    )?;
    let second = seed_trust(
        &manager,
        "pairing-trust-a",
        "challenge-trust-a",
        "device-a",
        9,
        151,
    )?;
    assert_eq!(first.trusted_peer().revision(), PairingRevision::INITIAL);
    assert_eq!(second.trusted_peer().revision(), PairingRevision::INITIAL);

    let peers = TrustStore::list_trusted_peers(&store)?;
    assert_eq!(peers.len(), 2);
    assert_eq!(peers[0].device_id().as_str(), "device-a");
    assert_eq!(peers[1].device_id().as_str(), "device-b");

    let device_a = DeviceId::new("device-a")?;
    let revocation = manager.revoke_trusted_peer(RevokeTrustedPeer {
        device_id: device_a.clone(),
        expected_revision: PairingRevision::INITIAL,
        revoked_at: PairingTimestamp::from_unix_millis(160),
    })?;
    let revoked = required_peer(&store, &device_a)?;
    assert!(revoked.is_revoked());
    assert_eq!(revoked.revision(), PairingRevision::new(1));
    assert_eq!(
        revoked.last_verified_at(),
        PairingTimestamp::from_unix_millis(160)
    );
    assert_eq!(
        revocation
            .snapshot()
            .pairing_sessions()
            .get(&pairing_id("pairing-trust-a")?)
            .map(PairingSession::state),
        Some(PairingState::Revoked)
    );
    Ok(())
}

#[test]
fn duplicate_identity_and_active_replacement_policy_matrix_are_enforced() -> TestResult {
    let (_store, manager) = new_manager(false, false)?;
    seed_trust(
        &manager,
        "pairing-original",
        "challenge-original",
        "device-original",
        8,
        150,
    )?;
    let duplicate_key = pairing_id("pairing-duplicate-key")?;
    verify(
        &manager,
        &duplicate_key,
        "challenge-duplicate-key",
        "device-other",
        8,
    )?;
    let duplicate = establish(
        &manager,
        duplicate_key,
        None,
        TrustDecision::Trust,
        151,
    );
    assert!(matches!(
        duplicate,
        Err(PairingError::DuplicateIdentityKey { .. })
    ));

    let different_key = pairing_id("pairing-different-key")?;
    verify(
        &manager,
        &different_key,
        "challenge-different-key",
        "device-original",
        9,
    )?;
    let forbidden = establish(
        &manager,
        different_key,
        Some(PairingRevision::INITIAL),
        TrustDecision::Replace,
        152,
    );
    assert!(matches!(
        forbidden,
        Err(PairingError::DuplicateDeviceIdentity { .. })
    ));

    let (_store, replacement_manager) = new_manager(true, false)?;
    seed_trust(
        &replacement_manager,
        "pairing-replace-original",
        "challenge-replace-original",
        "device-replace",
        8,
        150,
    )?;
    let replacement = pairing_id("pairing-replace-new")?;
    verify(
        &replacement_manager,
        &replacement,
        "challenge-replace-new",
        "device-replace",
        9,
    )?;
    let explicit_required = establish(
        &replacement_manager,
        replacement.clone(),
        Some(PairingRevision::INITIAL),
        TrustDecision::Trust,
        151,
    );
    assert!(matches!(
        explicit_required,
        Err(PairingError::DuplicateDeviceIdentity { .. })
    ));
    let replaced = establish(
        &replacement_manager,
        replacement,
        Some(PairingRevision::INITIAL),
        TrustDecision::Replace,
        151,
    )?;
    assert_eq!(replaced.trusted_peer().revision(), PairingRevision::new(1));
    assert_eq!(replaced.trusted_peer().peer_identity_key(), &key(9)?);
    Ok(())
}

#[test]
fn revoked_replacement_policy_matrix_is_enforced() -> TestResult {
    for allow_revoked_replacement in [false, true] {
        let (store, manager) = new_manager(true, allow_revoked_replacement)?;
        seed_trust(
            &manager,
            "pairing-revoked-original",
            "challenge-revoked-original",
            "device-revoked",
            8,
            150,
        )?;
        let device = DeviceId::new("device-revoked")?;
        manager.revoke_trusted_peer(RevokeTrustedPeer {
            device_id: device.clone(),
            expected_revision: PairingRevision::INITIAL,
            revoked_at: PairingTimestamp::from_unix_millis(160),
        })?;
        let replacement = pairing_id("pairing-revoked-replacement")?;
        verify(
            &manager,
            &replacement,
            "challenge-revoked-replacement",
            "device-revoked",
            9,
        )?;
        let trust_decision = establish(
            &manager,
            replacement.clone(),
            Some(PairingRevision::new(1)),
            TrustDecision::Trust,
            170,
        );
        assert!(matches!(
            trust_decision,
            Err(PairingError::RevokedPeer { .. })
        ));
        let replacement_decision = establish(
            &manager,
            replacement,
            Some(PairingRevision::new(1)),
            TrustDecision::Replace,
            170,
        );
        if allow_revoked_replacement {
            let peer = replacement_decision?;
            assert!(!peer.trusted_peer().is_revoked());
            assert_eq!(peer.trusted_peer().revision(), PairingRevision::new(2));
        } else {
            assert!(matches!(
                replacement_decision,
                Err(PairingError::RevokedPeer { .. })
            ));
            assert!(required_peer(&store, &device)?.is_revoked());
        }
    }
    Ok(())
}

#[test]
fn revoked_identity_cannot_rebind_to_another_device_id() -> TestResult {
    let (store, manager) = new_manager(true, true)?;
    seed_trust(
        &manager,
        "pairing-rebind-original",
        "challenge-rebind-original",
        "device-rebind-original",
        8,
        150,
    )?;
    let original_device = DeviceId::new("device-rebind-original")?;
    manager.revoke_trusted_peer(RevokeTrustedPeer {
        device_id: original_device.clone(),
        expected_revision: PairingRevision::INITIAL,
        revoked_at: PairingTimestamp::from_unix_millis(160),
    })?;

    let attempted_rebind = pairing_id("pairing-rebind-attempt")?;
    verify(
        &manager,
        &attempted_rebind,
        "challenge-rebind-attempt",
        "device-rebind-new",
        8,
    )?;
    let before = store.snapshot()?;
    let result = establish(
        &manager,
        attempted_rebind,
        None,
        TrustDecision::Trust,
        170,
    );
    assert!(matches!(result, Err(PairingError::RevokedPeer { .. })));
    assert_eq!(store.snapshot()?, before);
    assert!(required_peer(&store, &original_device)?.is_revoked());
    assert!(TrustStore::lookup_trusted_peer(
        &store,
        &DeviceId::new("device-rebind-new")?
    )?
    .is_none());
    Ok(())
}

#[test]
fn concurrent_pairing_creation_has_one_winner() -> TestResult {
    let (_store, manager) = new_manager(false, false)?;
    let manager = Arc::new(manager);
    let handles = (0..8)
        .map(|_| {
            let manager = Arc::clone(&manager);
            thread::spawn(move || -> PairingResult<PairingMutation> {
                manager.create_session(CreatePairingSession {
                    pairing_id: PairingId::new("pairing-concurrent")?,
                    bridge_identity: BridgeIdentity::new(
                        BridgeId::new(BRIDGE_ID)?,
                        PairingPublicKey::new([1; 32])?,
                    ),
                    session_id: None,
                    created_at: PairingTimestamp::from_unix_millis(100),
                })
            })
        })
        .collect::<Vec<_>>();
    let mut successes = 0;
    let mut duplicates = 0;
    for handle in handles {
        match handle
            .join()
            .map_err(|_| io::Error::other("pairing creation worker panicked"))?
        {
            Ok(_) => successes += 1,
            Err(PairingError::DuplicatePairing { .. }) => duplicates += 1,
            Err(error) => return Err(error.into()),
        }
    }
    assert_eq!(successes, 1);
    assert_eq!(duplicates, 7);
    Ok(())
}

#[test]
fn concurrent_trust_replacement_has_one_revision_winner() -> TestResult {
    let (store, manager) = new_manager(true, false)?;
    seed_trust(
        &manager,
        "pairing-concurrent-original",
        "challenge-concurrent-original",
        "device-concurrent",
        8,
        150,
    )?;
    let first = pairing_id("pairing-concurrent-first")?;
    let second = pairing_id("pairing-concurrent-second")?;
    verify(
        &manager,
        &first,
        "challenge-concurrent-first",
        "device-concurrent",
        9,
    )?;
    verify(
        &manager,
        &second,
        "challenge-concurrent-second",
        "device-concurrent",
        10,
    )?;

    let manager = Arc::new(manager);
    let commands = [(first.clone(), 160_u64), (second.clone(), 161_u64)];
    let handles = commands
        .into_iter()
        .map(|(pairing_id, timestamp)| {
            let manager = Arc::clone(&manager);
            thread::spawn(move || {
                establish(
                    &manager,
                    pairing_id,
                    Some(PairingRevision::INITIAL),
                    TrustDecision::Replace,
                    timestamp,
                )
            })
        })
        .collect::<Vec<_>>();
    let mut successes = 0;
    let mut stale = 0;
    for handle in handles {
        match handle
            .join()
            .map_err(|_| io::Error::other("trust replacement worker panicked"))?
        {
            Ok(_) => successes += 1,
            Err(PairingError::StaleTrustRevision { .. }) => stale += 1,
            Err(error) => return Err(error.into()),
        }
    }
    assert_eq!(successes, 1);
    assert_eq!(stale, 1);
    let peer = required_peer(&store, &DeviceId::new("device-concurrent")?)?;
    assert_eq!(peer.revision(), PairingRevision::new(1));
    let snapshot = store.snapshot()?;
    let states = [first, second]
        .into_iter()
        .map(|pairing_id| {
            snapshot
                .pairing_sessions()
                .get(&pairing_id)
                .map(PairingSession::state)
                .ok_or_else(|| io::Error::other("replacement pairing missing").into())
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == PairingState::TrustEstablished)
            .count(),
        1
    );
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == PairingState::IdentityVerified)
            .count(),
        1
    );
    Ok(())
}

#[test]
fn stale_trust_revision_and_timestamp_fail_atomically() -> TestResult {
    let (store, manager) = new_manager(true, false)?;
    seed_trust(
        &manager,
        "pairing-stale-original",
        "challenge-stale-original",
        "device-stale",
        8,
        150,
    )?;
    let replacement = pairing_id("pairing-stale-replacement")?;
    verify(
        &manager,
        &replacement,
        "challenge-stale-replacement",
        "device-stale",
        9,
    )?;
    let before = store.snapshot()?;
    let stale = establish(
        &manager,
        replacement.clone(),
        Some(PairingRevision::new(9)),
        TrustDecision::Replace,
        160,
    );
    assert!(matches!(
        stale,
        Err(PairingError::StaleTrustRevision { .. })
    ));
    assert_eq!(store.snapshot()?, before);

    let regressed = establish(
        &manager,
        replacement,
        Some(PairingRevision::INITIAL),
        TrustDecision::Replace,
        149,
    );
    assert!(matches!(
        regressed,
        Err(PairingError::TrustTimestampRegression { .. })
    ));
    assert_eq!(store.snapshot()?, before);
    Ok(())
}

#[test]
fn event_ordering_and_registry_deltas_are_deterministic() -> TestResult {
    let (_store, manager) = new_manager(false, false)?;
    let pairing = pairing_id("pairing-events")?;
    verify(
        &manager,
        &pairing,
        "challenge-events",
        "device-events",
        8,
    )?;
    let trust = establish(
        &manager,
        pairing.clone(),
        None,
        TrustDecision::Trust,
        150,
    )?;
    let trust_event = trust
        .state_update()
        .event()
        .ok_or_else(|| io::Error::other("trust event missing"))?;
    assert_eq!(
        trust_event.revision().get(),
        trust_event.previous_revision().get() + 1
    );
    let mut saw_lifecycle = false;
    let mut saw_trust = false;
    let mut saw_pairing_registry = false;
    let mut saw_trust_registry = false;
    for change in trust_event.changes() {
        match change {
            BridgeStateChange::Pairing(event) => match event.kind() {
                PairingEventKind::Lifecycle { .. } => saw_lifecycle = true,
                PairingEventKind::Trust { .. } => saw_trust = true,
            },
            BridgeStateChange::PairingSessions(delta) => {
                saw_pairing_registry = delta.replaced() == [pairing.clone()];
            }
            BridgeStateChange::TrustedPeers(delta) => {
                saw_trust_registry = delta.inserted() == [DeviceId::new("device-events")?];
            }
            _ => {}
        }
    }
    assert!(saw_lifecycle);
    assert!(saw_trust);
    assert!(saw_pairing_registry);
    assert!(saw_trust_registry);

    let revocation = manager.revoke_trusted_peer(RevokeTrustedPeer {
        device_id: DeviceId::new("device-events")?,
        expected_revision: PairingRevision::INITIAL,
        revoked_at: PairingTimestamp::from_unix_millis(160),
    })?;
    let event = revocation
        .event()
        .ok_or_else(|| io::Error::other("revocation event missing"))?;
    let pairing_events = event
        .changes()
        .iter()
        .filter_map(|change| match change {
            BridgeStateChange::Pairing(event) => Some(event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(pairing_events.len(), 2);
    assert!(matches!(
        pairing_events[0].kind(),
        PairingEventKind::Lifecycle { .. }
    ));
    assert!(matches!(
        pairing_events[1].kind(),
        PairingEventKind::Trust { .. }
    ));
    Ok(())
}

#[test]
fn identical_inputs_produce_identical_snapshots_and_events() -> TestResult {
    let (first_store, first_manager) = new_manager(false, false)?;
    let (second_store, second_manager) = new_manager(false, false)?;
    let mut first_changes = Vec::new();
    let mut second_changes = Vec::new();
    for (manager, changes) in [
        (&first_manager, &mut first_changes),
        (&second_manager, &mut second_changes),
    ] {
        for value in ["pairing-b", "pairing-a"] {
            let mutation = manager.create_session(CreatePairingSession {
                pairing_id: pairing_id(value)?,
                bridge_identity: bridge_identity()?,
                session_id: None,
                created_at: PairingTimestamp::from_unix_millis(100),
            })?;
            changes.push(
                mutation
                    .state_update()
                    .event()
                    .ok_or_else(|| io::Error::other("creation event missing"))?
                    .changes()
                    .to_vec(),
            );
        }
    }
    assert_eq!(first_store.snapshot()?, second_store.snapshot()?);
    assert_eq!(first_changes, second_changes);
    let sessions = first_manager.list_sessions()?;
    assert_eq!(sessions[0].id().as_str(), "pairing-a");
    assert_eq!(sessions[1].id().as_str(), "pairing-b");
    Ok(())
}

#[test]
fn pairing_public_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PairingManager>();
    assert_send_sync::<PairingSession>();
    assert_send_sync::<TrustedPeer>();
    assert_send_sync::<RustCryptoPairingProvider>();
}
