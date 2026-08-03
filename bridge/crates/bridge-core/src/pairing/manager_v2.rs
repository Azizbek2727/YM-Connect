use std::sync::Arc;

use ym_connect_protocol::v1::{CapabilitySet, ProtocolVersion};

use crate::{
    BridgeStateDraft, BridgeStateStore, DeviceId, PairingCapabilities, PairingChallenge,
    PairingCryptoProvider, PairingError, PairingId, PairingPolicy, PairingRequest, PairingResponse,
    PairingResult, PairingRevision, PairingSession, PairingState, PairingTimestamp, SessionId,
    StateUpdate, TrustDecision, TrustMetadata, TrustedPeer,
};

const TRANSCRIPT_DOMAIN: &[u8] = b"ym-connect/pairing/v1/transcript";

/// Read-only abstraction over immutable trusted-peer snapshots.
pub trait TrustStore: Send + Sync {
    /// Looks up one trusted peer.
    fn lookup_trusted_peer(&self, device_id: &DeviceId) -> PairingResult<Option<Arc<TrustedPeer>>>;
    /// Lists trusted peers in deterministic device-identifier order.
    fn list_trusted_peers(&self) -> PairingResult<Vec<Arc<TrustedPeer>>>;
}

impl TrustStore for BridgeStateStore {
    fn lookup_trusted_peer(&self, device_id: &DeviceId) -> PairingResult<Option<Arc<TrustedPeer>>> {
        Ok(self.snapshot()?.trusted_peers().get_shared(device_id))
    }

    fn list_trusted_peers(&self) -> PairingResult<Vec<Arc<TrustedPeer>>> {
        let snapshot = self.snapshot()?;
        Ok(snapshot
            .trusted_peers()
            .keys()
            .filter_map(|key| snapshot.trusted_peers().get_shared(key))
            .collect())
    }
}

/// Creates an idle pairing session.
#[derive(Clone, Debug, PartialEq)]
pub struct CreatePairingSession {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Bridge identity.
    pub bridge_identity: crate::BridgeIdentity,
    /// Optional existing Bridge session binding.
    pub session_id: Option<SessionId>,
    /// Creation timestamp.
    pub created_at: PairingTimestamp,
}

/// Creates and attaches a challenge.
#[derive(Clone, Debug, PartialEq)]
pub struct CreatePairingChallenge {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected revision.
    pub expected_revision: PairingRevision,
    /// Challenge value.
    pub challenge: PairingChallenge,
}

/// Applies a direct lifecycle transition.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionPairing {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected revision.
    pub expected_revision: PairingRevision,
    /// Requested state.
    pub state: PairingState,
    /// Operation timestamp.
    pub timestamp: PairingTimestamp,
}

/// Records a peer response.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceivePairingResponse {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected revision.
    pub expected_revision: PairingRevision,
    /// Signed response.
    pub response: PairingResponse,
    /// Observation timestamp.
    pub received_at: PairingTimestamp,
}

/// Verifies identity and pairing confirmation.
#[derive(Clone, Debug, PartialEq)]
pub struct VerifyPairingIdentity {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected revision.
    pub expected_revision: PairingRevision,
    /// Verification timestamp.
    pub verified_at: PairingTimestamp,
}

/// Establishes or replaces trust.
#[derive(Clone, Debug, PartialEq)]
pub struct EstablishPairingTrust {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected pairing revision.
    pub expected_revision: PairingRevision,
    /// Expected current trust revision for replacement, or `None` for first trust.
    pub expected_trust_revision: Option<PairingRevision>,
    /// Explicit decision.
    pub decision: TrustDecision,
    /// Immutable metadata.
    pub metadata: TrustMetadata,
    /// Trust timestamp.
    pub trusted_at: PairingTimestamp,
}

/// Revokes a trusted peer.
#[derive(Clone, Debug, PartialEq)]
pub struct RevokeTrustedPeer {
    /// Device identifier.
    pub device_id: DeviceId,
    /// Expected trust revision.
    pub expected_revision: PairingRevision,
    /// Revocation timestamp.
    pub revoked_at: PairingTimestamp,
}

/// Successful pairing mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct PairingMutation {
    session: Arc<PairingSession>,
    update: StateUpdate,
}

impl PairingMutation {
    /// Returns committed session.
    #[must_use]
    pub const fn session(&self) -> &Arc<PairingSession> {
        &self.session
    }

