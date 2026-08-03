use std::sync::Arc;

use crate::{PairingEvent, PairingId, SessionStateTransition, TransportEvent};

use super::{
    BridgeLifecycleState, BridgeStateData, BridgeStateSnapshot, CapabilityOwner, ConnectionId,
    ConnectorId, DeviceId, SessionId, StateRegistry, StateRegistryValue, StateRevision,
};

/// Deterministic key-level changes for one state registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryDelta<K> {
    inserted: Arc<[K]>,
    replaced: Arc<[K]>,
    removed: Arc<[K]>,
}

impl<K> RegistryDelta<K> {
    fn new(inserted: Vec<K>, replaced: Vec<K>, removed: Vec<K>) -> Self {
        Self {
            inserted: inserted.into(),
            replaced: replaced.into(),
            removed: removed.into(),
        }
    }

    /// Returns inserted keys in deterministic order.
    #[must_use]
    pub fn inserted(&self) -> &[K] {
        &self.inserted
    }

    /// Returns replaced keys in deterministic order.
    #[must_use]
    pub fn replaced(&self) -> &[K] {
        &self.replaced
    }

    /// Returns removed keys in deterministic order.
    #[must_use]
    pub fn removed(&self) -> &[K] {
        &self.removed
    }

    /// Returns whether the delta contains no changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inserted.is_empty() && self.replaced.is_empty() && self.removed.is_empty()
    }
}

impl<K> RegistryDelta<K>
where
    K: Clone,
{
    fn between<V>(before: &StateRegistry<V>, after: &StateRegistry<V>) -> Self
    where
        V: StateRegistryValue<Key = K>,
    {
        let mut inserted = Vec::new();
        let mut replaced = Vec::new();
        let mut removed = Vec::new();

        for key in before.keys() {
            match after.get(key) {
                Some(after_value) if before.get(key) != Some(after_value) => {
                    replaced.push(key.clone());
                }
                Some(_) => {}
                None => removed.push(key.clone()),
            }
        }

        for key in after.keys() {
            if !before.contains_key(key) {
                inserted.push(key.clone());
            }
        }

        Self::new(inserted, replaced, removed)
    }
}

/// Typed state change committed in one revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BridgeStateChange {
    /// Bridge lifecycle state changed.
    Lifecycle {
        /// Previous lifecycle value.
        previous: BridgeLifecycleState,
        /// Committed lifecycle value.
        current: BridgeLifecycleState,
    },
    /// Immutable configuration snapshot changed.
    Configuration,
    /// Session lifecycle changed through the Session Manager.
    SessionLifecycle(SessionStateTransition),
    /// Transport connection lifecycle or session binding changed.
    Transport(TransportEvent),
    /// Pairing lifecycle or trust state changed.
    Pairing(PairingEvent),
    /// Session registry changed.
    Sessions(RegistryDelta<SessionId>),
    /// Device registry changed.
    Devices(RegistryDelta<DeviceId>),
    /// Connector registry changed.
    Connectors(RegistryDelta<ConnectorId>),
    /// Transport connection registry changed.
    Connections(RegistryDelta<ConnectionId>),
    /// Pairing-session registry changed.
    PairingSessions(RegistryDelta<PairingId>),
    /// Trusted-peer registry changed.
    TrustedPeers(RegistryDelta<DeviceId>),
    /// Capability registry changed.
    Capabilities(RegistryDelta<CapabilityOwner>),
}

/// Immutable event published after one successful state commit.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeStateEvent {
    previous_revision: StateRevision,
    revision: StateRevision,
    changes: Arc<[BridgeStateChange]>,
    snapshot: BridgeStateSnapshot,
}

impl BridgeStateEvent {
    pub(super) fn between(
        previous_revision: StateRevision,
        revision: StateRevision,
        before: &BridgeStateData,
        after: &BridgeStateData,
        mut session_transitions: Vec<SessionStateTransition>,
        snapshot: BridgeStateSnapshot,
    ) -> Self {
        let mut changes = Vec::new();

        if before.lifecycle != after.lifecycle {
            changes.push(BridgeStateChange::Lifecycle {
                previous: before.lifecycle,
                current: after.lifecycle,
            });
        }
        if before.configuration != after.configuration {
            changes.push(BridgeStateChange::Configuration);
        }

        session_transitions.sort_by(|left, right| {
            left.session_id()
                .cmp(right.session_id())
                .then_with(|| left.timestamp().cmp(&right.timestamp()))
                .then_with(|| left.session_revision().cmp(&right.session_revision()))
                .then_with(|| left.previous().cmp(&right.previous()))
                .then_with(|| left.current().cmp(&right.current()))
        });
        changes.extend(
            session_transitions
                .into_iter()
                .map(BridgeStateChange::SessionLifecycle),
        );

        changes.extend(
            transport_events_between(before, after)
                .into_iter()
                .map(BridgeStateChange::Transport),
        );

        changes.extend(
            pairing_events_between(before, after)
                .into_iter()
                .map(BridgeStateChange::Pairing),
        );

        let sessions = RegistryDelta::between(&before.sessions, &after.sessions);
        if !sessions.is_empty() {
            changes.push(BridgeStateChange::Sessions(sessions));
        }

        let devices = RegistryDelta::between(&before.devices, &after.devices);
        if !devices.is_empty() {
            changes.push(BridgeStateChange::Devices(devices));
        }

        let connectors = RegistryDelta::between(&before.connectors, &after.connectors);
        if !connectors.is_empty() {
            changes.push(BridgeStateChange::Connectors(connectors));
        }

        let connections = RegistryDelta::between(&before.connections, &after.connections);
        if !connections.is_empty() {
            changes.push(BridgeStateChange::Connections(connections));
        }

        let pairing_sessions =
            RegistryDelta::between(&before.pairing_sessions, &after.pairing_sessions);
        if !pairing_sessions.is_empty() {
            changes.push(BridgeStateChange::PairingSessions(pairing_sessions));
        }

        let trusted_peers = RegistryDelta::between(&before.trusted_peers, &after.trusted_peers);
        if !trusted_peers.is_empty() {
            changes.push(BridgeStateChange::TrustedPeers(trusted_peers));
        }

        let capabilities = RegistryDelta::between(&before.capabilities, &after.capabilities);
        if !capabilities.is_empty() {
            changes.push(BridgeStateChange::Capabilities(capabilities));
        }

        Self {
            previous_revision,
            revision,
            changes: changes.into(),
            snapshot,
        }
    }

