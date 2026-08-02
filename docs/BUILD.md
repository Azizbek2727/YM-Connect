# Building YM Connect

<!-- cspell:words anchore attest attestations bufbuild codeql dependabot macos notarization openssf playwright protoc sarif sbom scorecard spdx syft temurin webkit x86 -->

## Overview

YM Connect is a multi-platform workspace containing four coordinated implementation surfaces:

- canonical Protocol Buffer schemas and generated models under `shared/`;
- a Rust Bridge daemon and browser native-messaging host under `bridge/`;
- a TypeScript browser Extension under `extension/`; and
- an Android application built with Gradle under `android/`.

The repository root owns toolchain versions and the commands used by local development, continuous integration, and release automation. Contributors must run repository commands from the repository root unless a section explicitly identifies a component-level command.

Generated protocol models are committed. The schemas in `shared/protocol/proto/` are the source of truth; generated TypeScript, Rust, Java, and Kotlin files must never be edited manually.

## Supported platforms

The continuous-integration matrix validates the desktop components on:

| Platform | GitHub-hosted image | Primary architecture |
|---|---|---|
| Ubuntu Linux | `ubuntu-24.04` | x86-64 |
| macOS | `macos-15-intel` | x86-64 |
| Windows | `windows-2022` | x86-64 |

Android builds use Java 17 and the Android SDK selected by the Android project. Linux is the reference host for Android release builds and browser automation. macOS and Windows are required validation hosts for the Bridge, native-host integration, JavaScript workspace, and Gradle model projects.

A contributor may develop on another recent Linux distribution or on Apple silicon, but release readiness is determined by the supported CI matrix.

## Prerequisites

Install these tools before bootstrapping the repository:

| Tool | Required version | Source of truth |
|---|---:|---|
| Git | 2.43 or newer | Host package manager |
| Node.js | `24.18.0` | `.node-version` |
| pnpm | `11.17.0` | `package.json` `packageManager` |
| Rust | `1.97.1` | `rust-toolchain.toml` |
| Java | `17` | Gradle build configuration |
| Gradle | `9.5.0` | Gradle wrapper |
| Buf CLI | `1.69.0` | Repository and CI configuration |
| Protocol Buffer compiler | `3.13.0` | Protocol descriptor command and CI configuration |
| Android SDK | Versions declared by `android/` | Android Gradle project |
| Chromium, Firefox, WebKit | Versions managed by Playwright | Extension lockfile |

Release engineering and supply-chain validation also require GitHub CLI with attestation support and `cargo-deny`. CI supplies Syft through the pinned Anchore SBOM action. Platform signing tools are available only in protected release environments when signing is enabled.

Do not substitute a globally installed Gradle distribution for the wrapper. Do not install a different pnpm version globally and bypass Corepack.

## Repository bootstrap

Clone the repository and enter the checkout:

```bash
git clone https://github.com/ym-connect/ym-connect.git
cd ym-connect
```

Enable the package manager version declared by the repository:

```bash
corepack enable
```

Confirm the principal tool versions:

```bash
node --version
pnpm --version
rustc --version
cargo --version
java -version
./gradlew --version
buf --version
protoc --version
```

The expected Node version begins with `v24.18.0`, pnpm reports `11.17.0`, Rust reports `1.97.1`, Java reports version 17, Gradle reports `9.5.0`, Buf reports `1.69.0`, and protoc reports `3.13.0`.

## Workspace installation

Install all JavaScript workspace dependencies using the frozen lockfile:

```bash
pnpm install --frozen-lockfile
```

The root shortcut runs the same approved installation command:

```bash
pnpm bootstrap
```

A frozen install must not modify `pnpm-lock.yaml`. After installation, verify that the repository remains clean:

```bash
git diff --exit-code -- pnpm-lock.yaml
```

Rust and Gradle dependencies are resolved when their build commands run. Bridge workspace commands use `--locked`; Gradle uses the wrapper and, when Android is present, strict dependency-verification metadata under `gradle/verification-metadata.xml`.

## Protocol generation

The complete generation pipeline builds the descriptor set, invokes the checked-in Buf generation template, and regenerates canonical fixtures:

```bash
pnpm protocol:generate
```

The root alias is:

```bash
pnpm generate
```

Generation performs these operations in order:

1. `protoc` builds `shared/protocol/descriptor/ymconnect-v1.pb` with imports and source information.
2. Node calculates `shared/protocol/descriptor/ymconnect-v1.pb.sha256`.
3. Buf executes `shared/protocol/buf.gen.yaml`.
4. The protocol-fixtures package rewrites the canonical JSON and binary vectors.

