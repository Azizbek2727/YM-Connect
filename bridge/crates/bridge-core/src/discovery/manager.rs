use std::sync::Arc;

use ym_connect_protocol::v1::ProtocolVersion;

use crate::{
    BridgeStateDraft, BridgeStateStore, DiscoveredPeer, DiscoveryAdvertisement, DiscoveryError,
    DiscoveryEvent, DiscoveryFilter, DiscoveryPeerKey, DiscoveryPolicy, DiscoveryResult,
    DiscoveryRevision, DiscoverySnapshot, DiscoverySource, DiscoveryState, DiscoveryTimestamp,
    StateError, StateUpdate,
};

use super::version_key;

/// Command that creates or refreshes one provider-specific discovered-peer record.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiveDiscoveryAdvertisement {
    source: DiscoverySource,
    advertisement: DiscoveryAdvertisement,
    observed_at: DiscoveryTimestamp,
    expected_revision: Option<DiscoveryRevision>,
}

impl ReceiveDiscoveryAdvertisement {
    /// Creates an advertisement-observation command.
    #[must_use]
    pub const fn new(
        source: DiscoverySource,
        advertisement: DiscoveryAdvertisement,
        observed_at: DiscoveryTimestamp,
        expected_revision: Option<DiscoveryRevision>,
    ) -> Self {
        Self {
            source,
            advertisement,
            observed_at,
            expected_revision,
        }
    }
}

/// Command that records successful provider-specific authenticity validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidateDiscoveredPeer {
    peer_key: DiscoveryPeerKey,
    expected_revision: DiscoveryRevision,
    validated_at: DiscoveryTimestamp,
}

impl ValidateDiscoveredPeer {
    /// Creates an advertisement-validation command.
    #[must_use]
    pub const fn new(
        peer_key: DiscoveryPeerKey,
        expected_revision: DiscoveryRevision,
        validated_at: DiscoveryTimestamp,
    ) -> Self {
        Self {
            peer_key,
            expected_revision,
            validated_at,
        }
    }
}

/// Command that performs one explicitly permitted discovered-peer lifecycle transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionDiscoveredPeer {
    peer_key: DiscoveryPeerKey,
    expected_revision: DiscoveryRevision,
    requested_state: DiscoveryState,
    timestamp: DiscoveryTimestamp,
}

impl TransitionDiscoveredPeer {
    /// Creates a lifecycle-transition command.
    #[must_use]
    pub const fn new(
        peer_key: DiscoveryPeerKey,
        expected_revision: DiscoveryRevision,
        requested_state: DiscoveryState,
        timestamp: DiscoveryTimestamp,
    ) -> Self {
        Self {
            peer_key,
            expected_revision,
            requested_state,
            timestamp,
        }
    }
}

/// Command that expires every due nonterminal advertisement in one transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpireDiscoveredPeers {
    observed_at: DiscoveryTimestamp,
}

impl ExpireDiscoveredPeers {
    /// Creates an expiration-sweep command.
    #[must_use]
    pub const fn new(observed_at: DiscoveryTimestamp) -> Self {
        Self { observed_at }
    }
}

/// Command that removes one discovered-peer record from Bridge State.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoveDiscoveredPeer {
    peer_key: DiscoveryPeerKey,
    expected_revision: DiscoveryRevision,
    removed_at: DiscoveryTimestamp,
}

impl RemoveDiscoveredPeer {
    /// Creates a registry-removal command.
    #[must_use]
    pub const fn new(
        peer_key: DiscoveryPeerKey,
        expected_revision: DiscoveryRevision,
        removed_at: DiscoveryTimestamp,
    ) -> Self {
        Self {
            peer_key,
            expected_revision,
            removed_at,
        }
    }
}

/// Result of one successful discovered-peer mutation.
#[derive(Clone, Debug, PartialEq)]
pub struct DiscoveryMutation {
    peer: Option<Arc<DiscoveredPeer>>,
    state_update: StateUpdate,
}