    /// Returns Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate {
        &self.update
    }
}

/// Successful trust mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct TrustMutation {
    session: Arc<PairingSession>,
    peer: Arc<TrustedPeer>,
    update: StateUpdate,
}

impl TrustMutation {
    /// Returns committed session.
    #[must_use]
    pub const fn session(&self) -> &Arc<PairingSession> {
        &self.session
    }

    /// Returns committed peer.
    #[must_use]
    pub const fn trusted_peer(&self) -> &Arc<TrustedPeer> {
        &self.peer
    }

    /// Returns Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate {
        &self.update
    }
}

/// Runtime-independent Pairing Core coordinator.
#[derive(Clone)]
pub struct PairingManager {
    state: BridgeStateStore,
    policy: PairingPolicy,
    crypto: Arc<dyn PairingCryptoProvider>,
}

impl std::fmt::Debug for PairingManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingManager")
            .field("state", &self.state)
            .field("policy", &self.policy)
            .field("crypto", &self.crypto)
            .finish()
    }
}

impl PairingManager {
    /// Creates a manager.
    #[must_use]
    pub fn new(
        state: BridgeStateStore,
        policy: PairingPolicy,
        crypto: Arc<dyn PairingCryptoProvider>,
    ) -> Self {
        Self { state, policy, crypto }
    }

    /// Returns policy.
    #[must_use]
    pub const fn policy(&self) -> &PairingPolicy {
        &self.policy
    }

    /// Creates an idle pairing session.
    pub fn create_session(&self, command: CreatePairingSession) -> PairingResult<PairingMutation> {
        self.crypto
            .validate_ed25519_public_key(command.bridge_identity.identity_key())?;
        let id = command.pairing_id.clone();
        let update = self.state.update_with(|draft| {
            if draft.pairing_sessions().contains_key(&id) {
                return Err(PairingError::DuplicatePairing {
                    pairing_id: id.clone(),
                });
            }
            if let Some(session_id) = &command.session_id
                && !draft.sessions().contains_key(session_id)
            {
                return Err(PairingError::MissingSession {
                    session_id: session_id.clone(),
                });
            }
            draft
                .pairing_sessions_mut()
                .insert(PairingSession::new(
                    command.pairing_id.clone(),
                    command.bridge_identity.clone(),
                    command.session_id.clone(),
                    command.created_at,
                ))?;
            Ok(())
        })?;
        pairing_mutation(&id, update)
    }

    /// Creates a challenge and transitions to `ChallengeCreated`.
    pub fn create_challenge(
        &self,
        command: CreatePairingChallenge,
    ) -> PairingResult<PairingMutation> {
        self.crypto
            .validate_x25519_public_key(command.challenge.bridge_ephemeral_key())?;
        let id = command.pairing_id.clone();
        let expected_expiry = command
            .challenge
            .created_at()
            .as_unix_millis()
            .checked_add(self.policy.challenge_lifetime_ms())
            .ok_or_else(|| PairingError::InvalidChallengeLifetime {
                pairing_id: id.clone(),
            })?;
        if command.challenge.expires_at().as_unix_millis() != expected_expiry {
            return Err(PairingError::InvalidChallengeLifetime { pairing_id: id });
        }
        let id = command.pairing_id.clone();
        let challenge_id = command.challenge.id().clone();
        let update = self.state.update_with(|draft| {
            if draft.pairing_sessions().values().any(|session| {
                session
                    .challenge()
                    .is_some_and(|challenge| challenge.id() == &challenge_id)
            }) {
                return Err(PairingError::DuplicateChallenge {
                    challenge_id: challenge_id.clone(),
                });
            }
            let current = required(draft, &id)?;
            validate_revision_time(
                current.as_ref(),
                command.expected_revision,
                command.challenge.created_at(),
            )?;
            validate_transition(current.as_ref(), PairingState::ChallengeCreated)?;
            let next = current
                .next(
                    PairingState::ChallengeCreated,
                    command.challenge.created_at(),
                    Some(command.challenge.clone()),
                    None,
                    false,
                )
                .ok_or(PairingError::RevisionExhausted)?;
            draft.pairing_sessions_mut().replace(next)?;
            Ok(())
        })?;
        pairing_mutation(&id, update)
    }

