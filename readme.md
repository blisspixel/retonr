# Retonr

Local-first, fidelity-gated re-expression of machine-generated and rough drafts in
your own writing style.

Retonr takes text that may still carry an upstream model's characteristic phrasing,
token-selection signals, or embedded document artifacts and reconstructs eligible
prose through a local model. It learns from writing the user owns or is authorized
to use, compiles that evidence into an inspectable style profile, and moves the
draft toward the user's voice while protecting facts, structure, formatting, and
declared constraints. If a candidate cannot pass the configured validation policy,
Retonr returns the original.

The product is designed to operate offline after required models are installed.
Its provider-neutral output path minimizes supported source-form signals and
metadata carried forward into the new artifact. It cannot delete upstream service
logs, prove human authorship, or guarantee that a classifier or watermark detector
will not recognize a source. It is not a generic humanizer, a detector-evasion
service, or a writing coach.

## Status

Early implementation is active. The first model-free vertical slice now includes
a Rust workspace, versioned domain contracts, UTF-8 plain-text parsing and
reassembly, typed protected values, strict candidate gates, independent semantic
assessment, deterministic selection, document-atomic abstention, redacted rewrite
records, a candidate-check CLI, and a synthetic positive and hard-negative
evaluation suite.

The next internal layer is also implemented but is not a qualified user-facing
rewrite command yet. It separates immutable artifact identity, qualification, and
activation; defines a backend-neutral bounded inference port and deterministic fake;
uses a loopback-only native Ollama adapter with digest-drift checks; and routes a
proposal-only grounded strategy through the same common engine. Real artifact
qualification, broader semantic calibration, durable artifact storage, and the
complete CLI workflow remain open.

The current literal strategy accepts mechanical punctuation changes only when the
alphanumeric token sequence is unchanged. It intentionally abstains on open-domain
rewriting because semantic equivalence has not yet been qualified. Core interfaces
remain provisional until a pinned real model path passes the cross-platform
qualification and evaluation gates.

`Retonr` is the selected public project identity. The name and namespace decision
is recorded in [ADR 0006](docs/decisions/0006-retonr-public-identity.md). Preliminary
conflict screening supports publication of the source repository, but it is not a
legal opinion or a substitute for formal review before packaged product releases.

## Run the current slice

```console
cargo run --locked -p retonr-cli -- check fixtures/cli/source.txt fixtures/cli/candidate.txt --format text
cargo run --locked -p rewrite-eval -- crates/eval/fixtures/core.json
```

The first command validates a caller-supplied complete candidate without invoking a
model. The second runs the checked-in positive and hard-negative fidelity suite. Both
surfaces omit raw document text. The check command also supports repeated
`--protect` values and `--fail-on-abstain` for automation.

Implemented behavior is intentionally narrower than the planned CLI below.

## Product contract

- The user owns the profile and its evidence.
- Declared rules outrank inferred tendencies.
- Exact literals, protected spans, and supported document structure pass strict
  deterministic checks.
- Semantic risk is assessed with calibrated, versioned evaluators. It is not
  described as a formal proof of equivalence.
- Every generative strategy passes the same validation cascade.
- Hard failures or disallowed uncertainty cause abstention.
- Document-atomic mode returns the original byte-for-byte when any required unit
  fails.
- Profile learning is explicit, provenance-aware, reversible, and never trained
  on raw candidates.
- Supported source-form signals and document metadata are inspected and handled by
  explicit policy instead of being silently copied into a rewritten artifact.
- Source-form reduction is reported as a bounded technical result, never as proof
  of privacy, human authorship, or universal provenance removal.
- Core rewriting remains local. Networked backends require explicit opt-in.

## Intended users

The initial audience is privacy-sensitive technical and professional writers who
use generated or rough drafts but do not want upstream model phrasing and supported
source artifacts to remain the final form of their work. They want a locally
reconstructed draft that sounds like them without silently changing claims, names,
quantities, links, code, or document structure.

The initial supported language will be English. Additional languages must pass
their own evaluation and model-qualification gates before they are advertised.