Verify that generation is deterministic and that no committed output changes:

```bash
pnpm generate:check
```

Validate schema style and compile the Buf module:

```bash
pnpm protocol:lint
pnpm protocol:build
```

Validate golden fixtures:

```bash
pnpm protocol:fixtures:check
```

For a pull request that changes protocol schemas, compare compatibility with the default branch:

```bash
pnpm protocol:breaking
```

Protocol changes require compatibility analysis even when Buf reports no wire-format break. Capability meaning, authentication sequencing, trust semantics, limits, defaults, and persisted representations are part of the compatibility contract.

## TypeScript build

Build every JavaScript and TypeScript workspace package that exposes a build script:

```bash
pnpm build:js
```

Run JavaScript syntax and repository linting:

```bash
pnpm lint:js
pnpm lint:workspace
```

Run declaration and package type checks:

```bash
pnpm typecheck
```

Run all JavaScript tests:

```bash
pnpm test:js
```

The TypeScript CI workflow runs these commands on Ubuntu, macOS, and Windows. The official Protobuf-ES packages are pinned in `pnpm-workspace.yaml`; `.github/scripts/verify-official-protobuf-es.mjs` rejects unsupported alternative runtimes.

## Rust build

Build the Bridge workspace through the root command:

```bash
pnpm build:rust
```

This executes:

```bash
cargo build --workspace --all-features --locked
```

Lint the complete Bridge workspace:

```bash
pnpm lint:rust
```

Run the Bridge test suite:

```bash
pnpm test:rust
```

Validate the standalone generated Rust protocol crate:

```bash
cargo fmt --manifest-path shared/generated/rust/Cargo.toml -- --check
cargo build --manifest-path shared/generated/rust/Cargo.toml
cargo test --manifest-path shared/generated/rust/Cargo.toml
```

Cargo lockfile enforcement is mandatory for the Bridge workspace. The standalone generated protocol crate has exact direct dependency pins but no separate lockfile. Never edit the root `Cargo.lock` manually.

## Gradle build

Use the repository wrapper on every platform. In Git Bash, Linux, and macOS:

```bash
./gradlew --no-daemon assemble
```

In PowerShell or Command Prompt:

```powershell
.\gradlew.bat --no-daemon assemble
```

The root package command is:

```bash
pnpm build:android
```

Run Android lint and unit tests:

```bash
pnpm lint:android
pnpm test:android
```

Verify synchronized repository versions:

```bash
./gradlew --no-daemon verifyRepositoryVersion
```

Build and test the standalone generated JVM model projects:

```bash
./gradlew --no-daemon -p shared/generated/java build
./gradlew --no-daemon -p shared/generated/kotlin build
```

When Android sources exist, strict dependency verification must succeed:

```bash
./gradlew --no-daemon --dependency-verification=strict help
```

Dependency-verification metadata is a reviewed supply-chain file. Update it only as part of an intentional Gradle dependency change.

## Extension build

The Extension participates in the pnpm workspace. Build it through the root JavaScript build command:

```bash
pnpm build:js
```

Run its unit and integration tests through:

```bash
pnpm test:js
```

Run its lint and type checks through:

```bash
pnpm lint:workspace
pnpm typecheck
```

The Extension package owns browser manifests, adapter inventory, content scripts, background logic, native-messaging integration, packaging, and Playwright tests. Page-provided data remains untrusted and must not define executable adapter behavior.

Release packages are collected from the Extension's build-output locations under `extension/dist/`, `extension/artifacts/`, or `extension/build/`.

## Bridge build

Build the production Bridge daemon and native host with:

```bash
pnpm build:rust
```

Validate all Bridge targets and features with:

```bash
pnpm lint:rust
pnpm test:rust
```

The Bridge owns client sessions, pairing, trust storage, message routing, native-host IPC, platform process recovery, and release installers. Tests must cover malformed IPC frames, failed authentication, replay rejection, trust revocation, process termination, restart recovery, and installer repair or removal behavior as applicable.

Release outputs are collected from `bridge/dist/`, `bridge/installers/`, `target/dist/`, and eligible final binaries in `target/release/`. Intermediate Cargo files are excluded from release bundles.

## Android build

Build all Android variants declared by the project:

```bash
pnpm build:android
```

Run local JVM tests:

```bash
pnpm test:android
```

Run Android lint:

```bash
pnpm lint:android
```

For release preparation, inspect outputs under:

```text
android/app/build/outputs/apk/
android/app/build/outputs/bundle/
```