    /// Returns the revision observed before the commit.
    #[must_use]
    pub const fn previous_revision(&self) -> StateRevision {
        self.previous_revision
    }

    /// Returns the committed revision.
    #[must_use]
    pub const fn revision(&self) -> StateRevision {
        self.revision
    }

    /// Returns changes in deterministic subsystem order.
    #[must_use]
    pub fn changes(&self) -> &[BridgeStateChange] {
        &self.changes
    }

    /// Returns the immutable snapshot produced by the commit.
    #[must_use]
    pub const fn snapshot(&self) -> &BridgeStateSnapshot {
        &self.snapshot
    }
}

fn transport_events_between(
    before: &BridgeStateData,
    after: &BridgeStateData,
) -> Vec<TransportEvent> {
    let mut events = Vec::new();

    for (connection_id, connection) in after.connections.iter() {
        match before.connections.get(connection_id) {
            None => events.push(TransportEvent::lifecycle(
                connection_id.clone(),
                connection.transport_id().clone(),
                None,
                Some(connection.state()),
                connection.revision(),
                connection.updated_at(),
            )),
            Some(previous) => {
                if previous.state() != connection.state() {
                    events.push(TransportEvent::lifecycle(
                        connection_id.clone(),
                        connection.transport_id().clone(),
                        Some(previous.state()),
                        Some(connection.state()),
                        connection.revision(),
                        connection.updated_at(),
                    ));
                }
                if previous.session_id() != connection.session_id() {
                    events.push(TransportEvent::session_binding(
                        connection_id.clone(),
                        connection.transport_id().clone(),
                        previous.session_id().cloned(),
                        connection.session_id().cloned(),
                        connection.revision(),
                        connection.updated_at(),
                    ));
                }
            }
        }
    }

    events.sort_by(|left, right| {
        left.connection_id()
            .cmp(right.connection_id())
            .then_with(|| left.timestamp().cmp(&right.timestamp()))
            .then_with(|| left.connection_revision().cmp(&right.connection_revision()))
            .then_with(|| left.sort_rank().cmp(&right.sort_rank()))
    });
    events
}

fn pairing_events_between(
    before: &BridgeStateData,
    after: &BridgeStateData,
) -> Vec<PairingEvent> {
    let mut events = Vec::new();

    for (pairing_id, session) in after.pairing_sessions.iter() {
        match before.pairing_sessions.get(pairing_id) {
            None => events.push(PairingEvent::lifecycle(
                pairing_id.clone(),
                None,
                Some(session.state()),
                session.revision(),
                session.updated_at(),
            )),
            Some(previous) if previous.state() != session.state() => {
                events.push(PairingEvent::lifecycle(
                    pairing_id.clone(),
                    Some(previous.state()),
                    Some(session.state()),
                    session.revision(),
                    session.updated_at(),
                ));
            }
            Some(_) => {}
        }
    }

    for (device_id, peer) in after.trusted_peers.iter() {
        match before.trusted_peers.get(device_id) {
            None => events.push(PairingEvent::trust(
                device_id.clone(),
                None,
                Some(peer.is_revoked()),
                peer.revision(),
                peer.last_verified_at(),
            )),
            Some(previous) if previous != peer => events.push(PairingEvent::trust(
                device_id.clone(),
                Some(previous.is_revoked()),
                Some(peer.is_revoked()),
                peer.revision(),
                peer.last_verified_at(),
            )),
            Some(_) => {}
        }
    }

    events.sort_by(|left, right| {
        left.sort_rank()
            .cmp(&right.sort_rank())
            .then_with(|| {
                left.pairing_id()
                    .map(PairingId::as_str)
                    .cmp(&right.pairing_id().map(PairingId::as_str))
            })
            .then_with(|| {
                left.device_id()
                    .map(DeviceId::as_str)
                    .cmp(&right.device_id().map(DeviceId::as_str))
            })
            .then_with(|| left.timestamp().cmp(&right.timestamp()))
            .then_with(|| left.revision().cmp(&right.revision()))
    });
    events
}
