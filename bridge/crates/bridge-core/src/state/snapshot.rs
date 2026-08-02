use std::sync::Arc;

use ym_connect_protocol::v1::{BrowserDescriptor, DeviceDescriptor, SessionEstablished};

use crate::BridgeConfig;

use super::{CapabilityRegistration, StateRegistry};

/// Deterministic session registry.
pub type SessionRegistry = StateRegistry<SessionEstablished>;

/// Deterministic device registry.
pub type DeviceRegistry = StateRegistry<DeviceDescriptor>;

/// Deterministic browser connector registry.
pub type ConnectorRegistry = StateRegistry<BrowserDescriptor>;

/// Deterministic capability ownership registry.
pub type CapabilityRegistry = StateRegistry<CapabilityRegistration>;

/// Bridge process lifecycle represented in runtime state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum BridgeLifecycleState {
    /// Core state exists but the application lifecycle has not started.
    #[default]
    Initializing,
    /// The Bridge application is running.
    Running,
    /// Graceful shutdown is in progress.
    Stopping,
    /// The Bridge stopped cleanly.
    Stopped,
    /// The Bridge lifecycle terminated because of an error.
    Failed,
}

/// Monotonic revision assigned to a committed state snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct StateRevision(u64);

impl StateRevision {
    /// Initial state revision.
    pub const INITIAL: Self = Self(0);

    pub(super) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric revision.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BridgeStateData {
    pub(super) lifecycle: BridgeLifecycleState,
    pub(super) configuration: Arc<BridgeConfig>,
    pub(super) sessions: SessionRegistry,
    pub(super) devices: DeviceRegistry,
    pub(super) connectors: ConnectorRegistry,
    pub(super) capabilities: CapabilityRegistry,
}

impl BridgeStateData {
    pub(super) fn new(configuration: BridgeConfig) -> Self {
        Self {
            lifecycle: BridgeLifecycleState::Initializing,
            configuration: Arc::new(configuration),
            sessions: SessionRegistry::new(),
            devices: DeviceRegistry::new(),
            connectors: ConnectorRegistry::new(),
            capabilities: CapabilityRegistry::new(),
        }
    }
}

/// Immutable, internally shared view of all Bridge runtime state.
#[derive(Clone, Debug, PartialEq)]
pub struct BridgeStateSnapshot {
    revision: StateRevision,
    data: Arc<BridgeStateData>,
}

impl BridgeStateSnapshot {
    pub(super) fn new(revision: StateRevision, data: Arc<BridgeStateData>) -> Self {
        Self { revision, data }
    }

    /// Returns the snapshot revision.
    #[must_use]
    pub const fn revision(&self) -> StateRevision {
        self.revision
    }

    /// Returns the Bridge lifecycle state.
    #[must_use]
    pub fn lifecycle(&self) -> BridgeLifecycleState {
        self.data.lifecycle
    }

    /// Returns the immutable configuration snapshot.
    #[must_use]
    pub fn configuration(&self) -> &BridgeConfig {
        self.data.configuration.as_ref()
    }

    /// Returns the immutable session registry.
    #[must_use]
    pub fn sessions(&self) -> &SessionRegistry {
        &self.data.sessions
    }

    /// Returns the immutable device registry.
    #[must_use]
    pub fn devices(&self) -> &DeviceRegistry {
        &self.data.devices
    }

    /// Returns the immutable connector registry.
    #[must_use]
    pub fn connectors(&self) -> &ConnectorRegistry {
        &self.data.connectors
    }

    /// Returns the immutable capability registry.
    #[must_use]
    pub fn capabilities(&self) -> &CapabilityRegistry {
        &self.data.capabilities
    }

    /// Returns whether both snapshots contain identical state, ignoring revision metadata.
    #[must_use]
    pub fn same_state(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

/// Mutable transaction-local state used by [`super::BridgeStateStore::update`].
///
/// A draft is never shared with observers. The store commits it atomically only when its final
/// content differs from the current state.
#[derive(Debug)]
pub struct BridgeStateDraft {
    data: BridgeStateData,
}

impl BridgeStateDraft {
    pub(super) fn from_data(data: &BridgeStateData) -> Self {
        Self { data: data.clone() }
    }

    pub(super) fn into_data(self) -> BridgeStateData {
        self.data
    }

    /// Returns the draft lifecycle state.
    #[must_use]
    pub fn lifecycle(&self) -> BridgeLifecycleState {
        self.data.lifecycle
    }

    /// Replaces the lifecycle state and returns whether it changed.
    #[must_use]
    pub fn set_lifecycle(&mut self, lifecycle: BridgeLifecycleState) -> bool {
        if self.data.lifecycle == lifecycle {
            return false;
        }
        self.data.lifecycle = lifecycle;
        true
    }

    /// Returns the draft configuration.
    #[must_use]
    pub fn configuration(&self) -> &BridgeConfig {
        self.data.configuration.as_ref()
    }

    /// Replaces the configuration and returns whether it changed.
    #[must_use]
    pub fn set_configuration(&mut self, configuration: BridgeConfig) -> bool {
        if self.data.configuration.as_ref() == &configuration {
            return false;
        }
        self.data.configuration = Arc::new(configuration);
        true
    }

    /// Returns the mutable session registry.
    pub const fn sessions_mut(&mut self) -> &mut SessionRegistry {
        &mut self.data.sessions
    }

    /// Returns the mutable device registry.
    pub const fn devices_mut(&mut self) -> &mut DeviceRegistry {
        &mut self.data.devices
    }

    /// Returns the mutable connector registry.
    pub const fn connectors_mut(&mut self) -> &mut ConnectorRegistry {
        &mut self.data.connectors
    }

    /// Returns the mutable capability registry.
    pub const fn capabilities_mut(&mut self) -> &mut CapabilityRegistry {
        &mut self.data.capabilities
    }
}
