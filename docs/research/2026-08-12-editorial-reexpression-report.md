# Editorial re-expression under uncertain text provenance

## Abstract

Text generated or revised by a language model can carry several unrelated forms of
source evidence: statistical choices in tokens, semantic or structural marks,
signed document manifests, mutable metadata, external provider records, and generic
classifier signals. These mechanisms have different trust models, observability,
robustness, and legal significance. Treating them all as an "AI watermark" produces
bad engineering and unreliable claims.

This report defines the research and engineering program for Retonr, a local-first,
fidelity-gated editorial re-expression system. Retonr reconstructs eligible prose
under the user's explicit brief and authorized style evidence, validates protected
facts and structure, reports named editorial improvements, and abstains when a safe
rewrite cannot be established. Its live engine does not query or optimize a
watermark detector or generic AI classifier.

The report contributes a layered provenance taxonomy, a strict separation between
editorial quality and source-signal research, a non-destructive document transaction
model, an exact-runtime assurance case, and a preregistered watermark evaluation
protocol. It also defines deliberately narrow claims. Re-expression can change
source-wording signals, but Retonr does not prove human authorship, universal
watermark removal, detector evasion, anonymity, or legal compliance. The research
goal is not to erase uncertainty. It is to make uncertainty explicit while building
an exceptionally controlled editorial tool.

## Status and evidence cutoff

This is a research and system-design report, not a completed empirical paper.
Reviewed evidence is current through August 12, 2026. Work published later may
change the provider ledger, threat model, or experimental baseline.

The report follows the evidence vocabulary and publication threshold in
[Research program](README.md). In particular:

- A peer-reviewed result is bounded by its scheme, implementation, data, language,
  operating point, and threat model.
- A preprint is a provisional report, not independent confirmation.
- A provider statement can establish public policy or claimed deployment while
  leaving the mechanism and its performance unverifiable.
- A local observation is not evidence until its frozen artifact bundle is
  reproducible from a clean environment.
- Unknown marking state remains unknown. It is not rewritten as disabled,
  watermark-free, or absent.

## Research question

The central question is:

> Can a local system help a person reconstruct an authorized draft in their own
> expression, measurably improve named editorial qualities, preserve declared facts
> and document structure, and accurately report provenance uncertainty without
> becoming a detector-evasion system?

This decomposes into six questions:

1. Can fidelity gates keep material semantic and structural failures below a
   predeclared risk bound while retaining useful rewrite coverage?
2. Does an evidence-backed personal profile improve blind owner preference over the
   strongest simple prompt baseline?
3. Do explainable editorial-lint rules reduce accepted quality defects without
   becoming a proxy AI-authorship classifier?
4. Can exact local runtime qualification support the negative claim that no known
   intentional watermark is enabled in the inspected stack?
5. Can format adapters preserve non-target state and accurately handle credentials,
   signatures, metadata, formulas, and unsupported features?
6. Under an isolated research protocol, how do ordinary editorial transformations
   affect named public watermark schemes without influencing live rewriting?

Each question can fail independently. A favorable detector result cannot rescue a
fidelity failure. Style preference cannot rescue document corruption. Local
execution cannot prove that model weights contain no learned source signature.

## Contributions and non-contributions

### Intended contributions

1. **A layered source-evidence model.** Statistical watermarks, signed manifests,
   metadata, external logs, and generic classifiers are represented as different
   evidence planes with different claims.
2. **A noninterference boundary.** Watermark and source-classifier outputs cannot
   enter live generation, retry, ranking, acceptance, profile learning, or feedback.
3. **A fidelity-first editorial objective.** The system optimizes useful personal
   expression only among candidates that passed independent hard gates.
4. **An explainable anti-slop loop.** Concrete editorial findings can guide a
   rewrite without asserting machine authorship.
5. **A non-destructive artifact model.** Sources remain immutable, long documents
   are edited through bounded units, and format owners verify every claimed
   preservation property before commit.
6. **An exact-stack assurance case.** Support binds to model, tokenizer, template,
   runtime, sampler, processors, parameters, hardware, and execution path rather
   than a mutable model family name.
