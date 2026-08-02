use std::sync::Arc;

use super::{
    BridgeLifecycleState, BridgeStateData, BridgeStateSnapshot, CapabilityOwner, ConnectorId,
    DeviceId, SessionId, StateRegistry, StateRegistryValue, StateRevision,
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
    /// Session registry changed.
    Sessions(RegistryDelta<SessionId>),
    /// Device registry changed.
    Devices(RegistryDelta<DeviceId>),
    /// Connector registry changed.
    Connectors(RegistryDelta<ConnectorId>),
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
