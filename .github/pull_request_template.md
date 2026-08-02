# Pull request

<!-- cspell:words anchore attest attestations bufbuild codeql dependabot macos notarization openssf playwright protoc sarif sbom scorecard spdx syft temurin webkit x86 -->

## Summary

Describe the behavior changed by this pull request and the reason for the change.

## Scope

- [ ] The change is limited to the stated component or infrastructure area.
- [ ] The approved architecture remains unchanged, or an approved architecture decision is linked.
- [ ] The canonical protocol remains unchanged, or the protocol compatibility review is linked.

## Validation

List the exact commands executed and their results.

- [ ] `pnpm generate:check`
- [ ] `pnpm format:check`
- [ ] `pnpm lint`
- [ ] `pnpm typecheck`
- [ ] `pnpm test`
- [ ] `pnpm build`

Mark commands that are not applicable and explain why in the validation notes.

## Compatibility and generated code

- [ ] Generated files are current and were produced by repository commands.
- [ ] Lockfiles are current and were not edited manually.
- [ ] Protocol compatibility was evaluated when schemas, envelopes, capabilities, or persistence formats changed.
- [ ] Browser, Bridge, and Android minimum-version implications are documented.

## Security and privacy

- [ ] Untrusted browser or network input is validated at the boundary.
- [ ] Authentication, pairing, trust storage, IPC, and cryptographic changes include negative tests.
- [ ] No credentials, signing material, personal data, or telemetry were added.
- [ ] Dependency and license effects were reviewed.

## Release impact

- [ ] User-visible behavior is documented in `CHANGELOG.md` when applicable.
- [ ] Installer, extension-store, Android, rollback, or migration effects are described.
- [ ] Release artifacts remain reproducible and verifiable.

## Evidence

Provide logs, screenshots, test reports, or artifact digests needed to review the change.
