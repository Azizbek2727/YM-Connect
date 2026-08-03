//! Runtime-independent transport contracts and connection lifecycle orchestration.
//!
//! Transport Core defines opaque message delivery interfaces, deterministic connection records,
//! lifecycle validation, session binding, and Bridge State integration. Concrete WebSocket,
//! Native Messaging, Android, in-memory, serialization, compression, cryptography, discovery, and
//! pairing implementations belong in later modules.

mod error;
mod interfaces;
mod manager;
mod model;

pub use error::{TransportError, TransportOperation, TransportResult};
pub use interfaces::{TransportConnection, TransportFactory, TransportFuture};
pub use manager::{
    BindTransportSession, CloseTransportConnection, CreateTransportConnection,
    TransitionTransportConnection, TransportManager, TransportMutation, UnbindTransportSession,
};
pub use model::{
    TransportCapabilities, TransportConnectionSnapshot, TransportEndpoint,
    TransportEndpointAddress, TransportEndpointRole, TransportEvent, TransportEventKind,
    TransportFeature, TransportMessageEnvelope, TransportModelError, TransportRevision,
    TransportState, TransportStatistics, TransportTimestamp,
};

pub(crate) use model::TransportConnectionParts;

#[cfg(test)]
mod tests;