## Architecture at a glance

```text
Authorized style evidence
          |
          v
   Profile compiler -----> Immutable style profile
                                  |
Input -> Document adapter -> Rewrite units and adapter state
                                  |
                                  v
                    Risk analysis and planning
                                  |
                                  v
                    Candidate generation strategy
                                  |
                                  v
                         Common validation cascade
                                  |
                                  v
                       Lexicographic candidate choice
                                  |
                     +------------+------------+
                     |                         |
                   pass                      abstain
                     |                         |
                     v                         v
              Verified reassembly       Original input
                     |
                     v
              Output and rewrite record
```

Generation proposes candidates. The engine accepts, rejects, or abstains through
explicit policy.

## Planned interfaces

- A scriptable CLI with stdin, structured output, diff, dry-run, traces,
  completion, and stable exit codes
- A first-party local JSON API with versioned schemas
- MCP over standard input and Streamable HTTP, including the current metadata and
  discovery lifecycle plus named-client compatibility where qualified
- Thin Agent Skills `SKILL.md` packages that call the stable MCP or API surface
- A cross-platform Tauri desktop application
- Local voice-assisted style interviews with editable transcripts and a complete
  non-voice path
- A narrowly documented offline compatibility adapter for completed, supported
  text-only response payloads

Windows, macOS, and Linux are supported product targets from the first executable
milestone, not a later porting exercise.

## Planned CLI shape

```console
retonr profile create personal
retonr profile ingest samples/ --profile personal
retonr profile interview --profile personal
retonr rewrite draft.md --profile personal --mode balanced --diff
retonr check draft.md --profile personal --format json
retonr model qualify qwen3.5:9b
```

Command names and schemas remain provisional until the first CLI contract is
validated with real workflows.

## Screenshots

The README will contain real screenshots captured from passing release builds:

1. The first complete CLI vertical slice will add rewrite, abstention, diff, and
   trace screenshots.
2. The desktop beta will add onboarding, rewrite review, profile editing, model
   management, and accessible diff screenshots.
3. The voice release candidate will add the local voice interview and editable
   transcript screenshots.
4. Release screenshots will be recaptured on supported platforms when platform
   behavior differs.

Screenshots will not be mocked in a way that suggests unfinished functionality is
already available. Capture and accessibility requirements are defined in
[the screenshot policy](docs/screenshots/README.md).

## Documentation

- [Current implementation state](docs/current-state.md)
- [Product definition](docs/product.md)
- [Architecture](docs/architecture.md)
- [Product and interface design](docs/design.md)
- [Technology stack](docs/technology.md)
- [Evaluation strategy](docs/evaluation.md)
- [Security and privacy](docs/security.md)
- [Engineering quality](docs/quality.md)
- [Versioned roadmap](docs/roadmap.md)
- [Phase execution plans](docs/planning/README.md)
- [Next-phase research ledger](docs/research/2026-08-11-next-phases.md)
- [Naming status](docs/naming.md)
- [Architecture decisions](docs/decisions/README.md)
- [Proposed evaluation data policy](docs/governance/data-policy.md)
- [Proposed user research protocol](docs/governance/user-research.md)
- [Contributing](CONTRIBUTING.md)
- [Security reporting](SECURITY.md)

## What makes the project defensible

Personal voice imitation alone is already available in commercial and local-first
products. The project must earn its place through the combined system:

- Explicit, editable, versioned constraints
- Conservative fact and structure validation
- Honest abstention with machine-readable reasons
- Format-aware TXT, Markdown, and bounded DOCX support
- Private local operation and user-owned evidence
- Auditable CLI, API, MCP, and rewrite records
- Evaluation against simple prompting and retrieval baselines

If the compiled-profile approach does not outperform those baselines without a
material fidelity regression, the added complexity is not justified.

## License

The source is licensed under [Apache-2.0](LICENSE). Model artifacts require separate
manifest, provenance, and license review before activation or distribution. Formal
name review remains required before packaged product distribution.