Android release signing is not available to pull-request workflows. Protected release environments supply signing identities only when release signing is enabled.

## Running tests

Run every currently available test suite:

```bash
pnpm test
```

This is the aggregate command for JavaScript, Rust, and Android tests.

Run the repository's complete validation sequence:

```bash
pnpm check
```

`pnpm check` executes generated-code verification, formatting checks, all lint gates, type checks, all tests, and all builds. It is the closest local equivalent to the required pull-request checks once Bridge, Extension, and Android sources are all present.

For staged revisions where a later component directory has not yet been introduced, run every command applicable to the directories present and use:

```bash
node .github/scripts/component-status.mjs
```

The script reports whether Bridge, Extension, Android, Playwright, and the complete platform set are present. CI uses the same detection and automatically activates component gates as each approved directory lands.

## Running Playwright

Install the browser binaries declared by the Extension workspace.

On Ubuntu:

```bash
pnpm --dir extension exec playwright install --with-deps
```

On macOS and Windows:

```bash
pnpm --dir extension exec playwright install
```

Run the repository JavaScript tests, which include the Extension's Playwright projects:

```bash
pnpm test:js
```

Playwright tests must use isolated browser profiles and temporary native-host registrations. They must not depend on a contributor's installed browser profile, production trust store, or personal Yandex account state.

The dedicated workflow stores `extension/playwright-report/` and `extension/test-results/` when those directories are produced.

## Browser and native-host integration

Cross-platform integration validation requires both `bridge/` and `extension/`.

Run the same sequence used by the integration workflow:

```bash
pnpm build:js
pnpm build:rust
pnpm test:js
pnpm test:rust
```

Install Playwright browsers before the test commands when the Extension includes Playwright configuration.

Integration tests must use the production native-messaging framing and platform registration code. Tests may substitute temporary filesystem roots, ports, process supervisors, and trust stores, but they must not replace the protocol, authentication handshake, or IPC implementation with a different test-only design.

## Running CI locally

The authoritative workflow files are under `.github/workflows/`. Local parity is obtained by running the same repository commands rather than maintaining a separate local pipeline.

Run the primary gates:

```bash
node .github/scripts/validate-release-tag.mjs --version-only
node .github/scripts/verify-lockfiles.mjs
node .github/scripts/verify-official-protobuf-es.mjs
node .github/scripts/verify-workflow-pins.mjs
pnpm generate:check
pnpm format:check
pnpm spellcheck
pnpm lint:js
pnpm lint:workspace
pnpm typecheck
pnpm test:js
pnpm build:js
```

When Bridge exists:

```bash
pnpm lint:rust
pnpm test:rust
pnpm build:rust
cargo deny check
```

When Android exists:

```bash
./gradlew --no-daemon verifyRepositoryVersion
pnpm lint:android
pnpm test:android
pnpm build:android
./gradlew --no-daemon --dependency-verification=strict help
```

Confirm that generated files and lockfiles remain unchanged:

```bash
git diff --exit-code -- shared/generated shared/protocol/descriptor shared/protocol/fixtures
git diff --exit-code -- pnpm-lock.yaml Cargo.lock gradle/verification-metadata.xml
```

Linux-only GitHub Actions emulation tools may be used for workflow debugging, but they do not replace the hosted macOS and Windows matrix results.

## Formatting and spelling

Apply repository formatting:

```bash
pnpm format
```

Check formatting without modifying files:

```bash
pnpm format:check
```

Run the repository spelling policy:

```bash
pnpm spellcheck
```

Formatting commands may update only source and documentation formatting. Generated protocol output must be changed through `pnpm protocol:generate`.

## Security and dependency validation

Verify repository supply-chain policy:

```bash
node .github/scripts/verify-lockfiles.mjs
node .github/scripts/verify-official-protobuf-es.mjs
node .github/scripts/verify-workflow-pins.mjs
```

Run Cargo license, advisory, ban, and source checks:

```bash
cargo deny check
```

Run Gradle strict dependency verification when Android exists:

```bash
./gradlew --no-daemon --dependency-verification=strict help
```

Pull requests are also evaluated by GitHub Dependency Review. Main-branch and scheduled workflows generate an SPDX JSON SBOM and run OpenSSF Scorecard analysis.

## Troubleshooting

### Frozen pnpm installation reports a changed lockfile

Confirm that Node and pnpm match the pinned versions. Remove `node_modules` directories, enable Corepack again, and repeat the frozen installation. Do not regenerate the lockfile unless dependency manifests intentionally changed.

### `protoc` or Buf cannot be found