    /// Applies a lifecycle transition.
    pub fn transition(&self, command: TransitionPairing) -> PairingResult<PairingMutation> {
        let id = command.pairing_id.clone();
        let update = self.state.update_with(|draft| {
            let current = required(draft, &id)?;
            validate_revision_time(
                current.as_ref(),
                command.expected_revision,
                command.timestamp,
            )?;
            validate_transition(current.as_ref(), command.state)?;
            if command.state == PairingState::Expired {
                let challenge = current.challenge().ok_or_else(|| {
                    PairingError::state_invariant("expiration requires a challenge")
                })?;
                if !challenge.is_expired(command.timestamp) {
                    return Err(PairingError::ChallengeNotExpired {
                        pairing_id: id.clone(),
                        challenge_id: challenge.id().clone(),
                    });
                }
            }
            let next = current
                .next(command.state, command.timestamp, None, None, false)
                .ok_or(PairingError::RevisionExhausted)?;
            draft.pairing_sessions_mut().replace(next)?;
            Ok(())
        })?;
        pairing_mutation(&id, update)
    }

    /// Records a response and consumes the challenge once.
    pub fn receive_response(
        &self,
        command: ReceivePairingResponse,
    ) -> PairingResult<PairingMutation> {
        let id = command.pairing_id.clone();
        let update = self.state.update_with(|draft| {
            let current = required(draft, &id)?;
            validate_revision_time(
                current.as_ref(),
                command.expected_revision,
                command.received_at,
            )?;
            let challenge = current.challenge().ok_or_else(|| {
                PairingError::state_invariant("response requires a challenge")
            })?;
            if current.challenge_consumed() {
                return Err(PairingError::ReplayDetected {
                    pairing_id: id.clone(),
                    challenge_id: challenge.id().clone(),
                });
            }
            validate_transition(current.as_ref(), PairingState::ResponseReceived)?;
            if challenge.id() != command.response.challenge_id() {
                return Err(PairingError::ChallengeMismatch {
                    pairing_id: id.clone(),
                    expected: challenge.id().clone(),
                    actual: command.response.challenge_id().clone(),
                });
            }
            if challenge.is_expired(command.received_at) {
                return Err(PairingError::ChallengeExpired {
                    pairing_id: id.clone(),
                    challenge_id: challenge.id().clone(),
                });
            }
            validate_protocol(&self.policy, command.response.request())?;
            let _ = negotiate(&self.policy, command.response.request())?;
            if let Some(session_id) = current.session_id()
                && !draft.sessions().contains_key(session_id)
            {
                return Err(PairingError::MissingSession {
                    session_id: session_id.clone(),
                });
            }
            let next = current
                .next(
                    PairingState::ResponseReceived,
                    command.received_at,
                    None,
                    Some(command.response.clone()),
                    true,
                )
                .ok_or(PairingError::RevisionExhausted)?;
            draft.pairing_sessions_mut().replace(next)?;
            Ok(())
        })?;
        pairing_mutation(&id, update)
    }

    /// Verifies the recorded Ed25519 proof and X25519/HKDF/ChaCha20-Poly1305 confirmation.
    pub fn verify_identity(
        &self,
        command: VerifyPairingIdentity,
    ) -> PairingResult<PairingMutation> {
        let snapshot = self.state.snapshot()?;
        let current = snapshot
            .pairing_sessions()
            .get_shared(&command.pairing_id)
            .ok_or_else(|| PairingError::PairingNotFound {
                pairing_id: command.pairing_id.clone(),
            })?;
        validate_revision_time(
            current.as_ref(),
            command.expected_revision,
            command.verified_at,
        )?;
        validate_transition(current.as_ref(), PairingState::IdentityVerified)?;
        let challenge = current.challenge().ok_or_else(|| {
            PairingError::state_invariant("verification requires challenge")
        })?;
        let response = current.response().ok_or_else(|| {
            PairingError::state_invariant("verification requires response")
        })?;
        let negotiated = negotiate(&self.policy, response.request())?;
        let transcript = transcript(current.as_ref(), challenge, response.request(), &negotiated);
        self.crypto
            .validate_ed25519_public_key(response.request().identity_key())?;
        self.crypto
            .validate_x25519_public_key(response.request().ephemeral_key())?;
        self.crypto.verify_ed25519(
            response.request().identity_key(),
            &transcript,
            response.signature(),
        )?;
        self.crypto.verify_key_agreement_confirmation(
            challenge.bridge_ephemeral_key(),
            response.request().ephemeral_key(),
            &transcript,
            response.confirmation_tag(),
        )?;
        self.transition(TransitionPairing {
            pairing_id: command.pairing_id,
            expected_revision: command.expected_revision,
            state: PairingState::IdentityVerified,
            timestamp: command.verified_at,
        })
    }

