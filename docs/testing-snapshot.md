# Snapshot testing guide

This guide is for someone testing a development snapshot build of the Retonr
command-line interface. It explains exactly what the build does today, what it
deliberately refuses to do, and what feedback is useful.

Read [Current state](current-state.md) for the authoritative list of implemented
behavior. This guide summarizes it for a hands-on session and does not extend it.

## What you are testing

Retonr is a local-first editorial engine. The snapshot's primary useful workflow is
**fidelity checking**. It also exposes pre-model source inspection, deterministic
editorial lint, a provisional `rewrite` command that does not start a runtime, and
offline artifact administration. The experimental read-only TUI presents source
inspection, lint, and supplied-candidate checking together.

You bring two documents. The first is your original. The second is a complete
rewritten version of it, produced by any tool or by hand. Retonr answers one
question: did the rewrite change anything that was supposed to stay fixed?

It checks quantities, URLs, email addresses, identifiers, declared protected
terms, document structure, newline shape, and unsafe control characters. If the
rewrite preserved all of them, Retonr reports `rewritten` and can write the
accepted bytes out. If it did not, Retonr reports `abstained` with a reason and
gives you back the exact original bytes instead.

The snapshot does not generate text. It never contacts the network.

The Linux archive also contains the internal `retonr-isolation` helper so its exact
bytes can participate in runtime-package review. The snapshot CLI does not invoke
that helper, and the helper is not a standalone user command.

Each archive includes `THIRD-PARTY-NOTICES.txt` with complete upstream legal
materials for its locked default shipping dependencies and Rust standard library.
Crate archives are checked against their locked checksums. Retained upstream
materials cover declarations omitted from published crate archives; their exact
source, version, and content digest are pinned in the repository.

The source repository also contains a development-only
`rewrite-eval --ollama-bound-preflight` command. It is not part of this snapshot CLI
workflow. When a developer explicitly supplies a versioned plan, that command may
contact only its IP-literal loopback Ollama endpoint. It sends the complete read-only
preflight over one directly connected retained HTTP/1 transport and checks native
connection attribution before traffic and after every fully drained response. It
does not generate, acquire, activate, qualify, or authorize a model. macOS refuses
the command before HTTP. Successful Windows and Linux reports remain unqualified and
explicitly do not prove exclusive socket ownership or application-handler execution.
Linux selects socket rows through bounded `NETLINK_SOCK_DIAG` and still depends on a
complete visible same-UID descriptor view. Within that view, the holder scanner
retains the proc root, anchors each process with a pidfd, strictly parses the bounded
effective-UID status records, and inspects descriptor links relative to the held
process directory. A second anchored status read rejects effective-UID drift across
descriptor inspection. Access denial or an incomplete view fails closed.

Linux development libraries also implement managed user, network, and PID namespace
isolation, retained-handle launch, loopback-only transport, namespace-local process
attestation, and exact native-load observation when host policy permits. The managed
target inherits a seccomp socket allowlist before launch: `socket()` permits only
`AF_INET` and `AF_INET6`, every other socket family and `io_uring_setup` are denied,
and target reobservation requires seccomp mode 2. A development-only `rewrite-eval`
library API joins those boundaries with a retained runtime package, exact connection
evidence, the cloud-disable declaration and startup marker, and read-only Ollama
observation. Its inert report has no CLI surface and explicitly does not prove model
use, effective-runtime identity, or qualification. Windows managed isolation and
exact native-load binding are unsupported. macOS managed isolation, attached
attribution, and native-load binding are unsupported. The provider cloud-disable
contract has an empty production reviewed-runtime allowlist, so the managed report
remains unreviewed.

The repository's Linux CI does not treat an uncontrolled worker as proof of this
native boundary. Ordinary tests may accept only a typed access-denied compatibility
outcome when host proc policy blocks observation. Linux CI uses three separate native
proof gates. The managed attestor runs networkless as the caller UID with all
capabilities dropped and no-new-privileges set. The statically linked worker fixture
runs networkless with only `CAP_SETPCAP` and `CAP_CHECKPOINT_RESTORE` on its root test
process. The latter permits exact `/proc/<pid>/map_files` observation. The fixture
drops both capabilities before worker execution and requires the exact worker, native closure,
private GGUF mapping, stable reobservation, and post-exit rejection path. The
controlled source-build fixture runs in a privileged, networkless container and requires native success for two
namespace-private quota-bounded tmpfs attempts, descendant drain, helper tree
commitment, verified host export, independent application rehash, portable original
path and ordinary Unix permission-bit commitment, special-bit rejection, durable
publication, reacquisition, and blocked schema-2 review. All three executions contribute to the
workspace LLVM profile before the 80 percent line floor is checked.

