# Bridge Session Manager

The Bridge Session Manager is the runtime-independent lifecycle orchestrator for Bridge sessions.
It does not own authoritative runtime state. Every create, restore, update, transition, close, and
expiration operation is committed through `BridgeStateStore`.

## Ownership boundary

`BridgeStateStore` remains the single source of truth. `SessionManager` contains only:

- a cloneable Bridge State handle;
- an immutable `SessionPolicy`.

The module contains no Tokio resources, sockets, browser connections, transport implementation,
protocol serialization, cryptography, discovery, pairing, or platform-specific integration.

## Session record

`BridgeSession` is immutable and contains:

- validated `SessionId`;
- `SessionTimestamp` creation time;
- `SessionTimestamp` last activity time;
- `SessionLifecycleState`;
- validated `DeviceId`;
- validated `ConnectorId`;
- generated protobuf `CapabilitySet`;
- generated protobuf `ProtocolVersion`;
- monotonic `SessionRevision`;
- deterministic `SessionMetadata`.

Protocol versions and capabilities are not duplicated as Bridge-specific protocol models. The
canonical generated protobuf types are stored directly.

## Finite state machine

Allowed direct transitions are:

```text
Created     -> Negotiating
Created     -> Closing
Negotiating -> Active
Negotiating -> Closing
Active      -> Suspended
Active      -> Closing
Suspended   -> Active
Suspended   -> Closing
Closing     -> Closed
```

`Closed` is terminal. Repeating the current state or requesting any edge not listed above returns a
structured `SessionManagerError` without committing Bridge State or publishing an event.

`CloseSession` advances an open session to `Closing`. Calling it again with the current revision
advances `Closing` to `Closed`.

## Transaction model

The manager uses `BridgeStateStore::update_with` for typed domain transactions. Validation and
mutation occur while the state write transaction is active, so duplicate checks, optimistic
revision checks, and lifecycle transitions cannot race with another writer.

A failed operation rolls back all draft changes. No partial metadata, timestamp, lifecycle, or
registry update is observable.

Session-local revisions and Bridge State revisions serve different purposes:

- `SessionRevision` detects stale concurrent operations on one session;
- `StateRevision` identifies the complete committed Bridge State snapshot.

A successful session mutation increments both the session-local revision and the Bridge State
revision. Creation starts at session revision zero. A semantically identical update commits
nothing and increments neither revision.

## Commands

The public command types are:

- `CreateSession`;
- `RestoreSession`;
- `UpdateSession`;
- `SuspendSession`;
- `ResumeSession`;
- `CloseSession`;
- `RemoveExpiredSessions`.

Commands carry explicit `SessionTimestamp` values. The manager owns no clock, asynchronous runtime,
or background expiration task. This makes transaction behavior deterministic and allows callers to
select the authoritative time source at the application boundary.

## Validation

Creation and restoration validate:

- duplicate session identifiers;
- missing device associations;
- missing connector associations;
- conflicting live device/connector associations when policy enforcement is enabled;
- timestamp order;
- expiration;
- protocol major version;
- unknown, unspecified, or duplicate capabilities;
- required capabilities that were not negotiated as supported.

Updates and transitions additionally validate:

- session existence;
- optimistic session revision;
- timestamp monotonicity;
- inactivity expiration;
- terminal state;
- finite-state-machine edges;
- session revision exhaustion.

## Expiration

`RemoveExpiredSessions` performs one atomic deterministic sweep. A session is expired when:

```text
observed_at - last_activity_at >= policy.inactivity_timeout
```

Expired identifiers are removed in `SessionId` order. If any session contains a future activity
timestamp relative to the supplied observation time, the complete sweep is rejected and no session
is removed.

The Session Manager does not schedule expiration. Runtime owners may call the API at their chosen
cadence without moving state or timer ownership into the manager.

## Events

Every successful lifecycle transition records a typed `SessionStateTransition` in the resulting
`BridgeStateEvent`. The transition contains:

- session identifier;
- previous lifecycle state;
- new lifecycle state;
- session-local revision;
- operation timestamp.

The enclosing `BridgeStateEvent` contains the previous and committed Bridge State revisions and the
complete immutable snapshot.

Creation and restoration use `None` as the previous lifecycle state. Expiration removal uses `None`
as the new lifecycle state. Multiple transitions in one expiration sweep are sorted by session
identifier before publication.

Rejected transitions emit no event.

## Read APIs

`LookupSession`, `ListSessions`, and `SessionExists` read immutable Bridge State snapshots.
`ListSessions` returns shared immutable records in deterministic `SessionId` order.

## Concurrency

`SessionManager`, `BridgeSession`, and operation result types are `Send + Sync`. Concurrent writers
are serialized by Bridge State. Concurrent operations using the same expected session revision
produce at most one successful mutation; later contenders receive `StaleRevision`.

Observers receive immutable events through the existing Bridge State message-passing subscriber
API and cannot mutate manager or store internals.
