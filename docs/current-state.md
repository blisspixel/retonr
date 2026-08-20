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
| `rewrite-ollama` | IP-literal loopback-only native API adapter with bounded bodies, explicit parameters, exact candidate-contract discovery, candidate and structured completion, terminal-stop enforcement, concurrency, cancellation, and pre-call and post-call identity checks |
| `rewrite-app` | Model-free candidate check, provisional grounded path, pinned source-preserving regular-file offline import, exact manifest-driven artifact-set folder import with whole-tree publication and inert structural registration, read-only single-file managed inventory with application-owned result DTOs, read-only artifact-set inventory with application-owned result DTOs, pending-operation inspection, backup-backed explicit repository migration, selected orphan reconciliation, selected set-root reconciliation, crash-recoverable inactive single-file and artifact-set removal with exact pinned-lock capability binding, verified single-file runtime artifact lease groundwork, a repository-owned whole-tree artifact-set lease, an inert managed-process runtime attestor, a cancellable pair-extraction service, and an informational shadow join of independently produced claim comparison with no eligibility authority |
| `retonr` | Provisional `check` command with file or multiline standard-input documents, an explicit non-replacing output policy, opt-in `--in-place` (`-i`) with an implied sibling backup for a regular file, escaped interactive terminal rendering, a terminal raw-output double opt-in, escaped `--diff`, `--dry-run`, and redacted `--trace`; a pre-model `inspect` command that inventories one file or directory: encoding, BOM, newline kind, control-class counts, sibling sidecar presence, and skipped child reasons, without stripping bytes, following links, or validating a Content Credential; `--recursive` is a bounded walk that skips hidden names, `target`, and `node_modules`; a `rewrite` command that validates one source, optionally inspects `--data-dir` for an active generation binding and an exact `--artifact-id`, then attaches in-process fake-backend conformance when that recovered qualification names the retained fake backend, or fails closed otherwise; dedicated `version` and read-only `doctor` recovery commands that name migrate or removal-recovery follow-up without mutation; generated `completions` scripts and a section-1 `man` page from the live CLI definition; plus an explicit-root offline model-artifact CLI for single-file import, exact artifact-set folder import, read-only `list` of registered single-file installations, read-only `inspect` of one registered artifact's declared facts, inventory, set inventory, pending-operation inspection, confirmed repository migration, selected reconciliation, selected set reconciliation, inactive removal, exact removal recovery, inactive set removal, exact set-removal recovery, and optional read-only `device-evidence` (`fitr`) of `fitr.retonr.evidence.v1` without qualification or a repository |
| `rewrite-eval` | Versioned positive and hard-negative suite, transformation coverage, four baseline contracts with an offline no-rewrite CLI and recovered fake-conformance attach for generative kinds, five balanced synthetic editorial groups, a writing-sample library, a research-only watermark-refusal corpus, an independent claim-shadow calibration runner, and redacted aggregate reporting |
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

## Verified locally

The August 16, 2026 Windows development checkpoint passes:

```console
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo nextest run --locked --workspace --all-features --no-fail-fast
cargo llvm-cov --locked --workspace --all-features --fail-under-lines 80
cargo test --locked --workspace --all-features --doc
cargo doc --locked --workspace --all-features --no-deps
cargo deny check
cargo audit --db target/advisory-db-clean
npm run lint:markdown
pwsh -NoProfile -File scripts/check-repository.ps1
cargo +nightly check --locked --manifest-path fuzz/Cargo.toml --bins
cargo run --locked -p rewrite-eval -- --editorial-corpus crates/eval/fixtures/editorial_quality_v1.json
cargo run --locked -p rewrite-eval -- --editorial-corpus crates/eval/fixtures/editorial_slop_v1.json
cargo build --locked --workspace --release
```

All 506 Rust unit, integration, and process tests pass. Two process helpers are
intentionally ignored by the ordinary runner and exercised by isolated parent tests.
Two further artifact-set lease cases are Unix only and run in continuous
integration. Documentation tests also pass. The measured Rust line coverage is 91.44
percent overall. The repository's 80 percent line coverage floor passes with margin.

The local nightly toolchain can type-check both fuzz targets. The cargo-fuzz project
supports its libFuzzer execution path on Unix-like targets, so Windows is not
reported as a local fuzz-execution result. Continuous integration runs both bounded
targets under the Linux sanitizer-backed fuzz smoke job. The pinned `cargo-nextest`
0.9.143 run also passes. Its Windows-only override gives the console-interrupt
integration test exclusive test-thread capacity because the helper owns process-wide
console state; the test remains concurrent with the full suite on macOS and Linux.