An opt-in managed-preflight library call can return the unchanged report with a
separate inert package-declared typed runtime-build binding. Only the exact entrypoint
is joined to managed process and native-load evidence; target, revision, and other
package semantics are not independently live-observed. Cleanup is complete before
return; the process is not retained, no effective state is constructed, and model
use, handler execution, and qualification remain false.

Other development-only libraries can bind one inert installed model package to one
exact verified idle Ollama v0.32.15 inventory and details observation, and can run the
neutral local-judge contract over an already-preflighted retained stream. These are
not snapshot CLI commands. The static binding consumes an opaque, nonserializable,
single-use receipt issued by the exact preflight runner and leaves model loaded, model
used, handler, effective identity, and qualification false. Retained-session input is
limited to an absolute 4 MiB of UTF-8 before wire serialization or completion
traffic. The judge executor emits a separate nonserializable transport receipt, but
that receipt does not prove managed isolation, handler execution, model load or use,
candidate generation, effective identity, semantics, or qualification.

The retained Ollama session also has an opt-in v0.32.15 completion profile that
requires two equal singleton runtime memory reports after generation. Its separate
nonserializable receipt proves stable runtime-reported residency on that transport
only. Runtime memory size is not package inventory size, and the receipt does not
prove handler execution, model use, resident-page identity, effective identity, or
qualification. The legacy completion path is unchanged.

An admission-gated development library operation composes that profile with the
managed Linux process, runtime package lease, native observer, direct connection,
static model binding, and post-generation revalidation. It keeps the immutable model
artifact digest distinct from the mutable Ollama inventory digest and records the
runtime-reported effective context length. The exact runtime must have a reviewed
production cloud-disable disposition before launch. Because the allowlist is empty,
this snapshot blocks the operation before generation. It has no CLI surface and does
not prove model weight use, handler execution, complete effective identity, semantic
correctness, or qualification.

The same snapshot also administers exact local model artifacts offline. These
commands copy, inspect, migrate, or remove local files. They do not download,
qualify, activate, or run a model:

```console
retonr --data-dir <DIRECTORY> model import <ARTIFACT> --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model import-set <SOURCE_ROOT> --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model inventory
retonr --data-dir <DIRECTORY> model inventory-set
retonr --data-dir <DIRECTORY> model pending-operations
retonr --data-dir <DIRECTORY> model migrate --yes
retonr --data-dir <DIRECTORY> model reconcile --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model reconcile-set --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model remove --artifact-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model recover-removal --artifact-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model remove-set --artifact-set-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model recover-set-removal --artifact-set-id <SHA256> --installation-generation <N> --yes
```

## Setup

1. Download the archive for your platform and verify its SHA-256 digest against
   the value in the release notes.
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
| `unchanged_no_eligible_content` | There was nothing eligible to change |

When `status` is `abstained`, `reason` explains which gate failed. The common
ones are `protected_value_changed` (a number, URL, email, identifier, or declared
term differs), `structure_changed` (paragraph or newline shape differs), and
`unsafe_text` (the candidate introduced control characters).

An abstention is a success, not an error. The command still exits `0` unless you
passed `--fail-on-abstain`. Exit `2` means your invocation was wrong, `3` means a
policy refusal, `4` means an input exceeded a limit.

## Development evaluation corpus

The source repository's evaluation tool runs 49 deterministic fidelity and structure
cases with exact status, reason, and output expectations. It separately validates 120
synthetic editorial cases across five groups, for 169 development cases total. The
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
  Rewording a sentence is correctly rejected as an unverifiable change. The
  calibrated semantic evaluator that would accept paraphrases is later work.
- **The reason does not say which value changed.** Reports are content-redacted
  by policy. `--diff` shows an escaped line comparison; it does not name the
  protected value that failed.
- **`rewrite` is not model-backed.** It can run the model-free path and a retained
  fake-conformance development binding, but it does not start a runtime or produce a
  qualified local-model rewrite.
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
