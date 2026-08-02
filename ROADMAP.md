# Roadmap

The architecture, protocol, validation rules, and implementation sequence are frozen for the
first production release.

## Release 0.1 — Platform foundation

- Canonical generated Protocol Buffer bindings for TypeScript, Rust, Java, and Kotlin.
- Stable `BrowserConnector` and `PlayerAdapter` API packages.
- Production Rust Bridge daemon and native-host mode.
- Per-user platform IPC with peer authentication and bounded framing.
- Security handshake, pairing, encrypted client sessions, replay protection, and trust
  persistence.
- Windows, macOS, and Linux native-host manifests, daemon services, installers, repair,
  upgrade, and uninstall behavior.
- Chromium and Firefox end-to-end browser integration against a real Bridge.
- Windows process recovery and single-instance enforcement.
- Cross-language golden fixtures, compatibility tests, fuzzing, and CI validation.

## Release 0.2 — Android core

- Local-network onboarding and runtime permission handling.
- Discovery, QR pairing, trusted-device management, and certificate pinning.
- Device and player selection, now-playing projection, and core playback controls.
- Reconnection across sleep, process death, Wi-Fi changes, and Bridge restarts.
- Optional persistent background connectivity and widget command orchestration.
- API 26 through current-API unit, instrumentation, and managed-device validation.

## Release 1.0 — Production hardening

- Signed installers and updates for all Tier 1 desktop platforms.
- Store-ready Chromium and Firefox extension packages.
- Reproducible Android release bundle.
- Upgrade and rollback validation across the compatibility window.
- Performance, memory, CPU, battery, failure-injection, and long-running recovery gates.
- Software bills of materials, provenance attestations, license reports, and incident runbooks.

## Post-1.0 evolution

Post-1.0 work is capability-led and compatibility-preserving. New providers, search, library,
lyrics, queue editing, desktop clients, or alternate transports require evidence that the
baseline control path remains secure, maintainable, and correctly negotiated.
