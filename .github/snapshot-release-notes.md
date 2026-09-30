# Development snapshot

This is a development snapshot for hands-on testing. It is **not** a milestone
release under the version policy in
[the roadmap](https://github.com/blisspixel/retonr/blob/main/docs/roadmap.md).
Milestone 0.2 is still in progress, and no milestone has been released.

## Changes in this snapshot

- Adds optional layout bounds on `retonr check`: an edit level, a character budget,
  and an expansion ceiling. These gates decide acceptance before a candidate is
  written. They do not qualify a model.
- Adds `retonr lint` for single-document or comparative inspection of conversational
  residue, cliches, and synthetic patterns. Findings do not create release
  qualification evidence.
- Reconciles stored candidate attempts before Prepared activation. Only an entirely
  pristine plan may activate. The check does not repair, delete, or fabricate
  evidence.
- Persists the local generation-qualification store through schema 19. The store
  writes terminal evidence, one judge-execution cohort, one phase-policy denial,
  one resource-attempt result, one receipt set, one deterministic evaluation, one
  repeatability terminal result, one qualification record, one invalidation, and
  one selection. Each write is its own immediate transaction. A stored row grants
  no qualification, activation, or live-use authority.
- Updates current-state, planning, roadmap, and readme documentation. The final
  qualification compiler, activation decisions, and active generation bindings are
  not implemented. No model and runtime combination is qualified.

## What these artifacts are

- Unsigned and unnotarized. There is no code signature, no notarization, no
  build attestation, and no software bill of materials.
- Not the documented distribution path. The planned bootstrap installers
  described in
  [Installation and distribution](https://github.com/blisspixel/retonr/blob/main/docs/distribution.md)
  are not published, and nothing here should be treated as an installer.
- Not a stable channel. No published pointer resolves to this tag.
- Built and smoke-tested on the three targets listed below. **This is not a
  support claim.** The Windows and Linux Arm64 rows and the macOS Intel slice in
  the release target matrix have no evidence here.
- The Linux archive also contains `retonr-isolation`, an internal
  managed-runtime helper. It is packaged for exact-byte review and is not a
  standalone user command or evidence that model-backed rewrite is available.

Verify the SHA-256 digest of any asset you download against the list below
before running it.

## Built targets

| Target | Runner |
| --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` |
| `aarch64-apple-darwin` | `macos-latest` |
| `x86_64-pc-windows-msvc` | `windows-latest` |

## What the binary does today

`retonr check` validates a complete candidate rewrite against a source document
without using a model. It reports whether the candidate preserved protected
values, structure, and literal token content, and it can write the accepted
bytes, or the exact original after an abstention, to a new file. Optional edit
level, character budget, and expansion ceiling flags enforce layout bounds.

`retonr lint` inspects one document, or compares it with a candidate, for
conversational residue, cliches, and synthetic patterns. `retonr rewrite` runs
the current model-free rewrite transaction. `retonr inspect` performs pre-model
source inventory. `retonr model` administers exact local model artifacts offline.
These commands do not download, qualify, activate, or run a model.

Model-backed rewrite, managed runtime execution, profiles, agents, and the desktop
application are not exposed by this binary. See
[Current state](https://github.com/blisspixel/retonr/blob/main/docs/current-state.md),
which is the only authority for implemented behavior.

## Known limits

- `check` and `rewrite` accept only UTF-8 plain-text documents up to 16 MiB.
- The candidate must be a complete replacement document, not a patch.
- The current evaluator accepts only literal, token-preserving changes.
  Open-domain paraphrases abstain by design.
- No public API, schema, package, executable name, or configuration namespace
  is frozen. Any of them may change without a migration in `0.x`.
- Unkeyed content digests in reports are identifiers, not anonymization.

## What this software does not claim

Retonr does not erase upstream provider records, prove human authorship, defeat
any classifier or detector, or satisfy an external disclosure obligation.

## Reporting

Read
[the snapshot testing guide](https://github.com/blisspixel/retonr/blob/main/docs/testing-snapshot.md)
before filing anything. For a suspected security issue, follow
[SECURITY.md](https://github.com/blisspixel/retonr/blob/main/SECURITY.md) and use
private vulnerability reporting rather than a public issue. No tagged version is
supported for production use.