7. **A reproducible watermark study protocol.** Calibration, power, attacks,
   multiplicity, mixed content, and quality are evaluated under frozen procedures.

### Non-contributions

Retonr does not claim:

- A universal definition or detector for AI-generated text
- Proof of human or model authorship
- A universal watermark-removal method
- Detector evasion or a guaranteed negative classification
- Deletion of provider logs, remote manifests, prompts, account records, backups,
  or third-party copies
- Formal semantic equivalence for unrestricted natural language
- A legal conclusion about disclosure, ownership, standard editing, substantial
  alteration, or editorial responsibility
- That local execution is automatically private, secure, unbiased, or unmarked

## Terminology and mechanism taxonomy

### Source-evidence planes

| Plane | Examples | Observable by Retonr | Strongest bounded claim |
| --- | --- | --- | --- |
| Linguistic generation signal | Green-list token bias, distribution-preserving sampling, semantic marks, multi-bit payloads | Only with a matching public or authorized scheme, configuration, tokenizer, and detector | Evidence for one mark under one calibrated procedure |
| Artifact binding | C2PA manifest, PDF signature, OOXML signature | Yes for supported carriers and validators | Defined bytes or parts match a signed claim under a named trust policy |
| Mutable artifact state | XMP, OOXML properties, comments, hidden fields, Unicode controls | Yes for supported formats | A field or sequence is present and changed or preserved as reported |
| External record | Provider prompt and output logs, fingerprints, account history, manifest repository | Only through an explicitly authorized external interface | A named external system returned a named record under its current policy |
| Generic source inference | AI-text classifier, stylometric model, provider-agnostic detector | Probabilistically, if separately exercised | A model score under its test distribution, not embedded provenance |
| Editorial quality | Repetition, canned transitions, vague attribution, inflated headings, punctuation density | Yes under versioned rules and supported languages | A named editorial finding under a declared policy |

These planes must not be collapsed. A valid Content Credential is not a statistical
watermark. A provider log is not carried in copied text. A generic classifier has no
secret embedded evidence. An editorial anti-pattern is not proof that a model wrote
the passage.

### Linguistic watermark families

The literature through the cutoff includes:

- Token-bias or green-list schemes that favor a keyed token subset
- Distribution-preserving or distortion-free sampling schemes that correlate
  samples with keyed randomness
- Semantic, sentence-level, syntactic, topical, or post-hoc schemes that move some
  signal beyond exact token overlap
- Multi-bit schemes that encode a model, tenant, request class, or other payload
- Public-verification or signature schemes that separate verification from private
  generation authority
- In-model schemes trained or distilled into weights
- Mixed, localized, sequential, and colliding schemes

