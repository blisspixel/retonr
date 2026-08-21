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
| `rewrite-model` | Separate immutable single-file and canonical artifact-set identities; strict package-source and transformation evidence; complete runtime-package and model-package manifests; inert native-load observations; typed runtime-build and effective-state derivation from package and load evidence; frozen qualification v1 authority; and a distinct inert claim-extraction qualification v2 record and ID that cannot enter v1 activation |
| `rewrite-model-store` | Durable SQLite schema 6 for legacy artifact authority, inert artifact-set installation generations, crash-recoverable artifact-set removal journals, runtime-build, effective-state, effective-package, qualification-v2, runtime-package, model-package, and native-load evidence; exact supported-schema inspection; single-epoch exact-handle backup and atomic migration from schemas 1 through 5; recursive relationship checks; immediate v1 activation transactions; invalidation; active-removal protection; fail-closed recovery; and bounded coherent artifact-state inventory |
| `rewrite-inference` | Backend-neutral bounded discovery, adapter-admitted output-contract digests, candidate generation, a distinct claim-output contract, structured completion, and a provider-neutral local-judge attempt contract with exact choices, sorted rubric clauses, and bounded cited byte spans; content-redacted debug and error surfaces, cancellation, deadlines, and deterministic fakes |
| `rewrite-grounded` | Structured masked prompt envelope, exact inference policy, proposal-only candidates, and redacted generation provenance |
| `rewrite-ollama` | IP-literal loopback-only native API adapter with bounded bodies, explicit parameters, exact candidate-contract discovery, candidate and structured completion, terminal-stop enforcement, concurrency, cancellation, pre-call and post-call identity checks, coherent read-only runtime, inventory, model-description, and residency preflight; a caller-supplied retained HTTP/1 session with one preflight, an absolute 4 MiB UTF-8 completion-input ceiling enforced before wire serialization or completion traffic, structured completions, connection callbacks, monotonically ordered response checkpoints, nonserializable content-free request and response receipts, and an opt-in v0.32.15 nine-response completion profile that binds two equal post-generation runtime-reported residency observations while proving neither handler, model use, resident-page identity, effective identity, nor qualification; no connector, pool, retry, reconnect, or fallback path; and inert exact-version cloud-disable declaration and startup-marker evidence whose production reviewed-runtime allowlist is empty |
| `rewrite-ollama-package` | Strict bounded parsing of the admitted Ollama manifest-v2 layer shape and GGUF v3 metadata and tensor table, followed by deterministic reconstruction of one canonical six-member model artifact set and semantic model-package manifest; no network, mutable tag authority, qualification, activation, load, or execution |
| `rewrite-runtime-isolation` | Linux managed prelaunch isolation with retained-helper validation, user, network, and PID namespaces, loopback-only networking, descriptor closure, no-new-privileges and capability reduction, a target-inherited seccomp allowlist that permits only `AF_INET` and `AF_INET6` through `socket()`, denies every other socket family and `io_uring_setup`, and requires seccomp mode 2 during target reobservation, retained-handle target launch, bounded startup streams, one namespace-local loopback and socket-diagnostics capability, reobservation, and process-tree teardown; deterministic unsupported results on Windows and macOS |
| `rewrite-runtime-attestor` | Safe bounded facade over native attached-listener and exact established-connection evidence: Windows owner-PID tables plus retained process and executable handles; Linux bounded `NETLINK_SOCK_DIAG` dump and exact retained-cookie queries plus visible same-UID descriptor ownership, pidfd, namespace, and retained executable object; a Linux managed observer that consumes exact launch facts and a namespace-local diagnostics capability; Linux object-bound native-load observation; deterministic unsupported results for exact native-load binding on Windows and for attached, managed, and native-load observation on macOS; redacted inert evidence only |
| `rewrite-app` | Model-free candidate check; provisional grounded path; exact offline single-file and artifact-set lifecycle; schema-6 migration; whole-tree runtime and model package leases; retained runtime code-member handles; static package attestation; inert installed-Ollama model reconstruction, managed import, package persistence, and readback; an older inert managed-process attestor with caller-supplied loaded-component evidence; a cancellable pair-extraction service; and an informational shadow join with no eligibility authority. It does not compose the managed Linux trust chain into generation |
| `retonr` | Provisional `check` command with file or multiline standard-input documents, an explicit non-replacing output policy, opt-in `--in-place` (`-i`) with an implied sibling backup for a regular file, escaped interactive terminal rendering, a terminal raw-output double opt-in, escaped `--diff`, `--dry-run`, and redacted `--trace`; a pre-model `inspect` command that inventories one file or directory: encoding, BOM, newline kind, control-class counts, sibling sidecar presence, and skipped child reasons, without stripping bytes, following links, or validating a Content Credential; `--recursive` is a bounded walk that skips hidden names, `target`, and `node_modules`; a `rewrite` command that validates one source, optionally inspects `--data-dir` for an active generation binding and an exact `--artifact-id`, then attaches in-process fake-backend conformance when that recovered qualification names the retained fake backend, or fails closed otherwise; dedicated `version` and read-only `doctor` recovery commands that name migrate or removal-recovery follow-up without mutation; generated `completions` scripts and a section-1 `man` page from the live CLI definition; plus an explicit-root offline model-artifact CLI for single-file import, exact artifact-set folder import, read-only `list` of registered single-file installations, read-only `inspect` of one registered artifact's declared facts, inventory, set inventory, pending-operation inspection, confirmed repository migration, selected reconciliation, selected set reconciliation, inactive removal, exact removal recovery, inactive set removal, exact set-removal recovery, and optional read-only `device-evidence` (`fitr`) of `fitr.retonr.evidence.v1` without qualification or a repository |
| `rewrite-eval` | A 49-case versioned positive and hard-negative suite with exact expectation results and transformation coverage, four baseline contracts with an offline no-rewrite CLI and recovered fake-conformance attach for generative kinds, 120 cases across five balanced synthetic editorial groups, a writing-sample library, a research-only watermark-refusal corpus, an independent claim-shadow calibration runner, a versioned hybrid scorecard library and CLI that bind and execute exact deterministic suite pairs before normalizing blinded order-swapped triage observations, a typed retained-session local-judge executor that runs both orders after hard gates and returns a separate limited transport receipt, a version-gated v0.32.15 static installed-package-to-inventory binding that consumes an opaque nonserializable exact-runner receipt, a versioned non-generative Ollama observe or verify preflight, a separate native attached-process preflight that remains response-unbound, a retained-connection preflight with repeated native attribution, and a Linux-only managed preflight library that binds retained runtime-package, isolation, process, connection, provider-declaration, read-only API, and native-load evidence and can additionally return an inert package-declared typed `RuntimeBuildIdentity`; the scorecard remains caller-declared and triage-only, runtime target and revision semantics in that build identity are not independently live-observed, the managed process is closed before the build binding returns, no effective state is constructed, the new execution and binding surfaces have no CLI, and every preflight remains unqualified |
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

