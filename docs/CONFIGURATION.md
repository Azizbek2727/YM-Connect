# Bridge Configuration

<!-- cspell:words usize -->

## Scope

The Bridge configuration subsystem is owned by `ym-connect-bridge-core`. It resolves a
validated immutable configuration snapshot before the daemon constructs the asynchronous
runtime or application dependencies.

The core performs no filesystem, network, browser, or platform integration. The daemon is
limited to process startup: it loads configuration, constructs the runtime and production
dependencies, and starts the Bridge application lifecycle.

## Resolution order

Configuration is resolved from lowest to highest precedence:

1. Built-in defaults.
2. Typed `BridgeConfigLayer` values added through `BridgeConfigLoader::with_layer`.
3. Process environment variables.

Typed layers are applied in insertion order, so a later layer overrides an earlier layer.
Environment variables override every typed layer. Environment loading can be disabled with
`BridgeConfigLoader::without_environment` for deterministic embedding and tests.

The core does not select a configuration file format or perform filesystem access. A
non-environment source can parse its input outside the core and provide a strongly typed
`BridgeConfigLayer`.

## Configuration model

The resolved model contains immutable logging and runtime sections. All fields are private
and exposed only through read-only accessors. The resolved snapshot has no mutation API and
is passed to startup components only after every enabled source has been validated.

| Field | Rust type | Default |
| --- | --- | --- |
| `logging.level` | `LogLevel` | `Info` |
| `runtime.worker_threads` | `RuntimeWorkerThreads` | `Automatic` |

`RuntimeWorkerThreads::Automatic` delegates worker-count selection to the asynchronous
runtime. `RuntimeWorkerThreads::Fixed` contains a `NonZeroUsize`, so an invalid zero value
cannot enter the resolved runtime configuration.

## Environment variables

| Variable | Field | Accepted values | Default |
| --- | --- | --- | --- |
| `YM_CONNECT_LOG_LEVEL` | `logging.level` | `off`, `error`, `warn`, `warning`, `info`, `debug`, `trace` | `info` |
| `YM_CONNECT_RUNTIME_WORKER_THREADS` | `runtime.worker_threads` | `auto` or a positive integer | `auto` |

Text values are matched without case sensitivity. Values are not trimmed; surrounding
whitespace is invalid. Environment values must be valid Unicode.

## Loading API

Use the production default loader for built-in defaults plus environment overrides:

```rust
use ym_connect_bridge_core::BridgeConfig;

let config = BridgeConfig::load()?;
```

Add one or more typed layers before environment overrides:

```rust
use std::num::NonZeroUsize;

use ym_connect_bridge_core::{
    BridgeConfig, BridgeConfigLayer, LogLevel, RuntimeWorkerThreads,
};

let fixed_workers = NonZeroUsize::new(4)
    .map(RuntimeWorkerThreads::fixed)
    .unwrap_or_default();

let config = BridgeConfig::loader()
    .with_layer(
        BridgeConfigLayer::new()
            .with_log_level(LogLevel::Debug)
            .with_runtime_worker_threads(fixed_workers),
    )
    .load()?;
```

The example uses a constant nonzero value. Production callers that derive the value from
input must validate it before creating a typed layer.

## Validation errors

`ConfigError` is structured rather than message-only. It exposes:

- `ConfigField`, the stable field identifier such as `logging.level`;
- `ConfigSource`, including the originating environment variable;
- `ConfigErrorKind`, which distinguishes non-Unicode input, unsupported values, and integer
  parsing failures.

Integer parsing failures preserve the standard parser error as the error source. The daemon
reports the complete error chain and exits before constructing the asynchronous runtime.

## Current boundaries

This subsystem does not implement networking, WebSocket transport, discovery, pairing,
Native Messaging, player adapters, or browser integration. Configuration fields for those
modules must be added only with the module that owns and validates them.
