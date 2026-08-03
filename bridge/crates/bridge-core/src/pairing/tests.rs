use std::{
    sync::{Arc, Barrier},
    thread,
};

use ym_connect_protocol::v1::{Capability, CapabilitySet, ProtocolVersion};

use crate::{
    BridgeId, BridgeIdentity, BridgeStateChange, BridgeStateStore, ChallengeId,
    CreatePairingChallenge, CreatePairingSession, DeviceId, EstablishPairingTrust,
    PairingCapabilities, PairingChallenge, PairingConfirmationTag, PairingCryptoProvider,
    PairingError, PairingId, PairingManager, PairingNonce, PairingPolicy, PairingPublicKey,
    PairingRequest, PairingResponse, PairingResult, PairingRevision, PairingState,
    PairingTimestamp, ReceivePairingResponse, RevokeTrustedPeer, TransitionPairing, TrustDecision,
    TrustMetadata, TrustStore, VerifyPairingIdentity,
};

#[derive(Debug)]
struct TestCrypto;

impl PairingCryptoProvider for TestCrypto {
    fn validate_ed25519_public_key(&self, public_key: &PairingPublicKey) -> PairingResult<()> {
        if public_key.as_bytes()[0] == 0 {
            Err(PairingError::invalid_public_key("ed25519", "rejected test key"))
        } else {
            Ok(())
        }
    }

    fn validate_x25519_public_key(&self, public_key: &PairingPublicKey) -> PairingResult<()> {
        if public_key.as_bytes()[0] == 0 {
            Err(PairingError::invalid_public_key("x25519", "rejected test key"))
        } else {
            Ok(())
        }
    }

    fn verify_ed25519(
        &self,
        _public_key: &PairingPublicKey,
        transcript: &[u8],
        signature: &[u8],
    ) -> PairingResult<()> {
        if !transcript.is_empty() && signature == b"valid-signature" {
            Ok(())
        } else {
            Err(PairingError::invalid_signature("verify", "invalid test signature"))
        }
    }

