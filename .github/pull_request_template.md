# Pull request

## What and why

Describe the defect or gap this closes, then the change. State the behavior
before and after.

## Named invariants

List the invariant IDs from `docs/invariants.md` that this change affects. A
change may not weaken an invariant to preserve a version number.

## Evidence

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --workspace --all-features`
- [ ] `cargo test --locked --workspace --all-features --doc`
- [ ] `cargo llvm-cov --locked --workspace --all-features --fail-under-lines 80`
- [ ] `npm run lint:markdown`
- [ ] `pwsh -NoProfile -File scripts/check-repository.ps1`

## Documentation

- [ ] `docs/current-state.md` updated, or no implemented behavior changed
- [ ] Planned behavior is not described as available
- [ ] No generated-by or co-author attribution, emoji, en dash, or em dash

## Regression fixtures

Every fixed fidelity or structure-preservation defect needs a minimized fixture.
Name it here, or state why none applies.
