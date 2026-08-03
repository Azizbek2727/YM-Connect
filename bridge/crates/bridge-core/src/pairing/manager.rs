use std::sync::Arc;

use ym_connect_protocol::v1::{CapabilitySet, ProtocolVersion};

use crate::{
    BridgeStateStore, ChallengeId, DeviceId, PairingCapabilities, PairingChallenge,
    PairingCryptoProvider, PairingError, PairingId, PairingPolicy, PairingRequest, PairingResponse,
    PairingResult, PairingRevision, PairingSession, PairingState, PairingTimestamp, SessionId,
    StateUpdate, TrustDecision, TrustMetadata, TrustedPeer,
};

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
        Ok(self
            .snapshot()?
            .trusted_peers()
            .values()
            .cloned()
            .map(Arc::new)
            .collect())
    }
}

/// Creates an idle pairing session.
#[derive(Clone, Debug, Eq, PartialEq)]
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

/// Creates and attaches a challenge to an idle pairing session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePairingChallenge {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected pairing revision.
    pub expected_revision: PairingRevision,
    /// Challenge value.
    pub challenge: PairingChallenge,
}

/// Transitions a pairing lifecycle state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionPairing {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected pairing revision.
    pub expected_revision: PairingRevision,
    /// Requested state.
    pub state: PairingState,
    /// Operation timestamp.
    pub timestamp: PairingTimestamp,
}

/// Records a received pairing response.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReceivePairingResponse {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected pairing revision.
    pub expected_revision: PairingRevision,
    /// Response value.
    pub response: PairingResponse,
    /// Observation timestamp.
    pub received_at: PairingTimestamp,
}

/// Verifies the recorded peer identity and key confirmation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifyPairingIdentity {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected pairing revision.
    pub expected_revision: PairingRevision,
    /// Verification timestamp.
    pub verified_at: PairingTimestamp,
}

/// Establishes or replaces trust for a verified peer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EstablishPairingTrust {
    /// Pairing identifier.
    pub pairing_id: PairingId,
    /// Expected pairing revision.
    pub expected_revision: PairingRevision,
    /// Explicit trust action.
    pub decision: TrustDecision,
    /// Immutable trust metadata.
    pub metadata: TrustMetadata,
    /// Trust timestamp.
    pub trusted_at: PairingTimestamp,
}

/// Revokes a trusted peer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevokeTrustedPeer {
    /// Peer device identifier.
    pub device_id: DeviceId,
    /// Expected trust-record revision.
    pub expected_revision: PairingRevision,
    /// Revocation timestamp.
    pub revoked_at: PairingTimestamp,
}

/// Result of a successful pairing mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct PairingMutation {
    session: Arc<PairingSession>,
    state_update: StateUpdate,
}

impl PairingMutation {
    /// Returns the committed pairing session.
    #[must_use]
    pub const fn session(&self) -> &Arc<PairingSession> { &self.session }
    /// Returns the Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate { &self.state_update }
}

/// Result of trust establishment.
#[derive(Clone, Debug, PartialEq)]
pub struct TrustMutation {
    session: Arc<PairingSession>,
    trusted_peer: Arc<TrustedPeer>,
    state_update: StateUpdate,
}

impl TrustMutation {
    /// Returns the committed pairing session.
    #[must_use]
    pub const fn session(&self) -> &Arc<PairingSession> { &self.session }
    /// Returns the committed trust record.
    #[must_use]
    pub const fn trusted_peer(&self) -> &Arc<TrustedPeer> { &self.trusted_peer }
    /// Returns the Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate { &self.state_update }
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
    /// Creates a Pairing Manager.
    #[must_use]
    pub fn new(
        state: BridgeStateStore,
        policy: PairingPolicy,
        crypto: Arc<dyn PairingCryptoProvider>,
    ) -> Self {
        Self { state, policy, crypto }
    }