impl DiscoveryMutation {
    fn new(peer: Option<Arc<DiscoveredPeer>>, state_update: StateUpdate) -> Self {
        Self { peer, state_update }
    }

    /// Returns the resulting immutable peer, or `None` after registry removal.
    #[must_use]
    pub fn peer(&self) -> Option<&DiscoveredPeer> {
        self.peer.as_deref()
    }

    /// Returns the committed Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate {
        &self.state_update
    }
}

/// Result of one deterministic expiration sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct ExpiredDiscoveries {
    peer_keys: Arc<[DiscoveryPeerKey]>,
    state_update: StateUpdate,
}

impl ExpiredDiscoveries {
    fn new(peer_keys: Vec<DiscoveryPeerKey>, state_update: StateUpdate) -> Self {
        Self {
            peer_keys: peer_keys.into(),
            state_update,
        }
    }

    /// Returns expired peer keys in deterministic registry order.
    #[must_use]
    pub fn peer_keys(&self) -> &[DiscoveryPeerKey] {
        &self.peer_keys
    }

    /// Returns the committed Bridge State update.
    #[must_use]
    pub const fn state_update(&self) -> &StateUpdate {
        &self.state_update
    }
}

/// Runtime-independent discovery orchestrator backed exclusively by Bridge State transactions.
#[derive(Clone, Debug)]
pub struct DiscoveryManager {
    state: BridgeStateStore,
    policy: Arc<DiscoveryPolicy>,
}

impl DiscoveryManager {
    /// Creates a Discovery Manager over an existing Bridge State store and immutable policy.
    #[must_use]
    pub fn new(state: BridgeStateStore, policy: DiscoveryPolicy) -> Self {
        Self {
            state,
            policy: Arc::new(policy),
        }
    }

    /// Returns a cloneable handle to the authoritative Bridge State store.
    #[must_use]
    pub fn state_store(&self) -> BridgeStateStore {
        self.state.clone()
    }

    /// Returns the immutable discovery policy.
    #[must_use]
    pub fn policy(&self) -> &DiscoveryPolicy {
        self.policy.as_ref()
    }

    /// Creates or refreshes one provider-specific peer observation.
    ///
    /// Provider-specific authenticity validation remains outside Discovery Core. A runtime owner
    /// invokes [`crate::DiscoveryProvider::validate_advertisement`] before calling
    /// [`Self::validate_peer`].
    ///
    /// # Errors
    ///
    /// Returns a structured policy, freshness, revision, lifecycle, or Bridge State error.
    pub fn receive_advertisement(
        &self,
        command: ReceiveDiscoveryAdvertisement,
    ) -> DiscoveryResult<DiscoveryMutation> {
        let peer_key = DiscoveryPeerKey::new(
            command.advertisement.bridge_id().clone(),
            command.source.clone(),
            command.advertisement.transport_id().clone(),
        );
        let protocol_version = validate_advertisement_with_policy(
            self.policy.as_ref(),
            &peer_key,
            &command.advertisement,
            command.observed_at,
        )?;
        let result_key = peer_key.clone();
        let update = self.state.update_with::<DiscoveryError>(move |draft| {
            receive_in_draft(draft, peer_key, command, protocol_version)
        })?;

        mutation_from_update(&result_key, update, true)
    }

    /// Records successful provider-specific authenticity validation.
    ///
    /// # Errors
    ///
    /// Returns a structured lookup, policy, expiration, revision, lifecycle, timestamp, or state
    /// error.
    pub fn validate_peer(
        &self,
        command: ValidateDiscoveredPeer,
    ) -> DiscoveryResult<DiscoveryMutation> {
        let result_key = command.peer_key.clone();
        let policy = Arc::clone(&self.policy);
        let update = self.state.update_with::<DiscoveryError>(move |draft| {
            let current = current_peer(draft, &command.peer_key)?;
            validate_active_current(&current, command.expected_revision, command.validated_at)?;
            validate_transition(&current, DiscoveryState::Validated)?;
            let selected = validate_advertisement_with_policy(
                policy.as_ref(),
                current.key(),
                current.current_advertisement(),
                command.validated_at,
            )?;
            if version_key(&selected) != version_key(current.protocol_version()) {
                return Err(DiscoveryError::state_invariant(format!(
                    "selected protocol version changed for {} under immutable policy",
                    current.key()
                )));
            }
            transition_current(
                draft,
                current.as_ref(),
                DiscoveryState::Validated,
                command.validated_at,
            )
        })?;

        mutation_from_update(&result_key, update, true)
    }

