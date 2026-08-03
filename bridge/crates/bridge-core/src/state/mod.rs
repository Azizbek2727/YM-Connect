//! Runtime-independent, deterministic Bridge state management.
//!
//! The state store is the single source of truth for Bridge lifecycle, configuration, sessions,
//! pairing sessions, trusted peers, devices, browser connectors, transport connections, and
//! capability ownership. It uses only standard-library synchronization and message passing; no
//! asynchronous runtime, platform, transport implementation, browser, or operating-system
//! integration is required.

mod error;
mod event;
mod identifier;
mod registry;
mod snapshot;
mod store;

pub use error::{StateError, StateLock, StateReceiveError};
pub use event::{BridgeStateChange, BridgeStateEvent, RegistryDelta};
pub use identifier::{
    CapabilityOwner, ConnectionId, ConnectorId, DeviceId, PairingId, SessionId,
    StateIdentifierError, StateIdentifierKind, TransportId, TrustId,
};
pub use registry::{
    CapabilityRegistration, RegistryFailure, RegistryKind, RegistryMutation, RegistryOperation,
    RegistryStateError, StateRegistry, StateRegistryValue,
};
pub use snapshot::{
    BridgeLifecycleState, BridgeStateDraft, BridgeStateSnapshot, CapabilityRegistry,
    ConnectionRegistry, ConnectorRegistry, DeviceRegistry, PairingRegistry, SessionRegistry,
    StateRevision, TrustedPeerRegistry,
};
pub use store::{
    BridgeStateStore, BridgeStateSubscription, NotificationSummary, StateUpdate, SubscriptionId,
};

pub(super) use snapshot::BridgeStateData;

#[cfg(test)]
mod tests;
