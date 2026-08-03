use std::{sync::Arc, thread};

use ym_connect_protocol::v1::{Capability, CapabilitySet, ProtocolVersion};

use crate::{
    BridgeId, BridgeIdentity, BridgeStateChange, BridgeStateStore, ChallengeId,
    CreatePairingChallenge, CreatePairingSession, DeviceId, EstablishPairingTrust,
    PairingCapabilities, PairingChallenge, PairingConfirmationTag, PairingCryptoProvider,
    PairingError, PairingId, PairingManager, PairingNonce, PairingPolicy, PairingPublicKey,
    PairingRequest, PairingResponse, PairingResult, PairingRevision, PairingState,
    PairingTimestamp, ReceivePairingResponse, RevokeTrustedPeer, TransitionPairing, TrustDecision,
    TrustMetadata, VerifyPairingIdentity,
};

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

fn capabilities() -> PairingCapabilities {
    PairingCapabilities::new(CapabilitySet {
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
        parameters: Default::default(),
    })
    .unwrap_or_else(|error| panic!("capabilities: {error}"))
}

fn manager(replacement: bool, revoked_replacement: bool) -> PairingManager {
    PairingManager::new(
        BridgeStateStore::default(),
        PairingPolicy::new(
            ProtocolVersion { major: 1, minor: 2, patch: 0 },
            capabilities(),
            1_000,
            replacement,
            revoked_replacement,
        )
        .unwrap_or_else(|error| panic!("policy: {error}")),
        Arc::new(TestCrypto),
    )
}

fn key(byte: u8) -> PairingPublicKey {
    PairingPublicKey::new([byte; 32]).unwrap_or_else(|error| panic!("key: {error}"))
}

fn pairing_id(value: &str) -> PairingId {
    PairingId::new(value).unwrap_or_else(|error| panic!("pairing id: {error}"))
}

fn challenge(id: &str, created_at: u64) -> PairingChallenge {
    PairingChallenge::new(
        ChallengeId::new(id).unwrap_or_else(|error| panic!("challenge id: {error}")),
        PairingNonce::new([3; 32]).unwrap_or_else(|error| panic!("nonce: {error}")),
        key(4),
        PairingTimestamp::from_unix_millis(created_at),
        PairingTimestamp::from_unix_millis(created_at + 1_000),
    )
    .unwrap_or_else(|error| panic!("challenge: {error}"))
}

fn response(challenge_id: &str, device_id: &str, identity_byte: u8) -> PairingResponse {
    PairingResponse::new(
        ChallengeId::new(challenge_id).unwrap_or_else(|error| panic!("challenge id: {error}")),
        PairingRequest::new(
            DeviceId::new(device_id).unwrap_or_else(|error| panic!("device id: {error}")),
            key(identity_byte),
            key(6),
            ProtocolVersion { major: 1, minor: 2, patch: 0 },
            capabilities(),
        ),
        Arc::<[u8]>::from(&b"valid-signature"[..]),
        PairingConfirmationTag::new([7; 16]).unwrap_or_else(|error| panic!("tag: {error}")),
    )
}

fn create_and_send(manager: &PairingManager, id: &PairingId, challenge_id: &str) {
    manager
        .create_session(CreatePairingSession {
            pairing_id: id.clone(),
            bridge_identity: BridgeIdentity::new(
                BridgeId::new("bridge-1").unwrap_or_else(|error| panic!("bridge: {error}")),
                key(1),
            ),
            session_id: None,
            created_at: PairingTimestamp::from_unix_millis(100),
        })
        .unwrap_or_else(|error| panic!("create: {error}"));
    manager
        .create_challenge(CreatePairingChallenge {
            pairing_id: id.clone(),
            expected_revision: PairingRevision::INITIAL,
            challenge: challenge(challenge_id, 110),
        })
        .unwrap_or_else(|error| panic!("challenge: {error}"));
    manager
        .transition(TransitionPairing {
            pairing_id: id.clone(),
            expected_revision: PairingRevision::new(1),
            state: PairingState::ChallengeSent,
            timestamp: PairingTimestamp::from_unix_millis(120),
        })
        .unwrap_or_else(|error| panic!("send: {error}"));
}

