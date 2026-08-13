# Current implementation state

## Checkpoint

The public repository completed the milestone 0.1 technical evidence and is
implementing milestone 0.2 under the `Retonr` project identity. External contracts
remain provisional. Public source availability does not freeze package, protocol,
or stored-data contracts.

## Implemented

| Component | Current behavior |
| --- | --- |
| `rewrite-types` | Versioned document, candidate, gate, status, reason, edit, and redacted record contracts |
| `rewrite-text-adapter` | Bounded UTF-8 parsing, optional BOM retention, newline fingerprints, exact no-edit output, apply, reparse, and verification |
| `rewrite-engine` | Cancellation, typed value protection, sentinel integrity, hard gates, semantic port, deterministic reason priority, lexicographic selection, and document-atomic abstention |
| `rewrite-model` | Separate immutable artifact, qualification, invalidation, activation-decision, and active-binding contracts |
| `rewrite-model-store` | Durable SQLite artifact records, content-bound qualification identities, immediate activation transactions, invalidation, active-removal protection, mandatory byte-verification callbacks, fail-closed recovery, and bounded coherent artifact-state inventory |
| `rewrite-inference` | Backend-neutral bounded discovery and generation contracts, cancellation and deadlines, stable redacted errors, and deterministic fake |
| `rewrite-grounded` | Structured masked prompt envelope, exact inference policy, proposal-only candidates, and redacted generation provenance |
| `rewrite-ollama` | IP-literal loopback-only native API adapter with bounded bodies, explicit parameters, concurrency, cancellation, and pre-call and post-call identity checks |
| `rewrite-app` | Model-free candidate check, provisional grounded path, cancellable regular-file offline import, and read-only artifact reconciliation with shared locking, exact names, bounded hashing, stable metadata checks, deterministic classifications, and no storage mutation |
| `retonr` | Provisional `check` command with bounded file reads, JSON or text reports, protected terms, and optional fatal abstention |
| `rewrite-eval` | Versioned positive and hard-negative suite, transformation coverage, four baseline contracts, two balanced synthetic editorial groups, and redacted aggregate reporting |
| Fuzz targets | Protection round trips and plain-text no-edit byte identity |

The literal semantic evaluator accepts only an identical case-folded alphanumeric
and newline-token sequence. Punctuation can change. Lexical content, newline
placement, unsafe controls, protected values, or structure cannot.

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

All 175 Rust unit, integration, and process tests pass. One process helper is
intentionally ignored by the ordinary runner and exercised by its isolated parent
test. Documentation tests also pass. The measured Rust line coverage is 91.34
percent overall. The repository's 80 percent line coverage floor passes with margin.

The local nightly toolchain can type-check both fuzz targets. The cargo-fuzz project
supports its libFuzzer execution path on Unix-like targets, so Windows is not
reported as a local fuzz-execution result. Continuous integration runs both bounded
targets under the Linux sanitizer-backed fuzz smoke job. `cargo-nextest` is not
installed in the local environment, so this checkpoint used the documented
`cargo test` fallback.

The latest exact-main remote evidence before this focused inventory branch is
revision `e03c6cd5fc79b519a277923030a17b985e959dd6` in the passing
[quality workflow](https://github.com/blisspixel/retonr/actions/runs/31671590402).
Remote evidence for the inventory branch remains pending publication. The retained
jobs cover Windows, macOS, and Linux Rust checks, repository policy, Markdown,
coverage, dependency and supply-chain policy, fuzz smoke, proxy isolation,
concurrency, and the Ubuntu loopback-only network namespace.

The custom audit database path bypasses a corrupt user-level RustSec cache containing
a duplicate advisory ID. The clean database loaded 1,216 advisories and the current
230-crate graph passed. Dependency sources and licenses pass policy. Reviewed
duplicate-version warnings now include the target-only capability filesystem
dependency tree and the two transitive `syn` major versions. Continuous integration
uses its own clean runner database.

## Deliberate limitations

- The current CLI checks a supplied candidate. The grounded application path is not
  exposed as a CLI command yet.
- The editorial corpus contract and 39 synthetic fixtures across two groups are
  implemented, but no lint scanner, rule catalog, or live anti-slop ranking path is
  implemented yet.
- Only UTF-8 plain text up to 16 MiB is accepted.
- Durable artifact lifecycle state, recovery, single-file offline import, and
  read-only managed-byte inventory are implemented. Inventory verifies registered
  files, reports manifest-only state, and identifies safe orphan candidates and
  conflicts without mutation. Artifact-set and folder import, downloads,
  runtime-native pulls, orphan repair or reclamation, managed-byte removal, CLI
  commands, and exact real-artifact qualification are not implemented. Local
  application-owned filesystems are the qualified boundary; network filesystem
  locking and replacement semantics are not.
- The Ollama adapter is fake-server tested but has not been qualified against a real
  pinned runtime and model artifact on the three operating systems.
- The grounded path can safely accept only literal-mode token-preserving changes
  under the current evaluator. Open-domain paraphrases and broader modes abstain.
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

1. Complete the mutation side of the headless artifact lifecycle: exclusively
   reverify and reconcile selected orphans, safely remove inactive managed bytes,
   and expose import, inventory, and recovery through explicit model commands.
2. Link redacted generation provenance into the durable rewrite transaction schema.
3. Add typed claim and invariant evidence without describing it as semantic proof,
   then calibrate an independent semantic evaluator.
4. Complete stdin, safe diff, dry-run, trace, cancellation, terminal safety, and
   raw-output policy before exposing grounded rewriting in the CLI.
5. Run exact artifact qualification and selective-risk reporting on declared
   hardware tiers.
6. Capture the model-backed rewrite, abstention, diff, and trace CLI screenshots only
   after the 0.2 completion evidence passes. The current candidate-check rendering is
   limited to already implemented model-free behavior.

Later work follows the dependency order in the
[phase execution plan index](planning/README.md).
