use std::{fmt, future::Future, pin::Pin, sync::Arc};

use crate::{
    ConnectionId, TransportCapabilities, TransportEndpoint, TransportId, TransportMessageEnvelope,
    TransportResult, TransportStatistics,
};

/// Runtime-independent boxed future returned by transport interfaces.
pub type TransportFuture<'a, T> =
    Pin<Box<dyn Future<Output = TransportResult<T>> + Send + 'a>>;

/// Runtime-independent connection contract implemented by concrete transports.
///
/// Implementations may use any asynchronous runtime internally. The public contract depends only
/// on [`std::future::Future`] and does not expose WebSocket frames, Native Messaging packets,
/// Protocol Buffer encoding, compression, or cryptographic primitives.
pub trait TransportConnection: fmt::Debug + Send + Sync + 'static {
    /// Returns the stable connection identifier.
    fn connection_id(&self) -> &ConnectionId;

    /// Returns the concrete transport implementation identifier.
    fn transport_id(&self) -> &TransportId;

    /// Returns the immutable endpoint descriptor.
    fn endpoint(&self) -> &TransportEndpoint;

    /// Returns the concrete connection capabilities.
    fn capabilities(&self) -> &TransportCapabilities;

    /// Returns a best-effort immutable statistics snapshot.
    fn statistics(&self) -> TransportStatistics;

    /// Sends one opaque transport envelope.
    fn send<'a>(&'a self, envelope: TransportMessageEnvelope) -> TransportFuture<'a, ()>;

    /// Receives one opaque transport envelope.
    fn receive<'a>(&'a self) -> TransportFuture<'a, TransportMessageEnvelope>;

    /// Requests graceful closure of the concrete connection.
    fn close<'a>(&'a self) -> TransportFuture<'a, ()>;
}

/// Runtime-independent factory contract implemented by each concrete transport.
///
/// The factory creates I/O handles only. It does not own Bridge sessions or mutate Bridge State.
/// Callers coordinate lifecycle records through [`crate::TransportManager`].
pub trait TransportFactory: fmt::Debug + Send + Sync + 'static {
    /// Returns the concrete transport implementation identifier.
    fn transport_id(&self) -> &TransportId;

    /// Returns capabilities shared by connections created by this factory.
    fn capabilities(&self) -> &TransportCapabilities;

    /// Creates one concrete connection for an endpoint descriptor.
    fn create<'a>(
        &'a self,
        connection_id: ConnectionId,
        endpoint: TransportEndpoint,
    ) -> TransportFuture<'a, Arc<dyn TransportConnection>>;
}
