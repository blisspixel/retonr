# Engineering quality standard

## Definition of done

Work is complete only when:

- Behavior is tied to a requirement or defect.
- Architecture boundaries remain intact.
- Success, failure, cancellation, and abstention paths are tested.
- Formatting, linting, tests, documentation, and policy checks pass.
- Overall Rust line coverage remains at least 80 percent.
- Fidelity-critical crates meet their higher project threshold.
- Windows, macOS, and Linux checks pass where behavior can differ.
- Security, privacy, accessibility, and dependency implications are reviewed.
- Documentation and examples match the implemented behavior.
- No relevant defect found during the work is left as an untracked note.

Coverage is a floor, not a substitute for meaningful assertions.

## Defect severity and disposition

Defects use a versioned project policy and retain the affected revision, artifact,
platform, supported workflow, reproduction or evidence, worst credible impact,
preconditions, reachability, scope, detection source, and linked regression. Frequency
alone cannot reduce the severity of a safety-boundary failure.

| Severity | Project definition |
| --- | --- |
| Critical | A reachable supported path can cause unauthorized code or authority, material private-data disclosure, accepted output that reverses or invents a claim or corrupts a protected critical value, unrecoverable user-data loss, an inaccessible core consent or fidelity decision with no equivalent supported path, distribution without required rights, signing or update compromise, or broad install or migration corruption. |
| High | Severe fidelity, security, privacy, accessibility, licensing, packaging, or availability impact that is bounded to uncommon preconditions, a narrow population, or a reliable recovery path and does not meet the critical definition. |
| Medium | Material incorrect or degraded behavior with a safe workaround and no breached safety, authority, data-retention, or distribution boundary. |
| Low | Localized cosmetic, diagnostic, documentation, or low-impact behavior that does not mislead a safety decision. |

The component owner and relevant domain owner classify each defect with one
independent release reviewer. Security, fidelity, data stewardship, accessibility,
legal or licensing, and release engineering own their respective impact decisions.
Cross-domain defects take the highest applicable severity. Reclassification requires
new retained evidence, the same domain ownership, and independent review; the history
is append-only.

A critical release blocker cannot be risk-accepted or waived. It closes only through
a verified fix, evidence that the report is invalid, or removal or narrowing of the
affected capability. Closure records cause, affected versions and artifacts, the
regression or qualification fixture, verification on applicable platforms, and any
user communication. High-risk acceptance requires an explicit record with owner,
scope, mitigation, affected versions, user impact, expiry, and review point.

A surviving mutant is triaged as a defect when it exposes a missing material decision
or assertion. A mutant that can bypass a critical boundary blocks release until it is
killed, made unreachable by a reviewed design change, or the affected capability is
removed. Other high-risk mutants require the same evidence and disposition as a high
defect. Phase exit language about critical defects refers to this policy.

## Rust standards

### Language and toolchain

- Rust 1.97.1 is pinned for the initial implementation.
- Edition 2024 and Cargo resolver 3 are used.
- The lockfile is committed.
- Warnings are denied in continuous integration.
- Stable Rust is the product toolchain. Nightly is limited to isolated fuzzing or
  diagnostics jobs.

### Error handling

- Libraries return typed errors with stable categories.
- `anyhow` is limited to binaries and infrastructure boundaries.
- Library code does not call `process::exit`.
- Production code does not use `unwrap`, `expect`, `panic`, `todo`, or
  `unimplemented` for recoverable input or runtime conditions.
- Errors preserve context without leaking content, credentials, or full paths by
  default.
- Cancellation is not reported as an internal error.

### Safety

- Pure domain crates use `#![forbid(unsafe_code)]`.
- Required FFI is isolated in a small infrastructure crate.
- Every unsafe block has a local safety argument and dedicated tests.
- Untrusted input is bounded before allocation, parsing, generation, or decompression.

### API design

- Public types have one responsibility and explicit invariants.
- Stored and wire schemas are versioned.
- Persistence representations do not leak into public application contracts.
- Feature flags are additive and independently tested.
- Configuration precedence is documented and deterministic.
- Time, randomness, filesystem, model, and network dependencies are injected where
  they affect tests.

## No god files or modules

The repository policy script enforces a default maximum of 500 nonblank,
noncomment lines per production Rust file. Clippy's `too_many_lines` lint enforces a
default maximum of 100 lines per function. An exception requires a checked-in
rationale with an owner and a plan to prevent further growth.

When the desktop frontend is introduced, the repository policy script enforces 350
nonblank, noncomment lines per production TypeScript file and 200 per production TSX
component. Tests, fixtures, and generated contracts are excluded and reviewed through
their own gates. Exceptions use the same checked-in rationale process. Frontend
feature logic is split by workflow and state ownership, not by arbitrary line ranges.

Review guidance is stricter than the hard limit:

- A production module approaching 300 lines receives a responsibility review.
- A function approaching 50 lines receives a decomposition review.
- A type with unrelated mutation responsibilities is split even if it is short.
- Catch-all `utils`, `helpers`, and `manager` modules are not dumping grounds.
- Files are split by domain behavior, not arbitrary line ranges.
- Tests may be larger when a data table is clearer in one place, but shared fixtures
  and builders still have explicit owners.

