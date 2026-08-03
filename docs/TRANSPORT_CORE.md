# Bridge Transport Core

Transport Core defines the runtime-independent communication boundary used by future YM Connect
transports. It contains no concrete transport implementation.

## Ownership boundary

`BridgeStateStore` remains the single source of truth for connection lifecycle records and session
bindings. `TransportManager` owns only a cloneable Bridge State handle.

Concrete `TransportConnection` objects are I/O handles. They are never stored in Bridge State and
never mutate Bridge State directly. Runtime owners coordinate concrete connection operations with
`TransportManager` lifecycle transactions.

Transport Core owns no:

- Tokio runtime or runtime-specific synchronization;
- sockets, listeners, WebSocket frames, HTTP, HTTPS, or TLS;
- Native Messaging packets or browser integration;
- Android transport implementation;
- discovery, pairing, reconnection, compression, or cryptography;
- Protocol Buffer encoding or decoding.

## Identifiers

`TransportId` identifies one concrete transport implementation, such as a future secure WebSocket,
Native Messaging, in-memory, or Android persistent transport.

`ConnectionId` identifies one connection lifecycle record. Both identifiers are validated strong
types and participate in deterministic ordering.

## Endpoint abstraction

`TransportEndpoint` is an immutable transport-independent descriptor containing:

- `TransportId`;
- validated opaque `TransportEndpointAddress`;
- `TransportEndpointRole` (`Dialer`, `Listener`, or `Peer`).

Only the matching concrete `TransportFactory` interprets the opaque address. Transport Core does
not parse URLs, native-host names, Android identifiers, socket addresses, or platform handles.

## I/O interfaces

`TransportConnection` is an object-safe, `Send + Sync` contract for:

- identity and endpoint inspection;
- capability inspection;
- best-effort statistics snapshots;
- sending opaque envelopes;
- receiving opaque envelopes;
- requesting graceful closure.

`TransportFactory` creates concrete connection handles for endpoint descriptors. Factories perform
I/O construction only. They do not own sessions or state records.

Both interfaces return `TransportFuture`, which is based exclusively on `std::future::Future`.
Concrete implementations may use Tokio or another executor internally without exposing it through
the core API.

Concrete implementations report create, send, receive, and close failures through
`TransportError::operation_failed`. The error carries `TransportId`, optional `ConnectionId`, typed
`TransportOperation`, stable implementation-defined code, and diagnostic message without exposing
a runtime-specific error type through the public contract.

## Message abstraction

`TransportMessageEnvelope` contains:

- an optional `SessionId` association;
- an immutable opaque byte payload.

The payload has no transport-level encoding semantics. Protocol serialization, Protocol Buffer
encoding, WebSocket framing, Native Messaging packetization, compression, and encryption remain
outside Transport Core.

## Capabilities

`TransportCapabilities` is an immutable deterministic feature declaration. Supported feature flags
include:

- reliable delivery;
- ordered delivery;
- bidirectional delivery;
- persistent connectivity;
- multiplexing;
- local-only operation;
- secure-channel ownership.

A transport may also declare a non-zero maximum envelope size. Capability declarations describe
transport behavior; they do not duplicate negotiated protocol `CapabilitySet` values.

## Connection state record

`TransportConnectionSnapshot` is immutable and contains:

- `ConnectionId`;
- `TransportId`;
- endpoint descriptor;
- transport capabilities;
- `TransportState`;
- optional bound `SessionId`;
- monotonic `TransportRevision`;
- creation timestamp;
- last lifecycle or binding timestamp.

Bridge State stores records in a deterministic `ConnectionRegistry` backed by the existing generic
copy-on-write state registry.

## Finite state machine

Allowed direct transitions are:

```text
Created       -> Connecting
Created       -> Closing
Connecting    -> Connected
Connecting    -> Closing
Connected     -> Authenticated
Connected     -> Closing
Authenticated -> Closing
Closing       -> Closed
```

`Closed` is terminal. Repeating a state or requesting an edge not listed above returns a structured
`TransportError` without committing state or publishing an event.

`CloseTransportConnection` advances a live connection to `Closing`. Calling it again with the
current revision advances `Closing` to `Closed`.

## Session binding

Transport Core never creates, updates, closes, or expires sessions.

`BindTransportSession` requires:

- an existing connection;
- an authenticated connection state;
- the expected current `TransportRevision`;
- a monotonic operation timestamp;
- an existing `SessionId` in Bridge State;
- no existing binding on the connection.

`UnbindTransportSession` removes the association without mutating the session. Session ownership
remains entirely inside `SessionManager`.

A session may later be removed independently. The binding records the session identity used by the
connection and does not transfer lifecycle ownership to Transport Core.

## Transactions and concurrency

Creation, lifecycle transitions, binding, unbinding, and closure execute through
`BridgeStateStore::update_with`.

Validation and mutation occur while the Bridge State write transaction is active. Duplicate
connection checks, optimistic revision checks, lifecycle validation, and binding validation cannot
race with another writer.

`TransportRevision` detects stale operations on one connection. `StateRevision` identifies the
complete committed Bridge State snapshot.

Rejected operations roll back completely. They do not change connection records, increment state
revisions, or notify subscribers.

## Events

Typed `TransportEvent` values are derived from committed before-and-after connection snapshots.
They are not maintained in a second mutable event store.

Events represent:

- lifecycle changes;
- session-binding changes.

Each event contains:

- `ConnectionId`;
- `TransportId`;
- connection-local revision;
- operation timestamp;
- typed previous and current values.

Transport events are ordered deterministically by connection identifier, timestamp, revision, and
event kind. The enclosing `BridgeStateEvent` contains the previous and committed State revisions
and the complete immutable snapshot.

## Extensibility

A future secure WebSocket transport can implement `TransportFactory` and `TransportConnection`
while keeping TLS, WebSocket framing, and Tokio resources inside its adapter crate.

A future Native Messaging transport can use the same opaque envelope and lifecycle contracts while
keeping native-host packet framing and browser process integration outside the core.

A future Android persistent transport can advertise persistence and secure-channel capabilities
while keeping Android networking and process lifecycle concerns in its platform implementation.

An in-memory implementation may be added later for integration tests without changing public
Transport Core models or Bridge State event semantics.