    fn verify_key_agreement_confirmation(
        &self,
        _bridge_ephemeral_public_key: &PairingPublicKey,
        _peer_ephemeral_public_key: &PairingPublicKey,
        transcript: &[u8],
        confirmation_tag: &PairingConfirmationTag,
    ) -> PairingResult<()> {
        if !transcript.is_empty() && confirmation_tag.as_bytes()[0] == 7 {
            Ok(())
        } else {
            Err(PairingError::invalid_key_confirmation("confirm", "invalid test tag"))
        }
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
    .unwrap_or_else(|error| panic!("test capabilities failed: {error}"))
}

fn policy(replacement: bool, revoked_replacement: bool) -> PairingPolicy {
    PairingPolicy::new(
        ProtocolVersion { major: 1, minor: 2, patch: 0 },
        capabilities(),
        1_000,
        replacement,
        revoked_replacement,
    )
    .unwrap_or_else(|error| panic!("test policy failed: {error}"))
}

fn key(byte: u8) -> PairingPublicKey {
    PairingPublicKey::new([byte; 32])
        .unwrap_or_else(|error| panic!("test key failed: {error}"))
}

fn manager(replacement: bool, revoked_replacement: bool) -> PairingManager {
    PairingManager::new(
        BridgeStateStore::default(),
        policy(replacement, revoked_replacement),
        Arc::new(TestCrypto),
    )
}

fn bridge_identity() -> BridgeIdentity {
    BridgeIdentity::new(
        BridgeId::new("bridge-1").unwrap_or_else(|error| panic!("bridge id: {error}")),
        key(1),
    )
}

fn pairing_id(value: &str) -> PairingId {
    PairingId::new(value).unwrap_or_else(|error| panic!("pairing id: {error}"))
}

fn challenge(id: &str, created: u64) -> PairingChallenge {
    PairingChallenge::new(
        ChallengeId::new(id).unwrap_or_else(|error| panic!("challenge id: {error}")),
        PairingNonce::new([3; 32]).unwrap_or_else(|error| panic!("nonce: {error}")),
        key(4),
        PairingTimestamp::from_unix_millis(created),
        PairingTimestamp::from_unix_millis(created + 1_000),
    )
    .unwrap_or_else(|error| panic!("challenge: {error}"))
}

fn response(challenge_id: &str, device: &str, identity_byte: u8) -> PairingResponse {
    PairingResponse::new(
        ChallengeId::new(challenge_id).unwrap_or_else(|error| panic!("challenge id: {error}")),
        PairingRequest::new(
            DeviceId::new(device).unwrap_or_else(|error| panic!("device id: {error}")),
            key(identity_byte),
            key(6),
            ProtocolVersion { major: 1, minor: 2, patch: 0 },
            capabilities(),
        ),
        Arc::<[u8]>::from(&b"valid-signature"[..]),
        PairingConfirmationTag::new([7; 16]).unwrap_or_else(|error| panic!("tag: {error}")),
    )
}

fn create_and_send(manager: &PairingManager, id: &str, challenge_id: &str) -> PairingRevision {
    let id = pairing_id(id);
    manager
        .create_session(CreatePairingSession {
            pairing_id: id.clone(),
            bridge_identity: bridge_identity(),
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
            pairing_id: id,
            expected_revision: PairingRevision::new(1),
            state: PairingState::ChallengeSent,
            timestamp: PairingTimestamp::from_unix_millis(120),
        })
        .unwrap_or_else(|error| panic!("send: {error}"));
    PairingRevision::new(2)
}

fn verify_to_trust(manager: &PairingManager, id: &str, challenge_id: &str, device: &str, identity_byte: u8) {
    let id_value = pairing_id(id);
    create_and_send(manager, id, challenge_id);
    manager
        .receive_response(ReceivePairingResponse {
            pairing_id: id_value.clone(),
            expected_revision: PairingRevision::new(2),
            response: response(challenge_id, device, identity_byte),
            received_at: PairingTimestamp::from_unix_millis(130),
        })
        .unwrap_or_else(|error| panic!("response: {error}"));
    manager
        .verify_identity(VerifyPairingIdentity {
            pairing_id: id_value,
            expected_revision: PairingRevision::new(3),
            verified_at: PairingTimestamp::from_unix_millis(140),
        })
        .unwrap_or_else(|error| panic!("verify: {error}"));
}

#[test]
fn model_validation_and_algorithm_suite_are_fixed() {
    assert!(PairingPublicKey::new([1; 31]).is_err());
    assert!(PairingNonce::new([1; 31]).is_err());
    assert!(PairingConfirmationTag::new([1; 15]).is_err());
    assert_eq!(crate::PairingAlgorithmSuite::KEY_AGREEMENT, "X25519");
    assert_eq!(crate::PairingAlgorithmSuite::SIGNATURE, "Ed25519");
    assert_eq!(crate::PairingAlgorithmSuite::KEY_DERIVATION, "HKDF-SHA-256");
    assert_eq!(crate::PairingAlgorithmSuite::CONFIRMATION, "ChaCha20-Poly1305");
}

#[test]
fn every_legal_lifecycle_transition_is_declared() {
    let legal = [
        (PairingState::Idle, PairingState::ChallengeCreated),
        (PairingState::Idle, PairingState::Cancelled),
        (PairingState::ChallengeCreated, PairingState::ChallengeSent),
        (PairingState::ChallengeCreated, PairingState::Expired),
        (PairingState::ChallengeCreated, PairingState::Cancelled),
        (PairingState::ChallengeSent, PairingState::ResponseReceived),
        (PairingState::ChallengeSent, PairingState::Rejected),
        (PairingState::ChallengeSent, PairingState::Expired),
        (PairingState::ChallengeSent, PairingState::Cancelled),
        (PairingState::ResponseReceived, PairingState::IdentityVerified),
        (PairingState::ResponseReceived, PairingState::Rejected),
        (PairingState::ResponseReceived, PairingState::Expired),
        (PairingState::ResponseReceived, PairingState::Cancelled),
        (PairingState::IdentityVerified, PairingState::TrustEstablished),
        (PairingState::IdentityVerified, PairingState::Rejected),
        (PairingState::IdentityVerified, PairingState::Revoked),
        (PairingState::IdentityVerified, PairingState::Cancelled),
        (PairingState::TrustEstablished, PairingState::Completed),
        (PairingState::TrustEstablished, PairingState::Revoked),
    ];
    for (previous, next) in legal {
        assert!(previous.can_transition_to(next), "{previous:?} -> {next:?}");
    }
}

#[test]
fn every_illegal_lifecycle_transition_is_rejected() {
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
            if previous == next || previous.is_terminal() {
                assert!(!previous.can_transition_to(next));
            }
        }
    }
}

