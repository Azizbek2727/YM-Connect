# Bridge Pairing Core Security Invariants

Pairing Core is the runtime-independent trust-establishment authority for the Bridge. Every
successful pairing operation preserves the following invariants.

## Identity

- A bridge identity is a validated stable identifier and one 32-byte Ed25519 public key.
- A peer identity is a validated `DeviceId` and one 32-byte Ed25519 public key.
- Public keys are accepted only after the configured cryptographic provider validates them.
- One active peer identity key maps to at most one device identifier.
- One device identifier has at most one active trusted identity key unless explicit replacement is
  authorized by policy and decision.

## Trust establishment

- Trust is established only after verification of an Ed25519 signature over the complete canonical
  pairing transcript.
- The transcript binds bridge identity, peer identity, challenge identifier and nonce, both X25519
  ephemeral public keys, selected protocol version, negotiated pairing capabilities, and optional
  `SessionId`.
- Pairing completion and trust insertion or replacement commit atomically through Bridge State.
- Network location, transport identity, display names, and metadata never establish trust.

## Replay and freshness

- Every challenge has a unique validated identifier, a 32-byte nonce, creation time, and expiry.
- A response received after expiry is rejected and transitions the pairing session to `Expired`.
- A challenge is consumed at most once. A second response for the same challenge is rejected as a
  replay without modifying trust.
- Terminal pairing sessions retain challenge identity while Bridge State is retained.
- Operation timestamps never move backwards.

## Session binding

- Pairing may bind its transcript to an existing `SessionId`, but Pairing Core never owns session
  lifecycle.
- The session must exist when pairing starts and when trust is established.
- The optional session identifier is signed, preventing substitution.

## Persistence and secret handling

- `BridgeStateStore` is the only mutable source of truth for pairing sessions and trusted peers.
- `TrustStore` is a read abstraction over immutable trusted-peer snapshots, not a second mutable
  store.
- Secret keys, X25519 private material, shared secrets, derived keys, and authentication tags are
  never stored in Bridge State or trust metadata.

## Cryptographic lifecycle

- Only X25519, Ed25519, HKDF-SHA-256, and ChaCha20-Poly1305 are permitted.
- X25519 and Ed25519 public keys are exactly 32 bytes.
- X25519 private keys remain owned by the cryptographic provider or runtime owner.
- HKDF-SHA-256 derives pairing-confirmation key material using the X25519 shared secret and the
  canonical transcript as context.
- ChaCha20-Poly1305 authenticates pairing confirmation only; transport encryption is outside this
  module.
- Derived key material is discarded by the provider after verification.

## Rotation and revocation

- Identity-key replacement is disabled by default.
- Replacement requires both policy permission and `TrustDecision::Replace`.
- Revocation is monotonic for a trust-record revision; a revoked record cannot be refreshed or
  silently reactivated.
- Re-pairing a revoked device requires explicit policy permission and produces a new trust revision.

## Downgrade resistance

- Pairing accepts only the configured protocol major version.
- The selected version must be the highest mutually supported version.
- The selected version and negotiated capabilities are signed in the transcript.
- Selecting a lower mutually supported version is rejected as a downgrade attempt.
- Required local pairing capabilities must remain present after deterministic negotiation.

## Duplicate handling

- Duplicate pairing-session and challenge identifiers are rejected.
- A trusted identity key already assigned to another device is rejected.
- A device presenting a different identity key is rejected unless explicit replacement is allowed.

## Lifecycle and transactions

- Pairing lifecycle transitions are validated by a closed finite-state machine.
- `Completed`, `Rejected`, `Expired`, `Revoked`, and `Cancelled` are terminal.
- Illegal or repeated transitions return structured errors.
- Rejected transactions do not increment Bridge State revision, emit events, or partially mutate
  pairing or trust registries.

## Scope boundary

Pairing Core contains no WebSocket, TLS, Native Messaging, Android networking, discovery, mDNS,
browser integration, UI, QR rendering, operating-system keychain, transport encryption, or
runtime-specific cryptographic wrapper.