Additive semantic package contracts now distinguish an exact runtime package from
an exact model package. Runtime members have complete static roles and load policy;
model members bind weights, tokenizer, prompt templates, parameters, license, source,
and transformation evidence without pretending that evidence-only files affect
output. A native-load observation separately binds one retained process witness to
the exact reviewed runtime package and observed executable mappings. Linux can build
that observation from retained package-member file objects and `/proc/PID/map_files`
under strict limits. Windows returns unsupported because its admitted public mapping
APIs do not bind a mapped section to an exact retained file object. macOS returns
unsupported. These records remain evidence, not activation or qualification.

Qualification v2 is a separate, inert, content-addressed record for exactly the
claim-extraction role. It binds the artifact set, effective-package evidence, runtime
build, effective state, source and context ceilings, prompt, claim-output and
claim-operation contracts, request and threshold policies, language policy, hardware
envelope, qualification suite, retained result evidence, license decision, and
qualification outcome. Its bounded decoder reloads and rechecks all four subject
records. It has no `authorizes` method and cannot enter existing v1 activation or
recovery. SQLite schema 4 persists the five immutable evidence records
in separate tables and adds a bounded installed-set table with a unique portable
set-root key and distinct positive generation. Schema 5 adds a separate
artifact-set removal journal. Schema 6 adds immutable runtime-package,
model-package, and native-load tables with relationship foreign keys. Migration
creates all additive tables empty and never infers package, load, installation, or
authority evidence from legacy state. Every dependent
write and read reloads canonical record bytes,
recomputes indexed identities, and recursively cross-checks the complete subject. The
v1 tables and serialized records remain unchanged, and migration grants no authority.
The installed-set record is structural persistence only. It does not prove that the
root or member bytes exist, grant a lease, attest a runtime, qualify a package, or
authorize claim extraction. A lease is a separate verified operation that reads that
record and independently reverifies the managed bytes. The application now writes it only after an exact local
folder import verifies and publishes every manifest member under the content-derived
managed root. That observation does not turn the durable record into authority.
Package leases and the inert installed-Ollama import consume managed-set state, but
no user-facing generation path consumes it yet.
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

The complete current trust-chain slice is held to these repository gates:

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

