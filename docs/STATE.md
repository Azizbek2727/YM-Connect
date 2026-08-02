# Bridge State

<!-- cspell:words BTreeMap mpsc RwLock -->

## Scope

The Bridge State subsystem is owned entirely by `ym-connect-bridge-core`. It is the single
source of truth for mutable runtime state and remains independent of Tokio, operating-system
APIs, platform IPC, networking, browser integration, and transport processing.

The Bridge daemon does not own runtime state. It constructs `BridgeApplication`; the
application creates the state store from the validated configuration and exposes cloned store
handles to future core services.

## State model

Every state snapshot contains:

- the Bridge lifecycle state;
- the immutable resolved `BridgeConfig`;
- the session registry;
- the device registry;
- the browser connector registry;
- the capability ownership registry.

Session, device, connector, and capability payloads use canonical generated Protocol Buffer
models. The state subsystem does not define competing wire models and performs no Protocol
Buffer encoding or decoding.

The initial snapshot uses revision zero, lifecycle `Initializing`, the supplied configuration,
and empty registries.

## Snapshots and revisions

`BridgeStateSnapshot` is immutable and internally shared. A snapshot remains unchanged after
later store updates. Registry content is held in deterministic `BTreeMap` key order, and values
are shared through `Arc` references when snapshots are cloned.

`BridgeStateStore` owns an atomic monotonic `u64` revision counter. Each transaction that
changes state creates exactly one new revision. An idempotent transaction returns the current
snapshot without changing the revision or publishing an event.

The subsystem does not expose a serialization format. Deterministic serialization therefore
belongs to the protocol or persistence layer that consumes a snapshot. Snapshot equality,
registry iteration, and event change ordering are deterministic within the state subsystem.

## Update transactions

`BridgeStateStore::update` accepts a closure operating on a transaction-local
`BridgeStateDraft`. The closure cannot access internal synchronization primitives and the draft
is never visible to subscribers.

A transaction follows these steps:

1. Clone the current immutable state into a private draft.
2. Apply and validate all requested mutations.
3. Reject the complete transaction if the closure returns a `StateError`.
4. Compare the final draft with the current state.
5. Commit one new revision only when the state differs.
6. Build an immutable snapshot and typed event.
7. Publish the event to active subscribers in revision order.

Update panics are caught when the build uses unwinding. The current state remains unchanged and
the store remains usable. Release builds configured with panic abort retain the workspace's
process-abort policy.

## Registries

`StateRegistry<V>` provides production APIs for:

- insertion with duplicate rejection;
- explicit replacement with missing-key rejection;
- idempotent upsert;
- strict removal;
- optional removal;
- lookup by validated key;
- shared-value lookup;
- deterministic key, value, and entry iteration;
- clearing and empty-state inspection.

Canonical record identifiers are validated before mutation. Session, device, and connector
identifiers must be nonempty. Registry errors include the registry, attempted operation,
failure kind, and key or identifier error when available.

The capability registry stores `CapabilityRegistration`, which associates a runtime
`CapabilityOwner` with a canonical generated `CapabilitySet`. This association is internal
runtime ownership metadata rather than a duplicate protocol model.

## Subscribers

`BridgeStateStore::subscribe` atomically registers a subscriber and captures its initial
snapshot. Subsequent `BridgeStateEvent` values are delivered through standard-library `mpsc`
message passing.

Events include:

- the previous and committed revisions;
- typed subsystem changes;
- sorted inserted, replaced, and removed registry keys;
- the complete committed immutable snapshot.

Subscriber code never executes while the store is locked, so subscriber panics cannot corrupt
or poison the state store. Delivery to each active subscription is ordered by committed
revision. Dropped or explicitly unsubscribed receivers are removed without affecting other
subscribers. Unsubscribe removes the sender and drains already queued events before returning.

Delivery queues are lossless and unbounded. Consumers must continuously drain active
subscriptions or unsubscribe when state events are no longer needed.

## Errors

`StateError` distinguishes:

- poisoned internal synchronization;
- exhausted revision or subscription counters;
- isolated update panics;
- structured registry failures;
- caller-defined transaction rejection codes and messages.

Subscription receive operations use `StateReceiveError` to distinguish an empty queue, a timed
receive deadline, and a disconnected subscription.

## Current boundaries

This subsystem does not implement WebSocket transport, Native Messaging, BrowserConnector,
PlayerAdapter, discovery, pairing, cryptography, transport processing, Protocol Buffer message
processing, or browser integration. Future managers and adapters must interact with runtime
state exclusively through `BridgeStateStore` snapshots, transactions, and subscriptions.
