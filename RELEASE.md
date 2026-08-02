# Release Process

## Preconditions

A release is produced from a protected tag on the default branch. The working tree must be
clean, generated outputs must match their schemas, lockfiles must be current, and all required
CI checks must pass on the release commit.

## Versioning

YM Connect uses Semantic Versioning for the repository release. Protocol packages, connector
APIs, adapter APIs, persisted stores, installers, and applications also carry explicit
compatibility versions. A repository version increase does not bypass their independent
compatibility rules.

Synchronize the release version in `VERSION`, `package.json`, the Cargo workspace, Android
version metadata, extension manifests, native-host manifests, installer metadata, and the
changelog. The verification task rejects inconsistent versions.

## Release validation

Run from a clean clone:

```bash
corepack enable
pnpm install --frozen-lockfile
pnpm generate:check
pnpm check
pnpm build
pnpm test
cargo deny check
./gradlew --no-daemon lint test assemble
```

Release CI additionally performs cross-platform installer tests, browser end-to-end tests,
Android managed-device tests, protocol compatibility tests, fuzzing, dependency review,
license scanning, secret scanning, and recovery scenarios.

## Artifacts

Publish only CI-produced artifacts associated with the protected tag:

- Bridge daemon and native-host binaries for each Tier 1 target;
- signed desktop installers and checksums;
- Chromium and Firefox extension packages;
- Android application bundle and APK checksums;
- generated protocol descriptor set and compatibility fixtures;
- source archive;
- CycloneDX or SPDX software bills of materials;
- dependency and license reports; and
- SLSA-compatible provenance attestations.

## Signing

Tags are signed. Desktop installers, macOS bundles, Windows binaries, Android artifacts, and
published checksums use platform-appropriate signing identities held in protected release
environments. Signing credentials are never available to pull-request workflows.

## Publication order

1. Publish Bridge binaries and installers without enabling automatic rollout.
2. Verify clean installation, repair, upgrade, downgrade rejection, and uninstall.
3. Publish browser extensions after their minimum compatible Bridge version is available.
4. Publish Android after the minimum compatible Bridge version is available.
5. Enable staged rollout and monitor local crash and user-reported compatibility signals.

YM Connect does not collect telemetry. Release monitoring relies on CI, store health reports,
and voluntary user reports.

## Rollback

A rollback never reuses a tag or artifact filename. Revoke affected signing or trust material
when required, publish a higher patch version containing the last known-good implementation,
and document minimum safe versions and upgrade order. Persisted-store migrations must have a
validated rollback or forward-repair path before release.