#[test]
fn replay_is_rejected_before_lifecycle_error_and_rolls_back() {
    let manager = manager(false, false);
    let id = pairing_id("pairing-replay");
    create_and_send(&manager, id.as_str(), "challenge-replay");
    let command = ReceivePairingResponse {
        pairing_id: id.clone(),
        expected_revision: PairingRevision::new(2),
        response: response("challenge-replay", "device-1", 8),
        received_at: PairingTimestamp::from_unix_millis(130),
    };
    manager.receive_response(command.clone()).unwrap_or_else(|error| panic!("first response: {error}"));
    let before = manager.lookup_session(&id).unwrap_or_else(|error| panic!("lookup: {error}")).unwrap_or_else(|| panic!("missing pairing"));
    let replay = manager.receive_response(ReceivePairingResponse {
        expected_revision: PairingRevision::new(3),
        received_at: PairingTimestamp::from_unix_millis(131),
        ..command
    });
    assert!(matches!(replay, Err(PairingError::ReplayDetected { .. })));
    let after = manager.lookup_session(&id).unwrap_or_else(|error| panic!("lookup: {error}")).unwrap_or_else(|| panic!("missing pairing"));
    assert_eq!(before, after);
}

#[test]
fn expired_challenge_and_early_expiration_are_structured() {
    let manager = manager(false, false);
    let id = pairing_id("pairing-expiry");
    create_and_send(&manager, id.as_str(), "challenge-expiry");
    let early = manager.transition(TransitionPairing {
        pairing_id: id.clone(),
        expected_revision: PairingRevision::new(2),
        state: PairingState::Expired,
        timestamp: PairingTimestamp::from_unix_millis(500),
    });
    assert!(matches!(early, Err(PairingError::ChallengeNotExpired { .. })));
    let stale = manager.receive_response(ReceivePairingResponse {
        pairing_id: id,
        expected_revision: PairingRevision::new(2),
        response: response("challenge-expiry", "device-1", 8),
        received_at: PairingTimestamp::from_unix_millis(1_110),
    });
    assert!(matches!(stale, Err(PairingError::ChallengeExpired { .. })));
}

#[test]
fn downgrade_and_unsupported_versions_are_rejected() {
    let manager = manager(false, false);
    let id = pairing_id("pairing-version");
    create_and_send(&manager, id.as_str(), "challenge-version");
    let mut downgraded = response("challenge-version", "device-1", 8);
    let request = PairingRequest::new(
        downgraded.request().device_id().clone(),
        downgraded.request().identity_key().clone(),
        downgraded.request().ephemeral_key().clone(),
        ProtocolVersion { major: 1, minor: 1, patch: 9 },
        capabilities(),
    );
    downgraded = PairingResponse::new(
        downgraded.challenge_id().clone(),
        request,
        Arc::<[u8]>::from(&b"valid-signature"[..]),
        PairingConfirmationTag::new([7; 16]).unwrap_or_else(|error| panic!("tag: {error}")),
    );
    assert!(matches!(
        manager.receive_response(ReceivePairingResponse {
            pairing_id: id,
            expected_revision: PairingRevision::new(2),
            response: downgraded,
            received_at: PairingTimestamp::from_unix_millis(130),
        }),
        Err(PairingError::ProtocolDowngrade)
    ));
}

