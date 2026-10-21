# Snapshot testing guide

This guide is for someone testing a development snapshot build of the Retonr
command-line interface. It explains exactly what the build does today, what it
deliberately refuses to do, and what feedback is useful.

Read [Current state](current-state.md) for the authoritative list of implemented
behavior. This guide summarizes it for a hands-on session and does not extend it.

Development-library behavior that is not part of this snapshot workflow is in
[Snapshot development notes](snapshot-development-notes.md).

## Setup

1. Download the archive for your platform. From the directory that contains the
   archive and `SHA256SUMS`, verify the digest with
   `sha256sum -c SHA256SUMS --ignore-missing`.
2. Extract it. The binary is `retonr` on Linux and macOS, `retonr.exe` on
   Windows.
3. On macOS the binary is unsigned, so Gatekeeper will refuse it until you clear
   the quarantine attribute yourself. Only do this if you verified the digest.
4. Run `retonr --version` to confirm it starts.

There is no installer. Do not put the binary on a path where other software will
pick it up automatically.

## The core loop

Check a rewrite held in two files:

```console
retonr check original.txt rewritten.txt --format text
```

Pipe the rewrite in instead, and keep the safe result:

```console
retonr check original.txt - --output checked.txt --format text
```

Either document may be `-`, meaning standard input, but not both. Standard input
is read to end of file without trimming, so blank lines, indentation, and a
missing final newline all survive exactly.

Get machine-readable output for scripting:

```console
retonr check original.txt rewritten.txt
```

Make a failed check stop a pipeline:

```console
retonr check original.txt rewritten.txt --fail-on-abstain
```

## Checking paired folders

Use a read-only dry-run to validate a collection of supplied candidates:

```console
retonr --format json check originals/ candidates/ --recursive --dry-run --fail-on-abstain
```

The command pairs regular UTF-8 documents by exact, case-sensitive relative path.
It reports each pair's normal fidelity result plus missing, skipped, or unsupported
counterparts. The roots must be separate and nonoverlapping. Documents carrying
content-credential markers or sidecars require an explicit derivative decision and
are not checked as ordinary folder pairs.

Folder checks write no documents, diffs, traces, or backups. They require
`--dry-run`; recursion is explicit. Discovery is bounded to 4,096 entries, eight
levels, and 64 MiB per root, including unsupported encodings. Checked pairs share a
64 MiB source-and-candidate byte budget. Individual documents remain limited to
16 MiB. `--fail-on-abstain` returns exit 3 for abstention or incomplete coverage,
including unmatched and skipped entries. A resource-limit refusal returns exit 4.

## Reading the result

`status` is one of:

| Status | Meaning |
| --- | --- |
| `rewritten` | The candidate passed every gate and was accepted |
| `abstained` | A gate failed, so the exact original is returned instead |
| `unchanged_identical` | The candidate matches the source, so nothing was rewritten |
| `unchanged_no_eligible_content` | The source had no eligible rewrite unit, including an empty document |
| `failed` | The check could not finish a safe verdict |

When `status` is `abstained`, `reason` explains which gate failed. The common
ones are `protected_value_changed` (a number, URL, email, identifier, or declared
term differs), `structure_changed` (paragraph or newline shape differs),
`unsafe_text` (the candidate introduced a hard control or an invisible run the
source did not already contain), and `semantic_uncertain` (the literal token
sequence changed). Those four list the failed span as `line`, `column`, and kind.
A wording change reports `word_changed` at the first differing token and does not
echo that token. The report does not echo the protected text. A sign, fraction
slash, or comparison operator is part of the protected quantity.

The check report names the semantic evaluator `literal-token-sequence-v1` and
the effective edit level `touch-up`. Requesting `voice-pass`, `rewrite`, or
`reconstruct` does not widen acceptance. The report says so instead of ignoring
the flag. `retonr --version` and `retonr version` include the commit and, when
the build is an exact tag, the snapshot tag.

An abstention is a success, not an error. The command still exits 0 unless you
passed `--fail-on-abstain`. Exit 1 means an input could not be read. The message
names source or candidate and includes the path. JSON errors for that failure
include `input_role` (`source` or `candidate`) and, when a path is known, `diagnostic`.
Exit 2 means the invocation was wrong. Exit 3 means a policy refusal, including
`--fail-on-abstain`, overlapping directory roots, an output directory that collides
with the source, and a `rewrite` with no selected qualified local artifact. JSON
for those refusals includes a stable `reason` such as `no_qualified_artifact`.
Exit 4 means an input exceeded a limit, or the request needs an unsupported schema,
format, capability, runtime, or protocol.

