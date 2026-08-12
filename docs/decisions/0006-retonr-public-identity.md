# ADR 0006: Retonr public project identity

- Status: accepted
- Decision owners: project maintainers
- Decision gate: public source repository authorization
- Last reviewed: 2026-08-12
- Supersedes: [ADR 0002](0002-private-codename.md)

## Context

Private implementation used `tonr` as a temporary codename while product scope and
naming criteria were refined. The project now needs a coherent public repository,
executable, documentation identity, and local checkout path without coupling the
internal architecture to a brand.

The product locally reconstructs machine-generated and rough drafts through an
evidence-backed user style profile. It minimizes supported source-form signals and
handles supported document artifacts while preserving intent and declared
constraints. The name must describe re-expression without claiming untraceability,
detector evasion, human authorship, or universal provenance removal.

## Decision drivers

- Use a pronounceable name with an understandable relationship to the product.
- Match the maintainer's compact naming pattern without inventing an opaque word.
- Support a clean executable and repository namespace.
- Remain broad enough for CLI, desktop, API, MCP, skills, profiles, and voice flows.
- Keep internal crate boundaries independent from presentation branding.

## Options considered

### Keep the `tonr` codename

This avoids an immediate rename but retains a name with known active AI product
collisions and no longer reflects the selected public identity.

### Use an obvious rewrite or reclaiming term

Names based on reframe, redraft, reclaim, and similar common verbs were easier to
explain but had direct product, repository, domain, or trademark conflicts.

### Use `Retonr`

`Retonr` reads as "re-tone-er": a tool that returns a draft to the user's tone. It
preserves continuity with the private codename, matches the maintainer's naming
pattern, and passed the preliminary exact-namespace and confusing-use screen.

## Decision

Use `Retonr` as the public project and product identity. Use lowercase `retonr` for
the GitHub repository, executable, public-facing package, command examples, and
future configuration namespaces unless a later compatibility decision requires a
qualified exception.

Keep internal library crates under the neutral `rewrite-*` namespace. Rename the
local checkout to `C:\GitHub\retonr` and publish the source repository at
`https://github.com/blisspixel/retonr`.

The preliminary screen authorizes public source publication. It is not a legal
opinion. Package, installer, application-store, hosted-service, and 1.0 publication
remain gated on formal legal review and a fresh namespace check.

## Consequences

### Positive

- The name explains the personal-tone outcome without overclaiming provenance.
- Repository, command, and product spelling are consistent.
- Neutral internal crates avoid unnecessary architectural churn.
- The public repository can develop in the open under one coherent identity.

### Negative

- The name alone does not explain document artifact handling or fidelity gates.
- Some users may need to see the pronunciation once.
- Formal clearance and namespace reservation remain future release work.

## Validation

- Repository scans contain no active `tonr` identifier or private-codename copy
  outside the superseded decision history.
- The workspace package and binary are named `retonr-cli` and `retonr`.
- Formatting, linting, tests, coverage, supply-chain, and repository-policy checks
  pass after the migration.
- The GitHub repository is public at the selected URL and the local checkout uses
  the selected directory.

## References

- [Naming status](../naming.md)
- [Current implementation state](../current-state.md)
- [Versioned roadmap](../roadmap.md)