Exact test and coverage totals are intentionally not copied from the previous
schema-5 checkpoint because the trust-chain slice adds target-specific crates and
fixtures. The release evidence is the passing workflow for the merged revision, not
a count embedded before that workflow finishes. Linux-native CI exercises SOCK_DIAG,
managed namespaces, retained launch, namespace-local attestation, and native-load
fixtures. Windows and macOS exercise their exact supported and unsupported contracts.
The workspace line-coverage gate remains at least 80 percent, with warnings treated
as errors. The local nightly check only type-checks fuzz targets on Windows; Linux CI
runs the bounded sanitizer-backed fuzz smoke.

The last completed public-main baseline before this slice is
`c3657edd126f164facc311719b831d6926e7c06d`. Its passing
[quality workflow](https://github.com/blisspixel/retonr/actions/runs/32440174921)
is historical baseline evidence only. The new trust-chain implementation must not be
described as public-main verified until its own merged `required` workflow passes.

## Runtime trust support matrix

| Capability | Linux | Windows | macOS |
| --- | --- | --- | --- |
| Attached listener and retained connection observation | Supported when socket, process, and descriptor visibility is complete; observation-only | Supported through documented owner-PID tables; observation-only | Unsupported |
| Managed prelaunch isolation and process attestation | Supported when unprivileged user, network, and PID namespaces and required process visibility are available | Unsupported | Unsupported |
| Exact native-load binding to retained package objects | Supported for the admitted file-backed executable mapping scope | Unsupported | Unsupported |
| Managed runtime-build identity | Supported as an inert package-declared binding after successful managed package, process, and native-load checks; only the exact entrypoint is joined to live evidence, other package semantics are not independently live-observed, and the process is closed before return | Unsupported | Unsupported |
| Qualified managed generation | Not implemented | Unsupported | Unsupported |

Support in this table describes the individual infrastructure contract, not model
qualification. Attached evidence is observation-only on every platform. Linux
managed evidence is also inert. The current read-only preflight composes the runtime
package, provider, isolation, native-load, and transport side and can construct a typed
runtime-build identity. The process is closed before that outcome returns. Separate
static model binding, runtime-reported residency, and retained-transport judge
receipts now exist, but they are not joined to that managed evidence. No effective
runtime state or model-use proof exists.

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
  lifecycle lock and file handle until use ends. No user-facing runtime operation
  uses that single-file lease yet. Exact manifest-driven artifact-set folder import is implemented at the
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
  so exclusive operations fail while it is live. Package attestation and the inert
  installed-Ollama import use managed-set boundaries, but no user-facing runtime
  operation does. Downloads,
  runtime-native pulls, bulk reconciliation, orphan deletion, runtime commands, and
  exact real-artifact qualification are not implemented. Effective-package evidence
  and qualification v2 have durable inert persistence but no production evidence
  producer, activation path, or CLI surface. An application-owned managed-process
  attestor can hash one live regular entrypoint, bind `RuntimeBuildIdentity` and
  `EffectiveRuntimeState`, and optionally persist those inert records. It does not
  activate a role, admit observed-only Ollama identity, or enable claim extraction.
  A newer static package service retains exact runtime code-member objects and model
  bytes but also grants no runtime authority. An offline installed-Ollama import
  reconstructs only the admitted manifest-v2 and GGUF-v3 package shape, publishes a
  canonical six-member managed set, and persists and reads back its inert model
  package. It has no CLI surface and does not qualify, activate, lease, load, or
  execute the model. A separate evaluation library can bind one such import to one
  exact verified idle Ollama v0.32.15 inventory and model-details observation. It
  requires the production backend identity `ollama_native` and reviewed source
  revision `b7871fc0d1d82fe109536efa3e0e8e411c766c75`. The version-scoped
  relationship checks the raw manifest digest, the exact config-plus-layers inventory
  size, GGUF, license, format, and a unique template match. The binding must consume
  the opaque, nonserializable receipt issued by the exact preflight runner for the
  plan and report. It proves only that static import-to-inventory relationship; model
  loaded, model used, application handler, effective identity, and qualification
  remain false. It also has no CLI surface. The
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
  and verified existing local inventory without generation. The
  separate attached preflight brackets that HTTP work with point-in-time listener,
  process-incarnation, and executable evidence on Windows and Linux. macOS returns
  unsupported because no admitted public unprivileged listener-owner API exists.
  Linux now selects listener and connection rows through bounded SOCK_DIAG and fails
  closed when ptrace or proc policy prevents a complete same-user
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
  A separate development-only library preflight now joins a retained runtime-package
  lease, Linux managed launch and isolation, namespace-local process and connection
  evidence, cloud-disable declaration and startup marker, read-only API observation,
  and exact native-load evidence. It reobserves the retained boundaries and closes the
  process tree. Its report remains inert and explicitly leaves application-handler
  proof, exclusive socket ownership, model load or use, effective-runtime identity,
  and qualification false. It has no CLI surface and does not consume the inert
  model-package import. The production cloud-disable allowlist is empty, so its exact
  runtime disposition remains unreviewed. No runtime and model combination is
  qualified.
  The managed target inherits a seccomp socket allowlist installed before launch.
  `socket()` permits only `AF_INET` and `AF_INET6`; every other socket family and
  `io_uring_setup` are denied, and reobservation requires seccomp mode 2.
  An opt-in API returns the unchanged report plus a separate redacted, inert managed
  build binding. That binding constructs only a package-declared typed
  `RuntimeBuildIdentity` after the managed package, process, and native-load join.
  The exact entrypoint is joined to live process and load evidence, but target,
  revision, and other package semantics are not independently live-observed. Cleanup
  is complete and `process_retained_after_return` is false. It explicitly lacks a
  generation-bound provider snapshot, effective output configuration, platform,
  framework, and driver evidence, compute backend and device placement, effective
  context capacity, and a retained live runtime. Effective runtime state, model load
  or use, application-handler execution, and qualification remain false.
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
- The versioned hybrid scorecard executes two exact deterministic suites, requires
  their complete corpus and fixed policy digests to match the selected plan, and
  reports exact expectation results and transformation coverage. It normalizes
  blinded, order-swapped structured judge observations only after both suites pass.
  Its serializable report still labels judge observations caller-declared and
  triage-only. A separate typed executor now runs both orders over one already
  preflighted retained Ollama stream and returns a nonserializable receipt binding the
  plan, rubric, observation batch, retained preflight, exact request and response
  digests, and response ordinals. That receipt does not prove managed isolation,
  handler execution, model load or use, candidate generation, effective identity,
  semantics, or qualification. Neither the scorecard nor the receipt can override
  hard gates or replace human release adjudication. The executor has no CLI surface.
  Every retained-session completion rejects UTF-8 input above the absolute 4 MiB
  ceiling before wire serialization or completion traffic.
  A separate opt-in retained-session profile for reviewed Ollama v0.32.15 sends one
  structured completion followed by two exact, equal singleton `/api/ps` observations
  around final version, inventory, and details checks. Its nonserializable receipt
  proves only stable runtime-reported post-generation residency on that transport.
  Runtime memory size is not package inventory size. Handler execution, model use,
  resident-page identity, effective identity, and qualification remain false. This
  profile is not joined to the local-judge executor or managed preflight.

## Next logical operations

The detailed handoff is in the
[0.2 grounded engine and CLI plan](planning/0.2-grounded-cli.md). The immediate order
is:

1. Preserve schema-6 lifecycle, migration, retained package-object, SOCK_DIAG,
   isolation, attestation, redaction, cancellation, and informational shadow
   boundaries.
2. Freeze and review one complete Ollama runtime package. The model-package import
   exists, but a runtime package still needs complete helpers, native dependencies,
   source, transformation, and license disposition. Add an exact runtime to the
   production cloud-disable allowlist only after that review passes.
3. Extend the managed operation so its process remains retained through execution and
   direct effective-state observation. Join its proven runtime build, the v0.32.15
   static model binding, exact model-package lease, runtime-reported residency, and
   local-judge receipt while collecting all six missing effective-state relationships.
   This is the next priority because the current managed outcome completes cleanup
   before return and neither static inventory nor API residency proves model use.
4. Keep attached Windows and Linux evidence as observation-only. Do not use it as a
   fallback for a failed managed launch. Windows exact native-load binding and
   managed isolation remain unsupported; macOS remains unsupported.
5. Add a distinct candidate-generation receipt over that same retained runtime and
   model boundary. Keep scorecard batches caller-declared and triage-only because the
   separate evidence bindings do not prove semantics or qualification.
6. Project the existing 49 deterministic and 120 editorial development cases, 169
   total, into preregistered smoke, calibration, and locked manifests without opening
   locked data during tuning.
7. Run smoke, the locked hybrid scorecard, repeatability, and supported-platform
   qualification in that order. Deterministic gates remain the only machine
   acceptance authority, and human adjudication remains the release authority.
8. Finish remaining CLI recovery and packaging evidence, then capture model-backed
   screenshots only from the complete passing release path.

Later work follows the dependency order in the
[phase execution plan index](planning/README.md).