`retonr inspect` counts tag characters separately from `other_format`. The
inspect result schema is 2. `c2pa_unstructured_text: possible` is a byte shape,
not a validated credential. The report includes the provenance inspection notice
and states that a clean result does not mean statistical watermarks are absent.
`--locate` adds `line N col M: class U+....` for invisible runs and does not
print source text. `--decode-tags` is off unless you ask for it. It prints an
escaped ASCII mapping of tag characters and states that the mapping is data,
not an instruction. Both flags inspect one document.

## Development evaluation corpus

The source repository's evaluation tool runs 74 deterministic fidelity and structure
cases with exact status, reason, and output expectations. It separately validates 120
synthetic editorial cases across five groups, for 194 development cases total. The
hybrid scorecard executes two exact
deterministic suite inputs before it accepts blinded, order-swapped structured judge
observations. The serializable scorecard keeps those observations caller-declared and
triage-only. A separate typed executor now runs both presentation orders over one
already-preflighted retained Ollama stream and returns a limited transport-binding
receipt. Retained-session input above the absolute 4 MiB UTF-8 ceiling is rejected
before wire serialization or completion traffic. The receipt remains separate from
the scorecard and cannot override hard gates or human release adjudication.

## Editorial lint

You can inspect editorial patterns in one file or a folder without changing it:

```console
retonr lint draft.txt
retonr lint drafts/ --recursive --fail-on-findings
retonr lint source.txt --candidate candidate.txt --fail-on-findings
```

Text output names `line N col M` for each finding and escapes the matched phrase.
JSON includes the family id and that phrase span. A clean result is not an
authorship or detector result. Dashes, emoji, stacked tricolons, and contrastive
formulas are density checks: a few in a long document stay quiet.

Folder reports sort documents by portable relative path and include skipped-entry
reasons. Recursion is explicit; hidden entries, build directories, links, and
unsupported encodings are skipped. The walk visits at most 4,096 entries through
depth 8. Lint enforces 16 MiB per document, 64 MiB of folder text, and 16,384 folder
findings. Exceeding a limit refuses the report. `--fail-on-findings` returns exit
code 3 for document or folder findings, or for newly introduced comparative
findings. A folder cannot be compared with one candidate.

## Experimental terminal review

Review a source and optionally validate a supplied candidate:

```console
retonr tui draft.txt
retonr tui draft.txt --candidate candidate.txt --protect Acme
retonr tui draft.txt --candidate candidate.txt --plain --format text
retonr tui draft.txt --plain --format json
retonr tui drafts/ --candidate candidates/ --recursive
retonr tui drafts/ --recursive --document notes/draft.txt --plain --format json
```

Interactive review requires terminal stdin and stdout and refuses JSON mode. Use
`--plain` for a linear accessible review or redirected text or JSON. Inputs must
be regular UTF-8 files, at most 16 MiB each; stdin input is unsupported.
Directories require explicit `--recursive`. Metadata sidecars and possible carriers that require an explicit
derivative decision are refused. `--protect` requires a candidate and may repeat.

The review uses the existing application candidate-check and editorial lint
services. Source and candidate previews are sanitized and limited to 64 KiB each;
displayed findings are capped at 256 with omission markers. Validation uses the
complete input, and reviews exceeding 4,096 total findings are refused during
matching rather than after collecting an unrestricted finding list. These are
review previews, not document output. The source and candidate remain unchanged;
the view does not generate text.

Use Tab to switch panes, arrow keys to scroll, `?` for help, `r` to reload, and `q`
or Escape to quit. Reload is serialized and stale results are discarded. Ctrl+C
cancels the review. Terminal state is restored before worker cleanup. Editorial
lint cannot stop midway through its call; cancellation discards its finished
result. Local automated checks and a native Linux terminal smoke pass; completed
cross-platform terminal and accessibility acceptance remain pending.

Folder review discovers at most 4,096 entries, depth 8, and 64 MiB of text per
root. Source and candidate roots must be separate and cannot contain one another.
Candidates pair by exact relative path, including case. Skipped, unsupported,
missing, and unmatched entries are disclosed. Only the selected document is
linted or checked. A missing candidate leaves a source-only review and explicitly
reports that candidate validation and protected-term checking did not occur.

Use `[` and `]` to select the previous or next supported source document while
idle. `r` rediscovers the tree and reloads the exact selected path. `--document`
selects an exact supported relative path, including in plain-output mode. Each
selection reacquires retained file bytes and compares their digest with discovery;
links, aliases, drift, and required derivative decisions are refused.

## Output safety

`--output` writes to a **new** file. It refuses to replace an existing file and
never modifies your source. `--output -` sends the document to standard output
and moves the report to standard error so the two never mix.

