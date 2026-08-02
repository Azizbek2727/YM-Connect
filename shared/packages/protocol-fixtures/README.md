# `@ym-connect/protocol-fixtures`

This private workspace package exposes the committed protocol conformance
vectors under `shared/protocol/fixtures/v1`.

Each fixture has a canonical JSON representation and the exact binary wire
representation. The manifest records the message type, file names, byte count,
and SHA-256 digest. Consumers verify the digest before decoding so fixture
corruption cannot be mistaken for a codec incompatibility.


Run `pnpm --filter @ym-connect/protocol-fixtures generate` to reproduce all fixture files and
the manifest. Generation is deterministic and incorporates the canonical descriptor SHA-256
digest, while `pnpm --filter @ym-connect/protocol-fixtures test` verifies every digest and both
JSON-to-binary and binary-to-JSON conformance.