Install protoc `3.13.0` and Buf `1.69.0`, then confirm both are on `PATH`. Do not use an unpinned system package when regenerating committed outputs.

### Generated-code verification reports changes

Run:

```bash
pnpm protocol:generate
```

Review every generated change. If no schema or generator input was intentionally changed, restore the generated files and investigate the local tool version.

### Cargo rejects the lockfile

Confirm Rust `1.97.1` is active:

```bash
rustup toolchain install 1.97.1 --profile minimal --component clippy,rustfmt
rustup default 1.97.1
```

Then rerun the locked command. Update `Cargo.lock` only through an intentional dependency update.

### Gradle wrapper download or checksum fails

Verify network access to the Gradle distribution service and confirm that `gradle-wrapper.jar`, `gradle-wrapper.properties`, `gradlew`, and `gradlew.bat` match the repository. The setup workflow validates wrapper integrity.

### Gradle dependency verification fails

A resolved artifact differs from reviewed metadata or a new dependency was introduced. Confirm the requested dependency and repository source, regenerate verification metadata using Gradle's supported mechanism, inspect every new checksum, and commit the reviewed metadata with the dependency change.

### Playwright cannot launch a browser on Linux

Run the Linux browser installation command with operating-system dependencies:

```bash
pnpm --dir extension exec playwright install --with-deps
```

Container environments must permit the sandbox configuration used by the Extension tests.

### Windows native-host tests leave registrations behind

Run the cleanup command supplied by the Bridge test suite, then remove only the temporary test registration root reported by the failed test. Do not delete production browser native-messaging keys or the user's trust store.

### macOS execution is blocked

Development binaries may require removal of quarantine metadata when they were copied from an archive. Release binaries require the protected signing and notarization pipeline when platform signing is enabled.

## Updating generated Protocol Buffer models

Use this sequence for an approved schema change:

1. Modify only files under `shared/protocol/proto/ymconnect/v1/`.
2. Run `pnpm protocol:lint`.
3. Run `pnpm protocol:build`.
4. Run `pnpm protocol:breaking` against `main`.
5. Run `pnpm protocol:generate`.
6. Run `pnpm protocol:fixtures:check`.
7. Run the TypeScript, Rust, Java, and Kotlin fixture tests.
8. Review the descriptor checksum and every generated diff.
9. Document compatibility and minimum-version effects in the pull request.

Generated Java, Kotlin, Rust, TypeScript, descriptor, and fixture files must be committed in the same change as their schema input.

## Dependency updates

### npm and pnpm

Update package manifests intentionally, then run:

```bash
pnpm install
pnpm check
pnpm install --frozen-lockfile
```

Review `pnpm-lock.yaml`, package license changes, lifecycle scripts, native binaries, and transitive dependency changes.

### Cargo

Use Cargo's package-specific update command with the exact dependency name and reviewed version. Then run:

```bash
pnpm lint:rust
pnpm test:rust
pnpm build:rust
cargo deny check
```

Review `Cargo.lock`, feature changes, build scripts, licenses, advisories, and duplicate-version effects.

### Gradle

Update the version in the relevant build file, refresh dependency-verification metadata, review all new checksums, and run:

```bash
pnpm lint:android
pnpm test:android
pnpm build:android
./gradlew --no-daemon --dependency-verification=strict help
```

### GitHub Actions

Dependabot updates action references. Every external `uses:` reference must remain pinned to a full 40-character commit SHA with a version comment. Verify the result with:

```bash
node .github/scripts/verify-workflow-pins.mjs
```

## Common developer workflows

### Shared utility change

```bash
pnpm --filter @ym-connect/core test
pnpm --filter @ym-connect/core typecheck
pnpm lint:workspace
pnpm format:check
```

### Protocol fixture change

```bash
pnpm protocol:generate
pnpm protocol:fixtures:check
pnpm generate:check
```

### Extension change

```bash
pnpm lint:workspace
pnpm typecheck
pnpm test:js
pnpm build:js
```

### Bridge change

```bash
pnpm lint:rust
pnpm test:rust
pnpm build:rust
```

### Android change

```bash
pnpm lint:android
pnpm test:android
pnpm build:android
```

### Cross-platform integration change

Run the Extension and Bridge commands locally, then rely on the hosted Ubuntu, macOS, and Windows integration matrix before merge.

### Documentation or infrastructure change

```bash
pnpm format:check
pnpm spellcheck
node .github/scripts/verify-workflow-pins.mjs
node .github/scripts/verify-lockfiles.mjs
```

## Release process

### Release preparation