fn verify(manager: &PairingManager, id: &PairingId, challenge_id: &str, device_id: &str, key_byte: u8) {
    create_and_send(manager, id, challenge_id);
    manager
        .receive_response(ReceivePairingResponse {
            pairing_id: id.clone(),
            expected_revision: PairingRevision::new(2),
            response: response(challenge_id, device_id, key_byte),
            received_at: PairingTimestamp::from_unix_millis(130),
        })
        .unwrap_or_else(|error| panic!("response: {error}"));
    manager
        .verify_identity(VerifyPairingIdentity {
            pairing_id: id.clone(),
            expected_revision: PairingRevision::new(3),
            verified_at: PairingTimestamp::from_unix_millis(140),
        })
        .unwrap_or_else(|error| panic!("verify: {error}"));
}

#[test]
fn approved_algorithms_and_model_lengths_are_fixed() {
    assert!(PairingPublicKey::new([1; 31]).is_err());
    assert!(PairingNonce::new([1; 31]).is_err());
    assert!(PairingConfirmationTag::new([1; 15]).is_err());
    assert_eq!(crate::PairingAlgorithmSuite::KEY_AGREEMENT, "X25519");
    assert_eq!(crate::PairingAlgorithmSuite::SIGNATURE, "Ed25519");
    assert_eq!(crate::PairingAlgorithmSuite::KEY_DERIVATION, "HKDF-SHA-256");
    assert_eq!(crate::PairingAlgorithmSuite::CONFIRMATION, "ChaCha20-Poly1305");
}

#[test]
fn lifecycle_matrix_rejects_every_unlisted_edge_and_terminal_reuse() {
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
                (PairingState::Idle, PairingState::ChallengeCreated | PairingState::Cancelled)
                    | (PairingState::ChallengeCreated, PairingState::ChallengeSent | PairingState::Expired | PairingState::Cancelled)
                    | (PairingState::ChallengeSent, PairingState::ResponseReceived | PairingState::Rejected | PairingState::Expired | PairingState::Cancelled)
                    | (PairingState::ResponseReceived, PairingState::IdentityVerified | PairingState::Rejected | PairingState::Expired | PairingState::Cancelled)
                    | (PairingState::IdentityVerified, PairingState::TrustEstablished | PairingState::Rejected | PairingState::Revoked | PairingState::Cancelled)
                    | (PairingState::TrustEstablished, PairingState::Completed | PairingState::Revoked)
            );
            assert_eq!(previous.can_transition_to(next), legal, "{previous:?} -> {next:?}");
        }
    }
}

#[test]
fn duplicate_pairing_replay_and_stale_revision_roll_back() {
    let manager = manager(false, false);
    let id = pairing_id("pairing-replay");
    create_and_send(&manager, &id, "challenge-replay");
    assert!(matches!(
        manager.create_session(CreatePairingSession {
            pairing_id: id.clone(),
            bridge_identity: BridgeIdentity::new(
                BridgeId::new("bridge-1").unwrap_or_else(|error| panic!("bridge: {error}")),
                key(1),
            ),
            session_id: None,
            created_at: PairingTimestamp::from_unix_millis(121),
        }),
        Err(PairingError::DuplicatePairing { .. })
    ));
    let first = ReceivePairingResponse {
        pairing_id: id.clone(),
        expected_revision: PairingRevision::new(2),
        response: response("challenge-replay", "device-1", 8),
        received_at: PairingTimestamp::from_unix_millis(130),
    };
    manager.receive_response(first.clone()).unwrap_or_else(|error| panic!("first: {error}"));
    assert!(matches!(
        manager.receive_response(ReceivePairingResponse {
            expected_revision: PairingRevision::new(3),
            received_at: PairingTimestamp::from_unix_millis(131),
            ..first
        }),
        Err(PairingError::ReplayDetected { .. })
    ));
    assert_eq!(
        manager.lookup_session(&id).unwrap_or_else(|error| panic!("lookup: {error}")).unwrap_or_else(|| panic!("missing")).state(),
        PairingState::ResponseReceived
    );
}

