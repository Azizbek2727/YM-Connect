# Contributing to YM Connect

## Prerequisites

Install the exact versions declared by `.node-version`, `package.json`,
`rust-toolchain.toml`, `gradle/wrapper/gradle-wrapper.properties`, and `buf.yaml`.
Use JDK 17 or later for Gradle and Android builds.

Enable Corepack and install the workspace from the lockfile:

```bash
corepack enable
pnpm install --frozen-lockfile
```

## Required validation

Run the complete local gate before submitting a change:

```bash
pnpm generate:check
pnpm format:check
pnpm lint
pnpm typecheck
pnpm test
pnpm build
```

Rust-only changes must also pass:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo deny check
```

Android changes must also pass:

```bash
./gradlew --no-daemon lint testDebugUnitTest assembleDebug
```

Protocol changes must also pass:

```bash
buf lint
buf build
buf breaking --against '.git#branch=main'
pnpm protocol:fixtures:check
```

## Frozen-contract guardrails

Architecture, protocol semantics, validation rules, and roadmap sequencing are frozen.
Implementation must preserve these properties:

- the Bridge owns sessions, trust, authorization, routing, and LAN exposure;
- connector, client, and encrypted security frames remain separate wire surfaces;
- browser code imports generated Protobuf bindings and never implements a second wire codec;
- `BrowserConnector` and `PlayerAdapter` remain stable, independently testable boundaries;
- adapter inventory is extension-owned and page observations are untrusted;
- old-session frames never cross a reconnect boundary;
- inbound operations are serialized where state ownership requires ordering;
- every untrusted boundary enforces structural validation, semantic validation, quotas,
  rate limits, and bounded resource use;
- cryptographic secrets never enter provider-page or browser-extension storage; and
- production artifacts contain no telemetry, remote executable code, or test-only providers.

A frozen-contract change follows the critical-issue process in [GOVERNANCE.md](GOVERNANCE.md).

## Change design

Keep changes narrowly scoped and atomic. A pull request must explain:

1. the user-visible or operational problem;
2. the affected ownership boundary;
3. failure and recovery behavior;
4. security and privacy impact;
5. compatibility impact;
6. tests added or changed; and
7. release or migration requirements.

## Coding standards

- TypeScript is strict and does not use `any`, unchecked casts at trust boundaries, dynamic
  evaluation, or handwritten Protobuf field mappings.
- Rust forbids unsafe code at the workspace level. Exceptions require a separate security
  review and a governance decision before the lint policy can change.
- Kotlin uses structured concurrency, explicit ownership, and immutable state projections.
- Logs must be local, bounded, redacted, and free of secrets, full URLs, authentication
  material, private keys, or provider account identifiers.
- Public APIs and protocol behavior require documentation and deterministic tests.

## Commit policy

Use imperative, scoped commit subjects. Keep generated code and the schema change that
produced it in the same commit. Do not mix unrelated formatting with behavior changes.

Contributions are made under Apache License 2.0. By submitting a contribution, you certify
that you have the right to do so under the Developer Certificate of Origin 1.1. Add a
`Signed-off-by` trailer to each commit.

## Reporting security defects

Do not open a public issue for a suspected vulnerability. Follow [SECURITY.md](SECURITY.md).
