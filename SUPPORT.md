# Support Policy

## Platform tiers

### Tier 1

Tier 1 platforms are release-blocking and receive automated build, integration, installer,
upgrade, recovery, and security validation:

- Windows 10 and Windows 11 on x86-64;
- macOS 13 or later on Apple silicon and x86-64;
- Ubuntu LTS on x86-64;
- current stable and previous stable Chromium-family desktop releases;
- current stable Firefox and the current Firefox ESR line; and
- Android API 26 through the current stable API level on arm64 and x86-64 emulators.

### Tier 2

Tier 2 platforms receive build validation and best-effort fixes but are not release-blocking:

- other systemd-based desktop Linux distributions on x86-64 or arm64; and
- compatible Chromium-family browsers that preserve Native Messaging behavior.

### Unsupported

Unsupported environments include mobile browsers, browsers without Native Messaging,
containerized desktop sessions without supported user-service and secure-storage access, and
operating systems outside vendor security support.

## Compatibility window

The current minor release interoperates with the two preceding minor releases when their
negotiated protocol ranges overlap. Security-frame or persisted-trust migrations may impose
a narrower window when required to eliminate an unsafe behavior; release notes state the
upgrade order and minimum versions.

## Provider behavior

Yandex Music web integration relies on documented browser media surfaces and observed UI
semantics rather than private provider APIs. Provider-dependent capabilities degrade
explicitly when reliable controls or observations are unavailable. Core failures should not
be disguised as successful commands.

## Support requests

Use repository issue templates for reproducible defects and feature proposals. Include
versions, platform, browser, Bridge diagnostics export, exact recovery steps, and whether the
issue reproduces with a clean extension profile. Remove account data and secrets.