    /// Performs one lifecycle transition that does not bypass advertisement receipt, provider
    /// validation, or registry removal.
    ///
    /// # Errors
    ///
    /// Returns a structured lookup, revision, expiration, lifecycle, timestamp, or state error.
    pub fn transition_peer(
        &self,
        command: TransitionDiscoveredPeer,
    ) -> DiscoveryResult<DiscoveryMutation> {
        let result_key = command.peer_key.clone();
        let update = self.state.update_with::<DiscoveryError>(move |draft| {
            let current = current_peer(draft, &command.peer_key)?;
            validate_active_current(&current, command.expected_revision, command.timestamp)?;
            if matches!(
                command.requested_state,
                DiscoveryState::AdvertisementReceived
                    | DiscoveryState::Validated
                    | DiscoveryState::Removed
            ) {
                return Err(DiscoveryError::InvalidTransition {
                    peer_key: current.key().clone(),
                    previous: current.state(),
                    requested: command.requested_state,
                });
            }
            validate_transition(&current, command.requested_state)?;
            validate_expiration_transition(&current, command.requested_state, command.timestamp)?;
            transition_current(
                draft,
                current.as_ref(),
                command.requested_state,
                command.timestamp,
            )
        })?;

        mutation_from_update(&result_key, update, true)
    }

    /// Expires all due nonterminal advertisements in deterministic registry order.
    ///
    /// # Errors
    ///
    /// Returns a structured timestamp, revision, registry, or Bridge State error. Any failure rolls
    /// back the entire sweep.
    pub fn expire_peers(
        &self,
        command: ExpireDiscoveredPeers,
    ) -> DiscoveryResult<ExpiredDiscoveries> {
        let mut expired = Vec::new();
        let update = self.state.update_with::<DiscoveryError>(|draft| {
            let keys = draft.discoveries().keys().cloned().collect::<Vec<_>>();
            for peer_key in keys {
                let current = current_peer(draft, &peer_key)?;
                if current.state().is_terminal()
                    || !current
                        .current_advertisement()
                        .is_expired_at(command.observed_at)
                {
                    continue;
                }
                validate_timestamp(&current, command.observed_at)?;
                transition_current(
                    draft,
                    current.as_ref(),
                    DiscoveryState::Expired,
                    command.observed_at,
                )?;
                expired.push(peer_key);
            }
            Ok(())
        })?;

        Ok(ExpiredDiscoveries::new(expired, update))
    }

    /// Removes one peer from the registry, including an already expired peer.
    ///
    /// # Errors
    ///
    /// Returns a structured lookup, revision, timestamp, exhaustion, registry, or state error.
    pub fn remove_peer(&self, command: RemoveDiscoveredPeer) -> DiscoveryResult<DiscoveryMutation> {
        let result_key = command.peer_key.clone();
        let update = self.state.update_with::<DiscoveryError>(move |draft| {
            let current = current_peer(draft, &command.peer_key)?;
            validate_revision(&current, command.expected_revision)?;
            validate_timestamp(&current, command.removed_at)?;
            let removal_revision = current.revision().checked_next().ok_or_else(|| {
                DiscoveryError::RevisionExhausted {
                    peer_key: current.key().clone(),
                }
            })?;
            let _ = draft
                .discoveries_mut()
                .remove(&command.peer_key)
                .map_err(StateError::from)?;
            draft.record_discovery_event(DiscoveryEvent::removed(
                command.peer_key,
                current.state(),
                removal_revision,
                command.removed_at,
            ));
            Ok(())
        })?;

        mutation_from_update(&result_key, update, false)
    }