    /// Establishes or replaces trust atomically with the lifecycle transition.
    pub fn establish_trust(
        &self,
        command: EstablishPairingTrust,
    ) -> PairingResult<TrustMutation> {
        let id = command.pairing_id.clone();
        let update = self.state.update_with(|draft| {
            let current = required(draft, &id)?;
            validate_revision_time(
                current.as_ref(),
                command.expected_revision,
                command.trusted_at,
            )?;
            validate_transition(current.as_ref(), PairingState::TrustEstablished)?;
            let request = current.request().ok_or_else(|| {
                PairingError::state_invariant("trust requires response")
            })?;
            if command.decision == TrustDecision::Reject {
                return Err(PairingError::TrustRejected {
                    device_id: request.device_id().clone(),
                });
            }
            if let Some(session_id) = current.session_id()
                && !draft.sessions().contains_key(session_id)
            {
                return Err(PairingError::MissingSession {
                    session_id: session_id.clone(),
                });
            }
            for peer in draft.trusted_peers().values() {
                if peer.device_id() != request.device_id()
                    && peer.peer_identity_key() == request.identity_key()
                    && !peer.is_revoked()
                {
                    return Err(PairingError::DuplicateIdentityKey {
                        existing_device_id: peer.device_id().clone(),
                    });
                }
            }
            let existing = draft.trusted_peers().get_shared(request.device_id());
            validate_expected_trust_revision(
                request.device_id(),
                command.expected_trust_revision,
                existing.as_deref(),
            )?;
            if let Some(peer) = existing.as_deref()
                && command.trusted_at < peer.last_verified_at()
            {
                return Err(PairingError::TrustTimestampRegression {
                    device_id: request.device_id().clone(),
                    previous: peer.last_verified_at(),
                    requested: command.trusted_at,
                });
            }
            let trust_revision = replacement_revision(
                &self.policy,
                command.decision,
                request,
                existing.as_deref(),
            )?;
            let peer = TrustedPeer::new(
                current.bridge_identity().clone(),
                request.device_id().clone(),
                request.identity_key().clone(),
                negotiate(&self.policy, request)?,
                request.protocol_version().clone(),
                command.trusted_at,
                command.metadata.clone(),
                trust_revision,
            );
            draft.trusted_peers_mut().upsert(peer)?;
            let next = current
                .next(
                    PairingState::TrustEstablished,
                    command.trusted_at,
                    None,
                    None,
                    false,
                )
                .ok_or(PairingError::RevisionExhausted)?;
            draft.pairing_sessions_mut().replace(next)?;
            Ok(())
        })?;
        let session = update
            .snapshot()
            .pairing_sessions()
            .get_shared(&id)
            .ok_or_else(|| PairingError::state_invariant("committed pairing missing"))?;
        let device_id = session
            .request()
            .ok_or_else(|| PairingError::state_invariant("committed request missing"))?
            .device_id();
        let peer = update
            .snapshot()
            .trusted_peers()
            .get_shared(device_id)
            .ok_or_else(|| PairingError::state_invariant("committed peer missing"))?;
        Ok(TrustMutation { session, peer, update })
    }