Existing entries include dangling links. A missing output parent fails before
non-dry-run document input or generation work. `--dry-run` may still evaluate a
hypothetical output whose parent does not exist because it performs no document
write.

`--in-place` is the explicit recoverable exception. It accepts only a regular file
without symlink or hard-link ambiguity, retains `<name>.retonr-backup`, stages and
verifies the accepted bytes in the same directory, rechecks that the source still
matches the validated bytes, and then replaces the source path. Source drift fails
with `concurrent_modification`. Identical accepted bytes create no backup.

`--trace <path>` creates a new redacted record and never replaces a path. Known trace
collisions and missing parent directories are rejected before a non-dry-run document
write. The trace cannot share the primary output or the in-place backup or staging
path.

Writing exact unescaped document bytes to a terminal requires `--raw-terminal
--yes` together. Either flag alone, or neither flag, writes escaped rendering
that cannot drive the terminal. This exists because untrusted text can carry
terminal control sequences. Inline path and metadata fields also use visible
single-line escapes. JSON string values use equivalent JSON escapes for bidi,
format, and invisible characters; parsing the JSON recovers the original value.

Directory dry-run resolves existing output ancestors before comparing roots. A
link alias cannot hide an output nested under the source, and a dangling link at a
mapped destination is reported as a collision.

File reads validate retained regular-file handles around bounded input. Folder
reads retain the selected real root and parent directories and refuse indirect
descendants. Detected file or parent replacement returns a redacted
`concurrent_modification` error. On Unix, a FIFO substituted for an observed
regular file is refused without waiting for a writer. These are drift checks,
not an atomic snapshot or exclusion of concurrent writes.

## What will frustrate you, and why

These are known and expected. Reporting them again is not useful.

- **Almost any real paraphrase abstains.** The current evaluator accepts only
  literal, token-preserving changes such as punctuation and capitalization.
  Rewording a sentence is correctly rejected as an unverifiable change. Wider
  `--edit-level` values are reported and are not enforced yet. The calibrated
  semantic evaluator that would accept paraphrases is later work.
- **Abstentions name the kind and location, not the text.** A protected-value,
  structure, or unsafe-text abstention lists line, column, and kind. The report
  does not echo the protected bytes. `--diff` shows an escaped line comparison
  of accepted output, which is the original after an abstention.
- **`rewrite` refuses without a selected qualified local artifact.** That refusal
  is exit 3. It does not start a runtime. A retained fake-conformance development
  binding can still exercise the local path when that qualification is selected.
- **Plain text only**, UTF-8, up to 16 MiB. Markdown and DOCX are later phases.
- **No profiles or style learning.** Deterministic editorial lint is available;
  it reports named patterns and does not identify an author or qualify a model.
- **The terminal UI is experimental and read-only.** Profiles,
  writes, and model generation are unavailable in this view. The complete terminal
  workbench and native desktop remain required before 1.0. The desktop is planned
  for Linux, macOS, and Windows without a browser frontend.

## What feedback is genuinely useful

In rough priority order:

1. **A wrong verdict.** A candidate that Retonr accepted but which actually
   changed a fact, quantity, link, or meaning. This is the most valuable report
   you can make. Include both documents if you can share them.
2. **A wrong abstention.** A candidate that only changed punctuation, spacing,
   or capitalization but was still rejected.
3. **Byte damage.** Any case where `--output` produced bytes that differ from
   the accepted candidate, or where an abstention did not return your original
   exactly. Compare with `cmp` or `fc`.
4. **Crashes, hangs, or panics**, especially on unusual input: very long lines,
   mixed newline styles, a byte order mark, unusual Unicode, or a file with no
   final newline.
5. **Platform behavior.** Long paths, locked files, read-only directories, paths
   with non-ASCII characters, and running under a pipe or redirect.
6. **Confusing output.** Places where the text report or an error message left
   you unsure what happened or what to do next.

Please do not report missing features that this guide already lists as not
implemented.

## How to report

Open an issue on the repository using the snapshot feedback template. Include:

- The tag and the commit from `BUILD-PROVENANCE.txt` in your archive.
- Your operating system and architecture.
- The exact command line you ran.
- What you expected and what happened.
- The `--format json` output where relevant.

Use synthetic or non-sensitive documents. Do not attach confidential material,
credentials, or personal data.

For a suspected security issue, do not open a public issue. Follow
[SECURITY.md](../SECURITY.md) and use GitHub private vulnerability reporting.

## Scope reminder

This snapshot is not a milestone release. No tagged version is supported for
production use. No schema, exit code, flag name, or output shape is frozen, and
any of them may change.