1. Synchronize the release version in every version-bearing component described by `RELEASE.md`.
2. Update `CHANGELOG.md`.
3. Run `pnpm generate:check`.
4. Run `pnpm check` from a clean checkout.
5. Run `cargo deny check`.
6. Run strict Gradle dependency verification.
7. Trigger the **Release Candidate** workflow for the exact commit.
8. Download the `ym-connect-release-candidate` artifact and verify its `SHA256SUMS`.
9. Perform clean install, repair, upgrade, downgrade rejection, recovery, and uninstall tests on supported platforms.

The release-candidate workflow requires Bridge, Extension, and Android source directories. It builds unsigned artifacts on Ubuntu, macOS, and Windows, generates an SPDX SBOM, produces SHA-256 checksums, verifies the bundle, and retains the candidate without publishing it.

### Tagged release

Create and push a signed tag matching the canonical repository version:

```bash
git tag -s "v$(cat VERSION)" -m "YM Connect v$(cat VERSION)"
git push origin "v$(cat VERSION)"
```

The **Tagged Release** workflow:

1. verifies that the tag exactly matches `VERSION`, `package.json`, and the Cargo workspace version;
2. requires all platform source directories;
3. regenerates and verifies protocol output;
4. executes JavaScript, Rust, and Gradle validation and builds;
5. collects platform artifacts;
6. optionally executes protected platform signing hooks when repository variable `ENABLE_RELEASE_SIGNING` equals `true`;
7. generates the source archive and SPDX SBOM;
8. creates and verifies `SHA256SUMS`;
9. generates GitHub build-provenance attestations; and
10. publishes the immutable assets to a GitHub Release.

Normal continuous integration does not receive signing or notarization credentials. Platform hooks are executed only in tagged release jobs and only when explicitly enabled.

### Artifact verification

Run the **Artifact Verification** workflow for the published tag, or verify locally:

```bash
gh release download "v$(cat VERSION)" --dir release
node .github/scripts/verify-checksums.mjs release
```

Verify GitHub attestations for each downloaded asset:

```bash
find release -type f ! -name SHA256SUMS ! -name SHA256SUMS.sig ! -name SHA256SUMS.pem -print0 | while IFS= read -r -d '' file; do
  gh attestation verify "$file" --repo ym-connect/ym-connect
done
```

The SBOM files must parse as JSON and the checksum verifier must report no missing, additional, unsafe, or modified files.

## Repository maintenance

Maintainers must keep these controls current:

- pinned tool versions and wrapper checksums;
- npm, Cargo, Gradle, and GitHub Actions lock or verification data;
- full-SHA GitHub Action references;
- CODEOWNERS coverage;
- repository labels and issue forms;
- protocol descriptors, generated bindings, and fixtures;
- Dependabot package directories and grouping rules;
- supported runner images and platform matrices;
- release artifact collection paths;
- signing environment protections; and
- security, support, and release documentation.

Synchronize labels after changing `.github/labels.json` by running the **Repository Labels** workflow. The synchronization creates or updates canonical labels without deleting unrelated operational labels.

When a runner image or action major version changes, review Node runtime requirements, runner minimum versions, caching behavior, permissions, and artifact compatibility before merging the update.

## Definition of Done before opening a pull request

A change is ready for review only when all applicable statements are true:

- The change preserves the approved architecture and ownership boundaries.
- Protocol changes are approved, compatibility-reviewed, generated, and fixture-tested.
- No generated file was edited manually.
- `pnpm install --frozen-lockfile` succeeds.
- `pnpm generate:check` succeeds.
- `pnpm format:check` succeeds.
- `pnpm spellcheck` succeeds.
- JavaScript lint, typecheck, tests, and build succeed.
- Rust formatting, Clippy, locked tests, and locked build succeed when Rust is affected.
- Gradle lint, tests, assembly, version validation, and strict dependency verification succeed when Android or JVM models are affected.
- Playwright and browser/native-host integration tests succeed when Extension or Bridge behavior is affected.
- Negative tests cover malformed or unauthorized inputs at changed trust boundaries.
- Lockfiles and dependency-verification metadata contain only intentional reviewed changes.
- `node .github/scripts/verify-workflow-pins.mjs` succeeds for workflow changes.
- `cargo deny check` succeeds for Rust dependency changes.
- Release artifact paths, installation, upgrade, rollback, and recovery effects are tested when packaging changes.
- User-visible changes are documented in `CHANGELOG.md`.
- The working tree contains no unexpected generated, build, test, credential, or machine-local files.
- The pull request template contains the exact commands and evidence used for validation.