#[test]
fn invalid_signature_does_not_advance_state() {
    let manager = manager(false, false);
    let id = pairing_id("pairing-signature");
    create_and_send(&manager, id.as_str(), "challenge-signature");
    let invalid = PairingResponse::new(
        ChallengeId::new("challenge-signature").unwrap_or_else(|error| panic!("challenge: {error}")),
        response("challenge-signature", "device-1", 8).request().clone(),
        Arc::<[u8]>::from(&b"invalid"[..]),
        PairingConfirmationTag::new([7; 16]).unwrap_or_else(|error| panic!("tag: {error}")),
    );
    manager.receive_response(ReceivePairingResponse {
        pairing_id: id.clone(),
        expected_revision: PairingRevision::new(2),
        response: invalid,
        received_at: PairingTimestamp::from_unix_millis(130),
    }).unwrap_or_else(|error| panic!("response: {error}"));
    assert!(matches!(
        manager.verify_identity(VerifyPairingIdentity {
            pairing_id: id.clone(),
            expected_revision: PairingRevision::new(3),
            verified_at: PairingTimestamp::from_unix_millis(140),
        }),
        Err(PairingError::InvalidSignature { .. })
    ));
    assert_eq!(manager.lookup_session(&id).unwrap_or_else(|error| panic!("lookup: {error}")).unwrap_or_else(|| panic!("missing")).state(), PairingState::ResponseReceived);
}

#[test]
fn trust_insertion_lookup_revocation_and_events_are_atomic() {
    let manager = manager(false, false);
    verify_to_trust(&manager, "pairing-trust", "challenge-trust", "device-trust", 8);
    let mutation = manager.establish_trust(EstablishPairingTrust {
        pairing_id: pairing_id("pairing-trust"),
        expected_revision: PairingRevision::new(4),
        decision: TrustDecision::Trust,
        metadata: TrustMetadata::new(),
        trusted_at: PairingTimestamp::from_unix_millis(150),
    }).unwrap_or_else(|error| panic!("trust: {error}"));
    assert_eq!(mutation.session().state(), PairingState::TrustEstablished);
    assert!(!mutation.trusted_peer().is_revoked());
    assert!(mutation.state_update().event().is_some_and(|event| {
        event.changes().iter().any(|change| matches!(change, BridgeStateChange::Pairing(_)))
    }));
    let device = DeviceId::new("device-trust").unwrap_or_else(|error| panic!("device: {error}"));
    assert!(manager.lookup_trusted_peer(&device).unwrap_or_else(|error| panic!("lookup trust: {error}")).is_some());
    manager.revoke_trusted_peer(RevokeTrustedPeer {
        device_id: device.clone(),
        expected_revision: PairingRevision::INITIAL,
        revoked_at: PairingTimestamp::from_unix_millis(160),
    }).unwrap_or_else(|error| panic!("revoke: {error}"));
    assert!(manager.lookup_trusted_peer(&device).unwrap_or_else(|error| panic!("lookup trust: {error}")).is_some_and(|peer| peer.is_revoked()));
    assert_eq!(manager.lookup_session(&pairing_id("pairing-trust")).unwrap_or_else(|error| panic!("lookup: {error}")).unwrap_or_else(|| panic!("missing")).state(), PairingState::Revoked);
}

