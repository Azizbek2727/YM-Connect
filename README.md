# YM Connect

YM Connect is a local-control system for operating Yandex Music playback in a desktop
browser from an Android device. The system keeps discovery, trust, authorization, routing,
and client transport in a native desktop Bridge instead of exposing browser extension
internals to the local network.

YM Connect is independent and unofficial. It is not affiliated with or endorsed by Yandex.

## Architecture

```text
Android client
    ⇅ authenticated and encrypted LAN session
Desktop Bridge daemon
    ⇅ authenticated per-user local IPC
Browser native-messaging host
    ⇅ canonical ConnectorEnvelope frames
Browser Connector
    ⇅ validated, quota-bound observations and commands
PlayerAdapter
    ⇅ provider page
Yandex Music web player
```

The architecture has four stable boundaries:

1. Canonical Protocol Buffer schemas and generated language bindings in `shared/`.
2. The Rust Bridge and native-host boundary in `bridge/`.
3. The browser-neutral `BrowserConnector` and provider-neutral `PlayerAdapter` APIs in
   `extension/`.
4. The Android transport, trust, repository, and presentation layers in `android/`.

The Bridge owns protocol sessions, trust state, player routing, active-player selection,
replay protection, and LAN exposure. The browser extension owns trusted adapter inventory
and treats page-originated data as untrusted. Android stores private keys in Android
Keystore and treats Bridge state as a projection of Bridge-owned authority.

## Security posture

- Pairing is explicit and user-mediated.
- Client sessions are authenticated, encrypted, replay-protected, and bound to a trusted
  Bridge identity.
- Browser-to-daemon IPC is local to the signed-in operating-system user and validates peer
  identity where the platform supports it.
- Provider pages never receive Bridge keys, client keys, native IPC handles, or network
  credentials.
- Every page, native-messaging, IPC, and network boundary enforces schema validation,
  quotas, ordering, and rate limits.
- The project contains no telemetry, analytics, advertising SDKs, or remote executable code.

See [SECURITY.md](SECURITY.md) for reporting and support policy.

## Repository layout

| Directory | Responsibility |
| --- | --- |
| `shared/` | Protocol schemas, generated bindings, shared APIs, fixtures, and compatibility tests. |
| `bridge/` | Rust daemon, native host, IPC, trust store, installers, and platform integration. |
| `extension/` | Chromium and Firefox extension builds, connector core, adapters, and browser tests. |
| `android/` | Android application, pairing, transport, now-playing UI, widgets, and tests. |
| `docs/` | Frozen architecture, protocol rules, threat model, operations, and release documentation. |
| `.github/` | CI, release automation, security scanning, templates, and dependency policy. |

## Supported baseline

The production baseline is Windows 10 or later, macOS 13 or later, and supported desktop
Linux distributions with systemd user services. Browser support targets current stable and
previous stable Chromium-family releases plus current stable and previous ESR-compatible
Firefox releases. Android support starts at API 26.

Detailed support tiers are defined in [SUPPORT.md](SUPPORT.md).

## Prerequisites

- Node.js 24.18.0
- pnpm 11.17.0 through Corepack
- Rust 1.97.1 with Rustfmt and Clippy
- Buf CLI 1.69.0
- JDK 17 or later
- Android SDK Platform 37 and Build Tools selected by Android Gradle Plugin 9.3.1

## Bootstrap

```bash
corepack enable
pnpm install --frozen-lockfile
pnpm generate
pnpm check
pnpm build
pnpm test
```

Android commands use the checked-in Gradle wrapper:

```bash
./gradlew :android:app:assembleDebug
./gradlew :android:app:testDebugUnitTest
```

Rust commands use the pinned toolchain:

```bash
cargo build --workspace --locked
cargo test --workspace --locked
```

## Design constraints

The architecture, protocol semantics, validation rules, and implementation roadmap are
frozen. A change to a frozen contract requires a reproducible critical implementation issue,
a written impact analysis, a compatibility plan, and maintainer approval under
[GOVERNANCE.md](GOVERNANCE.md). Ordinary implementation preference is not sufficient.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), and
[GOVERNANCE.md](GOVERNANCE.md) before opening a change.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