    /// Revokes a trusted peer and any trust-established pairing aggregates atomically.
    pub fn revoke_trusted_peer(&self, command: RevokeTrustedPeer) -> PairingResult<StateUpdate> {
        self.state.update_with(|draft| {
            let current = draft
                .trusted_peers()
                .get_shared(&command.device_id)
                .ok_or_else(|| PairingError::TrustNotFound {
                    device_id: command.device_id.clone(),
                })?;
            if current.revision() != command.expected_revision {
                return Err(PairingError::StaleTrustRevision {
                    device_id: command.device_id.clone(),
                    expected: command.expected_revision,
                    actual: current.revision(),
                });
            }
            if current.is_revoked() {
                return Err(PairingError::RevokedPeer {
                    device_id: command.device_id.clone(),
                });
            }
            if command.revoked_at < current.last_verified_at() {
                return Err(PairingError::TrustTimestampRegression {
                    device_id: command.device_id.clone(),
                    previous: current.last_verified_at(),
                    requested: command.revoked_at,
                });
            }
            draft.trusted_peers_mut().replace(
                current
                    .revoked(command.revoked_at)
                    .ok_or(PairingError::RevisionExhausted)?,
            )?;
            let affected = draft
                .pairing_sessions()
                .values()
                .filter(|session| {
                    session.state() == PairingState::TrustEstablished
                        && session.request().is_some_and(|request| {
                            request.device_id() == &command.device_id
                        })
                })
                .map(|session| session.id().clone())
                .collect::<Vec<_>>();
            for pairing_id in affected {
                let session = draft
                    .pairing_sessions()
                    .get_shared(&pairing_id)
                    .ok_or_else(|| {
                        PairingError::state_invariant("revocation pairing disappeared")
                    })?;
                let next = session
                    .next(
                        PairingState::Revoked,
                        command.revoked_at,
                        None,
                        None,
                        false,
                    )
                    .ok_or(PairingError::RevisionExhausted)?;
                draft.pairing_sessions_mut().replace(next)?;
            }
            Ok(())
        })
    }

    /// Looks up a pairing session.
    pub fn lookup_session(&self, id: &PairingId) -> PairingResult<Option<Arc<PairingSession>>> {
        Ok(self.state.snapshot()?.pairing_sessions().get_shared(id))
    }

    /// Lists pairing sessions in deterministic order.
    pub fn list_sessions(&self) -> PairingResult<Vec<Arc<PairingSession>>> {
        let snapshot = self.state.snapshot()?;
        Ok(snapshot
            .pairing_sessions()
            .keys()
            .filter_map(|key| snapshot.pairing_sessions().get_shared(key))
            .collect())
    }
}

fn required(draft: &BridgeStateDraft, id: &PairingId) -> PairingResult<Arc<PairingSession>> {
    draft
        .pairing_sessions()
        .get_shared(id)
        .ok_or_else(|| PairingError::PairingNotFound {
            pairing_id: id.clone(),
        })
}

fn validate_revision_time(
    session: &PairingSession,
    expected: PairingRevision,
    at: PairingTimestamp,
) -> PairingResult<()> {
    if session.revision() != expected {
        return Err(PairingError::StaleRevision {
            pairing_id: session.id().clone(),
            expected,
            actual: session.revision(),
        });
    }
    if at < session.updated_at() {
        return Err(PairingError::TimestampRegression {
            pairing_id: session.id().clone(),
            previous: session.updated_at(),
            requested: at,
        });
    }
    Ok(())
}

fn validate_transition(session: &PairingSession, requested: PairingState) -> PairingResult<()> {
    if session.state().is_terminal() {
        return Err(PairingError::TerminalPairing {
            pairing_id: session.id().clone(),
            state: session.state(),
        });
    }
    if !session.state().can_transition_to(requested) {
        return Err(PairingError::InvalidTransition {
            pairing_id: session.id().clone(),
            previous: session.state(),
            requested,
        });
    }
    Ok(())
}

fn validate_protocol(policy: &PairingPolicy, request: &PairingRequest) -> PairingResult<()> {
    let local = policy.protocol_version();
    let remote = request.protocol_version();
    if remote.major != local.major || version_tuple(remote) > version_tuple(local) {
        return Err(PairingError::UnsupportedProtocolVersion);
    }
    if version_tuple(remote) < version_tuple(local) {
        return Err(PairingError::ProtocolDowngrade);
    }
    Ok(())
}

fn negotiate(policy: &PairingPolicy, request: &PairingRequest) -> PairingResult<PairingCapabilities> {
    let local = policy.capabilities().canonical();
    let remote = request.capabilities().canonical();
    let supported = local
        .supported
        .iter()
        .copied()
        .filter(|value| remote.supported.binary_search(value).is_ok())
        .collect::<Vec<_>>();
    if local
        .required
        .iter()
        .any(|value| supported.binary_search(value).is_err())
        || remote
            .required
            .iter()
            .any(|value| supported.binary_search(value).is_err())
    {
        return Err(PairingError::MissingRequiredCapabilities);
    }
    let mut required = local
        .required
        .iter()
        .chain(remote.required.iter())
        .copied()
        .collect::<Vec<_>>();
    required.sort_unstable();
    required.dedup();
    PairingCapabilities::new(CapabilitySet {
        supported,
        required,
        parameters: local.parameters.clone(),
    })
    .map_err(Into::into)
}