The [KGW green-list method](https://proceedings.mlr.press/v202/kirchenbauer23a.html)
and [SynthID-Text](https://www.nature.com/articles/s41586-024-08025-4) are mature
reference points with official implementations. The 2025 and 2026 frontier includes
more semantic robustness, payload capacity, localization, distortion control,
multilingual evaluation, and spoofing defenses. Many frontier claims still rely on
one paper's benchmark and need independent reproduction.

### Detector semantics

A detector is a complete hypothesis-testing procedure, not an oracle. Its result
depends on normalization, tokenizer, repeated-event policy, eligible length,
windows, key set, score, threshold, abstention policy, and calibration population.

A positive result may support:

> This span contains sufficient evidence for configuration X under detector
> procedure D at the stated operating point.

It does not, by itself, establish who wrote the ideas, whether a person substantially
edited the prose, whether a policy was violated, or whether the text is true.

A negative result may support only:

> Procedure D did not find sufficient evidence for configuration X in this input.

It does not show that the text is human-authored or that no other mark, metadata,
provider record, or processing history exists.

## Current provider evidence

The provider landscape is deployment-specific, not provider-wide. The full dated
ledger is in
[Provider marking practices and Retonr implications](2026-08-12-provider-marking-practices.md).

| Provider or surface | Evidence through the cutoff | Unknowns that matter |
| --- | --- | --- |
| Supported Claude models and named hosted surfaces | Anthropic states that supported Claude output carries a model-level text mark | Exact model roster, algorithm, keys, detector, thresholds, languages, and independent performance evidence |
| Gemini app and web | Google documents SynthID-Text deployment and publishes a reproducible operator-controlled design | Exact hosted keys, detector, and coverage of API, AI Studio, Vertex, Workspace, and third-party surfaces |
| OpenAI hosted text | Current public implementation status is unresolved | Whether any current text mark is deployed, where, and with what detector |
| Microsoft hosted text | Current public implementation status is unresolved | Text-specific mechanisms and surface coverage |
| Meta, Mistral, and Cohere hosted text | Code commitments exist, but comparable deployed text marking was not established in reviewed official materials | Mechanism, rollout, detector, and exact covered surfaces |
| Open-weight local models | Operator-controlled runtime behavior | Undisclosed learned signals in weights and modifications made by a third-party host |

Anthropic's current documentation is unusually consequential but remains a provider
statement with undisclosed technical components. It also says that a mark can
indicate processing such as proofreading, translation, summarization, or file
conversion rather than origin of the ideas. Google offers stronger public
reproducibility for the general SynthID-Text method, but a locally configured key is
not Google's production key and one documented Gemini surface does not imply every
surface.

Signing a transparency code establishes a commitment, not proof of implementation
coverage. Absence of public documentation establishes an unknown, not an absence.

## Scientific boundary

### What watermarking can do

Under a cooperative and calibrated deployment, watermarks can add useful evidence
for provider accountability, platform moderation, incident investigation,
training-data hygiene, or origin tracking. Keyed designs can achieve low false
positive rates in their qualified populations, and public work shows meaningful
robustness to some edits.

### What constrains watermarking

Text offers limited encoding capacity. Short, factual, deterministic, constrained,
or low-entropy output provides fewer acceptable alternatives. Increasing signal can
consume quality, diversity, latency, payload, privacy, or spoofing margin. The exact
trade depends on the design.

Paraphrase, translation, cropping, dilution, repeated editing, watermark collision,
key learning, stealing, and spoofing are established attack classes. Semantic marks
can survive more lexical change, but then depend on learned representations that may
misread quantities, negation, entities, modality, technical meaning, or language.

[Watermarks in the Sand](https://proceedings.mlr.press/v235/zhang24o.html) gives an
impossibility result under stated quality-oracle and perturbation assumptions. It
does not show that every practical attack is cheap. It does rule out an
unconditional statement that all quality-preserving transformations can be stopped
under those assumptions.

### Quality claims require independent measurement

"Distortion-free" has scheme-specific mathematical meanings. It may describe an
expectation over random keys, marginal token distribution, or computational
indistinguishability without a key. It does not mean that every marked completion is
identical in task quality to the unmarked completion that would otherwise appear.

Watermark studies therefore need independent measures of factuality, protected
entities and quantities, task completion, native human preference, diversity, and
latency. Perplexity, embedding similarity, BERTScore, or an LLM judge alone cannot
establish semantic preservation.

## Retonr system model

### Inputs and output

A rewrite transaction receives:

- Immutable source artifact and exact media type
- Format-owned eligible and protected ranges
- Authorized, versioned profile evidence
- Explicit channel and edit mode
- Optional document-specific editorial brief
- Exact runtime, artifact, tokenizer, template, and parameter identity
- Atomicity, destination, provenance, privacy, and network policies

It returns one of:

- A separately staged and verified derivative
- An unchanged source because no eligible change was justified
- A typed abstention with stable reasons
- A failure that commits no output

### Ordering

The transaction order is:

1. Snapshot and inventory source bytes.
2. Inspect format structure, protected values, provenance carriers, metadata, and
   unsupported state without model execution.
3. Build a bounded document map and ask only high-value clarification questions.
4. Produce proposals for explicit eligible units through bounded context packets.
5. Run deterministic literal, structure, format, and safety gates.
6. Run calibrated semantic assessment with uncertainty and abstention.
7. Run declared style and editorial-lint evaluation only on candidates that passed
   every hard gate.
8. Reassemble through the owning adapter and verify the complete artifact against
   the original source.
9. Stage, flush, and commit according to explicit atomicity and collision policy.
10. Produce a content-minimized local report derived from source and output.

### Selection rule

Candidate selection is lexicographic:

1. Reject hard failures.
2. Reject uncertainty disallowed by the selected policy.
3. Require the calibrated semantic floor.
4. Require explicit user constraints.
5. Compare personal style and channel fit.
6. Compare fluency and named editorial-lint improvement.
7. Use mode-specific edit cost as a final tie breaker.

No blended score allows style, fluency, lint improvement, detector movement, or
surface novelty to compensate for a failed fact or structure gate.

## Editorial lint is not source detection

Retonr can aggressively reduce recurring editorial defects while refusing to call
them proof of AI authorship. Initial explainable rule families include:

- Assistant greetings, offers for further help, and answer-production residue
- Canned openings, transitions, scene-setting, and conclusions
- Repeated thesis statements, recap paragraphs, and duplicate conclusions
- Inflated sectioning and unnecessary fragments
- Formulaic rhetorical templates
- Excessive emoji, dash, exclamation, bold, or parenthetical use under the selected
  profile and channel
- Vague attribution and unsupported confidence
- Redundant qualifiers, throat clearing, generic intensifiers, and abstract filler
- Uniform sentence or paragraph rhythm that conflicts with authorized evidence
- Phrase repetition across adjacent units or a complete document
- Introduced quotation styling, which also triggers a fidelity review

Rules operate on context, density, repetition, language, channel, and user policy.
Protected quotations, technical terms, code, accessibility data, and intentional
choices are exclusions. The report states that named findings decreased. It never
states that the result became more human or less detectable.

The complete rule and reporting contract is in
[Editorial lint and the anti-slop quality loop](../editorial-lint.md).

## Noninterference architecture

The live editorial path and watermark research path are separate trust domains.

| Live editorial system | Isolated research system |
| --- | --- |
| Processes the user's selected artifact | Processes synthetic, public, copied, or explicitly consented fixtures |
| Uses authorized style evidence and an editorial brief | Uses named public embedders, detectors, attacks, and synthetic keys |
| Applies fidelity, structure, format, style, and editorial-lint checks | Measures calibration, power, attacks, spoofing, quality, and drift |
| Has no detector, key, watermark label, or source-classifier input | Has no callable edge into generation, retry, ranking, acceptance, or profiles |
| Produces a derivative, unchanged result, abstention, or failure | Produces an access-controlled run bundle and claim ledger |

Aggregate reviewed research can update documentation or motivate a separately
reviewed architecture decision. Per-row scores, labels, attack features, learned
substitutions, thresholds, and keys cannot flow back into shipping behavior.

Dependency-graph inspection, command-surface inspection, fixture-only input checks,
network denial, and build-graph checks enforce this boundary. A violation blocks
both the experiment and the release.

## Exact local runtime assurance

A model family name cannot establish output policy. A mark may be introduced through
model training, tokenizer behavior, chat templates, logits processors, samplers,
speculative decoding, server defaults, middleware, postprocessing, a compatibility
proxy, or an opaque remote backend.

Qualification therefore binds to an effective artifact set:

- Model files and immutable upstream revision
- Tokenizer and normalization assets
- Prompt and chat templates
- Runtime executable, libraries, build options, and digests
- Effective sampling and generation parameters
- Logits processors and stopping processors
- Speculative model and acceptance algorithm
- Parsers, renderers, middleware, plugins, and postprocessors
- Configuration sources and precedence
- Network endpoints and remote execution boundaries
- Operating system, architecture, backend, accelerator, and drivers

The bounded release statement is:

> No known intentional watermark was enabled in the inspected runtime,
> configuration, and postprocessing path identified by this qualification record.

This does not mean `watermark-free`. Static inspection can miss trained or
obfuscated behavior. Differential tests can miss weak, conditional, or undisclosed
signals. A local review cannot inspect a remote provider's serving stack or logs.
Future detectors can assign a signal not known at qualification time.

Every artifact or configuration change invalidates the affected assurance record.
The complete audit protocol is defined in
[Local watermark assurance for controlled runtimes](2026-08-12-local-watermark-assurance.md).

## Provenance and derivative handling

C2PA 2.4 supports unstructured-text wrappers using invisible Unicode variation
selectors, structured-text manifest blocks, PDF, and OOXML carriers. Invisible
characters and metadata cannot be stripped generically without risking credential,
language, accessibility, security, or format damage.

Retonr inspects supported carriers before normalization or generation. It preserves
unknown state by default and keeps the source unchanged. If an edit invalidates a
recognized signature or hard binding, the default outcome is blocked until the
user explicitly chooses a qualified derivative workflow. The derivative does not
inherit an invalid signature as though it remained valid.

Sanitation is a separate operation for a documented security, privacy,
interoperability, accessibility, or repair purpose. It requires exact preview,
explicit selection, format verification, separate output, and an exact removal
report. A recognized transparency mark is not generic junk.

This integrity boundary does not make Retonr a compliance oracle. It prevents silent
corruption and false provenance claims while leaving publication and disclosure
decisions with the user or deployer. See
[Provenance, marking, and derivative handling](../provenance.md).

## Large documents and folders

A long document is never treated as one unconstrained prompt. Retonr:

1. Builds a model-free inventory and immutable unit map.
2. Creates source-linked global guidance under a bounded context budget.
3. Asks a few document-specific questions when an answer can prevent a wrong
   editorial assumption.
4. Gives one proposal request authority over one exact unit or connected region.
5. Validates units, regions, cross-references, and the reassembled document against
   original source, not only a summary.
6. Stages outputs under document or selection atomicity before commit.

The report names changed ranges, eligible-text change ratios, approximate counts,
style and lint differences, formulas and protected state, fidelity outcomes,
runtime identities, and abstentions. Page-count comparison is valid only under a
named renderer, fonts, locale, operating system, page settings, and export path.

For XLSX, formulas, cached values, cell types, styles, merges, validations, names,
relationships, macros, charts, external links, and unsupported parts remain
protected. Editing eligible prose in cells is a post-1.0 capability that must earn
its own adapter contract.

## Experimental protocol

The detailed preregistration is in
[Text watermark evaluation protocol](2026-08-12-watermark-evaluation-protocol.md).
Its core requirements follow.

### Freeze the complete detector procedure

A procedure includes normalizer, tokenizer, context rule, repeated-event rule,
score, aggregator, window family, key set, payload set, thresholds, abstention, and
runtime. Changing any member creates a new procedure and invalidates prior
calibration for that change.

### Separate data by provenance group

Fixture development, detector training where required, calibration, and locked test
sets are disjoint. Splits group source document, prompt family, author, repository,
near duplicates, and every derived transformation. Locked results are opened once
under a preregistered analysis.

Negative strata include native human text, edited human text, templated and
repetitive text, unmarked output from the same and other generators, other keys,
other schemes, translations, mixed documents, code, markup, quotations, and
low-entropy structures. Positive strata cross scheme, implementation, key,
generator, tokenizer, sampling, task, language, length, entropy, payload, mixture,
and sequential order.

### Calibrate the actual decision

Analytic null distributions are accepted only when the complete implementation and
population satisfy their assumptions. Otherwise, calibrate the complete decision
empirically on independent negative data. Repeated contexts are not counted as
independent without a scheme-specific justification.

Reports include point estimates, confidence bounds, sample size, abstention, and
operating point per stratum. Zero observed false positives is reported with a
one-sided upper bound, never as zero false-positive probability.

Window, span, key, payload, language, attack, and interim search create
multiplicity. The preregistration controls the complete family through a frozen
maximum-statistic null, valid family-wise procedure, hierarchical gate, or another
justified method. Picking the strongest window after looking at the test set is
exploratory.

### Evaluate attacks and composition

The locked matrix includes ordinary edits and bounded adversarial transformations:

- Substitution, insertion, deletion, formatting, cropping, and dilution
- Paraphrase at fixed nonadaptive settings
- Translation and asymmetric cross-language paths
- Mixed marked and unmarked regions
- Sequential and colliding marks
- Watermark stealing and spoofing under named access and query budgets
- Detector drift and environment reproducibility

Every transformed result independently passes protected-fact, semantic, structure,
and task-quality checks. An attack that corrupts the required meaning is not counted
as a fidelity-preserving success.

### Study ordinary Retonr behavior descriptively

Retonr may be run under its normal editorial objective on locked copies without
scheme identity or detector access. The study estimates paired before-and-after
detection outcomes beside fidelity and quality. This is a descriptive local
observation, not a removal claim.

Any generation, retry, ranking, or acceptance access to a detector invalidates the
experiment.

## Falsifiable hypotheses

Each study instantiates numeric values before opening locked results.

| Hypothesis | Required result | Failure condition |
| --- | --- | --- |
| Detector false-positive control | Upper simultaneous confidence bound meets the target in every required negative stratum | Any required stratum misses the bound or cannot reproduce the decision |
| Detector power | Lower simultaneous confidence bound meets the target above the predeclared eligible-evidence floor | Bound misses the target or abstention exceeds its ceiling |
| Domain stability | Every required language, script, code, content, and model stratum retains calibration | A pooled result hides a failing stratum |
| Edit robustness | Power meets the attack-specific target after a fixed edit that passes independent fidelity | Power, eligibility, or fidelity fails |
| Spoofing resistance | Upper bound on fidelity-valid spoof success stays below its target | A bounded attacker crosses the target |
| Mixed and sequential behavior | Each constituent mark meets calibrated detection and attribution criteria | Collision, overwriting, or multiplicity invalidates the claim |
| Editorial value | Owner preference and named lint quality improve over the strongest simple baseline | No meaningful gain or a material fidelity regression |
| Runtime negative claim | Static, configuration, process, network, and differential evidence support the exact reviewed path | Uninspectable processor, drift, hidden fallback, or unexplained divergence |
| Reproducibility | Independent clean reconstruction produces the same categorical outcomes and bounded numeric differences | Artifact, environment, or nondeterministic drift changes a decision |

Negative, contradictory, and inconclusive results remain in the public claim ledger.
They are not discarded because they weaken the product story.

## Ethics, misuse, and responsibility

Editorial control is a legitimate user interest. A person can revise an intern's
draft, use a study guide to organize an independent report, apply a house style, or
ask an editor to refine a rough document. Retonr automates bounded parts of that
workflow while retaining explicit evidence and review.

The analogy does not settle every school, employer, publisher, contract, election,
professional, or jurisdictional rule. The user remains responsible for obligations
that apply to the work. The project remains responsible for obligations that apply
to its own distribution, service mode, claims, dependencies, and data handling. A
user-responsibility notice cannot waive a project duty.

Retonr reduces misuse incentives by refusing to expose detector scores, adaptive
detector queries, watermark-removal modes, key recovery, authorship certificates,
or automatic provenance stripping. Research artifacts protect keys, private
provider outputs, personal corpora, and operational attack details.

The core is viewpoint-neutral. It does not send drafts to a provider, regulator,
employer, or platform for content review. It does not add mandatory branding,
generated-by text, hidden markers, content telemetry, or remote policy enforcement.
This architectural position coexists with accurate derivative handling and honest
documentation of current legal uncertainty.

## Limitations

1. Natural-language semantic validation remains probabilistic. Hard literal and
   format checks do not prove unrestricted meaning equivalence.
2. Provider deployments are partly opaque and can change without a reproducible
   public artifact.
3. Negative runtime assurance cannot exclude unknown trained, conditional,
   obfuscated, inherited, or future-detectable signals.
4. Watermark results depend strongly on eligible length, entropy, tokenizer,
   language, domain, key, detector, threshold, and editing history.
5. Human-written controls are heterogeneous and can overlap model distributions.
6. A style profile can leak identity and sensitive associations even when raw
   samples stay local.
7. Long-document summaries can omit dependencies. They remain untrusted guidance,
   never the fidelity source of truth.
8. Format preservation is capability-specific. Opening a file successfully is not
   proof that every structure or behavior survived.
9. Current legal interpretations can change and differ by role and jurisdiction.
10. Paper-level documentation is not empirical evidence until the experiments are
    implemented, frozen, run, reproduced, and independently reviewed.

## Reproducibility package

Every empirical report should publish or preserve, subject to rights and security:

- Commit and dirty-state record
- Operating system, architecture, hardware, accelerator, driver, and locale
- Model, tokenizer, template, runtime, processor, adapter, and validator digests
- Dependency lockfiles, vendored-source manifest, build flags, and compiler version
- Watermark scheme, implementation, synthetic key commitment, detector procedure,
  and calibration identity
- Prompt and data manifests with rights, provenance groups, exclusions, and split
  digests
- Preregistered hypotheses, estimands, thresholds, multiplicity plan, and stop rules
- Raw categorical outcomes and content-minimized sufficient statistics
- Fidelity, task, quality, style, and editorial-lint outcomes
- Logs needed to prove research and live-path isolation
- Exact commands for build, generation, detection, analysis, and report rendering
- Negative, contradictory, aborted, drifted, and invalidated runs
- Independent reproduction record

Private text, provider secrets, production keys, and short guessable content hashes
do not become public artifacts. Access-controlled components still receive immutable
manifests so their absence and review boundary are visible.

## Logical research and implementation order

1. Freeze product invariants and source-evidence vocabulary.
2. Complete the deterministic plain-text transaction and excellent CLI without a
   watermark detector dependency.
3. Implement transparent editorial lint and strong fidelity fixtures.
4. Qualify one exact local runtime and its complete output path.
5. Add profile and guided-brief baselines and prove owner value.
6. Earn Markdown preservation and structured-text provenance preflight.
7. Build the isolated watermark fixture harness with public schemes and synthetic
   keys.
8. Reproduce baseline papers and validate the detector procedure before studying
   Retonr behavior.
9. Freeze and execute calibration, locked attacks, mixed-content, and ordinary-edit
   studies.
10. Independently reproduce the result bundle and publish negative findings.
11. Add agent interfaces only through the qualified application service.
12. Earn bounded DOCX handling, including OOXML signatures and C2PA carriers.
13. Build the native desktop after CLI and agent contracts are complete.
14. Freeze stable contracts and publish a 1.0 support and limitation matrix.

No step has a calendar estimate. Later work depends on earlier evidence, not a date.

## Open research agenda

- Can deterministic and transparent lint rules deliver most perceived anti-slop
  value before learned detectors are considered?
- Which profile representations improve blind owner preference without topic or
  phrase leakage?
- How should semantic assessment calibrate cross-unit dependencies in long
  documents?
- Can exact local runtime assurance detect configuration drift across all supported
  backends without importing their full dependency graphs?
- How often do public watermark detectors miscalibrate on templated, edited,
  translated, low-entropy, multilingual, and mixed-source text?
- How do ordinary high-quality edits affect token, semantic, sequential, and
  localized schemes at matched fidelity?
- Which 2026 semantic, multi-bit, localization, and spoofing-defense results survive
  independent reproduction?
- Can a user-controlled transformation record explain collaboration more usefully
  than probabilistic authorship classification?
- Does a relational, time-aware preference ledger outperform a simpler explicit
  rule and sample store? A graph database or learned temporal knowledge graph is
  justified only by incremental evidence.

## Conclusion

The project makes sense if it remains narrower than the anger that motivated it.
The strongest professional response to overbroad marking and unreliable AI-text
classification is not an equally overbroad removal claim. It is a reference-grade
editorial system that gives users control, preserves facts and formats, improves
named quality defects, minimizes remote exposure, and states exactly what it knows.

Re-expression will often change upstream wording patterns. That is an ordinary
consequence of editing, not proof that every source signal disappeared. Retonr's
credibility depends on keeping that distinction intact: forceful about editorial
sovereignty, conservative about scientific claims, exact about document integrity,
and willing to abstain when the evidence is not good enough.

## Companion evidence records

- [Text watermarking literature map](2026-08-12-watermark-literature-map.md)
- [Text watermark science and Retonr implications](2026-08-12-text-watermark-science.md)
- [Provider marking practices and Retonr implications](2026-08-12-provider-marking-practices.md)
- [Text provenance, marking, and editorial control](2026-08-12-provenance-policy.md)
- [Text watermark evaluation protocol](2026-08-12-watermark-evaluation-protocol.md)
- [Local watermark assurance for controlled runtimes](2026-08-12-local-watermark-assurance.md)
- [Provider-neutral local runtime research](2026-08-12-provider-neutral-runtimes.md)
