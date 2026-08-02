# Shared contracts

`shared/` is the single source of truth for every wire-level and runtime-neutral contract
used by the Bridge, browser extension, and Android application.

## Ownership

- `protocol/proto/ymconnect/v1/` defines the canonical Protocol Buffer schema.
- `protocol/descriptor/` contains the canonical descriptor set used for reflection and
  compatibility analysis.
- `protocol/fixtures/v1/` contains byte-for-byte golden vectors.
- `generated/` contains committed language bindings. Generated files are never edited by hand.
- `packages/core/` contains protocol-neutral utilities and interfaces shared by JavaScript
  runtimes.
- `packages/protocol-fixtures/` exposes the golden vectors to test suites.
- `tools/protocol-codegen/` contains the deterministic local protoc plugin.

## Compatibility rules

The protocol uses semantic versions with strict major-version compatibility. A peer may
connect only when its advertised version range overlaps the local range and every required
capability is available. Minor versions are additive. Existing field numbers, enum numeric
values, oneof memberships, and message semantics are immutable within protocol major version
1. Removed fields and enum values must remain reserved.

Unknown fields are retained or ignored according to the language runtime. Unknown enum values
must not be treated as authorization to perform an operation. Capability checks are always
performed before command dispatch.

## Generation

From the repository root:

```bash
pnpm protocol:generate
pnpm generate:check
```

The generator reads only canonical `.proto` files and emits deterministic TypeScript, Rust,
Java Lite, and Kotlin source. Java regeneration uses Protocol Buffer compiler 3.13.0, the
compiler that produced the committed descriptor and Java source. The fixture generator writes
canonical JSON and binary vectors, binds their manifest to the descriptor digest, and is checked
by all language bindings.

## Security boundaries

Messages received from pages, native messaging, platform IPC, local-network peers, QR payloads,
and persisted trust state are untrusted. Consumers must apply the validators and framing limits
from `@ym-connect/core` before routing or acting on decoded data.
