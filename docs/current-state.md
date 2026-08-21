# Current implementation state

## Checkpoint

The public repository completed the milestone 0.1 technical evidence and is
implementing milestone 0.2 under the `Retonr` project identity. 0.1 is not a tagged
milestone release; INV-Q04 applies from the first completed 0.2 closeout. External
contracts remain provisional. Public source availability does not freeze package,
protocol, or stored-data contracts.

## Implemented

| Component | Current behavior |
| --- | --- |
| `rewrite-types` | Versioned document, candidate, gate, status, reason, edit, rewrite-record v2, redacted generation provenance, content-redacted typed claim-evidence contracts, and an inert extractor manifest |
| `rewrite-text-adapter` | Bounded UTF-8 parsing, optional BOM retention, newline fingerprints, exact no-edit output, apply, reparse, verification, and a content-redacted pre-model inventory of encoding, controls, and possible unstructured-text Content Credential wrappers |
| `rewrite-engine` | Cancellation, typed value protection, sentinel integrity, hard gates, closed structure and semantic evidence boundaries, deterministic claim comparison, an informational shadow claim-comparison gate with no eligibility authority, reason priority, lexicographic selection, and document-atomic abstention |
| `rewrite-model` | Separate immutable single-file and canonical artifact-set identities; a structurally validated inert installed-set record; content-addressed runtime-build, effective-state, and effective-package evidence; frozen qualification v1 authority; and a distinct inert claim-extraction qualification v2 record and ID that cannot enter v1 activation |
| `rewrite-model-store` | Durable SQLite schema v5 for legacy artifact authority, inert artifact-set installation generations, crash-recoverable artifact-set removal journals, and artifact-set, runtime-build, effective-state, effective-package, and qualification-v2 evidence; exact supported-schema inspection; single-epoch exact-handle backup and atomic v1/v2/v3/v4 migration; transactional relationship checks; immediate v1 activation transactions; invalidation; active-removal protection; fail-closed recovery; and bounded coherent artifact-state inventory |
| `rewrite-inference` | Backend-neutral bounded discovery, adapter-admitted output-contract digests, candidate generation, a distinct claim-output contract, and structured-completion contracts with content-redacted debug and error surfaces, cancellation, deadlines, and deterministic fakes |
| `rewrite-grounded` | Structured masked prompt envelope, exact inference policy, proposal-only candidates, and redacted generation provenance |
| `rewrite-ollama` | IP-literal loopback-only native API adapter with bounded bodies, explicit parameters, exact candidate-contract discovery, candidate and structured completion, terminal-stop enforcement, concurrency, cancellation, pre-call and post-call identity checks, coherent read-only runtime, inventory, model-description, and residency preflight, plus a separate directly connected retained HTTP/1 preflight with one handshake and no pool, retry, or reconnect path |
| `rewrite-runtime-attestor` | Safe bounded facade over native attached-listener and exact established-connection evidence: Windows owner-PID tables plus retained process and executable handles; Linux proc socket inode plus exactly one visible same-user descriptor holder, pidfd, namespace, and retained executable object; deterministic unsupported result on macOS; redacted inert witnesses only |
| `rewrite-app` | Model-free candidate check, provisional grounded path, pinned source-preserving regular-file offline import, exact manifest-driven artifact-set folder import with whole-tree publication and inert structural registration, read-only single-file managed inventory with application-owned result DTOs, read-only artifact-set inventory with application-owned result DTOs, pending-operation inspection, backup-backed explicit repository migration, selected orphan reconciliation, selected set-root reconciliation, crash-recoverable inactive single-file and artifact-set removal with exact pinned-lock capability binding, verified single-file runtime artifact lease groundwork, a repository-owned whole-tree artifact-set lease, an inert managed-process runtime attestor with caller-supplied loaded-component evidence, a cancellable pair-extraction service, and an informational shadow join of independently produced claim comparison with no eligibility authority |
| `retonr` | Provisional `check` command with file or multiline standard-input documents, an explicit non-replacing output policy, opt-in `--in-place` (`-i`) with an implied sibling backup for a regular file, escaped interactive terminal rendering, a terminal raw-output double opt-in, escaped `--diff`, `--dry-run`, and redacted `--trace`; a pre-model `inspect` command that inventories one file or directory: encoding, BOM, newline kind, control-class counts, sibling sidecar presence, and skipped child reasons, without stripping bytes, following links, or validating a Content Credential; `--recursive` is a bounded walk that skips hidden names, `target`, and `node_modules`; a `rewrite` command that validates one source, optionally inspects `--data-dir` for an active generation binding and an exact `--artifact-id`, then attaches in-process fake-backend conformance when that recovered qualification names the retained fake backend, or fails closed otherwise; dedicated `version` and read-only `doctor` recovery commands that name migrate or removal-recovery follow-up without mutation; generated `completions` scripts and a section-1 `man` page from the live CLI definition; plus an explicit-root offline model-artifact CLI for single-file import, exact artifact-set folder import, read-only `list` of registered single-file installations, read-only `inspect` of one registered artifact's declared facts, inventory, set inventory, pending-operation inspection, confirmed repository migration, selected reconciliation, selected set reconciliation, inactive removal, exact removal recovery, inactive set removal, exact set-removal recovery, and optional read-only `device-evidence` (`fitr`) of `fitr.retonr.evidence.v1` without qualification or a repository |
| `rewrite-eval` | Versioned positive and hard-negative suite, transformation coverage, four baseline contracts with an offline no-rewrite CLI and recovered fake-conformance attach for generative kinds, five balanced synthetic editorial groups, a writing-sample library, a research-only watermark-refusal corpus, an independent claim-shadow calibration runner, a versioned non-generative Ollama observe or verify preflight, a separate native attached-process preflight that remains response-unbound, and a retained-connection preflight with repeated native attribution; every preflight remains unqualified |
| Fuzz targets | Protection round trips and plain-text no-edit byte identity |

