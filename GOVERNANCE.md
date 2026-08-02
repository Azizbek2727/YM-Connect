# Governance

## Maintainer responsibilities

Maintainers protect protocol compatibility, security invariants, release integrity, and the
frozen architecture. They review changes, operate coordinated disclosure, approve releases,
and document decisions that affect users or contributors.

A release requires approval from at least two maintainers. A security-sensitive release
requires one approver who did not author the security change.

## Decision classes

### Implementation decisions

Ordinary implementation decisions may be approved through pull-request review when they do
not alter a frozen contract. Reviewers evaluate correctness, test coverage, bounded resource
use, platform behavior, and maintainability.

### Compatibility decisions

Changes to public APIs, generated bindings, persisted data, installer behavior, or supported
platforms require a written compatibility analysis and two maintainer approvals.

### Frozen-contract decisions

Architecture, protocol semantics, validation rules, and roadmap sequencing may change only
when a critical implementation issue is demonstrated. The proposal must include:

1. a reproducible failure or security defect;
2. evidence that the frozen design cannot be implemented safely or correctly;
3. affected platforms and released versions;
4. alternatives considered within the frozen design;
5. wire, storage, API, installer, and operational compatibility impact;
6. migration and rollback procedures; and
7. new tests that prevent recurrence.

A frozen-contract decision requires unanimous approval from active maintainers who are not
recused. The decision is recorded as an architecture decision record in `docs/adr/` before
implementation merges.

## Security conflicts

A maintainer with a personal, employment, or financial conflict must disclose it and recuse
from the affected decision. Security reporters may request an alternate reviewer.

## Release authority

Only maintainers designated in repository settings may create protected tags or approve
production publishing environments. Release artifacts are built by CI from a protected tag,
include software bills of materials and provenance attestations, and are signed according to
[RELEASE.md](RELEASE.md).

## Maintainer changes

New maintainers require sustained, high-quality contributions and two maintainer approvals.
Removal for inactivity is administrative and reversible. Removal for misconduct follows the
Code of Conduct enforcement process.
