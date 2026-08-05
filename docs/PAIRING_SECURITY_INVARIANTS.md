# Bridge Pairing Core Security Invariants

Pairing Core is the runtime-independent trust-establishment authority for the Bridge. Every
successful pairing operation preserves the following invariants.

## Identity

- A Bridge identity is a validated stable identifier and one 32-byte Ed25519 public key.
- A peer identity is a validated `DeviceId` and one 32-byte Ed25519 public key.
- Public keys are accepted only after the configured cryptographic provider validates them.
- One active peer identity key maps to at most one device identifier.
- One device identifier has at most one active trusted identity key unless replacement is authorized
  by both policy and an explicit trust decision.

## Trust establishment

- Trust is established only after verification of an Ed25519 signature over the complete canonical
  pairing transcript and successful key-agreement confirmation.
- The transcript has a fixed domain separator and binds the pairing identifier, Bridge identity,
  peer identity, challenge identifier and nonce, both X25519 ephemeral public keys, selected
  protocol version, offered and negotiated pairing capabilities, and optional `SessionId`.
- Pairing lifecycle advancement and trust insertion or replacement commit atomically through Bridge
  State.
- Network location, transport identity, display names, and metadata never establish trust.

## Replay and freshness

- Every challenge has a unique validated identifier, a 32-byte nonce, creation time, and exact
  policy-defined expiration time.
- A response observed at or after expiration is rejected without modifying pairing or trust state.
  Expiration is recorded only through the typed `Expired` lifecycle transition.
- A challenge is consumed at most once. A second response for the same challenge is rejected as a
  replay without modifying trust.
- Terminal pairing sessions retain challenge identity while Bridge State is retained.
- Operation timestamps never move backwards.

## Session binding

- Pairing may bind its transcript to an existing `SessionId`, but Pairing Core never owns session
  lifecycle.
- The session must exist when pairing starts, when a response is accepted, and when trust is
  established.
- The optional session identifier is signed, preventing substitution between sessions.

## Persistence and secret handling

- `BridgeStateStore` is the only mutable source of truth for pairing sessions and trusted peers.
- `TrustStore` is a read abstraction over immutable trusted-peer snapshots, not a second mutable
  store.
- Secret keys, X25519 private material, shared secrets, derived keys, and transport credentials are
  never stored in Bridge State or trust metadata.
- The runtime-owned ephemeral-secret source atomically removes an X25519 private key before it is
  used. Once key-agreement verification starts, success or failure consumes that key permanently.
- Invalid public keys are rejected before a private key is consumed.

## Cryptographic lifecycle

- Only X25519, Ed25519, HKDF-SHA-256, and ChaCha20-Poly1305 are permitted.
- X25519 and Ed25519 public keys are exactly 32 bytes.
- Ed25519 verification is strict and authenticates the domain-separated canonical transcript.
- The Bridge X25519 private key must match the public key recorded in the challenge.
- X25519 all-zero shared secrets and all-zero public keys are rejected.
- HKDF-SHA-256 derives pairing-confirmation key material using the X25519 shared secret and the
  transcript digest as context.
- ChaCha20-Poly1305 authenticates pairing confirmation only; transport encryption is outside this
  module.
- Private and derived key material is zeroized or discarded after the verification attempt.

## Rotation and revocation

- Active trust replacement is disabled by default.
- Every active-record replacement, including same-key replacement, requires policy permission,
  `TrustDecision::Replace`, and the exact current trust revision.
- Identity-key changes are never inferred from a device identifier or metadata.
- Revocation is monotonic for a trust-record revision; a revoked record cannot be refreshed or
  silently reactivated.
- Re-pairing a revoked device requires explicit revoked-replacement policy, an explicit replacement
  decision, and the exact revoked trust revision.
- Trust timestamps never move backwards.

## Downgrade resistance

- Pairing accepts only the configured protocol major version and rejects versions newer than the
  configured implementation.
- The selected version must be the highest mutually supported version.
- The selected version, offered capabilities, negotiated capabilities, and deterministic capability
  parameters are signed in the transcript.
- Selecting a lower mutually supported version is rejected as a downgrade attempt.
- Required local and remote pairing capabilities must remain present after deterministic
  negotiation.

## Duplicate handling

- Duplicate pairing-session and challenge identifiers are rejected.
- A trusted active identity key already assigned to another device is rejected.
- A device presenting a different identity key is rejected unless active or revoked replacement is
  explicitly authorized by the corresponding policy.

## Lifecycle and transactions

- Pairing lifecycle transitions are validated by a closed finite-state machine.
- `Completed`, `Rejected`, `Expired`, `Revoked`, and `Cancelled` are terminal.
- Illegal or repeated transitions return structured errors.
- Pairing records and trust records use optimistic revisions. Concurrent creation, replacement, and
  revocation operations have at most one winner for a given expected revision.
- Rejected transactions do not increment Bridge State revision, emit events, or partially mutate
  pairing or trust registries.
- Pairing lifecycle events, trust events, and registry deltas are derived from committed snapshots
  and emitted in deterministic subsystem and event-kind order.

## Scope boundary

Pairing Core contains no WebSocket, TLS, Native Messaging, Android networking, discovery, mDNS,
browser integration, UI, QR rendering, operating-system keychain, transport encryption, or
runtime-specific transport integration.
