# Current implementation state

## Checkpoint

The public repository completed the milestone 0.1 technical evidence and is
implementing milestone 0.2 under the `Retonr` project identity. External contracts
remain provisional. Public source availability does not freeze package, protocol,
or stored-data contracts.

## Implemented

| Component | Current behavior |
| --- | --- |
| `rewrite-types` | Versioned document, candidate, gate, status, reason, edit, rewrite-record v2, redacted generation provenance, and content-redacted typed claim-evidence contracts |
| `rewrite-text-adapter` | Bounded UTF-8 parsing, optional BOM retention, newline fingerprints, exact no-edit output, apply, reparse, and verification |
| `rewrite-engine` | Cancellation, typed value protection, sentinel integrity, hard gates, closed structure and semantic evidence boundaries, deterministic claim comparison, reason priority, lexicographic selection, and document-atomic abstention |
| `rewrite-model` | Separate immutable artifact, qualification, invalidation, activation-decision, and active-binding contracts |
| `rewrite-model-store` | Durable SQLite artifact records, content-bound qualification identities, immediate activation transactions, invalidation, active-removal protection, opaque exclusive-lock capability requirements for removal transitions, mandatory byte-verification callbacks, fail-closed recovery, and bounded coherent artifact-state inventory |
| `rewrite-inference` | Backend-neutral bounded discovery, adapter-admitted output-contract digests, candidate generation, and structured-completion contracts with content-redacted debug and error surfaces, cancellation, deadlines, and deterministic fakes |
| `rewrite-grounded` | Structured masked prompt envelope, exact inference policy, proposal-only candidates, and redacted generation provenance |
| `rewrite-ollama` | IP-literal loopback-only native API adapter with bounded bodies, explicit parameters, exact candidate-contract discovery, candidate and structured completion, terminal-stop enforcement, concurrency, cancellation, and pre-call and post-call identity checks |
| `rewrite-app` | Model-free candidate check, provisional grounded path, pinned source-preserving regular-file offline import, read-only managed inventory with application-owned result DTOs, pending-operation inspection, selected orphan reconciliation, crash-recoverable inactive removal with exact pinned-lock capability binding, and verified runtime artifact lease groundwork |
| `retonr` | Provisional `check` command plus an explicit-root offline model-artifact CLI for import, inventory, pending-operation inspection, selected reconciliation, inactive removal, and exact removal recovery |
| `rewrite-eval` | Versioned positive and hard-negative suite, transformation coverage, four baseline contracts, two balanced synthetic editorial groups, and redacted aggregate reporting |
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
debug views omit prompt and generated content. The current Ollama implementation
admits only the existing candidate schema. No claim-extraction schema, strategy,
role, qualification, or application evidence join is implemented.

## Verified locally

The August 13, 2026 Windows development checkpoint passes:

```console
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
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

All 323 Rust unit, integration, and process tests pass. Two process helpers are
intentionally ignored by the ordinary runner and exercised by isolated parent tests.
Documentation tests also pass. The measured Rust line coverage is 90.92
percent overall. The repository's 80 percent line coverage floor passes with margin.

The local nightly toolchain can type-check both fuzz targets. The cargo-fuzz project
supports its libFuzzer execution path on Unix-like targets, so Windows is not
reported as a local fuzz-execution result. Continuous integration runs both bounded
targets under the Linux sanitizer-backed fuzz smoke job. `cargo-nextest` is not
installed in the local environment, so this checkpoint used the documented
`cargo test` fallback.

The provider-neutral structured-completion slice passed at exact-main revision
`35ed2cdd43afa99a3cca51fed0b2f0e43c693a9a` in the passing
[quality workflow](https://github.com/blisspixel/retonr/actions/runs/31755221203).
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
  files. The grounded generation path is not exposed as a CLI command yet.
- The editorial corpus contract and 39 synthetic fixtures across two groups are
  implemented, but no lint scanner, rule catalog, or live anti-slop ranking path is
  implemented yet.
- Only UTF-8 plain text up to 16 MiB is accepted.
- Durable artifact lifecycle state, bounded staging recovery, pinned single-file
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
  lease yet. Artifact-set and folder import, downloads,
  runtime-native pulls, bulk reconciliation, orphan deletion, runtime commands, and
  exact real-artifact qualification are not implemented. The CLI requires one
  explicit `--data-dir`; its six artifact commands do not use the network or apply
  schema migrations. `pending-operations` reads only bounded durable state and
  returns exact prepared-removal generations without opening or hashing model
  bytes. The current product ceilings are 256 GiB per artifact, 4,096
  durable or storage entries, 512 GiB of aggregate inventory verification, and
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
- The typed claim contract and deterministic comparator are implemented, but no
  learned extractor or runtime-backed semantic evaluator is connected. The current
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
   DTOs, non-mutating pending-operation inspection, and process-level signal
   cancellation evidence as new consumers are added.
2. Preserve the completed rewrite-record v2, typed invariant summaries, typed claim
   evidence, and deterministic comparison boundary.
3. Add the distinct claim-extraction role, exact extractor manifest, strict ephemeral
   wire contract, and application-level cancellable pair operation. Join evidence to
   a two-phase engine path only after fake-backend conformance passes, then calibrate
   it independently from generators in shadow mode.
4. Add the local evaluation plan, run the currently installed 26B and 27B packages,
   and add the previously observed 8B package only after revalidation or separately
   approved acquisition. Start only after product-path evidence joins are complete.
5. Complete stdin, safe diff, dry-run, trace, terminal safety, and
   raw-output policy before exposing grounded rewriting in the CLI.
6. Run exact artifact qualification and selective-risk reporting on declared
   hardware tiers.
7. Capture the model-backed rewrite, abstention, diff, and trace CLI screenshots only
   after the 0.2 completion evidence passes. The current candidate-check rendering is
   limited to already implemented model-free behavior.

Later work follows the dependency order in the
[phase execution plan index](planning/README.md).