The editorial-pattern research package, portable artifact-set and runtime-identity
slice, distinct inert claim-extraction role, Windows nextest isolation repair,
effective-package evidence, qualification v2, schema-v4 artifact-set installation
persistence, the explicit backup-backed repository migration, the
repository-owned artifact-set lease, and the candidate-check standard-input and
output policy passed at exact-main
revision
`c7f0a39ecdb7b9392b09ad49327369b5ab6bd857`
in the passing
[quality workflow](https://github.com/blisspixel/retonr/actions/runs/31956893414).
The retained exact-main jobs cover Windows, macOS, and Linux Rust checks, repository
policy, Markdown, coverage, dependency and supply-chain policy, fuzz smoke, proxy
isolation, concurrency, and the Ubuntu loopback-only network namespace.

The custom audit database path bypasses a corrupt user-level RustSec cache containing
a duplicate advisory ID. The clean database loaded 1,216 advisories and the current
243-crate graph passed. Dependency sources and licenses pass policy. Reviewed
duplicate-version warnings now include the target-only capability filesystem
dependency tree and the two transitive `syn` major versions. Continuous integration
uses its own clean runner database.

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
  CLI requires one explicit `--data-dir`; its twelve artifact commands do not use the
  network. Only confirmed `model migrate` can apply a supported schema migration,
  and it first retains a verified repository-owned backup. The other eleven commands
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
- The Ollama adapter is fake-server tested but has not been qualified against a real
  pinned runtime and model artifact on the three operating systems.
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

1. Preserve the completed artifact lifecycle boundary, application-owned inventory
   DTOs, offline `import-set` CLI, read-only `inventory-set` CLI, non-mutating
   pending-operation inspection, and process-level signal
   cancellation evidence as new consumers are added.
2. Preserve the completed rewrite-record v2, typed invariant summaries, typed claim
   evidence, and deterministic comparison boundary.
3. Preserve the distinct inert claim-extraction role, canonical artifact-set
   manifest, runtime-build and effective-state identities, relationship-checked
   effective-package evidence, separate inert qualification v2, and schema-v5
   relationship-checked persistence without rewriting v1 or schema-v3 evidence.
   Preserve the distinct inert artifact-set installation generation, exact bounded
   folder import, offline `import-set` CLI, explicit backup-backed repository
   migration path,
   repository-owned artifact-set leases, read-only set inventory, selected
   set-root reconciliation, and crash-recoverable set removal without
   implying set authority. Preserve the completed managed-process attestor that
   writes inert runtime-build and effective-state records without role authority.
   Preserve the cancellable pair-extraction service, the informational
   shadow claim-comparison gate, the application shadow join, and the
   independent claim-shadow calibration runner. That gate has no eligibility
   authority.
4. The extractor manifest, claim-output contract, cancellable pair
   extraction service, engine shadow gate, application shadow join, and
   independent claim-shadow calibration runner exist. Pair extraction can
   compare completed evidence sets and record that comparison informatively.
   Calibration assigns fixture claim identities separately from generation
   and fails if the informational shadow changes hard-gate acceptance. It
   has no activation authority and cannot change acceptance. Keep Ollama on
   the candidate contract only.
5. Add the local evaluation plan, run the currently installed 26B and 27B packages,
   and add the previously observed 8B package only after revalidation or separately
   approved acquisition. Start only after product-path evidence joins are complete.
6. `check` output policy, `version`, `doctor`, `completions`, `man`,
   `inspect`, `model list`, `model inspect`, and `rewrite-eval --baseline`
   exist. `inspect` inventories one source or directory before rewrite.
   `--recursive` is a bounded walk that does not follow links. It does not
   parse credentials, follow external references, or strip bytes.
   `--in-place` replaces one regular source file after retaining a
   sibling backup. It is not a directory rewrite.
   Optional `model device-evidence` reads fitr measurement JSON without a
   repository and does not qualify. Missing fitr is not an error.
   `doctor` names exact recovery follow-up when schema migration or a
   prepared removal is pending. It does not migrate, recover, or activate.
   `rewrite` attaches in-process fake-backend conformance to a recovered
   fake-qualified binding and otherwise fails closed. It shares `--diff`,
   `--dry-run`, and `--trace` with `check`. Directory rewrite dry-run
   requires `--output-dir` and does not write files. Generative baseline
   kinds use that same recovered binding through `--data-dir` and otherwise
   fail closed. `list` and `inspect` are read-only and do not qualify or
   activate. Neither path starts a runtime or uses the network. Do not start
   Ollama or qualify a real model.
7. Run exact artifact qualification and selective-risk reporting on declared
   hardware tiers.
8. Capture the model-backed rewrite, abstention, diff, and trace CLI screenshots only
   after the 0.2 completion evidence passes. The current candidate-check rendering is
   limited to already implemented model-free behavior.

Later work follows the dependency order in the
[phase execution plan index](planning/README.md).
