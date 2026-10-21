# Development snapshot

This is a development snapshot for hands-on testing. It is **not** a milestone
release under the version policy in
[the roadmap](https://github.com/blisspixel/retonr/blob/main/docs/roadmap.md).
Milestone 0.2 is still in progress, and no milestone has been released.

## Changes in this snapshot

- `retonr --help` states that Retonr is a local-first copy editor. It makes small,
  checked copy edits toward the writer's own style. It adds no watermark, tracking,
  or attribution, and it makes no claim about detectors, authorship, or disclosure.
- `retonr inspect` counts tag characters separately from other format marks.
  The inspect result schema is 2. `--locate` reports line, column, and escaped
  code points for invisible runs. `--decode-tags` is off unless requested and
  prints an escaped ASCII mapping, marked as data rather than an instruction.
  `possible` on the unstructured text carrier is a byte shape, not a validated
  credential. Every inspect report includes the provenance notice and states
  that Retonr cannot detect statistical watermarks in word choice.
- `retonr check` rejects a candidate that adds or lengthens a retained invisible
  run the source did not already contain. That includes tag characters, variation
  selectors, the Mongolian vowel separator, and the other inventoried format marks
  that are not already hard-unsafe. Deleting such a run is still allowed. A source
  that already contains a hard control, bidi mark, or zero-width joiner must stay
  byte-identical.
- A `semantic_uncertain` abstention names the first changed literal token as
  `line N col M: word_changed altered` (or `missing` or `added`) and does not echo
  the token. `--diff` still compares the source with the accepted output, so an
  abstention remains `diff: unchanged`.
- JSON errors for an unreadable check or lint input include `input_role`. Policy
  refusals for a missing qualified artifact, overlapping directory roots, and an
  output directory that collides with the source include a stable `reason`.
- A changed quantity now abstains with `protected_value_changed`. A dropped or
  flipped sign, a unicode minus, a dropped currency sign, a removed fraction
  slash, and a flipped comparison are protected. Decimal points, percent signs,
  range hyphens, and collapsed fractions still abstain. Unrelated punctuation
  is not a protected quantity. The report names `line N col M: kind change`
  and does not echo the protected text.
- `retonr model device-evidence` accepts a fitr export whose need state is
  `INCONCLUSIVE`. Malformed evidence, an unknown state, and a dishonest
  disclaimer still fail.
- `retonr check --edit-level` reports `semantic_evaluator:
  literal-token-sequence-v1` and `edit_level_effective: touch-up`. A wider
  requested level notes that only punctuation and capitalization are accepted.
- `retonr rewrite` without a selected qualified local artifact exits 3 as
  `policy_refusal` and does not start a runtime. Identical accepted text is
  `unchanged_identical`. `--version` includes the commit and snapshot.
- `retonr lint` on an unreadable source or candidate now attaches a diagnostic
  naming the path and reports whether the source or candidate failed to read,
  matching `retonr check`.
- Managed Linux process observation tests use deterministic descriptor duplication
  when verifying closed diagnostics rejection, eliminating test race conditions
  during parallel test execution.
- Adds an experimental read-only terminal workbench: `retonr tui SOURCE`, with
  optional `--candidate FILE` and repeated `--protect TERM`. The three panes show
  findings, source, and candidate. `--plain` provides bounded text or JSON previews
  for redirected output and accessible terminals. Validation uses complete inputs;
  previews are truncated and escaped for safe display.
- Serializes terminal rendering with panic cleanup so a background panic cannot
  let a later frame hide the restored cursor.
- Regenerates private bootstrap source archives within the frozen 4,096 descriptor
  limit, with canonical readback and mutation checks. Public mutable-input
  normalization retains its existing held-file contract.
- Includes complete third-party license and notice materials for each platform's
  locked default shipping dependencies and the Rust standard library.
- Adds bounded recursive directory lint and read-only paired-directory candidate
  checks, with explicit inventory limits and per-file findings.
- Hardens retained-file reads against special files, ancestry replacement, and
  concurrent changes, and rechecks staged bytes before publication.
- Fixes controlled bootstrap assembly for the pinned Rust distribution archives:
  installer metadata stays separate while validated manifest-declared payloads
  merge into the toolchain. The actual archives pass the production extractor.
- Adds durable Active receipt, deterministic evaluation, and managed judge-cohort
  persistence. Adds cleanup-gated runtime observations, exact-source evidence
  assembly and publication, and an inert all-pass admission review compiler.
  These internal bridges do not admit a production runtime or qualify a model.
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
- Adds terminal generation qualification closeout for Passed and PolicyDenied
  outcomes and compiles the eval-owned `VerifiedGenerationQualification` authority
  from fresh durable readbacks and live authorities.
- Updates current-state, planning, roadmap, and readme documentation.
  Activation decisions and active generation bindings are
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
bytes, or the exact original after an abstention, to a new file. A changed
quantity abstains. Optional edit level, character budget, and expansion ceiling
flags enforce layout bounds.

`retonr lint` inspects one document, or compares it with a candidate, for
conversational residue, cliches, and synthetic patterns. `retonr rewrite` runs
the current model-free rewrite transaction and exits 3, without starting a
runtime, when no qualified local artifact is selected. `retonr inspect` performs pre-model
source inventory. `retonr model` administers exact local model artifacts offline.
These commands do not download, qualify, activate, or run a model.

`retonr tui` presents existing candidate validation and editorial lint in a
read-only terminal view. Interactive mode requires terminal input and output;
use `--plain` for redirected output. It does not generate or write documents.

Model-backed rewrite, managed runtime execution, profiles, agents, and the desktop
application are not exposed by this binary. See
[Current state](https://github.com/blisspixel/retonr/blob/main/docs/current-state.md),
which is the only authority for implemented behavior.

## Known limits

- Single-file `check`, `rewrite`, and `tui` accept only UTF-8 plain-text documents
  up to 16 MiB each. TUI accepts regular files and refuses stdin and directories.
- Recursive lint and paired-directory checks are read-only, bounded operations;
  they do not provide multi-document mutation or recovery transactions.
- The candidate must be a complete replacement document, not a patch.
- The current evaluator accepts punctuation and capitalization at touch-up.
  A changed quantity abstains. Open-domain paraphrases abstain by design.
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