#[test]
fn expiration_and_cancellation_are_terminal_and_deterministic() {
    let manager = manager(false, false);
    let expired = pairing_id("expired");
    create_and_send(&manager, &expired, "expired-challenge");
    assert!(matches!(
        manager.transition(TransitionPairing {
            pairing_id: expired.clone(),
            expected_revision: PairingRevision::new(2),
            state: PairingState::Expired,
            timestamp: PairingTimestamp::from_unix_millis(500),
        }),
        Err(PairingError::ChallengeNotExpired { .. })
    ));
    manager.transition(TransitionPairing {
        pairing_id: expired.clone(),
        expected_revision: PairingRevision::new(2),
        state: PairingState::Expired,
        timestamp: PairingTimestamp::from_unix_millis(1_110),
    }).unwrap_or_else(|error| panic!("expire: {error}"));
    assert!(matches!(
        manager.transition(TransitionPairing {
            pairing_id: expired,
            expected_revision: PairingRevision::new(3),
            state: PairingState::Cancelled,
            timestamp: PairingTimestamp::from_unix_millis(1_111),
        }),
        Err(PairingError::TerminalPairing { .. })
    ));
}

#[test]
fn downgrade_invalid_signature_and_invalid_public_keys_are_rejected() {
    let manager = manager(false, false);
    let id = pairing_id("security-validation");
    create_and_send(&manager, &id, "security-challenge");
    let downgraded = PairingResponse::new(
        ChallengeId::new("security-challenge").unwrap_or_else(|error| panic!("challenge: {error}")),
        PairingRequest::new(
            DeviceId::new("device-1").unwrap_or_else(|error| panic!("device: {error}")),
            key(8),
            key(6),
            ProtocolVersion { major: 1, minor: 1, patch: 9 },
            capabilities(),
        ),
        Arc::<[u8]>::from(&b"valid-signature"[..]),
        PairingConfirmationTag::new([7; 16]).unwrap_or_else(|error| panic!("tag: {error}")),
    );
    assert!(matches!(manager.receive_response(ReceivePairingResponse {
        pairing_id: id,
        expected_revision: PairingRevision::new(2),
        response: downgraded,
        received_at: PairingTimestamp::from_unix_millis(130),
    }), Err(PairingError::ProtocolDowngrade)));

    let invalid_manager = manager(false, false);
    let invalid_id = pairing_id("invalid-key");
    create_and_send(&invalid_manager, &invalid_id, "invalid-key-challenge");
    invalid_manager.receive_response(ReceivePairingResponse {
        pairing_id: invalid_id.clone(),
        expected_revision: PairingRevision::new(2),
        response: response("invalid-key-challenge", "device-invalid", 0),
        received_at: PairingTimestamp::from_unix_millis(130),
    }).unwrap_or_else(|error| panic!("response: {error}"));
    assert!(matches!(invalid_manager.verify_identity(VerifyPairingIdentity {
        pairing_id: invalid_id,
        expected_revision: PairingRevision::new(3),
        verified_at: PairingTimestamp::from_unix_millis(140),
    }), Err(PairingError::InvalidPublicKey { .. })));
}

