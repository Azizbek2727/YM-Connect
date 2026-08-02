# `ym-connect-protocol`

This crate contains the generated `prost` representation of YM Connect protocol major version
1. It is designed to be consumed by Bridge crates through a path dependency until the protocol
crate is published.

```toml
[dependencies]
ym-connect-protocol = { path = "../../../shared/generated/rust" }
```

```rust
use prost::Message;
use ym_connect_protocol::v1::ProtocolVersion;

let version = ProtocolVersion { major: 1, minor: 0, patch: 0 };
let bytes = version.encode_to_vec();
```

The crate contains no transport, cryptography, persistence, or policy implementation. Those
remain Bridge responsibilities.