    /// Looks up one immutable discovered-peer record.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn lookup_peer(
        &self,
        peer_key: &DiscoveryPeerKey,
    ) -> DiscoveryResult<Option<Arc<DiscoveredPeer>>> {
        Ok(self.state.snapshot()?.discoveries().get_shared(peer_key))
    }

    /// Lists immutable discovered-peer records in deterministic composite-key order.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn list_peers(&self) -> DiscoveryResult<Vec<Arc<DiscoveredPeer>>> {
        let snapshot = self.state.snapshot()?;
        Ok(snapshot
            .discoveries()
            .keys()
            .filter_map(|peer_key| snapshot.discoveries().get_shared(peer_key))
            .collect())
    }

    /// Returns an immutable aggregate discovery snapshot.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn snapshot(&self) -> DiscoveryResult<DiscoverySnapshot> {
        let snapshot = self.state.snapshot()?;
        let peers = snapshot
            .discoveries()
            .keys()
            .filter_map(|peer_key| snapshot.discoveries().get_shared(peer_key))
            .collect();
        Ok(DiscoverySnapshot::new(snapshot.revision(), peers))
    }

    /// Lists peers matching typed criteria in deterministic registry order.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn filter_peers(
        &self,
        filter: &DiscoveryFilter,
    ) -> DiscoveryResult<Vec<Arc<DiscoveredPeer>>> {
        Ok(self.snapshot()?.filtered(filter))
    }

    /// Lists peers matching typed criteria and one caller-supplied predicate in deterministic
    /// order.
    ///
    /// The caller is responsible for keeping the predicate deterministic and side-effect free.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn filter_peers_with(
        &self,
        filter: &DiscoveryFilter,
        predicate: impl Fn(&DiscoveredPeer) -> bool,
    ) -> DiscoveryResult<Vec<Arc<DiscoveredPeer>>> {
        Ok(self.snapshot()?.filtered_with(filter, predicate))
    }

    /// Returns whether a provider-specific peer key is registered.
    ///
    /// # Errors
    ///
    /// Returns a Bridge State synchronization error.
    pub fn peer_exists(&self, peer_key: &DiscoveryPeerKey) -> DiscoveryResult<bool> {
        Ok(self.state.snapshot()?.discoveries().contains_key(peer_key))
    }
}

fn receive_in_draft(
    draft: &mut BridgeStateDraft,
    peer_key: DiscoveryPeerKey,
    command: ReceiveDiscoveryAdvertisement,
    protocol_version: ProtocolVersion,
) -> DiscoveryResult<()> {
    let current = draft.discoveries().get_shared(&peer_key);
    match current {
        None => {
            if let Some(revision) = command.expected_revision {
                return Err(DiscoveryError::UnexpectedRevision { peer_key, revision });
            }
            let peer = DiscoveredPeer::new(
                peer_key.clone(),
                command.advertisement,
                command.observed_at,
                protocol_version,
            );
            let revision = peer.revision();
            let _ = draft
                .discoveries_mut()
                .insert(peer)
                .map_err(StateError::from)?;
            draft.record_discovery_event(DiscoveryEvent::advertisement(
                peer_key,
                None,
                true,
                revision,
                command.observed_at,
            ));
            Ok(())
        }
        Some(current) => {
            let expected_revision =
                command
                    .expected_revision
                    .ok_or_else(|| DiscoveryError::RevisionRequired {
                        peer_key: peer_key.clone(),
                    })?;
            validate_active_current(&current, expected_revision, command.observed_at)?;
            validate_advertisement_progression(&current, &command.advertisement)?;
            let advertisement_changed = current.current_advertisement() != &command.advertisement;
            let previous = current.state();
            let next = current
                .refreshed(command.advertisement, command.observed_at, protocol_version)
                .ok_or_else(|| DiscoveryError::RevisionExhausted {
                    peer_key: peer_key.clone(),
                })?;
            let revision = next.revision();
            let _ = draft
                .discoveries_mut()
                .replace(next)
                .map_err(StateError::from)?;
            draft.record_discovery_event(DiscoveryEvent::advertisement(
                peer_key,
                Some(previous),
                advertisement_changed,
                revision,
                command.observed_at,
            ));
            Ok(())
        }
    }
}