The literal semantic evaluator accepts only an identical case-folded alphanumeric
and newline-token sequence. Punctuation can change. Lexical content, newline
placement, unsafe controls, protected values, or structure cannot.

Typed claim evidence binds each bounded extraction to an exact unit, text digest,
extractor-manifest digest, completion state, confidence policy, and canonical evidence
digest. The deterministic comparator accepts only complete compatible sets, rejects an
empty nontrivial source extraction, retains unknown and below-threshold counts, and
binds the aggregate to both exact evidence sets. Extraction is not implemented and
remains probabilistic when added; comparison evidence is not semantic proof.

Inference capability discovery now lists sorted, unique schema digests instead of a
generic structured-output Boolean. Grounded generation and evaluation require an
exact digest match before backend work. The structured-completion port returns only
one bounded complete JSON value after exact artifact checks; its request and response
debug views omit prompt and generated content. The model domain now has a distinct,
inert claim-extraction artifact role. The current Ollama implementation still admits
only generation and the existing candidate schema. The claim-output contract and
an inert extractor manifest now exist. Ollama discovery still refuses that
digest. An application pair-extraction service can call a backend twice and
compare the results. Completed comparison evidence can be joined onto the
engine's informational shadow gate for candidate-check and grounded
transactions. The join is skipped when the backend does not admit the claim
contract, the payload is unusable, or extraction is incomplete. A claim
conflict cannot reject a candidate that already passed the hard gates. The
join has no activation path or product authority. An independent
`rewrite-eval` calibration runner assigns fixture claim identities, compares
them separately from generation, and records whether attaching that
informational shadow changed hard-gate acceptance. It cannot promote a claim
result into a hard gate. Qualification schema v1 explicitly rejects claim
extraction, and its activation APIs cannot accept the separate v2 identifier or record.

The model domain now also represents a canonical, path-bounded artifact set;
content-addressed runtime-build and effective-state records; and an effective-package
evidence record that joins those exact identities. The package record requires one
canonical purpose set for every artifact member and binds completeness, acquisition,
license review, transformation disposition, runtime load closure, and exclusion and
isolation evidence. Its private fields, byte-bounded relationship-aware decoding,
closed vocabularies, fixed canonical encoding, and frozen digest make identity
comparisons portable across Windows, macOS, and Linux. These records are inert
evidence vocabulary. They do not attest a live process, prove that supplied evidence
is true or complete, authorize a role, or upgrade a v1 qualification.