Line count is a signal. Cohesion and dependency direction decide the design.

## Required continuous integration

Every pull request runs:

```console
cargo fmt --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo nextest run --locked --workspace --all-features
cargo test --locked --workspace --all-features --doc
cargo llvm-cov --locked --workspace --all-features --fail-under-lines 80
cargo doc --locked --workspace --all-features --no-deps
cargo deny check
cargo audit
```

The operating-system matrix uses the pinned nextest runner when its verified release
artifact is available. If that external download fails, the job records the installer
failure and runs the same locked workspace and feature set with `cargo test`. Test
execution is not skipped because a test-runner distribution endpoint is temporarily
unavailable.

Every dependency-resolving Cargo command uses the committed lockfile. A stale
`Cargo.lock` fails instead of being rewritten on a runner. `RUSTDOCFLAGS=-D warnings`
applies to documentation builds. Functional and release
build jobs run on Windows, macOS, and Linux. Coverage runs on Linux x86_64 for stable
tooling behavior. Once installable artifacts exist, release-stage continuous
integration adds package installation and installed-binary smoke tests on each target
platform before the corresponding milestone can close.

Repository policy checks reject:

- Prohibited dash characters
- Emojis in repository-owned text and code
- Generated or tool authorship attribution
- Unapproved oversized Rust source files
- Broken required local documentation links

The 0.2 model-manifest check adds rejection of missing model license, source, size,
runtime, or checksum metadata before any model artifact can be activated or
distributed. Documentation must not claim that gate is active before its schema and
fixtures exist.

Once production Rust code exists, continuous integration is never considered green
without the coverage job.

Once desktop code exists, continuous integration also runs frontend formatting,
strict type checking, linting, unit and component tests, accessibility checks,
coverage, production build, dependency audit, and checked-in generated-contract
verification. Signed-binary desktop tests remain target-platform release jobs where
credentials or native packaging prevent ordinary pull-request execution.

Frontend coverage uses pinned Vitest and `@vitest/coverage-v8`. A general checked-in
configuration enforces the 80 percent line and branch floor. A second reviewed config
selects authorization presentation, IPC adapters, state reducers, transcript
confirmation, evidence admission, and cancellation modules and enforces 90 percent
branch coverage. Generated DTOs, third-party code, fixtures, and platform bootstrap
are excluded only through a reviewed list. Both reports are retained as CI artifacts.

## Coverage policy

- Repository line coverage floor: 80 percent
- Frontend line and branch coverage floor once introduced: 80 percent
- Deterministic validation and edit application target: at least 90 percent
- Adapter parse, apply, and verify logic target: at least 90 percent
- Rust authorization, IPC adapters, transcript confirmation, evidence admission,
  audio lifecycle, and cancellation target: at least 90 percent line coverage plus
  region trend review, explicit decision-table fixtures, and mutation testing
- Frontend authorization presentation, IPC adapters, state reducers, transcript
  confirmation, evidence admission, and cancellation target: at least 90 percent
  branch coverage
- Security-sensitive parser and authorization decisions: every branch represented by
  a test or a documented unreachable invariant
- Generated bindings, third-party fixtures, and platform bootstrap code may be
  excluded only through a reviewed coverage configuration

Release qualification retains coverage reports as build artifacts. A falling trend
is reviewed even when the floor still passes.

Rust branch instrumentation in the pinned `cargo-llvm-cov` toolchain is not a stable
numeric gate. The project does not claim one. A future Rust branch threshold requires
a pinned, cross-platform report pipeline and parser that is tested before the policy
changes.

## Test strategy

### Unit tests

Use small deterministic tests for domain types, rule precedence, plans, gates,
ranking, status decisions, migrations, and error translation.

### Property tests

Use generated data for:

- Span ordering and non-overlap
- Applying edits from right to left
- Byte identity outside approved ranges
- Serialize and deserialize round trips
- Tree consistency
- Invariant and sentinel round trips
- Retrieval determinism and per-source caps
- Atomic write planning

### Golden tests

Use reviewed fixtures for stable traces, CLI output, Markdown transformations, and
OOXML package behavior. Snapshots require semantic assertions and cannot be accepted
blindly after a broad change.

### Integration tests

- Use both a pure scripted inference fake and a fake model service at the HTTP
  boundary.
- Cover version, inventory, details, generation, artifact drift, malformed and
  unknown fields, incomplete output, nonempty reasoning, oversized headers and
  bodies, stalled connections, late responses, redirects, disconnects, 4xx, 5xx,
  timeouts, cancellation, and response decompression limits.
- Exercise the CLI as a process.
- Exercise SQLite migrations from every supported schema version.
- Exercise cancellation and timeout propagation.
- Exercise non-TTY stdin and output.
- Exercise local API and MCP schemas against compatibility fixtures.

### Fuzzing

Fuzz Markdown, JSON, XML, ZIP metadata, source spans, profile imports, trace imports,
API payloads, and sentinel restoration. Fuzz targets have memory and time limits and
retain every crashing input as a regression fixture.