fn current_peer(
    draft: &BridgeStateDraft,
    peer_key: &DiscoveryPeerKey,
) -> DiscoveryResult<Arc<DiscoveredPeer>> {
    draft
        .discoveries()
        .get_shared(peer_key)
        .ok_or_else(|| DiscoveryError::PeerNotFound {
            peer_key: peer_key.clone(),
        })
}

fn validate_active_current(
    peer: &DiscoveredPeer,
    expected_revision: DiscoveryRevision,
    timestamp: DiscoveryTimestamp,
) -> DiscoveryResult<()> {
    validate_revision(peer, expected_revision)?;
    if peer.state().is_terminal() {
        return Err(DiscoveryError::TerminalPeer {
            peer_key: peer.key().clone(),
            state: peer.state(),
        });
    }
    validate_timestamp(peer, timestamp)
}

fn validate_revision(
    peer: &DiscoveredPeer,
    expected_revision: DiscoveryRevision,
) -> DiscoveryResult<()> {
    if peer.revision() != expected_revision {
        return Err(DiscoveryError::StaleRevision {
            peer_key: peer.key().clone(),
            expected: expected_revision,
            actual: peer.revision(),
        });
    }
    Ok(())
}

fn validate_timestamp(peer: &DiscoveredPeer, timestamp: DiscoveryTimestamp) -> DiscoveryResult<()> {
    if timestamp < peer.last_observed_at() {
        return Err(DiscoveryError::TimestampRegression {
            peer_key: peer.key().clone(),
            previous: peer.last_observed_at(),
            requested: timestamp,
        });
    }
    Ok(())
}

fn validate_transition(peer: &DiscoveredPeer, requested: DiscoveryState) -> DiscoveryResult<()> {
    if !peer.state().can_transition_to(requested) {
        return Err(DiscoveryError::InvalidTransition {
            peer_key: peer.key().clone(),
            previous: peer.state(),
            requested,
        });
    }
    Ok(())
}

fn validate_expiration_transition(
    peer: &DiscoveredPeer,
    requested: DiscoveryState,
    timestamp: DiscoveryTimestamp,
) -> DiscoveryResult<()> {
    let advertisement = peer.current_advertisement();
    if requested == DiscoveryState::Expired {
        if !advertisement.is_expired_at(timestamp) {
            return Err(DiscoveryError::ExpirationNotReached {
                peer_key: peer.key().clone(),
                expires_at: advertisement.expires_at(),
                requested_at: timestamp,
            });
        }
    } else if advertisement.is_expired_at(timestamp) {
        return Err(DiscoveryError::AdvertisementExpired {
            peer_key: peer.key().clone(),
            expires_at: advertisement.expires_at(),
            observed_at: timestamp,
        });
    }
    Ok(())
}

fn transition_current(
    draft: &mut BridgeStateDraft,
    current: &DiscoveredPeer,
    requested: DiscoveryState,
    timestamp: DiscoveryTimestamp,
) -> DiscoveryResult<()> {
    let next = current.transitioned(requested, timestamp).ok_or_else(|| {
        DiscoveryError::RevisionExhausted {
            peer_key: current.key().clone(),
        }
    })?;
    let revision = next.revision();
    let previous = current.state();
    let peer_key = current.key().clone();
    let _ = draft
        .discoveries_mut()
        .replace(next)
        .map_err(StateError::from)?;
    draft.record_discovery_event(DiscoveryEvent::lifecycle(
        peer_key, previous, requested, revision, timestamp,
    ));
    Ok(())
}

