# Hybrid Rewrite Evaluation Plan

Date: 2026-08-21

## Decision

Retonr will use a gated scorecard, not one blended quality score. Deterministic
fidelity and structure checks remain the only machine acceptance authority. Blind
human comparison remains the release authority for ambiguous meaning and preference.
A version-locked local model judge may triage disagreements after hard gates pass,
but it cannot authorize output, override a failure, or replace adjudication.

## Implemented Foundation

The current repository already provides a substantial development corpus and exact
scorecard foundation:

| Asset | Implemented scope | Current authority |
| --- | --- | --- |
| [Core suite](../../crates/eval/fixtures/core.json) | 49 positive, identity, protected-value, semantic hard-negative, structure, unsafe-text, sentinel, and paraphrase cases | Exact status, reason, and returned-byte expectations |
| [Evaluation runner](../../crates/eval/src/lib.rs) | Total and per-category pass counts, redacted failures, and acceptable-transformation coverage | Deterministic development gate |
| [Hybrid scorecard](../../crates/eval/src/hybrid_scorecard.rs) | Exact two-suite execution, corpus and policy binding, plan-bound two-order observations, redacted normalization, and hard-gate precedence | Deterministic gate plus non-authoritative triage contract |
| [Neutral judge output](../../crates/inference/src/local_judge.rs) | Strict choice, rubric-clause, and cited byte-span contract with bounded parser | Structural output validation only |
| [Local judge executor](../../crates/eval/src/local_judge_execution.rs) | Hard gates before traffic, absolute 4 MiB retained-session input ceiling, two blinded orders on one preflighted retained stream, exact relationship validation, and a separate nonserializable receipt | Retained-transport binding only; no semantic or qualification authority |
| [Static Ollama model binding](../../crates/eval/src/local_ollama_model_binding.rs) | Exact v0.32.15 installed-package-to-idle-inventory and details relationship consuming an opaque nonserializable exact-runner receipt | Inert static evidence only |
| [Managed runtime build](../../crates/eval/src/local_ollama_managed_preflight/build_binding.rs) | Exact managed package, process, and native-load join constructs package-declared typed runtime-build identity; only the entrypoint is joined to live evidence | Inert binding; other package semantics are not independently live-observed, the process is closed, and effective state is absent |
| [Resident completion](../../crates/ollama/src/single_connection/session/residency.rs) | Opt-in v0.32.15 completion with two equal post-generation runtime residency reports | Runtime-reported residency only; no model-use or page-identity authority |
| [No-rewrite baseline](../../crates/eval/fixtures/no_rewrite_baseline_v1.json) | Frozen simple comparison system | Baseline evidence only |
| [Editorial corpora](../evaluation-corpora.md) | 120 synthetic cases across five checked-in groups | Corpus validation only; no live lint scanner yet |
| [Claim-shadow calibration](../../crates/eval/fixtures/claim_shadow_calibration_v1.json) | Confirms informational semantic evidence cannot change hard-gate acceptance | Non-authoritative calibration |
| [Local model protocol](2026-08-13-local-model-evaluation.md) | Smoke, development, locked, repeatability, human review, and resource methodology | Design contract pending trusted runtime execution |

Together, the checked-in deterministic and synthetic editorial foundations contain
169 cases. The 120 editorial cases validate corpus labels and controls; they are not
model outputs and do not imply that a live lint scanner exists.

The serializable version 1 scorecard still consumes and labels a strict
caller-declared observation batch; its rubric and judge-system digests remain
declarations. A separate typed executor can now produce that batch over one already
preflighted retained Ollama stream and return a nonserializable receipt binding the
plan, rubric, batch, exact requests, exact responses, and response ordinals. The
retained session rejects UTF-8 input above the absolute 4 MiB ceiling before wire
serialization or completion traffic. The receipt's evidence class is
retained-transport binding only. It does not prove managed isolation,
application-handler execution, model load or use, candidate generation, effective
identity, semantic correctness, or qualification. No current preflight is qualified
to generate the candidate outputs needed for a release judge run.

Other separate bindings narrow the remaining gap without closing it. A successful
managed preflight can now return an inert package-declared typed
`RuntimeBuildIdentity` after the exact entrypoint is joined to process and native-load
evidence. Target, revision, and other package semantics are not independently
live-observed; mandatory cleanup completes before return and no effective runtime
state is constructed. The separate static model binding consumes the opaque,
nonserializable, single-use receipt issued by the exact preflight runner. An opt-in
v0.32.15 retained completion can prove two stable runtime-reported residency
observations, but not handler execution, model use, resident-page identity, effective
identity, or qualification. Neither receipt is joined to the judge executor in one
retained managed operation.