fn validate_expected_trust_revision(
    device_id: &DeviceId,
    expected: Option<PairingRevision>,
    existing: Option<&TrustedPeer>,
) -> PairingResult<()> {
    match (expected, existing) {
        (None, None) => Ok(()),
        (Some(_), None) => Err(PairingError::TrustNotFound {
            device_id: device_id.clone(),
        }),
        (None, Some(_)) => Err(PairingError::TrustReplacementForbidden {
            device_id: device_id.clone(),
        }),
        (Some(expected), Some(peer)) if expected != peer.revision() => {
            Err(PairingError::StaleTrustRevision {
                device_id: device_id.clone(),
                expected,
                actual: peer.revision(),
            })
        }
        (Some(_), Some(_)) => Ok(()),
    }
}

fn replacement_revision(
    policy: &PairingPolicy,
    decision: TrustDecision,
    request: &PairingRequest,
    existing: Option<&TrustedPeer>,
) -> PairingResult<PairingRevision> {
    match existing {
        None if decision == TrustDecision::Trust || decision == TrustDecision::Replace => {
            Ok(PairingRevision::INITIAL)
        }
        None => Err(PairingError::TrustRejected {
            device_id: request.device_id().clone(),
        }),
        Some(peer)
            if peer.is_revoked()
                && (!policy.allow_revoked_replacement()
                    || decision != TrustDecision::Replace) =>
        {
            Err(PairingError::RevokedPeer {
                device_id: request.device_id().clone(),
            })
        }
        Some(peer)
            if peer.peer_identity_key() != request.identity_key()
                && (!policy.allow_trust_replacement()
                    || decision != TrustDecision::Replace) =>
        {
            Err(PairingError::DuplicateDeviceIdentity {
                device_id: request.device_id().clone(),
            })
        }
        Some(_) if decision != TrustDecision::Replace => {
            Err(PairingError::TrustReplacementForbidden {
                device_id: request.device_id().clone(),
            })
        }
        Some(peer) => peer
            .revision()
            .checked_next()
            .ok_or(PairingError::RevisionExhausted),
    }
}

fn version_tuple(version: &ProtocolVersion) -> (u32, u32, u32) {
    (version.major, version.minor, version.patch)
}

fn pairing_mutation(id: &PairingId, update: StateUpdate) -> PairingResult<PairingMutation> {
    let session = update
        .snapshot()
        .pairing_sessions()
        .get_shared(id)
        .ok_or_else(|| PairingError::state_invariant("committed pairing missing"))?;
    Ok(PairingMutation { session, update })
}

fn transcript(
    session: &PairingSession,
    challenge: &PairingChallenge,
    request: &PairingRequest,
    negotiated: &PairingCapabilities,
) -> Vec<u8> {
    let mut output = Vec::new();
    field(&mut output, TRANSCRIPT_DOMAIN);
    field(&mut output, session.id().as_str().as_bytes());
    field(
        &mut output,
        session.bridge_identity().id().as_str().as_bytes(),
    );
    field(
        &mut output,
        session.bridge_identity().identity_key().as_bytes(),
    );
    field(&mut output, request.device_id().as_str().as_bytes());
    field(&mut output, request.identity_key().as_bytes());
    field(&mut output, challenge.id().as_str().as_bytes());
    field(&mut output, challenge.nonce().as_bytes());
    field(&mut output, challenge.bridge_ephemeral_key().as_bytes());
    field(&mut output, request.ephemeral_key().as_bytes());
    field(
        &mut output,
        &request.protocol_version().major.to_be_bytes(),
    );
    field(
        &mut output,
        &request.protocol_version().minor.to_be_bytes(),
    );
    field(
        &mut output,
        &request.protocol_version().patch.to_be_bytes(),
    );
    capability_fields(&mut output, request.capabilities().canonical());
    capability_fields(&mut output, negotiated.canonical());
    field(
        &mut output,
        session
            .session_id()
            .map_or(&[][..], |value| value.as_str().as_bytes()),
    );
    output
}

fn capability_fields(output: &mut Vec<u8>, capabilities: &CapabilitySet) {
    for value in &capabilities.supported {
        field(output, &value.to_be_bytes());
    }
    for value in &capabilities.required {
        field(output, &value.to_be_bytes());
    }
    for (key, value) in &capabilities.parameters {
        field(output, key.as_bytes());
        field(output, value.as_bytes());
    }
}

fn field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}
