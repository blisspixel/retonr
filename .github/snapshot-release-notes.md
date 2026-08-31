# Development snapshot

This is a development snapshot for hands-on testing. It is **not** a milestone
release under the version policy in
[the roadmap](https://github.com/blisspixel/retonr/blob/main/docs/roadmap.md).
Milestone 0.2 is still in progress, and no milestone has been released.

## Changes in this snapshot

- Adds the schema-10 durable candidate-execution foundation: pretraffic checkpoints,
  atomic completed-or-failed persistence at trustworthy closeout boundaries,
  application-root evidence publication, bounded cold readback, and in-transaction
  reacquisition.
- Makes the Active generation-qualification operation own candidate publication,
  readback, receipt closeout, failed-attempt derivation, exact attempt-ledger closure,
  and the candidate-before-judge gate. These remain internal development boundaries
  and grant no qualification or model-use authority.
- Runs retained Linux executables by descriptor, including when `/proc` is private or
  mounted `noexec`, while preserving logical argument identity and deterministic
  environment validation.
- Tightens Linux managed and controlled-build isolation with inherited seccomp policy,
  namespace-bearing clone denial, bounded file-descriptor behavior, and an active
  canary around the one anonymous process-launch socket pair required by the pinned
  Rust launcher.
- Adds forced native Linux CI coverage for managed launch and controlled build,
  including the exact 64-descriptor boundary. The live native-closure observer keeps
  its narrow checkpoint-restore capability outside the zero-capability worker. The
  same gate corrects cross-platform lint and fixture line-ending failures.
- Updates current-state, planning, and roadmap documentation. The next implementation
  slice is bounded read-only reconciliation of durable candidate-attempt state before
  activation.

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
bytes, or the exact original after an abstention, to a new file.

`retonr rewrite` runs the current model-free rewrite transaction. `retonr inspect`
performs pre-model source inventory. `retonr model` administers exact local model
artifacts offline. These commands do not download, qualify, activate, or run a
model.

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
