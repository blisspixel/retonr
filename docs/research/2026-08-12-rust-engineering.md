# Rust engineering and release research

## Review status

Reviewed: August 12, 2026.

Scope: Rust language and Cargo policy, crate and API design, linting,
documentation, testing, unsafe code, cross-platform qualification, dependency
governance, reproducible builds, and release provenance.

This note records primary-source findings and planning consequences. It does not
claim that a proposed check is implemented or that a release is qualified.
Tool versions, target tiers, compiler behavior, and third-party tooling must be
revalidated when a gate is implemented or a pinned toolchain changes.

## Executive conclusions

The current Rust foundation is strong. The workspace pins Rust 1.97.1, the current
patched stable release at review time, and uses edition 2024, resolver 3, a committed
lockfile, workspace lint inheritance, `unsafe_code = "forbid"`, documented public
APIs, warnings-as-errors Clippy, rustdoc warnings, three-operating-system tests,
coverage, fuzz smoke, and dependency policy checks. Rust 1.97.1 specifically fixed
an LLVM miscompilation, which is useful evidence for pinning the patch release rather
than only a minor channel.

The main missing pieces are release-specific rather than basic code hygiene:

- Separate the exhaustive validation feature set from the feature set shipped to
  users. `--all-features` is appropriate for finding conditional-code defects but is
  not automatically an appropriate production build. The current `test-support`
  feature makes this distinction concrete.
- Treat `rust-version` as a verified support floor and `rust-toolchain.toml` as the
  exact repository toolchain. Add controlled stable and beta forward checks without
  letting moving channels produce release artifacts.
- Add targeted Miri and sanitizer lanes before accepting any first-party unsafe or
  native FFI. Preserve `forbid(unsafe_code)` in every existing domain and service
  crate.
- Add Rust API compatibility checks only when a Rust API is deliberately frozen.
  Continue to use separate schema and behavior conformance for CLI JSON, stored data,
  local API, MCP, and agent-facing contracts.
- Progress from lockfile scanning to dependency vetting, offline source closure,
  binary dependency metadata, per-artifact checksums, signatures, and build
  provenance before public release.
- Define reproducibility as an evidence-producing release workflow. A committed
  lockfile and `--locked` make dependency resolution deterministic; they do not by
  themselves prove byte-for-byte reproducible binaries.

Reference-grade does not mean enabling every available lint or experimental
mitigation. It means every guarantee has an owner, a stable input set, a fail-closed
gate, retained evidence, and a documented revalidation trigger.

## Durable engineering invariants

These invariants should survive tool, dependency, and product growth:

1. Stable Rust builds every shipping artifact. A date-pinned nightly may run an
   isolated diagnostic, fuzz, Miri, or sanitizer job but never silently becomes the
   product compiler.
2. The repository records both the exact compiler used for development and release
   and the minimum compiler version it claims to support. Both are tested according
   to their stated meaning.
3. Every existing first-party crate remains free of unsafe Rust. Any unavoidable FFI
   exception is isolated behind a safe interface in a dedicated adapter crate and
   requires explicit safety evidence.
4. Invalid states are rejected at trust boundaries and represented with domain
   types internally. Permissive parsing does not replace validation.
5. Every feature is additive unless an incompatibility is enforced with a compile
   error and documented as unsupported. Shipping features are explicit and are not
   inferred from the test matrix.
6. All supported targets compile, test, and package on their target operating
   system. Cross-compilation alone is not runtime support evidence.
7. No recoverable input, transport, cancellation, resource, filesystem, or model
   condition reaches panic or process abort.
8. Tests cover decisions and failure semantics, not only statements. Coverage is a
   floor; property, differential, fuzz, mutation, and compatibility evidence cover
   different defect classes.
9. A dependency is not accepted solely because it is popular, warning-free, or free
   of a known advisory. Source, license, maintainer risk, unsafe surface, build-time
   authority, and deployment criteria are reviewed separately.
10. Every distributed artifact maps to exact source, compiler, target, dependencies,
    build parameters, checksum, signature, provenance, and qualification evidence.
