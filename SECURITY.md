# Security Policy

## Supported versions

Security fixes are provided for the current minor release and the two preceding minor
releases when those releases remain protocol-compatible. Pre-release builds receive fixes
only on the active pre-release line.

## Reporting a vulnerability

Use GitHub private vulnerability reporting for the repository. Do not file a public issue,
post exploit details, or contact unrelated contributors.

Include the affected version, platform, threat model, reproduction steps, expected impact,
and any proposed mitigation. Remove personal data, account tokens, private keys, and provider
credentials from the report.

## Response targets

Maintainers target acknowledgment within three business days, initial triage within seven
business days, and a remediation plan within fourteen business days. Complex cross-platform
or coordinated-disclosure cases may require additional time; the reporter receives status
updates at least every fourteen days.

## Coordinated disclosure

Security work is handled in a restricted channel until a fix, regression tests, release
artifacts, and upgrade guidance are ready. Disclosure timing is agreed with the reporter and
may be accelerated when exploitation is active or public.

## Security invariants

- The desktop Bridge is the only component exposed to the LAN.
- Pairing is explicit and binds a client identity to a Bridge identity.
- Client traffic is mutually authenticated, encrypted, and replay-protected.
- Bridge-owned session identifiers are not invented by connectors or clients.
- Native IPC is restricted to the signed-in operating-system user and authenticates the
  expected peer where platform primitives permit it.
- Provider pages cannot define adapters, access native messaging, or obtain cryptographic
  material.
- Generated Protobuf bindings are the only wire-format implementation.
- Unknown enum values remain distinguishable; they are never silently mapped to a valid
  current semantic value.
- Request, correlation, session, clock-domain, revision, TTL, and state-transition rules are
  validated before dispatch.
- Queues, transfers, recursive structures, strings, collections, logs, caches, and concurrent
  operations have explicit bounds.
- Secrets are stored in operating-system secure storage or Android Keystore and are zeroized
  in memory where supported by the implementation language and library.
- Diagnostics are local, bounded, and redacted.

## Dependency and release security

Dependencies are pinned through lockfiles. CI performs advisory, license, secret, static,
malware-pattern, and generated-code checks. Release workflows use immutable action commit
identifiers, protected environments, software bills of materials, provenance attestations,
and signed tags and artifacts.

## Exclusions

Provider UI changes that only cause capability degradation are compatibility defects rather
than security vulnerabilities unless they enable code execution, data exposure, privilege
escalation, authentication bypass, or denial of service beyond documented limits.
