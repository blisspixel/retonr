# Current implementation state

## Checkpoint

The public repository is implementing milestone 0.1 under the `Retonr` project
identity. External contracts remain provisional. Public source availability does
not freeze package, protocol, or stored-data contracts.

## Implemented

| Component | Current behavior |
| --- | --- |
| `rewrite-types` | Versioned document, candidate, gate, status, reason, edit, and redacted record contracts |
| `rewrite-text-adapter` | Bounded UTF-8 parsing, optional BOM retention, newline fingerprints, exact no-edit output, apply, reparse, and verification |
| `rewrite-engine` | Cancellation, typed value protection, sentinel integrity, hard gates, semantic port, deterministic reason priority, lexicographic selection, and document-atomic abstention |
| `rewrite-model` | Separate immutable artifact, qualification, invalidation, activation-decision, and active-binding contracts |
| `rewrite-inference` | Backend-neutral bounded discovery and generation contracts, cancellation and deadlines, stable redacted errors, and deterministic fake |
| `rewrite-grounded` | Structured masked prompt envelope, exact inference policy, proposal-only candidates, and redacted generation provenance |
| `rewrite-ollama` | IP-literal loopback-only native API adapter with bounded bodies, explicit parameters, concurrency, cancellation, and pre-call and post-call identity checks |
| `rewrite-app` | Model-free candidate check plus provisional grounded application path through the same engine and adapter transaction |
| `retonr` | Provisional `check` command with bounded file reads, JSON or text reports, protected terms, and optional fatal abstention |
| `rewrite-eval` | Versioned positive and hard-negative suite, transformation coverage, four baseline contracts, and redacted aggregate reporting |
| Fuzz targets | Protection round trips and plain-text no-edit byte identity |

The literal semantic evaluator accepts only an identical case-folded alphanumeric
and newline-token sequence. Punctuation can change. Lexical content, newline
placement, unsafe controls, protected values, or structure cannot.

## Verified locally

The August 12, 2026 Windows development checkpoint passes:

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
cargo check --locked --manifest-path fuzz/Cargo.toml --all-targets
cargo build --locked --workspace --all-features --release
```

All 90 Rust unit, integration, process, and documentation tests pass. The measured
Rust line coverage is 90.50 percent overall. Engine orchestration is 93.29 percent,
sentinel protection is 97.14 percent, semantic validation is 100 percent, the
grounded application path is 89.87 percent, the grounded strategy is 94.49 percent,
the Ollama backend is 88.27 percent, and the text adapter is 91.21 percent.

The local stable toolchain can type-check fuzz targets. Sanitizer-backed fuzz
execution requires nightly and is configured as a Linux CI smoke job. Windows,
macOS, and Linux jobs are configured, but this document does not describe remote CI
as passing until an actual run confirms it.

The custom audit database path bypasses a corrupt user-level RustSec cache containing
a duplicate advisory ID. The clean database loaded 1,216 advisories and the 188-crate
dependency graph passed. Dependency sources and licenses pass policy. The two
transitive `syn` major versions remain an allowed cargo-deny warning pending upstream
convergence. Continuous integration uses its own clean runner database.

## Deliberate limitations

- The current CLI checks a supplied candidate. The grounded application path is not
  exposed as a CLI command yet.
- Only UTF-8 plain text up to 16 MiB is accepted.
- Artifact lifecycle contracts exist only in memory. Acquisition, durable storage,
  exact real-artifact qualification, and recovery are not implemented.
- The Ollama adapter is fake-server tested but has not been qualified against a real
  pinned runtime and model artifact on the three operating systems.
- The grounded path can safely accept only literal-mode token-preserving changes
  under the current evaluator. Open-domain paraphrases and broader modes abstain.
- UTF-16, Markdown, DOCX, profiles, persistence, API, MCP, skills, desktop, and voice
  are not implemented yet.
- The model-free evaluator does not assess open-domain paraphrases and must abstain
  on them.
- No public API, schema, package, executable name, or configuration namespace is
  frozen.
- Rewrite records use unkeyed SHA-256 identity digests. These are not anonymization
  and can permit dictionary attacks on short predictable text. Stable local traces
  require an installation-keyed digest decision.
- Screenshots remain gated on the complete release-build behaviors defined by the
  screenshot policy.
- Evaluation data and user-research policies are proposed, not approved. No
  non-synthetic collection is authorized.

## Next logical operations

The detailed handoff is in the
[0.2 grounded engine and CLI plan](planning/0.2-grounded-cli.md). The immediate order
is:

1. Approve or revise the proposed data, consent, user-research, and adjudication
   governance before collecting non-synthetic data.
2. Retain actual Windows, macOS, and Linux continuous-integration results and close
   the open 0.1 refinement findings.
3. Review the proposed artifact, inference, transport, and grounded-authority
   decision records at the 0.2 entry gate.
4. Add partial-body, in-flight deadline, proxy-environment, socket-denial, and
   concurrency conformance evidence for the local backend.
5. Link redacted generation provenance into the durable transaction schema and
   implement artifact lifecycle storage and recovery.
6. Add typed claim and invariant evidence without describing it as semantic proof,
   then calibrate an independent semantic evaluator.
7. Complete stdin, safe diff, dry-run, trace, cancellation, terminal safety, and
   raw-output policy before exposing grounded rewriting in the CLI.
8. Run exact artifact qualification and selective-risk reporting on declared
   hardware tiers.
9. Capture real CLI screenshots only after the 0.2 exit gate passes.

Later work follows the dependency order in the
[phase execution plan index](planning/README.md).