11. A reproducibility claim is made only for the target and packaging stage that was
    independently rebuilt and compared. Signing and notarization are recorded as
    later transformations when they introduce nondeterminism.
12. Automation assists review but never expands a compatibility, safety, or platform
    claim beyond retained evidence.

## Toolchain and language policy

### Findings

Cargo's `rust-version` field declares a supported Rust floor. Cargo uses it for
diagnostics and dependency selection, and its support expectations cover every
target and feature. Rustup's `rust-toolchain.toml` selects a repository toolchain and
can pin its components. These are related but different contracts.

Edition 2024 implies resolver 3 for ordinary packages. A virtual workspace must set
the resolver explicitly, as this repository does. Resolver 3 uses Rust-version-aware
fallback during dependency resolution. A lockfile still has priority when it is
usable.

The official Cargo CI guidance recommends testing the claimed Rust version, testing
new dependency resolution separately, and considering a pinned toolchain when new
warnings would otherwise break the main branch. Moving beta or nightly channels are
valuable early-warning systems but are not reproducible release inputs.

### Planning consequences

- Keep `rust-toolchain.toml` on an exact stable patch and keep `rust-version` aligned
  while Retonr supports only that exact build floor. If an older support floor is
  promised later, stop changing `rust-version` with every toolchain update and add a
  dedicated full floor-version matrix.
- Update the stable toolchain through a focused change. Review Rust and Cargo release
  notes, refresh Clippy expectations, run the complete platform and feature matrix,
  run qualification-sensitive tests, and retain the CI run before merging.
- Add non-release forward jobs for the current stable and beta channels. Stable is a
  dependency and compiler compatibility canary. Beta is an upcoming compiler and
  lint canary. A forward-job failure must be triaged, but a moving channel must not
  rewrite the lockfile or publish artifacts.
- Keep nightly tools date-pinned independently. Update a nightly only with the Miri,
  sanitizer, or fuzz tool that requires it, then rerun that lane's retained corpus.
- Use `cargo report future-incompatibilities` when Cargo reports affected
  dependencies. A future-incompatibility warning needs an issue or dependency action
  before the next stable toolchain is adopted.
- Treat a future Rust edition as a migration project. Run migration lints across all
  supported features and targets, review generated changes, then change the edition
  only after the existing edition is green.

### Primary sources