## Score Ordering

Every candidate is evaluated in this fixed order:

1. Runtime, package, isolation, prompt, corpus, and build identities match the frozen
   evaluation manifest.
2. Parsing, protected-value, structure, unsafe-text, schema, and resource gates pass.
3. Accepted-set semantic corruption and hard-negative acceptance are reported.
4. Useful transformation coverage and exact no-op behavior are reported.
5. Named editorial findings resolved, retained, and introduced are reported.
6. Blind human pairwise preference is measured on the common accepted subset.
7. Local judge triage identifies consistent preference, tie, abstention, order
   sensitivity, or disagreement for human review.
8. Latency, memory, throughput, cancellation, disk, and optional energy remain
   separate resource observations.

A result cannot trade a changed fact or structure violation for better style,
coverage, preference, or speed.

## Corpus Partitions

The public checked-in fixtures remain development data. The first generated-output
program will project a small smoke subset and a preregistered editorial subset from
the public contracts. Before qualification, we will freeze distinct manifests for:

- `smoke`: fast installation, schema, and execution checks;
- `development`: visible failures used for engineering repair;
- `calibration`: judge disagreement, threshold, and rubric calibration;
- `locked`: access-controlled release evidence never used for tuning;
- `red_team`: adversarial discovery without release-score substitution.

Every manifest binds its corpus digest, case and cluster identities, split-generation
revision and seed, annotation rubric, adjudication policy, deterministic thresholds,
baseline identities, exact system artifacts, prompt and schema digests, runtime and
isolation evidence, hardware class, operating system, and release build digest.

## Local Judge Protocol

The judge receives only cases whose compared candidates passed every deterministic
gate. Candidate origin is hidden. The frozen schedule presents each pair twice, once
as A/B and once as B/A, with one attempt per order and no retry hidden from the
report. The exact local judge package, runtime, prompt, output schema, sampling
parameters, and seed are part of the manifest.

The neutral attempt contract returns one of:

- first presented candidate preferred;
- second presented candidate preferred;
- tie;
- abstain because evidence is insufficient or the rubric is inapplicable.

It also returns bounded rubric-clause identifiers and source/candidate span
references. Free-form rationale is not admitted. The evaluation layer checks every
span against the exact presented UTF-8 input before producing the caller-declared
scorecard batch. The normalized paired outcome is:

- stable A or stable B when both presentation orders choose the same underlying
  candidate;
- stable tie when both return tie;
- abstained when either pass abstains;
- order-sensitive for every other combination.

Order-sensitive, judge-human disagreement, and invalid-schema cases enter a human
triage queue. They never become an automatic loss or win.

## Rubric

The initial rubric has separate clauses for:

- main-point and entailment preservation;
- factual, numerical, modal, attribution, and scope preservation;
- restraint and absence of unnecessary framing;
- named editorial findings resolved without neighboring defects;
- channel and audience fit;
- protected-term and quotation handling;
- clarity without flattening uncertainty or voice.

Hard-gate dimensions remain visible in the rubric so reviewers can identify an
annotation defect, but a judge answer cannot reverse the deterministic result.

## Calibration And Bias Controls

Before a judge version can triage locked results, it must run against an independent
calibration split with human labels. We report position bias, self-preference where a
candidate shares the judge family, tie and abstention rates, judge-human agreement,
per-clause confusion, and dependence by source/template/topic cluster. A generator is
never its only evaluator. Candidate order is randomized and swapped, and the report
keeps both raw bounded choices rather than only their normalization.

The judge is useful only if it reduces human review cost without systematically
changing the release decision. Calibration failure disables the judge lane and leaves
the deterministic plus human protocol intact.

## Zero-Cost Development Probe

On August 21, 2026, a non-authoritative local probe exercised the proposed structured
judge response on three synthetic candidate pairs, once in each presentation order.
It used the already-installed Ollama 0.32.15 runtime and two already-installed Qwen3
0.6B manifests. No artifact was downloaded and no candidate or response was retained.

| Local manifest ID | Declared model-layer digest | Structured responses | Order-stable pairs |
| --- | --- | ---: | ---: |
| `7df6b6e09427a769808717c0a93cadc4ae99ed4eb8bf5ca557c90846becea435` | `7f4030143c1c477224c5434f8272c662a8b042079a0a584f0a27a1684fe2e1fa` | 6 of 6 | 2 of 3 |
| `f6358994266d548dc070c564d49588fa3a6fac0cccd196bef38128aaa6ed22f3` | `31db9797248620627b127c47c75caa76ec2ce9b7de9dc0f2faca02ba59615818` | 6 of 6 | 1 of 3 |