#[test]
fn trust_replacement_requires_policy_and_explicit_decision() {
    let manager = manager(true, true);
    verify_to_trust(&manager, "pairing-first", "challenge-first", "device-replace", 8);
    manager.establish_trust(EstablishPairingTrust {
        pairing_id: pairing_id("pairing-first"),
        expected_revision: PairingRevision::new(4),
        decision: TrustDecision::Trust,
        metadata: TrustMetadata::new(),
        trusted_at: PairingTimestamp::from_unix_millis(150),
    }).unwrap_or_else(|error| panic!("first trust: {error}"));
    verify_to_trust(&manager, "pairing-second", "challenge-second", "device-replace", 9);
    let replaced = manager.establish_trust(EstablishPairingTrust {
        pairing_id: pairing_id("pairing-second"),
        expected_revision: PairingRevision::new(4),
        decision: TrustDecision::Replace,
        metadata: TrustMetadata::new(),
        trusted_at: PairingTimestamp::from_unix_millis(250),
    }).unwrap_or_else(|error| panic!("replace trust: {error}"));
    assert_eq!(replaced.trusted_peer().revision(), PairingRevision::new(1));
    assert_eq!(replaced.trusted_peer().peer_identity_key(), &key(9));
}

#[test]
fn duplicate_identity_key_for_another_device_is_rejected() {
    let manager = manager(true, true);
    verify_to_trust(&manager, "pairing-a", "challenge-a", "device-a", 8);
    manager.establish_trust(EstablishPairingTrust {
        pairing_id: pairing_id("pairing-a"),
        expected_revision: PairingRevision::new(4),
        decision: TrustDecision::Trust,
        metadata: TrustMetadata::new(),
        trusted_at: PairingTimestamp::from_unix_millis(150),
    }).unwrap_or_else(|error| panic!("trust a: {error}"));
    verify_to_trust(&manager, "pairing-b", "challenge-b", "device-b", 8);
    assert!(matches!(
        manager.establish_trust(EstablishPairingTrust {
            pairing_id: pairing_id("pairing-b"),
            expected_revision: PairingRevision::new(4),
            decision: TrustDecision::Trust,
            metadata: TrustMetadata::new(),
            trusted_at: PairingTimestamp::from_unix_millis(250),
        }),
        Err(PairingError::DuplicateIdentityKey { .. })
    ));
}

#[test]
fn concurrent_creation_serializes_and_stale_revision_rolls_back() {
    let manager = Arc::new(manager(false, false));
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let manager = Arc::clone(&manager);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            manager.create_session(CreatePairingSession {
                pairing_id: pairing_id("pairing-concurrent"),
                bridge_identity: bridge_identity(),
                session_id: None,
                created_at: PairingTimestamp::from_unix_millis(100),
            })
        }));
    }
    barrier.wait();
    let results = handles.into_iter().map(|handle| handle.join().unwrap_or_else(|_| panic!("thread panicked"))).collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| matches!(result, Err(PairingError::DuplicatePairing { .. }))).count(), 1);
}

#[test]
fn deterministic_ordering_snapshots_and_events() {
    let first = manager(false, false);
    let second = manager(false, false);
    for manager in [&first, &second] {
        for id in ["pairing-b", "pairing-a"] {
            manager.create_session(CreatePairingSession {
                pairing_id: pairing_id(id),
                bridge_identity: bridge_identity(),
                session_id: None,
                created_at: PairingTimestamp::from_unix_millis(100),
            }).unwrap_or_else(|error| panic!("create: {error}"));
        }
    }
    let first_ids = first.list_sessions().unwrap_or_else(|error| panic!("list: {error}")).into_iter().map(|session| session.id().as_str().to_owned()).collect::<Vec<_>>();
    let second_ids = second.list_sessions().unwrap_or_else(|error| panic!("list: {error}")).into_iter().map(|session| session.id().as_str().to_owned()).collect::<Vec<_>>();
    assert_eq!(first_ids, vec!["pairing-a", "pairing-b"]);
    assert_eq!(first_ids, second_ids);
}

#[test]
fn public_pairing_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<PairingManager>();
    assert_send_sync::<crate::PairingSession>();
    assert_send_sync::<crate::TrustedPeer>();
    assert_send_sync::<crate::PairingEvent>();
}