    /// Returns the immutable policy.
    #[must_use]
    pub const fn policy(&self) -> &PairingPolicy { &self.policy }

    /// Creates an idle pairing session through Bridge State.
    pub fn create_session(&self, command: CreatePairingSession) -> PairingResult<PairingMutation> {
        let pairing_id = command.pairing_id.clone();
        let state_update = self.state.update_with(|draft| {
            if draft.pairing_sessions().contains_key(&pairing_id) {
                return Err(PairingError::DuplicatePairing { pairing_id: pairing_id.clone() });
            }
            if let Some(session_id) = &command.session_id
                && !draft.sessions().contains_key(session_id)
            {
                return Err(PairingError::MissingSession { session_id: session_id.clone() });
            }
            draft.pairing_sessions_mut().insert(PairingSession::new(
                command.pairing_id.clone(),
                command.bridge_identity.clone(),
                command.session_id.clone(),
                command.created_at,
            ))?;
            Ok(())
        })?;
        mutation_from_update(&pairing_id, state_update)
    }

    /// Creates a challenge and transitions `Idle -> ChallengeCreated`.
    pub fn create_challenge(&self, command: CreatePairingChallenge) -> PairingResult<PairingMutation> {
        self.crypto.validate_x25519_public_key(command.challenge.bridge_ephemeral_key())?;
        let pairing_id = command.pairing_id.clone();
        let challenge_id = command.challenge.id().clone();
        let state_update = self.state.update_with(|draft| {
            for session in draft.pairing_sessions().values() {
                if session.challenge().is_some_and(|challenge| challenge.id() == &challenge_id) {
                    return Err(PairingError::DuplicateChallenge { challenge_id: challenge_id.clone() });
                }
            }
            let current = required_pairing(draft, &pairing_id)?;
            validate_revision_and_timestamp(
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
        mutation_from_update(&pairing_id, state_update)
    }

    /// Applies a direct lifecycle transition without adding response or trust data.
    pub fn transition(&self, command: TransitionPairing) -> PairingResult<PairingMutation> {
        let pairing_id = command.pairing_id.clone();
        let state_update = self.state.update_with(|draft| {
            let current = required_pairing(draft, &pairing_id)?;
            validate_revision_and_timestamp(current.as_ref(), command.expected_revision, command.timestamp)?;
            validate_transition(current.as_ref(), command.state)?;
            if matches!(command.state, PairingState::Expired) {
                let challenge = current.challenge().ok_or_else(|| PairingError::state_invariant("expired transition requires a challenge"))?;
                if !challenge.is_expired(command.timestamp) {
                    return Err(PairingError::ChallengeExpired {
                        pairing_id: pairing_id.clone(),
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
        mutation_from_update(&pairing_id, state_update)
    }

    /// Records a response and consumes the challenge exactly once.
    pub fn receive_response(&self, command: ReceivePairingResponse) -> PairingResult<PairingMutation> {
        let pairing_id = command.pairing_id.clone();
        let state_update = self.state.update_with(|draft| {
            let current = required_pairing(draft, &pairing_id)?;
            validate_revision_and_timestamp(current.as_ref(), command.expected_revision, command.received_at)?;
            validate_transition(current.as_ref(), PairingState::ResponseReceived)?;
            let challenge = current.challenge().ok_or_else(|| PairingError::state_invariant("response requires a challenge"))?;
            if current.challenge_consumed() {
                return Err(PairingError::ReplayDetected {
                    pairing_id: pairing_id.clone(),
                    challenge_id: challenge.id().clone(),
                });
            }
            if challenge.id() != command.response.challenge_id() {
                return Err(PairingError::ChallengeMismatch {
                    pairing_id: pairing_id.clone(),
                    expected: challenge.id().clone(),
                    actual: command.response.challenge_id().clone(),
                });
            }
            if challenge.is_expired(command.received_at) {
                return Err(PairingError::ChallengeExpired {
                    pairing_id: pairing_id.clone(),
                    challenge_id: challenge.id().clone(),
                });
            }
            validate_protocol_and_capabilities(&self.policy, command.response.request())?;
            if let Some(session_id) = current.session_id()
                && !draft.sessions().contains_key(session_id)
            {
                return Err(PairingError::MissingSession { session_id: session_id.clone() });
            }
            let next = current
                .next(
                    PairingState::ResponseReceived,
                    command.received_at,
                    None,
                    Some(command.response.request().clone()),
                    true,
                )
                .ok_or(PairingError::RevisionExhausted)?;
            draft.pairing_responses_mut().insert(command.response.clone())?;
            draft.pairing_sessions_mut().replace(next)?;
            Ok(())
        })?;
        mutation_from_update(&pairing_id, state_update)
    }

    /// Verifies Ed25519 identity proof and X25519/HKDF/ChaCha20-Poly1305 confirmation.
    pub fn verify_identity(&self, command: VerifyPairingIdentity) -> PairingResult<PairingMutation> {
        let snapshot = self.state.snapshot()?;
        let session = snapshot
            .pairing_sessions()
            .get_shared(&command.pairing_id)
            .ok_or_else(|| PairingError::PairingNotFound { pairing_id: command.pairing_id.clone() })?;
        validate_revision_and_timestamp(session.as_ref(), command.expected_revision, command.verified_at)?;
        validate_transition(session.as_ref(), PairingState::IdentityVerified)?;
        let challenge = session.challenge().ok_or_else(|| PairingError::state_invariant("identity verification requires a challenge"))?;
        let response = snapshot
            .pairing_responses()
            .get(session.id())
            .ok_or_else(|| PairingError::state_invariant("identity verification requires a response"))?;
        let transcript = canonical_transcript(session.as_ref(), challenge, response.request());
        self.crypto.validate_ed25519_public_key(response.request().identity_key())?;
        self.crypto.validate_x25519_public_key(response.request().ephemeral_key())?;
        self.crypto.verify_ed25519(response.request().identity_key(), &transcript, response.signature())?;
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

    /// Establishes trust and transitions `IdentityVerified -> TrustEstablished` atomically.
    pub fn establish_trust(&self, command: EstablishPairingTrust) -> PairingResult<TrustMutation> {
        let pairing_id = command.pairing_id.clone();
        let state_update = self.state.update_with(|draft| {
            let current = required_pairing(draft, &pairing_id)?;
            validate_revision_and_timestamp(current.as_ref(), command.expected_revision, command.trusted_at)?;
            validate_transition(current.as_ref(), PairingState::TrustEstablished)?;
            if matches!(command.decision, TrustDecision::Reject) {
                return Err(PairingError::TrustReplacementForbidden {
                    device_id: current.request().ok_or_else(|| PairingError::state_invariant("trust requires request"))?.device_id().clone(),
                });
            }
            let request = current.request().ok_or_else(|| PairingError::state_invariant("trust requires verified request"))?;
            if let Some(session_id) = current.session_id()
                && !draft.sessions().contains_key(session_id)
            {
                return Err(PairingError::MissingSession { session_id: session_id.clone() });
            }
            for peer in draft.trusted_peers().values() {
                if peer.device_id() != request.device_id()
                    && peer.peer_identity_key() == request.identity_key()
                    && !peer.is_revoked()
                {
                    return Err(PairingError::DuplicateIdentityKey { existing_device_id: peer.device_id().clone() });
                }
            }
            let existing = draft.trusted_peers().get_shared(request.device_id());
            let revision = match existing.as_deref() {
                None if matches!(command.decision, TrustDecision::Trust) => PairingRevision::INITIAL,
                None if matches!(command.decision, TrustDecision::Replace) => PairingRevision::INITIAL,
                None => return Err(PairingError::TrustReplacementForbidden { device_id: request.device_id().clone() }),
                Some(peer) if peer.is_revoked()
                    && (!self.policy.allow_revoked_replacement() || !matches!(command.decision, TrustDecision::Replace)) => {
                        return Err(PairingError::RevokedPeer { device_id: request.device_id().clone() });
                    }
                Some(peer) if peer.peer_identity_key() != request.identity_key()
                    && (!self.policy.allow_trust_replacement() || !matches!(command.decision, TrustDecision::Replace)) => {
                        return Err(PairingError::DuplicateDeviceIdentity { device_id: request.device_id().clone() });
                    }
                Some(peer) if !matches!(command.decision, TrustDecision::Replace) => {
                    return Err(PairingError::TrustReplacementForbidden { device_id: request.device_id().clone() });
                }
                Some(peer) => peer.revision().checked_next().ok_or(PairingError::RevisionExhausted)?,
            };
            let trusted_peer = TrustedPeer::new(
                current.bridge_identity().clone(),
                request.device_id().clone(),
                request.identity_key().clone(),
                negotiated_capabilities(&self.policy, request)?,
                request.protocol_version().clone(),
                command.trusted_at,
                command.metadata.clone(),
                revision,
            );
            draft.trusted_peers_mut().upsert(trusted_peer)?;
            let next = current
                .next(PairingState::TrustEstablished, command.trusted_at, None, None, false)
                .ok_or(PairingError::RevisionExhausted)?;
            draft.pairing_sessions_mut().replace(next)?;
            Ok(())
        })?;
        let snapshot = state_update.snapshot();
        let session = snapshot
            .pairing_sessions()
            .get_shared(&pairing_id)
            .ok_or_else(|| PairingError::state_invariant("committed pairing session missing"))?;
        let request = session.request().ok_or_else(|| PairingError::state_invariant("committed request missing"))?;
        let trusted_peer = snapshot
            .trusted_peers()
            .get_shared(request.device_id())
            .ok_or_else(|| PairingError::state_invariant("committed trusted peer missing"))?;
        Ok(TrustMutation { session, trusted_peer, state_update })
    }

    /// Revokes one trusted peer through Bridge State.
    pub fn revoke_trusted_peer(&self, command: RevokeTrustedPeer) -> PairingResult<StateUpdate> {
        self.state.update_with(|draft| {
            let current = draft
                .trusted_peers()
                .get_shared(&command.device_id)
                .ok_or_else(|| PairingError::TrustNotFound { device_id: command.device_id.clone() })?;
            if current.revision() != command.expected_revision {
                return Err(PairingError::StaleRevision {
                    pairing_id: PairingId::new(format!("trust:{}", command.device_id))?,
                    expected: command.expected_revision,
                    actual: current.revision(),
                });
            }
            if current.is_revoked() {
                return Err(PairingError::RevokedPeer { device_id: command.device_id.clone() });
            }
            if command.revoked_at < current.last_verified_at() {
                return Err(PairingError::TimestampRegression {
                    pairing_id: PairingId::new(format!("trust:{}", command.device_id))?,
                    previous: current.last_verified_at(),
                    requested: command.revoked_at,
                });
            }
            let next = current.revoked(command.revoked_at).ok_or(PairingError::RevisionExhausted)?;
            draft.trusted_peers_mut().replace(next)?;
            Ok(())
        })
    }

    /// Looks up a pairing session.
    pub fn lookup_session(&self, pairing_id: &PairingId) -> PairingResult<Option<Arc<PairingSession>>> {
        Ok(self.state.snapshot()?.pairing_sessions().get_shared(pairing_id))
    }

    /// Lists pairing sessions in deterministic identifier order.
    pub fn list_sessions(&self) -> PairingResult<Vec<Arc<PairingSession>>> {
        Ok(self
            .state
            .snapshot()?
            .pairing_sessions()
            .values()
            .cloned()
            .map(Arc::new)
            .collect())
    }
}

fn required_pairing(
    draft: &crate::BridgeStateDraft,
    pairing_id: &PairingId,
) -> PairingResult<Arc<PairingSession>> {
    draft
        .pairing_sessions()
        .get_shared(pairing_id)
        .ok_or_else(|| PairingError::PairingNotFound { pairing_id: pairing_id.clone() })
}

fn validate_revision_and_timestamp(
    session: &PairingSession,
    expected_revision: PairingRevision,
    timestamp: PairingTimestamp,
) -> PairingResult<()> {
    if session.revision() != expected_revision {
        return Err(PairingError::StaleRevision {
            pairing_id: session.id().clone(),
            expected: expected_revision,
            actual: session.revision(),
        });
    }
    if timestamp < session.updated_at() {
        return Err(PairingError::TimestampRegression {
            pairing_id: session.id().clone(),
            previous: session.updated_at(),
            requested: timestamp,
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

fn validate_protocol_and_capabilities(policy: &PairingPolicy, request: &PairingRequest) -> PairingResult<()> {
    let local = policy.protocol_version();
    let remote = request.protocol_version();
    if remote.major != local.major {
        return Err(PairingError::UnsupportedProtocolVersion);
    }
    if version_tuple(remote) < version_tuple(local) {
        return Err(PairingError::ProtocolDowngrade);
    }
    let _ = negotiated_capabilities(policy, request)?;
    Ok(())
}

fn negotiated_capabilities(
    policy: &PairingPolicy,
    request: &PairingRequest,
) -> PairingResult<PairingCapabilities> {
    let local = policy.capabilities().canonical();
    let remote = request.capabilities().canonical();
    let supported = local
        .supported
        .iter()
        .copied()
        .filter(|value| remote.supported.binary_search(value).is_ok())
        .collect::<Vec<_>>();
    if local.required.iter().any(|value| supported.binary_search(value).is_err())
        || remote.required.iter().any(|value| supported.binary_search(value).is_err())
    {
        return Err(PairingError::MissingRequiredCapabilities);
    }
    PairingCapabilities::new(CapabilitySet {
        supported,
        required: local
            .required
            .iter()
            .chain(remote.required.iter())
            .copied()
            .collect(),
        parameters: local.parameters.clone(),
    })
    .map_err(Into::into)
}

fn version_tuple(version: &ProtocolVersion) -> (u32, u32, u32) {
    (version.major, version.minor, version.patch)
}

fn mutation_from_update(pairing_id: &PairingId, state_update: StateUpdate) -> PairingResult<PairingMutation> {
    let session = state_update
        .snapshot()
        .pairing_sessions()
        .get_shared(pairing_id)
        .ok_or_else(|| PairingError::state_invariant("committed pairing session missing"))?;
    Ok(PairingMutation { session, state_update })
}

fn canonical_transcript(
    session: &PairingSession,
    challenge: &PairingChallenge,
    request: &PairingRequest,
) -> Vec<u8> {
    let mut output = Vec::new();
    append(&mut output, session.bridge_identity().id().as_str().as_bytes());
    append(&mut output, session.bridge_identity().identity_key().as_bytes());
    append(&mut output, request.device_id().as_str().as_bytes());
    append(&mut output, request.identity_key().as_bytes());
    append(&mut output, challenge.id().as_str().as_bytes());
    append(&mut output, challenge.nonce().as_bytes());
    append(&mut output, challenge.bridge_ephemeral_key().as_bytes());
    append(&mut output, request.ephemeral_key().as_bytes());
    append(&mut output, &request.protocol_version().major.to_be_bytes());
    append(&mut output, &request.protocol_version().minor.to_be_bytes());
    append(&mut output, &request.protocol_version().patch.to_be_bytes());
    for capability in &request.capabilities().canonical().supported {
        append(&mut output, &capability.to_be_bytes());
    }
    for capability in &request.capabilities().canonical().required {
        append(&mut output, &capability.to_be_bytes());
    }
    append(
        &mut output,
        session.session_id().map_or(&[][..], |session_id| session_id.as_str().as_bytes()),
    );
    output
}

fn append(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}