The first manifest also produced one stable preference for a candidate that added an
unsupported claim. More quantization bits did not eliminate order sensitivity in this
small probe. These observations are not model qualification, statistical calibration,
or evidence of an attested judge execution. The local manifests supplied the layer
digests, the runtime was not isolated, and the scorecard adapter did not execute the
calls. The result supports the existing boundary: structured output is useful, but a
small local judge remains triage-only and cannot override deterministic fidelity gates
or human adjudication. Paid spend was $0.

## Planned Release Report Shape

The planned content-free release report records:

- exact manifest and system identity;
- hard-gate totals and failures by risk category;
- accepted-set semantic error and transformation coverage denominators;
- editorial findings resolved, retained, introduced, and uncertain;
- stable A, stable B, tie, abstained, order-sensitive, and invalid judge counts;
- blinded human preference and adjudication outcomes;
- resource observations and repeatability drift;
- exclusions, cancellations, retries, and incomplete cases.

There is no universal quality number. A release result is a decision table whose first
failed mandatory gate blocks qualification.

## Implementation Order

1. Implemented: freeze a versioned scorecard plan and redacted report contract in
   `rewrite-eval`; execute and bind exact deterministic suite pairs; require the
   fixed hard-gate policy; bind observation batches to the full plan; normalize both
   presentation orders; retain only content-free aggregate evidence; and expose the
   complete operation through the `rewrite-eval` CLI.
2. Implemented: define the provider-neutral attempt schema, exact rubric and prompt
   contracts, retained-session two-order executor, fail-closed response relationship
   checks, absolute 4 MiB UTF-8 input ceiling before wire serialization or completion
   traffic, and separate limited transport-binding receipt. The scorecard remains
   caller-declared and triage-only, and the executor is a library rather than a CLI.
3. Implemented: bind one exact installed model package to one verified idle Ollama
   v0.32.15 inventory and details observation by consuming the exact runner's opaque,
   nonserializable receipt. Keep loaded, used, handler, effective identity, and
   qualification claims false.
4. Implemented: derive package-declared typed runtime-build identity from a successful
   managed package, process, and native-load join. Only the entrypoint is joined to
   live evidence, other package semantics are not independently live-observed, and
   cleanup is complete before return. Also retain the separate v0.32.15 receipt for
   two equal runtime-reported post-generation residency observations. Neither
   constructs effective runtime state or proves model use.
5. Next: review one exact runtime package, then extend the managed operation to retain
   the process through generation and direct effective-state observation. Join the
   runtime build, static model binding, model-package lease, residency receipt, and
   judge receipt without drift. This is required because the current managed outcome
   closes the process before return.
6. Add a separate candidate-generation receipt, then project the existing smoke and
   editorial cases into generated-output plans without opening locked data.
7. Extend the implemented two-order normalization with disagreement queues,
   calibration reports, and explicit judge-disable behavior.
8. Run repeated smoke and development evaluation, then freeze thresholds and the
   locked manifest.
9. Open locked output once under the preregistered decision rule and retain human
   adjudication as release authority.

## Acceptance Criteria

- The scorecard rejects unknown fields, duplicate cases, unbound identities,
  undeclared rubric clauses, missing presentation orders, hidden retries, and raw
  content in aggregate output.
- A hard-gate failure always blocks qualification regardless of later results.
- Swapping presentation order cannot silently change the normalized result.
- Judge triage can be disabled without changing deterministic or human outcomes.
- Locked cases cannot be used as prompts, examples, calibration data, or threshold
  inputs.
- Tests cover every decision-table branch, limit, redaction rule, cancellation path,
  order combination, and identity drift case.
- The complete run remains local and offline after explicitly approved artifact
  setup. Paid evaluation spend for this work remains $0.

## Research Basis

The approach follows the repository's existing [evaluation policy](../evaluation.md)
and [local model protocol](2026-08-13-local-model-evaluation.md). It treats documented
model-judge [position and self-preference bias](https://proceedings.neurips.cc/paper_files/paper/2023/hash/91f18a1287b398d378ef22505bf41832-Abstract-Datasets_and_Benchmarks.html)
as a calibration problem, not as permission to replace human semantic labels.
