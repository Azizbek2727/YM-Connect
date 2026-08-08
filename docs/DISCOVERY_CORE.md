# Bridge Discovery Core

Discovery Core defines the runtime-independent discovery model used by future YM Connect discovery
providers. It contains no concrete provider, networking, browser, platform, or Android
implementation.

## Ownership boundary

`BridgeStateStore` remains the single mutable source of truth for discovered-peer lifecycle records.
`DiscoveryManager` owns only a cloneable Bridge State handle and an immutable `DiscoveryPolicy`.
It owns no provider handles, tasks, sockets, clocks, caches, or mutable registry.

Concrete `DiscoveryProvider` implementations own provider-specific I/O and resource lifetimes. They
submit immutable advertisements to runtime orchestration but never mutate Bridge State directly.
Provider-specific authenticity validation remains outside Discovery Core. After validation succeeds,
the runtime owner records that result through `DiscoveryManager::validate_peer`.

Discovery Core owns no:

- mDNS, Bonjour, DNS-SD, UDP, TCP, HTTP, WebSocket, or TLS implementation;
- Native Messaging, browser connector, platform IPC, or Android discovery implementation;
- asynchronous runtime, task scheduler, retry loop, or provider process lifecycle;
- protocol serialization, compression, encryption, key storage, or trust persistence;
- parallel event bus, mutable cache, or provider-specific registry.

## Provider contract

`DiscoveryProvider` is an object-safe, `Send + Sync` abstraction for:

- returning a stable `DiscoverySource`;
- starting provider-specific discovery with a deterministic typed filter;
- receiving immutable advertisements;
- performing provider-specific advertisement authenticity validation;
- stopping provider-specific discovery.

Provider methods return `DiscoveryFuture`, which is based only on `std::future::Future`. A concrete
provider may use Tokio or another executor internally without exposing it through the core API.
Provider failures use `DiscoveryError::provider_operation_failed` with a source identifier, typed
operation, stable provider-defined code, and diagnostic message.

## Peer identity

`DiscoveryPeerKey` is the deterministic composite key for one provider-specific observation:

- `BridgeId` identifies the advertised Bridge;
- `DiscoverySource` identifies the provider or provider instance;
- `TransportId` identifies the advertised future connection transport.

The composite key permits the same Bridge to be observed through multiple providers or transports
without conflating provider provenance.

## Advertisement model

`DiscoveryAdvertisement` is immutable and contains:

- `BridgeId`;
- `TransportId`;
- generated `ProtocolVersion` values in deterministic ascending order;
- a validated generated `CapabilitySet` wrapped by `DiscoveryCapabilities`;
- Bridge version;
- signed discovery timestamp;
- expiration timestamp;
- `AdvertisementSignatureMetadata`;
- provider-specific opaque metadata stored in deterministic key order.

Protocol versions must be nonempty, unique, and have a nonzero major version. Capabilities reject
unknown, unspecified, duplicate, or internally inconsistent generated values. Advertisement
expiration must follow its discovery timestamp. Signed metadata requires nonempty algorithm, key
identifier, and signature bytes.

Signature bytes and provider metadata are opaque. Discovery Core does not interpret, verify,
serialize, or persist them.

## Policy and compatibility

`DiscoveryPolicy` is immutable and validates:

- locally supported generated protocol versions;
- required generated application capabilities;
- maximum advertisement lifetime;
- maximum accepted future clock skew;
- maximum provider metadata entries;
- maximum aggregate provider metadata size.

Receipt selects the highest protocol version supported by both policy and advertisement. Rejected
advertisements never enter Bridge State.

A refresh must use the current peer revision and a nonregressing operation timestamp. Signed
advertisement timestamps may advance. Reusing an identical signed timestamp with different
advertisement content is rejected as a conflict.

## Discovered-peer record

`DiscoveredPeer` is the immutable Bridge State record for one `DiscoveryPeerKey`. It contains:

- the composite peer key and its Bridge, source, and transport identifiers;
- current `DiscoveryState`;
- current immutable advertisement;
- last observation or lifecycle timestamp;
- selected generated protocol version;
- peer-local `DiscoveryRevision`;
- capabilities and metadata derived from the current advertisement.

Bridge State stores peers in a deterministic `DiscoveryRegistry`, backed by the existing generic
copy-on-write state registry. `DiscoverySnapshot` is an immutable aggregate view containing shared
peer records and the corresponding Bridge State revision; it is not a second mutable registry.

## Lifecycle

Allowed direct lifecycle transitions are:

```text
Idle                  -> Discovering
Idle                  -> AdvertisementReceived
Discovering           -> AdvertisementReceived
Discovering           -> Unavailable
Discovering           -> Expired
AdvertisementReceived -> Validated
AdvertisementReceived -> Unavailable
AdvertisementReceived -> Expired
Validated             -> AdvertisementReceived
Validated             -> Available
Validated             -> Unavailable
Validated             -> Expired
Available             -> AdvertisementReceived
Available             -> Unavailable
Available             -> Expired
Unavailable           -> Idle
Unavailable           -> Discovering
Unavailable           -> AdvertisementReceived
Unavailable           -> Expired
```

`Expired` and `Removed` are terminal outcomes. `Removed` is emitted as an event when the record is
physically removed and therefore is not retained as a registry value.

Advertisement receipt is a dedicated operation and may create or refresh a nonterminal record in
`AdvertisementReceived`. Provider validation and physical removal are also dedicated operations;
the generic transition command cannot bypass those boundaries.

An advertisement is expired when the operation timestamp is greater than or equal to its expiration
timestamp. Expiration sweeps examine peers in registry-key order and commit all due transitions in
one transaction. Already terminal records are ignored.

## Filtering

`DiscoveryFilter` supports deterministic typed criteria for:

- `BridgeId`;
- `TransportId`;
- discovery source;
- required generated capabilities;
- selected protocol version.

All typed criteria use ordered sets. Results preserve deterministic registry or snapshot order.
`filter_peers_with` and `DiscoverySnapshot::filtered_with` also accept a custom predicate; the
caller must keep that predicate deterministic and side-effect free.

## Transactions and concurrency

Creation, refresh, validation, lifecycle transitions, expiration, and removal execute through
`BridgeStateStore::update_with`.

Validation and mutation occur while the Bridge State write transaction is active. Composite-key
lookup, optimistic revision checks, timestamp checks, lifecycle validation, registry mutation, and
event metadata recording cannot race with another writer.

`DiscoveryRevision` detects stale operations on one peer. `StateRevision` identifies the complete
committed Bridge State snapshot. Concurrent first receipt, refresh, expiration, and removal are
serialized by Bridge State. Exactly one stale-revision-sensitive operation can win for a given
revision.

Rejected operations roll back completely. They do not change records, increment State revisions,
publish events, or produce registry deltas.

## Events

`BridgeStateEvent` remains the only published event system. Discovery Core introduces no event bus
or event store.

Transaction-local `DiscoveryEvent` metadata is committed only with the corresponding Bridge State
mutation. This is required for physical removal, where the resulting snapshot no longer contains
the removed record. Events represent:

- first advertisement receipt and refresh;
- lifecycle transitions;
- physical removal.

Each event contains the composite peer key, peer-local revision, operation timestamp, and typed
previous/current information. Events are ordered by peer key, timestamp, peer revision, and event
kind. The enclosing state event also includes deterministic `DiscoveryRegistry` insert, replacement,
and removal deltas plus the complete committed snapshot.

## Future provider compatibility

A future mDNS, Bonjour, or DNS-SD adapter can map provider records into immutable advertisements
while keeping multicast sockets, interface monitoring, service records, and verification inside its
adapter.

A future Native Messaging provider can expose locally discovered bridge information while keeping
browser process integration and native-host framing outside the core.

A future Android provider can use the same advertisement, policy, filter, lifecycle, revision, and
event contracts while keeping Android APIs, permissions, networking, and process lifecycle in the
Android implementation.