#[test]
fn trust_establishment_revocation_and_events_are_atomic() {
    let manager = manager(false, false);
    let id = pairing_id("trust-flow");
    verify(&manager, &id, "trust-challenge", "device-trust", 8);
    let trust = manager.establish_trust(EstablishPairingTrust {
        pairing_id: id.clone(),
        expected_revision: PairingRevision::new(4),
        decision: TrustDecision::Trust,
        metadata: TrustMetadata::new(),
        trusted_at: PairingTimestamp::from_unix_millis(150),
    }).unwrap_or_else(|error| panic!("trust: {error}"));
    assert_eq!(trust.session().state(), PairingState::TrustEstablished);
    assert!(!trust.trusted_peer().is_revoked());
    assert!(trust.state_update().event().is_some_and(|event| {
        event.changes().iter().any(|change| matches!(change, BridgeStateChange::Pairing(_)))
    }));
    let device_id = DeviceId::new("device-trust").unwrap_or_else(|error| panic!("device: {error}"));
    let revoked = manager.revoke_trusted_peer(RevokeTrustedPeer {
        device_id: device_id.clone(),
        expected_revision: PairingRevision::INITIAL,
        revoked_at: PairingTimestamp::from_unix_millis(160),
    }).unwrap_or_else(|error| panic!("revoke: {error}"));
    assert!(revoked.snapshot().trusted_peers().get(&device_id).is_some_and(crate::TrustedPeer::is_revoked));
    assert_eq!(revoked.snapshot().pairing_sessions().get(&id).map(crate::PairingSession::state), Some(PairingState::Revoked));
}

#[test]
fn duplicate_identity_and_replacement_policy_are_enforced() {
    let manager = manager(false, false);
    let first = pairing_id("first-trust");
    verify(&manager, &first, "first-challenge", "device-one", 8);
    manager.establish_trust(EstablishPairingTrust {
        pairing_id: first,
        expected_revision: PairingRevision::new(4),
        decision: TrustDecision::Trust,
        metadata: TrustMetadata::new(),
        trusted_at: PairingTimestamp::from_unix_millis(150),
    }).unwrap_or_else(|error| panic!("first trust: {error}"));
    let second = pairing_id("second-trust");
    verify(&manager, &second, "second-challenge", "device-two", 8);
    assert!(matches!(manager.establish_trust(EstablishPairingTrust {
        pairing_id: second,
        expected_revision: PairingRevision::new(4),
        decision: TrustDecision::Trust,
        metadata: TrustMetadata::new(),
        trusted_at: PairingTimestamp::from_unix_millis(151),
    }), Err(PairingError::DuplicateIdentityKey { .. })));
}

#[test]
fn concurrent_creation_and_stale_trust_updates_have_one_winner() {
    let manager = Arc::new(manager(false, false));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let manager = Arc::clone(&manager);
        handles.push(thread::spawn(move || manager.create_session(CreatePairingSession {
            pairing_id: pairing_id("concurrent"),
            bridge_identity: BridgeIdentity::new(
                BridgeId::new("bridge-1").unwrap_or_else(|error| panic!("bridge: {error}")),
                key(1),
            ),
            session_id: None,
            created_at: PairingTimestamp::from_unix_millis(100),
        })));
    }
    let successes = handles.into_iter().filter(|handle| {
        handle.join().unwrap_or_else(|_| panic!("thread panicked")).is_ok()
    }).count();
    assert_eq!(successes, 1);
}

#[test]
fn snapshots_lists_and_events_are_deterministic() {
    let first = manager(false, false);
    let second = manager(false, false);
    for manager in [&first, &second] {
        for value in ["pairing-b", "pairing-a"] {
            manager.create_session(CreatePairingSession {
                pairing_id: pairing_id(value),
                bridge_identity: BridgeIdentity::new(
                    BridgeId::new("bridge-1").unwrap_or_else(|error| panic!("bridge: {error}")),
                    key(1),
                ),
                session_id: None,
                created_at: PairingTimestamp::from_unix_millis(100),
            }).unwrap_or_else(|error| panic!("create: {error}"));
        }
    }
    let first_sessions = first.list_sessions().unwrap_or_else(|error| panic!("list: {error}"));
    let second_sessions = second.list_sessions().unwrap_or_else(|error| panic!("list: {error}"));
    assert_eq!(first_sessions, second_sessions);
    assert_eq!(first_sessions[0].id().as_str(), "pairing-a");
}

#[test]
fn pairing_public_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PairingManager>();
    assert_send_sync::<crate::PairingSession>();
    assert_send_sync::<crate::TrustedPeer>();
    assert_send_sync::<crate::RustCryptoPairingProvider>();
}
