# `@ym-connect/core`

`@ym-connect/core` is the runtime-neutral policy layer above the canonical
`ymconnect.v1` Protocol Buffer models. It is consumed by the Bridge, browser
Extension, and Android-facing JavaScript tooling.

The package provides:

- protocol and repository version constants;
- major/minor compatibility negotiation;
- capability normalization and negotiation;
- typed protocol errors;
- boundary validation for externally supplied messages;
- deterministic binary and JSON serialization helpers;
- length-delimited framing for IPC and stream transports;
- cryptographically secure opaque identifiers;
- small runtime contracts for clocks, codecs, validators, and policies.

It does not implement transport, persistence, cryptography, browser APIs, or
player-provider behavior. Those concerns remain in their owning top-level
components.