### Mutation testing

Scheduled mutation testing covers deterministic gates, ranking, planner decisions,
adapter verification, profile precedence, and authorization. The initial score is
recorded, weak assertions are repaired, and a meaningful project threshold is set
before 1.0.

### Real-model evaluation

Real-model tests are qualification jobs, not ordinary unit tests. They pin artifacts,
run on declared hardware, emit selective-risk reports, and never replace deterministic
or fake-backend coverage.

Benchmark data is versioned as smoke, development, calibration, locked, or red team.
Locked data never becomes prompt, tuning, or regression-training input. Qualification
is invalidated when an artifact, runtime, tokenizer, quantization, prompt, parameter,
evaluator, calibration, strategy, or locked-suite identity changes.

## Cross-platform test matrix

Required test cases include:

- LF and CRLF
- Byte order marks and final-newline preservation
- Unicode prose and combining characters
- Non-UTF-8 Unix paths
- Windows reserved names, long paths, file locks, and case collisions
- Symlinks and ambiguous overwrite targets
- Read-only files and directories
- Interrupted atomic replacement
- Missing model runtime and unreachable local service
- Cancellation during database, HTTP, model, and child-process work
- Tauri behavior on WebView2, WKWebView, and WebKitGTK
- Microphone permission, denial, device loss, and audio cancellation

Release packaging is built and smoke-tested on the operating system it targets.

## CLI quality

- Data uses standard output; diagnostics use standard error.
- Non-interactive use never prompts.
- Machine output has versioned schemas and stable codes.
- Terminal styling respects `NO_COLOR` and explicit controls.
- Progress and animation disappear in pipes and logs.
- Commands provide examples, recovery guidance, completion, and manual pages.
- Destructive actions are opt-in and recoverable where possible.
- Quiet mode is genuinely quiet.
- Verbose mode adds diagnostics without exposing content or secrets by default.
- Performance and memory are benchmarked on published hardware tiers.

## Desktop quality

Version 1.0 targets WCAG 2.2 AA and passes:

- Automated accessibility checks
- Keyboard-only workflows
- Screen-reader reviews on supported platforms
- High contrast and reduced motion
- Text scaling and zoom
- Accessible diff alternatives
- Permission, offline, and network-state clarity
- Loading, cancellation, empty, unsupported, abstained, and error states
- Cross-webview functional and visual checks

The interface uses a deliberate token system for color, type, spacing, motion, and
focus. It avoids generic component accumulation and keeps domain terminology
consistent with the CLI and API.

Desktop release evidence also requires:

- An explicit Tauri command manifest and capability allowlist
- Negative authorization tests for every privileged command
- Strict content security policy and no remote frontend assets
- Operation-ID and event-sequence tests for stale or late updates
- Browser-mode tests with mocked Tauri commands
- Instrumented WebdriverIO tests in a dedicated non-release feature, with required
  Tauri test plugins absent from the release dependency graph, capabilities, bundle,
  listeners, and permissions
- Black-box tests of the unmodified signed binary on WebView2, WKWebView, and
  WebKitGTK; macOS uses an external XCTest, accessibility, or equivalent harness
  selected and qualified by the desktop decision record
- Controlled per-platform visual baselines with human review
- Manual Narrator, VoiceOver, and Orca workflows

## Refinement passes

A milestone cannot close after the first correct implementation. It receives four
explicit refinement passes:

### Pass 1: correctness and boundaries

- Verify requirements and failure semantics.
- Remove duplicate logic and leaky abstractions.
- Check module ownership and dependency direction.
- Confirm errors, cancellation, and abstention are explicit.

### Pass 2: adversarial and cross-platform

- Add malformed, ambiguous, and resource-exhaustion cases.
- Exercise Windows, macOS, and Linux differences.
- Run properties, fuzz seeds, and mutation testing for changed critical paths.
- Verify no hidden network or filesystem assumptions.

### Pass 3: security, privacy, accessibility, and licenses

- Review trust boundaries and authority.
- Check logs, traces, deletion, credentials, model manifests, and corpus handling.
- Run accessibility reviews for affected UI or CLI surfaces.
- Review dependency and model licenses.

### Pass 4: simplification and release readiness

- Remove dead code, placeholders, speculative abstractions, and stale comments.
- Replace vague names and generic wrappers with domain language.
- Verify examples, migrations, upgrade paths, packaging, and rollback.
- Recapture screenshots from a passing build when visible behavior changed.

Each pass must improve tests or record why existing coverage is sufficient.

## Implementation hygiene checklist

Reviewers explicitly reject:

- Placeholder implementations presented as complete
- Comments that restate syntax instead of explaining an invariant
- Layers that only rename another layer
- Generic maps and strings where domain types prevent invalid states
- Repeated parsing or serialization at internal boundaries
- Unbounded retries, allocations, concurrency, or decompression
- Tests that assert only that code does not crash
- Snapshots accepted without understanding the change
- Broad compatibility claims without a conformance matrix
- Silent fallbacks that change privacy, network, model, or fidelity behavior
- Documentation that describes a planned feature as available

The standard applies regardless of how a change was produced.