- [Rust 1.97.1 release](https://blog.rust-lang.org/2026/07/16/Rust-1.97.1/)
- [Cargo Rust version](https://doc.rust-lang.org/cargo/reference/rust-version.html)
- [Cargo dependency resolver](https://doc.rust-lang.org/cargo/reference/resolver.html)
- [Edition 2024 resolver](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html)
- [Rustup toolchain overrides](https://rust-lang.github.io/rustup/overrides.html)
- [Rustup profiles](https://rust-lang.github.io/rustup/concepts/profiles.html)
- [Cargo continuous integration](https://doc.rust-lang.org/cargo/guide/continuous-integration.html)
- [Cargo future incompatibility reports](https://doc.rust-lang.org/cargo/commands/cargo-report-future-incompatibilities.html)
- [Rust edition migration](https://doc.rust-lang.org/edition-guide/editions/transitioning-an-existing-project-to-a-new-edition.html)

## Crate, API, and invariant design

### Findings

The Rust API Guidelines favor standard naming and conversion traits, validated
arguments, common trait implementations, private fields, newtypes, and deliberate
future-proofing. They explicitly reject applying the robustness principle as an
excuse to accept invalid input. Validation should be static through types where
practical and dynamic at the boundary otherwise.

Cargo features are unified through a dependency graph. Cargo therefore recommends
additive features and warns that feature combinations need an explicit testing
strategy. Removing features or moving existing APIs behind features can be a SemVer
break. `--all-features` validates one important combination, not every supported
combination.

### Planning consequences

- Keep domain contracts in leaf crates and adapters at the edge. Domain crates must
  not depend on HTTP, CLI, persistence, model runtime, MCP, desktop, or FFI types.
- Make construction enforce invariants. Prefer newtypes and private fields for IDs,
  byte ranges, limits, digests, versions, operation states, authority scopes, and
  validated paths.
- Keep wire and stored representations separate from application types. Conversion
  is the validation boundary and must reject unknown versions, invalid combinations,
  unbounded values, and ambiguous defaults deterministically.
- Document each public type's invariant, units, ownership, error behavior, and
  compatibility status. The current crate-level `deny(missing_docs)` policy is a
  strong baseline and should remain.
- Keep Cargo features capability-oriented and additive. Do not use a feature to
  select a mutually exclusive runtime or security policy when a typed runtime choice
  or separate crate works.
- Define a supported feature matrix containing at least default, no-default, each
  shipping feature group, and all-features. A feature combination not exercised by
  CI is not supported merely because it compiles on one developer machine.
- Define an exact shipping manifest for each binary. The release build must name the
  package and intended features and must exclude `test-support`, fuzz hooks,
  instrumented desktop hooks, and diagnostic-only capabilities. Keep the
  all-features build as a separate validation job.
- Use compile-time negative tests for mutually exclusive features if such a pair is
  unavoidable. Document that combination as unsupported.

### Primary sources

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust API Guidelines checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
- [Rust API input validation guidance](https://rust-lang.github.io/api-guidelines/dependability.html)
- [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)
- [Cargo SemVer compatibility](https://doc.rust-lang.org/cargo/reference/semver.html)

## Linting, documentation, and review discipline

### Findings

Clippy's `all` group is intended to be broadly reliable. The `pedantic` group is
stricter and expects project judgment. Clippy documents scoped exceptions as normal
when a lint does not fit. Rust lint reasons and the `expect` level allow an exception
to explain itself and become visible when it is no longer needed.

Rustdoc warns about broken intra-doc links by default, while missing documentation
is opt-in. Documentation examples can compile and execute as tests. Rustdoc tests
public interfaces and do not replace unit tests of private behavior.

### Planning consequences

- Retain `all` and `pedantic` at deny, the explicit anti-placeholder lints, and
  `-D warnings` in CI. Do not enable changing `nursery` or broad `restriction` groups
  without reviewing every lint in the pinned compiler.
- Prefer rewriting code over suppressing a lint. If a justified exception becomes
  necessary, use the narrowest scope and a reason-bearing `expect` where possible.
  A project-level exception needs an owner and revalidation trigger.
- Add Cargo's warnings-deny configuration to CI so non-Clippy first-party builds have
  the same warnings policy. Continue to run Clippy across the whole workspace, all
  targets, and the defined validation feature sets.
- Preserve `RUSTDOCFLAGS=-D warnings`, separate documentation tests, and
  `deny(missing_docs)`. Public examples must assert meaningful output or state, not
  only compile.
- Treat lint changes as part of a compiler update. Do not conceal new warnings with a
  global allow to make a toolchain bump pass.
- Keep repository size checks as design prompts, not as substitutes for cohesion,
  ownership, and dependency review.

### Primary sources

- [Clippy usage](https://doc.rust-lang.org/stable/clippy/usage.html)
- [Clippy lint configuration](https://doc.rust-lang.org/clippy/lint_configuration.html)
- [Rust lint levels](https://doc.rust-lang.org/rustc/lints/levels.html)
- [Rust diagnostic attributes](https://doc.rust-lang.org/reference/attributes/diagnostics.html)
- [Rustdoc lints](https://doc.rust-lang.org/rustdoc/lints.html)
- [Rustdoc tests](https://doc.rust-lang.org/rustdoc/documentation-tests.html)

## Unsafe code, FFI, Miri, and sanitizers

### Findings

Rust's safety boundary is not confined to the text of an unsafe block. Unsafe code
can depend on invariants established by safe code elsewhere in its module. The
Rustonomicon therefore identifies module privacy as a core tool for containing the
trusted surface. The Rust Book recommends small unsafe blocks wrapped by a safe
abstraction.

Miri detects several classes of undefined behavior, including invalid memory access,
uninitialized data use, alignment and type-invariant violations, and some data races.
It requires nightly and cannot execute every operating-system or foreign function.
An unsupported Miri operation is not evidence that the tested code is correct.

Rust sanitizers are unstable compiler features. AddressSanitizer, LeakSanitizer,
MemorySanitizer, and ThreadSanitizer cover different defect classes and have
different target support. The Rust documentation explicitly positions sanitizers as
complements to testing and fuzzing rather than complete proofs.

`cargo-fuzz` uses libFuzzer and sanitizer support. Its documented platform scope is
Unix-like x86-64 and Arm64, not Windows. Cross-platform product support therefore
cannot be inferred from a Linux fuzz job.

### Planning consequences

- Keep `forbid(unsafe_code)` in every existing crate. Prefer a process boundary or an
  audited safe dependency before introducing first-party FFI.
- If first-party unsafe becomes unavoidable, require an architecture decision before
  code lands. Create one dedicated adapter crate with explicit lint policy rather
  than weakening the workspace baseline for all crates.
- For every unsafe operation, document the caller obligations, local proof, aliasing,
  lifetime, alignment, initialization, ownership, thread, panic, and cancellation
  assumptions that apply. Keep invariant-bearing fields private.
- Expose only a safe, bounded facade to the application. Fuzz and property-test the
  safe facade, and use hostile safe implementations of any callback or trait that the
  unsafe code trusts.
- Add a date-pinned Miri job for pure critical crates now and make it mandatory for
  any future unsafe adapter. Record every `cfg(miri)` exclusion with a reason and an
  equivalent native test.
- Run Miri with multiple seeds for concurrency or allocation-sensitive critical
  code. Keep an ordinary `cargo miri test` lane where inter-test shared-state races
  matter; one-test-per-process acceleration changes that detection surface.
- Add target-qualified AddressSanitizer and ThreadSanitizer jobs when native code or
  unsafe is admitted. Use only officially supported target combinations and record
  exclusions. Continue real native tests on Windows even when a sanitizer is not
  supported there.
- Expand fuzzing from short pull-request smoke to retained, scheduled corpora for
  every untrusted parser and edit boundary. Minimize each crash and commit it as a
  deterministic regression fixture before closing the defect.
- State precisely that the current two fuzz targets run under the fuzz tool's default
  sanitizer. Do not describe two fuzz targets as two independent sanitizer classes.

### Primary sources

- [Unsafe Rust](https://doc.rust-lang.org/stable/book/ch20-01-unsafe-rust.html)
- [How safe and unsafe Rust interact](https://doc.rust-lang.org/stable/nomicon/safe-unsafe-meaning.html)
- [Containing unsafe invariants](https://doc.rust-lang.org/stable/nomicon/working-with-unsafe.html)
- [Miri](https://github.com/rust-lang/miri/)
- [Rust sanitizer support](https://doc.rust-lang.org/beta/unstable-book/compiler-flags/sanitizer.html)
- [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz)
- [Rust Fuzz Book](https://rust-fuzz.github.io/book/)

## Correctness and cross-platform evidence

### Findings

Rust target tiers describe compiler and standard-library support, not support for a
particular application. Tier 1 targets are built and tested by the Rust project.
Tier 2 targets are guaranteed to build but are not necessarily tested. Retonr still
needs native behavior, packaging, filesystem, terminal, network, and installation
evidence for every advertised target.

Cargo's test and feature behavior makes several independent dimensions easy to
collapse accidentally: target selection, feature selection, test harness, doctests,
release profile, and actual packaged binary. A green all-features library test does
not test the exact binary configuration that users receive.

### Planning consequences

- Define supported targets as exact Rust target triples plus operating-system and
  architecture floors. Do not use only the labels Windows, macOS, and Linux in a
  release manifest.
- Run format and host-independent static checks once, then run tests, release builds,
  package creation, and installed-binary smoke tests natively on each supported
  target.
- Test the exact shipping package and feature set in addition to all-targets and
  all-features validation. Exercise the installed executable as a subprocess using
  real standard streams, exit codes, paths, signals, and cancellation behavior.
- Preserve a separate doctest job. An accelerated test runner that omits doctests
  cannot replace it.
- Make the canonical test semantics explicit. An ordinary pull request may use a
  tested Cargo fallback when an acceleration tool cannot be installed, but release
  evidence must state which runner executed and cannot claim the unavailable runner
  passed.
- Keep the 80 percent repository line floor and higher critical-crate floors, but
  require decision tables for fidelity and authority logic even when instrumentation
  reports full coverage.
- Add explicit default, no-default, shipping, and all-feature jobs before additional
  runtime, protocol, or desktop features multiply the feature graph.
- Run release-profile black-box tests. Cargo documents that tests ignore the release
  profile's `panic = "abort"`, so ordinary unit tests do not exercise the exact panic
  strategy of a released binary.

### Primary sources

- [Rust platform support](https://doc.rust-lang.org/rustc/platform-support.html)
- [Rust target tier policy](https://doc.rust-lang.org/rustc/target-tier-policy.html)
- [Cargo build target and feature selection](https://doc.rust-lang.org/cargo/commands/cargo-build.html)
- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)

## Panic, overflow, performance, and diagnostics

### Findings

Cargo's release profile disables overflow checks by default. Retonr's current release
profile does not override that default. It also uses `panic = "abort"`, thin LTO, one
code-generation unit, and symbol stripping. Cargo advises measuring optimization
settings because higher optimization and smaller-code modes do not always produce
the expected result.

`panic = "abort"` makes an unexpected panic a process termination. This can be a
reasonable containment choice for a CLI, but only if all expected runtime conditions
are represented as typed outcomes and if file replacement and persistent-state
operations remain crash-safe.

### Planning consequences

- Enable release overflow checks unless representative benchmarks establish an
  unacceptable cost and every affected arithmetic boundary has reviewed checked,
  saturating, or explicitly wrapping semantics. Untrusted lengths, offsets, counts,
  and allocation calculations use checked arithmetic regardless of profile.
- Treat any panic reachable from supported input as a defect. Add release-binary
  tests for malformed input, resource exhaustion, cancellation, broken pipes, file
  races, and backend failure while `panic = "abort"` is active.
- Reassess `strip = "symbols"` before public artifacts. Define how support can map a
  crash to a build and useful stack location. Prefer a measured distribution and
  symbol-retention design over discarding diagnostic value implicitly.
- Keep release symbols or line information as separately access-controlled artifacts
  when platform tooling permits. Bind them to the exact unsigned binary digest and
  never include private source paths.
- Benchmark thin LTO and `codegen-units = 1` on every target class that ships. Retain
  settings only when the measured runtime, size, memory, build, and diagnostic
  tradeoff supports them.
- Verify exploit mitigations on the produced binary rather than assuming source-level
  memory safety activates every platform mitigation. Keep experimental nightly
  mitigations outside production until their target support and operational cost are
  qualified.

### Primary sources

- [Cargo profile settings](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Rust compiler exploit mitigations](https://doc.rust-lang.org/rustc/exploit-mitigations.html)
- [Rust compiler source path remapping](https://doc.rust-lang.org/rustc/remap-source-paths.html)

## Compatibility and contract freeze

### Findings

Cargo documents which Rust API changes are usually compatible, but Rust API SemVer
does not cover a binary's command-line grammar, JSON output, files, database schema,
HTTP API, MCP tools, agent skills, or behavioral semantics.

`cargo-semver-checks` compares Rustdoc representations and can use a Git revision as
the baseline for an unpublished crate. Its official documentation warns that it does
not detect every breaking change, that feature subsets matter, that target-specific
APIs may require a target matrix, and that its Rustdoc JSON dependency requires the
tool version to track compiler support.

### Planning consequences

- Do not impose a public Rust API compatibility promise on all internal crates.
  Identify the small crates, if any, whose Rust API is deliberately supported for
  external use.
- At contract freeze, pin `cargo-semver-checks` and compare each supported Rust API
  against the previous release tag with `--baseline-rev`. Run the supported target
  and feature matrix for target-dependent public APIs.
- Update `cargo-semver-checks` with the Rust toolchain because Rustdoc JSON formats
  can change. Treat the tool as a strong automated reviewer, not a complete proof.
- Keep separate compatibility fixtures for CLI help and exit codes, CLI JSON,
  persistence migrations, local API schemas and outcomes, MCP schemas and behavior,
  skill packages, and rewrite records.
- Require explicit compatibility disposition for every changed public contract:
  compatible, migrated, versioned in parallel, or intentionally breaking with the
  required version change and release note.
- Test old clients against new servers and new clients against supported old stored
  data where the product makes those promises. Schema diff alone does not establish
  behavioral compatibility.

### Primary sources

- [Cargo SemVer compatibility](https://doc.rust-lang.org/cargo/reference/semver.html)
- [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks)
- [Rust API future proofing](https://rust-lang.github.io/api-guidelines/future-proofing.html)

## Dependency and build-time supply chain

### Findings

A committed `Cargo.lock` records exact selected packages. Cargo's `--locked` rejects
a missing or changed lockfile, while `--frozen` combines locked resolution with
offline mode. Cargo also supports vendoring registry and Git sources and emits the
source-replacement configuration needed to use them.

RustSec's `cargo-audit` checks a lockfile or suitably instrumented binary against the
RustSec advisory database. `cargo-deny` separately covers advisories, licenses,
banned or duplicate crates, and source locations. Neither establishes that a
dependency's code has been reviewed for safe deployment.

Cargo Vet records trusted audits and explicit exemptions. Its `safe-to-deploy`
criterion requires enough review to reason about unsafe blocks and powerful imports.
Build dependencies and procedural macros deserve equivalent attention because Cargo
executes them during the build.

`cargo-auditable` embeds the resolved Rust dependency list into production binaries
and supports Linux, Windows, and macOS. This allows the shipped binary, rather than
only a nearby lockfile, to be checked later.

### Planning consequences

- Continue committing `Cargo.lock` and using `--locked` for every CI command that can
  resolve dependencies. Dependency updates occur only in reviewable changes with
  complete CI and qualification impact analysis.
- Keep `cargo-deny` and `cargo-audit` as separate gates. Retain machine-readable
  reports and advisory database identity for release evidence so a past result can
  be interpreted after the database changes.
- Treat duplicate-version warnings as review work. Either reduce a duplicate or
  record why the versions cannot be unified, what extra unsafe or attack surface they
  add, and what will trigger reconsideration.
- Introduce Cargo Vet before external plugin, protocol, desktop, audio, model-runtime,
  or packaging dependencies expand the graph. Put audit policy and exemptions under
  designated review ownership.
- Do not treat generated initial exemptions as completed audits. Bound each exemption
  to an exact crate and version, record why it exists, and ratchet the set down.
- Apply deployment criteria to normal dependencies and build-execution criteria to
  build dependencies and procedural macros. Review Git dependencies and patches as
  source changes, not merely version changes.
- Produce an offline release source bundle with vendored dependency sources and
  license material. Prove that the exact shipping targets build from that bundle with
  frozen dependency resolution and no network.
- Qualify `cargo-auditable` or an equivalent binary dependency record on all shipping
  formats. Verify that stripping, packaging, and signing preserve the metadata, then
  scan the final installable binary.
- Generate an SBOM from the exact shipping graph and bind it to artifact digests.
  A source-level workspace SBOM must not stand in for per-target, per-feature binary
  contents.

### Primary sources

- [Cargo lockfile guidance](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html)
- [Cargo vendor](https://doc.rust-lang.org/cargo/commands/cargo-vendor.html)
- [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [RustSec and cargo-audit](https://rustsec.org/)
- [cargo-deny checks](https://embarkstudios.github.io/cargo-deny/checks/index.html)
- [Cargo Vet](https://mozilla.github.io/cargo-vet/)
- [Cargo Vet built-in criteria](https://mozilla.github.io/cargo-vet/built-in-criteria.html)
- [cargo-auditable](https://github.com/rust-secure-code/cargo-auditable)

## Reproducible and attributable releases

### Findings

Reproducible Builds distinguishes source identity from build time and recommends
removing timestamps or deriving them from `SOURCE_DATE_EPOCH`. Rustc supports stable
source-path remapping, although linker-generated paths need separate attention.
Cargo build scripts can execute arbitrary work, should write only to `OUT_DIR`, and
can produce order-sensitive linker inputs.

SLSA provenance identifies an output by cryptographic digest and records how it was
produced. Higher build levels add authenticated provenance and stronger resistance to
tenant forgery and cross-build influence. Reproducibility and provenance answer
different questions: reproducibility detects unexplained differences, while
provenance records the asserted build process and identity.

### Planning consequences

- Define the reproducibility boundary separately for each target: compiled unsigned
  binaries, platform package payload, archive container, signature, notarization,
  and update metadata.
- Remove embedded wall-clock time, random ordering, absolute workspace paths,
  username, hostname, and mutable network results. Where a source-derived timestamp
  is required, use one validated `SOURCE_DATE_EPOCH` value and pass it through to
  child processes.
- Use Rust path remapping for source paths and inspect native linker and packaging
  outputs for remaining build paths. Do not claim reproducibility based only on
  setting one compiler flag.
- Keep build scripts rare. They write only to `OUT_DIR`, declare precise rerun inputs,
  use deterministic ordering, perform no undeclared network access, and never make
  product behavior depend on the build host.
- Build each unsigned release artifact twice in isolated clean environments with the
  same declared inputs. Compare bytes where possible and use a reviewed structural
  comparison only where the platform format contains unavoidable nondeterminism.
- Treat every unexplained difference as a release blocker. Document any normalized
  field, why it differs, and why normalization does not conceal executable changes.
- Sign only artifacts that passed comparison and qualification. Preserve the digest
  before signing and record signed and notarized derivative digests separately.
- Generate build provenance from the hosted build control plane, not from an
  untrusted repository script claiming facts about itself. Bind source revision,
  workflow identity, builder, compiler, target, lockfile, feature set, dependencies,
  outputs, and digests.
- Publish checksums, signatures, provenance, SBOMs, verification instructions, and
  the exact support matrix together. Test those instructions from a clean machine.
- Keep signing secrets outside build steps. Require key custody, backup, rotation,
  revocation, and loss procedures before signing is a release gate.

### Primary sources

- [SOURCE_DATE_EPOCH specification](https://reproducible-builds.org/specs/source-date-epoch/)
- [Reproducible build timestamps](https://reproducible-builds.org/docs/timestamps/)
- [Rust source path remapping](https://doc.rust-lang.org/rustc/remap-source-paths.html)
- [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
- [SLSA provenance](https://slsa.dev/spec/v1.2/provenance)
- [SLSA build requirements](https://slsa.dev/spec/v1.2/build-requirements)
- [Sigstore blob signing](https://docs.sigstore.dev/cosign/signing/signing_with_blobs/)

## Logical implementation gates

The order below is based on dependency and risk, not elapsed-time estimates.

### Gate 1: Freeze the engineering baseline

Required before expanding the CLI vertical slice:

- Record the exact shipping feature set separately from all-features validation.
- Add default, no-default, shipping, and all-feature checks.
- Add warnings-deny configuration to first-party Cargo builds.
- Decide release overflow checks and diagnostic symbol retention.
- Add stable and beta non-release compiler canaries.
- Make toolchain updates focused, reviewed changes with release-note review.

Exit evidence: the exact shipping binary and the exhaustive validation configuration
both pass on Windows, macOS, and Linux, and neither contains diagnostic-only features.

### Gate 2: Harden critical pure Rust boundaries

Required before Markdown, external agent interfaces, or new parsers enlarge the
untrusted-input surface:

- Add targeted Miri for deterministic critical crates.
- Expand property and fuzz targets around byte ranges, parsers, reassembly, limits,
  serialization, and cancellation.
- Commit every minimized crash as a regression fixture.
- Require checked arithmetic and bounded allocation at every untrusted size and
  offset boundary.

Exit evidence: critical crates pass unit, property, release-profile, Miri, retained
fuzz corpus, coverage, and mutation gates applicable to their decisions.

### Gate 3: Freeze public contracts deliberately

Required before stable API, MCP, or agent-plugin promises:

- Identify which Rust APIs, CLI forms, schemas, files, and protocol behaviors are
  public contracts.
- Add `cargo-semver-checks` only for deliberately stable Rust APIs.
- Add versioned compatibility fixtures and old/new consumer tests for every non-Rust
  contract.
- Define a release disposition for every compatibility change.

Exit evidence: an old supported consumer and the new implementation agree on shared
fixtures, and every incompatible change is migrated, versioned, or rejected.

### Gate 4: Admit native or unsafe code only through quarantine

Required before any FFI-backed inference, desktop, audio, or packaging component is
accepted:

- Prefer a process boundary or audited safe wrapper.
- If first-party unsafe remains necessary, isolate it in one adapter crate with a
  reviewed safety contract and safe facade.
- Add Miri where executable, target-qualified sanitizers, fuzzing, native tests, and
  dependency vetting.
- Prove cancellation, teardown, panic, concurrency, and resource ownership behavior.

Exit evidence: no existing crate weakens its unsafe policy, and the exceptional
adapter's complete trusted surface has retained review and dynamic evidence.

### Gate 5: Close the release supply chain

Required before public release candidates:

- Add Cargo Vet policy with reviewed ownership and bounded exemptions.
- Produce and verify the vendored offline source bundle.
- Build exact shipping features per native target.
- Embed or bind dependency metadata and generate per-artifact SBOMs.
- Rebuild and compare unsigned artifacts in isolated environments.
- Generate control-plane provenance, checksums, and signatures.
- Install, run, update, recover or roll back, and remove the signed packages on clean
  supported systems.

Exit evidence: every public artifact has a verified chain from source and declared
inputs through reproducible unsigned payload, signing transformation, installation,
and runtime smoke test.

## Roadmap consequences

- The CLI-first sequence is correct. Complete Gates 1 and 2 while the executable and
  contract surface are still small.
- Agent integrations depend on Gate 3 because agents need stable machine output,
  authority, cancellation, and error contracts. Rust API SemVer alone is
  insufficient.
- Markdown and DOCX adapters increase parser and structure-preservation risk. They
  require Gate 2 evidence before broad format claims.
- Ollama over a bounded loopback protocol preserves the current all-safe Rust core.
  A llama.cpp sidecar preserves that boundary better than in-process FFI and should
  remain the preferred initial architecture.
- Desktop and voice are the first likely sources of native and unsafe dependencies.
  Gate 4 must precede their integration, not follow feature completion.
- Public 1.0 artifacts require Gate 5. Signing alone does not satisfy reproducibility,
  provenance, offline rebuild, dependency audit, or installability.
- After 1.0, toolchain, dependencies, target floors, and protocol revisions change
  through the same gates. A maintenance release is not exempt because its feature
  delta is small.

## Revalidation triggers

Revisit this note when:

- Rust, Cargo, Clippy, rustdoc, Miri, sanitizer support, or the selected nightly
  revision changes.
- A supported target changes Rust tier, operating-system floor, architecture, linker,
  packaging, or signing requirements.
- A Cargo feature, build script, procedural macro, native dependency, or unsafe block
  is introduced.
- A Rust API, CLI schema, stored schema, API, MCP contract, or agent package becomes
  externally supported.
- Cargo Vet criteria, RustSec data, dependency sources, licenses, or maintainer risk
  changes.
- Rebuild comparison, SBOM generation, dependency embedding, signing, provenance, or
  verification tooling changes.
- Release evidence no longer reproduces from the retained source and declared build
  inputs.