fn mutation_from_update(
    peer_key: &DiscoveryPeerKey,
    update: StateUpdate,
    peer_expected: bool,
) -> DiscoveryResult<DiscoveryMutation> {
    let peer = update.snapshot().discoveries().get_shared(peer_key);
    if peer_expected && peer.is_none() {
        return Err(DiscoveryError::state_invariant(format!(
            "committed discovered peer {peer_key} is absent from its resulting snapshot"
        )));
    }
    if !peer_expected && peer.is_some() {
        return Err(DiscoveryError::state_invariant(format!(
            "removed discovered peer {peer_key} remains in its resulting snapshot"
        )));
    }
    Ok(DiscoveryMutation::new(peer, update))
}

fn validate_advertisement_with_policy(
    policy: &DiscoveryPolicy,
    peer_key: &DiscoveryPeerKey,
    advertisement: &DiscoveryAdvertisement,
    observed_at: DiscoveryTimestamp,
) -> DiscoveryResult<ProtocolVersion> {
    if advertisement.is_expired_at(observed_at) {
        return Err(DiscoveryError::AdvertisementExpired {
            peer_key: peer_key.clone(),
            expires_at: advertisement.expires_at(),
            observed_at,
        });
    }
    let maximum_discovered_at = observed_at
        .as_unix_millis()
        .saturating_add(policy.maximum_future_clock_skew_ms());
    if advertisement.discovered_at().as_unix_millis() > maximum_discovered_at {
        return Err(DiscoveryError::AdvertisementFromFuture {
            peer_key: peer_key.clone(),
            discovered_at: advertisement.discovered_at(),
            observed_at,
        });
    }
    if advertisement.lifetime_ms() > policy.maximum_advertisement_lifetime_ms() {
        return Err(DiscoveryError::AdvertisementLifetimeExceeded {
            peer_key: peer_key.clone(),
            lifetime_ms: advertisement.lifetime_ms(),
            maximum_ms: policy.maximum_advertisement_lifetime_ms(),
        });
    }
    let entries = advertisement.provider_metadata().len();
    let bytes = advertisement.metadata_size_bytes();
    if entries > policy.maximum_metadata_entries() || bytes > policy.maximum_metadata_bytes() {
        return Err(DiscoveryError::AdvertisementMetadataLimitExceeded {
            peer_key: peer_key.clone(),
            entries,
            bytes,
        });
    }
    for capability in policy.required_capabilities() {
        if !advertisement.capabilities().supports_raw(*capability) {
            return Err(DiscoveryError::MissingRequiredCapability {
                peer_key: peer_key.clone(),
                capability: *capability,
            });
        }
    }
    select_protocol_version(policy, peer_key, advertisement)
}

fn select_protocol_version(
    policy: &DiscoveryPolicy,
    peer_key: &DiscoveryPeerKey,
    advertisement: &DiscoveryAdvertisement,
) -> DiscoveryResult<ProtocolVersion> {
    let advertised = advertisement.supported_protocol_versions();
    policy
        .supported_protocol_versions()
        .iter()
        .rev()
        .find(|candidate| {
            advertised
                .binary_search_by_key(&version_key(candidate), version_key)
                .is_ok()
        })
        .cloned()
        .ok_or_else(|| DiscoveryError::NoCompatibleProtocolVersion {
            peer_key: peer_key.clone(),
        })
}

fn validate_advertisement_progression(
    current: &DiscoveredPeer,
    requested: &DiscoveryAdvertisement,
) -> DiscoveryResult<()> {
    let previous_timestamp = current.current_advertisement().discovered_at();
    let requested_timestamp = requested.discovered_at();
    if requested_timestamp < previous_timestamp {
        return Err(DiscoveryError::AdvertisementTimestampRegression {
            peer_key: current.key().clone(),
            previous: previous_timestamp,
            requested: requested_timestamp,
        });
    }
    if requested_timestamp == previous_timestamp && requested != current.current_advertisement() {
        return Err(DiscoveryError::AdvertisementConflict {
            peer_key: current.key().clone(),
            discovered_at: requested_timestamp,
        });
    }
    Ok(())
}
