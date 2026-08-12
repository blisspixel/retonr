# ADR 0002: Private codename and neutral internal namespaces

- Status: superseded by [ADR 0006](0006-retonr-public-identity.md)
- Decision owners: project maintainers
- Decision gate: private implementation authorization
- Last reviewed: 2026-08-11

## Context

The repository is private and implementation needs to proceed before a final product
name is selected. A premature public name would create avoidable migration and
clearance risk, while blocking all private work would delay product evidence that
can influence the eventual brand.

## Decision drivers

- Continue private implementation without reopening naming work.
- Keep a later rename bounded and reviewable.
- Prevent accidental publication under an uncleared name.

## Options considered

### Block implementation on naming

This avoids temporary presentation strings but couples technical validation to an
unrelated public-brand decision.

### Use a private codename with neutral internal crates

This permits implementation and evaluation while concentrating the future rename in
the executable, packaging, configuration, documentation, and presentation layers.

## Decision

Use `tonr` as a private development codename. Use `rewrite-*` names for internal
library and evaluation crates. Defer candidate-name work completely until the public
release track is reopened.

No public upload, package publication, namespace reservation, installer release, or
domain purchase is implied or authorized by this decision.

## Consequences

### Positive

- Engineering continues without treating a codename as a final brand.
- Internal dependency boundaries survive a presentation rename.
- Public clearance remains explicit.

### Negative

- Documentation and fixtures will require a coordinated rename later.
- The private executable temporarily carries the codename.

### Follow-up

- Keep public release blocked on the naming gate.
- Inventory all presentation and namespace surfaces before the final rename.
- Rerun compatibility, packaging, upgrade, and screenshot checks after renaming.

## Validation

Repository scans must show namespace-neutral library crates and no public release
automation. This decision was superseded when the public `Retonr` identity and
migration were recorded in ADR 0006.

## References

- [Naming status](../naming.md)
- [Versioned roadmap](../roadmap.md)