Qualification v2 is a separate, inert, content-addressed record for exactly the
claim-extraction role. It binds the artifact set, effective-package evidence, runtime
build, effective state, source and context ceilings, prompt, claim-output and
claim-operation contracts, request and threshold policies, language policy, hardware
envelope, qualification suite, retained result evidence, license decision, and
qualification outcome. Its bounded decoder reloads and rechecks all four subject
records. It has no `authorizes` method and cannot enter existing v1 activation or
recovery. SQLite schema v4 persists the five immutable evidence records
in separate tables and adds a bounded installed-set table with a unique portable
set-root key and distinct positive generation. Schema v5 adds a separate
artifact-set removal journal. Migration creates those tables empty and
never infers installation from evidence or legacy single-file state. Every dependent
write and read reloads canonical record bytes,
recomputes indexed identities, and recursively cross-checks the complete subject. The
v1 tables and serialized records remain unchanged, and migration grants no authority.
The installed-set record is structural persistence only. It does not prove that the
root or member bytes exist, grant a lease, attest a runtime, qualify a package, or
authorize claim extraction. A lease is a separate verified operation that reads that
record and independently reverifies the managed bytes. The application now writes it only after an exact local
folder import verifies and publishes every manifest member under the content-derived
managed root. That observation does not turn the durable record into authority, and
no runtime consumer uses it yet.
The application and CLI now expose migration only as an explicit confirmed operation
against an initialized repository. A current schema is an exact no-op. A supported
older schema is inspected under both exclusive lifecycle locks and one retained
SQLite write reservation. SQLite copies that locked logical state, including committed
WAL frames, into a bounded rollback-mode snapshot and serializes it into the exact held
repository file. Retonr re-reads and integrity-checks the same file handle, synchronizes
the file and directory, and then commits the supported migration within that same
reservation. The result reports the exact source and target schemas plus an opaque
retained backup key. Ordinary repository commands remain exact-schema and never
migrate implicitly.

## Verification

The August 20, 2026 Windows development checkpoint for the retained-connection
witness passes:

```console
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features --no-fail-fast
cargo llvm-cov --locked --workspace --all-features --fail-under-lines 80
cargo doc --locked --workspace --all-features --no-deps
cargo clippy -p rewrite-runtime-attestor --all-targets --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy -p rewrite-runtime-attestor --all-targets --target aarch64-apple-darwin -- -D warnings
cargo deny check
cargo audit --db target/advisory-db-clean
npm run lint:markdown
pwsh -NoProfile -File scripts/check-repository.ps1
cargo +nightly check --locked --manifest-path fuzz/Cargo.toml --bins
cargo build --locked --workspace --release
```

The workspace exposes 771 Rust unit, integration, and process tests on Windows. The
ordinary Cargo run passes 769 and retains two process helpers as intentional ignores
exercised by isolated parent tests. Linux-native connection fixtures and macOS
attached and bound command refusals are target-specific and run in continuous
integration. Documentation tests also pass. Measured Rust line coverage is 90.53
percent overall. The bound-preflight orchestration is 95.12 percent covered, the
retained HTTP session is 87.34 percent, its transport is 90.18 percent, the portable
connection-evidence contract is 93.66 percent, and the Windows established-row
observer is 96.85 percent. The repository's 80 percent line coverage floor passes
with margin.

The local nightly toolchain type-checks both fuzz targets. The cargo-fuzz project
supports its libFuzzer execution path on Unix-like targets, so Windows is not
reported as a local fuzz-execution result. Continuous integration runs both bounded
targets under the Linux sanitizer-backed fuzz smoke job. `cargo-nextest` is not
installed on this development host; the exact public-main workflow remains the
nextest authority.

The published retained-connection checkpoint is
`1ec96d1a1cbc1d8d77b736f0b28a2c53564cf8fc`. Its passing
[quality workflow](https://github.com/blisspixel/retonr/actions/runs/32439664137)
completed all 12 Windows, macOS, Linux, repository-policy, Markdown, coverage,
dependency and supply-chain, fuzz-smoke, loopback-only Ollama, and stable aggregate
`required` jobs. That checkpoint includes the one-retained-connection milestone
described in this document while keeping every resulting report inert and
unqualified.

The custom audit database path bypasses a corrupt user-level RustSec cache containing
a duplicate advisory ID. The clean database loaded 1,225 advisories and the current
247-dependency graph passed. Dependency sources and licenses pass policy. Reviewed
duplicate-version warnings include the target-only capability filesystem tree,
`ctrlc` and filesystem `nix` versions, Windows support versions, and the two
transitive `syn` major versions. Continuous integration uses its own clean runner
database.

## Deliberate limitations

- The current CLI checks a supplied candidate and administers exact local artifact
  files. `inspect` inventories one source or directory before rewrite. It reports
  encoding, BOM, newline kind, control-class counts, and sibling `.c2pa` or
  `.xmp` presence. Hidden names, `target`, `node_modules`, and symlinks are
  skipped with reasons. Without `--recursive`, child directories are skipped.
  With `--recursive`, the walk is bounded, does not follow links, and uses
  portable `/` relative paths. A UTF-8 BOM plus variation selectors is
  `possible`, not a valid Content Credential. External references are
  `not_checked`. The command does not strip bytes. `model list` and `model inspect` are read-only: they do not qualify,
  activate, download, or treat a report as mutation authority.
  `model device-evidence` (`fitr`) reads optional
  `fitr.retonr.evidence.v1` without a repository. It reports device
  measurement only. `qualified` stays false. Host names, config paths, and
  result paths stay out of the report. Missing fitr is not an error.
  `rewrite` accepts one source file or standard input under the same
  output and inspection policy as `check`, including `--diff`, `--dry-run`,
  and `--trace`. A directory source is a dry-run destination manifest:
  `--output-dir` is required, `--recursive` is bounded, collisions are
  refused, and the output root cannot nest with the source. No files are
  written. Optional `--data-dir` inspects an existing repository for an
  active generation binding. Optional `--artifact-id` must match that binding.
  When the recovered qualification names the retained fake backend, `rewrite`
  attaches in-process conformance, generates an identity candidate, and runs
  the common gates. It does not start a runtime, pull a model, or use the
  network. Any other recovered backend still fails closed. A recovered binding
  is not activation, a lease, or a qualification producer.
- `check` accepts either document from standard input, read to end of file without
  trimming, and preserves the byte order mark, newline kind, blank lines, surrounding
  whitespace, and final-newline state exactly. Both documents cannot share one stream.
  `--output` writes the accepted bytes, or the exact original after an abstention, to
  a new file that must not already exist, or to standard output, which moves the
  report to standard error. `--in-place` (`-i`) retains a sibling
  `<name>.retonr-backup` that must not already exist, then replaces a regular
  source file after same-directory staging. Standard input, `--output`, and
  symlinks are refused. Unchanged accepted bytes leave the source untouched.
  A terminal defaults to text; a pipe defaults to JSON. `-f` selects either.
  `--data-dir` is also `-D` or `RETONR_DATA_DIR`.
  Exact bytes reach a terminal only after the
  `--raw-terminal --yes` double opt-in and a warning. Without that double opt-in,
  a terminal receives escaped interactive rendering that neutralizes ANSI, OSC,
  C0, C1, carriage-return, hyperlink, clipboard, bidi, and invisible-control
  effects. Either flag alone stays escaped. `--diff` writes an escaped
  linear comparison of source and accepted output to standard error. `--dry-run`
  computes the report without creating `--output` or replacing the source.
  `--trace` writes the redacted
  rewrite record to a new file.
- The editorial corpus contract and 120 synthetic fixtures across five groups are
  implemented, but no lint scanner, rule catalog, or live anti-slop ranking path is
  implemented yet. A separate writing-sample library holds licensed pre-2018
  human excerpts and synthetic model-style impressions. A research-only watermark file
  refuses style-as-mark folklore and does not contain generated marks.
- Only UTF-8 plain text up to 16 MiB is accepted.
- Durable artifact lifecycle state, bounded single-file staging recovery, pinned single-file
  offline import, read-only managed-byte inventory, selected single-artifact orphan
  reconciliation, crash-recoverable inactive removal, and runtime artifact leases
  are implemented. Inventory verifies registered files,
  reports manifest-only state, and identifies orphan candidates and conflicts without
  mutation. Reconciliation requires one exact manifest, ignores earlier inventory
  evidence as authority, reacquires the exclusive lifecycle lock, and reverifies the
  current canonical file before atomically inserting any missing exact manifest and
  installation records or confirming that both existing records match. Removal
  selects one exact installation generation, rejects active or aliased
  bytes, journals preparation before deletion, resumes after interruption, and uses
  generation ordering so an old retry cannot delete a reinstall. The runtime lease
  boundary verifies current durable state and bytes, then retains the shared
  lifecycle lock and file handle until use ends. No real runtime consumer uses that
  lease yet. Exact manifest-driven artifact-set folder import is implemented at the
  application boundary and exposed as offline `model import-set`. The command
  verifies the complete local source tree, publishes the whole content-derived set
  root, and records only inert structural installation state. It does not qualify,
  activate, lease, or execute the set. Read-only `inventory-set` inspects managed
  set roots under the shared lifecycle lock, reports registered tree status,
  manifest-only set state, verified orphan set roots, tree conflicts, oversized
  planned trees, and aggregate unexpected set-root counts, and never creates,
  repairs, or removes anything. The report does not grant a lease, qualify a
  package, or authorize a role. Selected `reconcile-set` accepts one exact set
  manifest, ignores earlier inventory evidence as authority, reacquires the
  exclusive lifecycle lock, and reverifies the current canonical set tree before
  atomically inserting any missing exact set-manifest and installation records or
  confirming that both existing records match. It does not copy, replace, repair,
  delete, qualify, or activate the set. Selected `remove-set` accepts one exact
  set installation generation, ignores earlier inventory evidence as authority,
  reacquires the exclusive lifecycle lock, and reverifies the current canonical
  set tree before journaling preparation, deleting the verified tree, and
  completing the journal. Exact prepared set removals resume through
  `recover-set-removal` without callbacks or cancellation. A later exact reimport
  uses the next generation so an old retry cannot delete the reinstall. Set
  removal does not qualify, activate, lease, or grant role authority.
  Single-file `inventory`, `reconcile`, and `remove`
  do not inspect or mutate managed sets. Set `inventory-set`, `reconcile-set`,
  `remove-set`, and `recover-set-removal`
  do not inspect or mutate single-file artifacts. A repository-owned
  artifact-set lease reverifies the complete registered tree under the shared
  repository and storage lifecycle locks and retains that boundary for its lifetime,
  so exclusive operations fail while it is live. No real runtime consumer uses it
  yet. Downloads,
  runtime-native pulls, bulk reconciliation, orphan deletion, runtime commands, and
  exact real-artifact qualification are not implemented. Effective-package evidence
  and qualification v2 have durable inert persistence but no production evidence
  producer, activation path, or CLI surface. An application-owned managed-process
  attestor can hash one live regular entrypoint, bind `RuntimeBuildIdentity` and
  `EffectiveRuntimeState`, and optionally persist those inert records. It does not
  activate a role, admit observed-only Ollama identity, or enable claim extraction. The
  CLI requires one explicit `--data-dir`; its fourteen repository model commands do not use the
  network. Only confirmed `model migrate` can apply a supported schema migration,
  and it first retains a verified repository-owned backup. The other thirteen commands
  remain exact-schema and non-migrating. `pending-operations` reads only bounded durable state and
  returns exact prepared single-file and artifact-set removal generations without
  opening or hashing model bytes. The current product ceilings are 256 GiB per artifact or set member, 4,096
  durable or storage entries, 4,096 set members, 8,192 set-tree entries, 512 GiB of
  aggregate inventory or set-import verification, and
  1 MiB per manifest. Removal is not secure
  erasure and does not affect external copies, caches, backups, or provider records.
  Only local, application-owned storage on the tested platform and filesystem
  configurations is
  within the current boundary; network filesystem semantics and other Windows
  filesystem drivers are not qualified.
- The Ollama adapter is fake-server tested and its read-only preflight has observed
  and verified one existing local Ollama 0.32.14 inventory without generation. The
  separate attached preflight brackets that HTTP work with point-in-time listener,
  process-incarnation, and executable evidence on Windows and Linux. macOS returns
  unsupported because no admitted public unprivileged listener-owner API exists.
  Linux also fails closed when ptrace or proc policy prevents a complete same-user
  ownership view.
  That attached report uses independent requests, remains `response_bound: false`
  and `qualified: false`, and creates no runtime identity. The separate
  `--ollama-bound-preflight` command uses one retained direct HTTP/1 connection and
  checks exact reverse established-row attribution before traffic and after every
  fully drained response. Windows evidence is a context-binding PID, not exclusive
  socket ownership. Linux requires exactly one visible same-user descriptor holder,
  but cannot exclude holders hidden by UID, ptrace, proc-mount, PID-namespace, or
  security boundaries. macOS refuses before HTTP because no admitted public
  unprivileged tuple-to-process API is available. The bound report therefore states
  that exclusive socket ownership and application-handler execution are not proven.
  It remains `qualified: false` and creates no runtime, package, qualification,
  activation, or role identity. Executable bytes are not loaded-component closure.
  No current evidence establishes complete artifact-set or upstream identity,
  provider cloud disablement, OS isolation, or a qualified runtime and model.
- The grounded path can safely accept only literal-mode token-preserving changes
  under the current evaluator. Open-domain paraphrases and broader modes abstain.
- The typed claim contract and deterministic comparator are implemented. The engine
  can record independently produced comparison evidence on a separate informational
  shadow gate. The application can prepare that evidence from pair extraction and
  attach it to candidate-check or grounded transactions. That gate cannot authorize
  a rewrite or reject a candidate that already passed the hard gates. Literal-token
  failure still abstains. `rewrite-eval --claim-shadow-calibration` runs a
  checked-in fixture corpus through that same candidate-check path and fails if
  shadow evidence changes acceptance. No learned extractor or runtime-backed
  semantic evaluator is connected. The current
  synchronous semantic port is not the future runtime extraction boundary. The raw
  structured-completion port has no semantic authority. The Ollama adapter admits
  only the exact candidate contract currently advertised by discovery. This
  backend-wide admission does not qualify every inventoried artifact for that role.
- UTF-16, Markdown, DOCX, profiles, persistence, document briefs, file and folder
  transactions, API, MCP, Agent Skills, Agent Plugins, and native desktop are not
  implemented yet.
- The model-free evaluator does not assess open-domain paraphrases and must abstain
  on them.
- No public API, schema, package, executable name, or configuration namespace is
  frozen.
- Rewrite records use unkeyed SHA-256 identity digests. These are not anonymization
  and can permit dictionary attacks on short predictable text. Stable local traces
  require an installation-keyed digest decision.
- The README includes one reproducible Linux-first rendering of verbatim output from
  the current release-optimized candidate-check binary. Model-backed rewrite,
  abstention, diff, trace and native desktop screenshots remain gated on their
  complete release-build behaviors under the screenshot policy.
- Evaluation data and user-research policies are proposed, not approved. No
  non-synthetic collection is authorized.

## Next logical operations

The detailed handoff is in the
[0.2 grounded engine and CLI plan](planning/0.2-grounded-cli.md). The immediate order
is:

1. Preserve completed lifecycle, schema-v5 persistence, lease, migration, recovery,
   cancellation, claim-comparison, and informational shadow boundaries.
2. Retain the versioned read-only Ollama preflight. It performs no generation and
   cannot qualify a runtime or package.
3. Retain the native attached-process witness and the separate retained-connection
   preflight on Windows and Linux, plus deterministic macOS refusal. Neither report
   constructs runtime identity. Only the bound report claims one retained client
   transport with repeated native attribution, and it explicitly disclaims exclusive
   ownership and handler execution.
4. Replace Linux proc TCP row selection with bounded `NETLINK_SOCK_DIAG`, retaining
   the exact tuple, kernel socket cookie, inode, UID, and namespace scope. This is
   first because the proc TCP table is deprecated and every stronger Linux claim
   depends on reliable row selection.
5. Reconstruct the selected Ollama runtime and model as complete canonical runtime
   and artifact-set manifests with
   exact blob, tokenizer, template, license, upstream source, and transformation
   evidence, including native dependencies and runtime load closure. This must precede
   effective identity because a socket witness does not identify the complete code or
   model package.
6. Add version-gated provider cloud-disable evidence and OS-enforced denial of
   non-loopback outbound traffic for every participating Retonr and runtime process.
   Configuration claims alone cannot establish local-only execution.
7. Construct effective runtime identity only after transport, package,
   configuration, and isolation evidence join without drift. Then project the
   existing eight-case smoke and 39-case editorial protocol into
   versioned local generation plans. The old Gemma 4, Qwen3.6, and Ministral local
   observations have expired. Do not reacquire a model without separate approval.
8. Run smoke, locked evaluation, repeatability, and exact cross-platform
   qualification in that order. Preserve the existing hard gates as the only
   acceptance authority.
9. Finish remaining CLI recovery and packaging evidence, then capture model-backed
   screenshots only from the complete passing release path.

Later work follows the dependency order in the
[phase execution plan index](planning/README.md).
