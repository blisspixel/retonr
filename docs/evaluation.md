# Evaluation strategy

## Purpose

Evaluation is the product's core evidence. It determines whether a profile compiler
adds value over simple prompting, which model artifacts are supported, which document
features graduate, and where the engine must abstain.

The live engine is optimized for fidelity and personal usefulness. It is not
optimized for detector scores, source classification, or watermark disruption.

## Evaluation questions

1. Does an accepted rewrite preserve exact invariants and supported structure?
2. How often does the system accept a semantic corruption?
3. How much eligible input can it transform at the chosen risk level?
4. Does the user prefer its style over the best simple baseline?
5. Does the style score measure voice rather than topic or copied phrases?
6. How do model, quantization, hardware, document type, channel, and mode affect the
   result?
7. Are latency, memory, load time, disk use, and energy acceptable for local use?
8. Which languages and mixed-language patterns meet the same published fidelity
   floor without being hidden inside an aggregate score?
9. Do document-specific clarification questions improve main-point, audience, and
   owner-preference outcomes enough to justify their interruption cost?
10. Does a time-aware preference representation improve prediction over an explicit
    append-only ledger without weakening explainability, revocation, or deletion?

## Baselines

Every style claim is compared with:

1. No rewrite
2. One explicit instruction with no examples
3. A generated style description
4. Five retrieved examples in a direct prompt
5. An editable measured profile without the full compiler
6. A compact per-user adapter when hardware and licensing permit

The architecture is justified only when it improves over the strongest cheap
baseline on blind user preference without a material fidelity regression.

## Corpora

Every evaluation manifest declares one immutable suite kind:

- `smoke` for fast installation and contract checks
- `development` for visible behavioral repair
- `calibration` for fitting thresholds and confidence mappings
- `locked` for untouched release evidence
- `red_team` for adversarial discovery

The manifest records its schema, digest, provenance, license, split-generation
revision and seed, annotation rubric, adjudication policy, included and excluded
cases, registered thresholds, baseline identities, exact system artifacts, hardware,
operating system, and build digest.

Cases carry cluster IDs for shared source, template, topic, participant, and other
dependence introduced by suite construction. The registered analysis names the
independent unit before results are observed.

Locked data is access-controlled under the approved data policy. Its manifest digest
is visible, case access is least-privileged and logged, and broad disclosure retires
the suite for future locked qualification.

Locked cases never become prompts, regression training data, or threshold-tuning
data. A label defect creates a new suite version and invalidation record rather than
silently changing the old result.

### Authorized user corpus

Product validation uses a predeclared minimum effect size and power analysis to set
participant and task counts. Collection records consent, ownership, source channel,
date range, language, topic, and permitted research use.

For each participant:

- Training evidence builds the profile.
- Held-out natural writing estimates whether style measurements generalize.
- Scenario responses provide comparable communicative tasks.
- Topic-held-out partitions expose content leakage.
- No participant appears in both sides of a population-level classifier split.

The data plan includes revocation, deletion, retention, and a clear policy for
derived features and cached embeddings.

### Fidelity corpus

The fidelity corpus pairs acceptable paraphrases with minimal corruptions. Categories
include:

- Subject and object swaps
- Novel, missing, or changed entities
- Quantity, unit, currency, version, and percentage changes
- Date, time, duration, and timezone changes
- Negation and negated antonyms
- `may`, `can`, `should`, `will`, and `must`
- Conditions, exceptions, scope, and temporal order
- Comparatives, superlatives, and thresholds
- Attribution and quotation boundaries
- Coreference across sentences
- List item and value associations
- URLs, paths, identifiers, code, and citations
- Cross-block references and heading relationships

Every production failure becomes a minimized development regression fixture with an
owner and risk category. If a locked case exposes the defect, the locked case remains
isolated and the regression is an independently minimized neighboring example.

### Structure corpus

Markdown fixtures cover CommonMark cases, enabled extensions, malformed input, UTF-8
offsets, escapes, nested lists, tables, reference links, raw HTML, code, LF, CRLF,
byte order marks, and final-newline behavior.

DOCX fixtures cover every declared supported and unsupported feature. A fixture is
not considered preserved merely because a ZIP file can be reopened.

## Metrics

### Selective fidelity

Report these together:

- Corruption acceptance: corrupt candidates accepted divided by corrupt candidates
  evaluated
- Accepted-set semantic error: corrupt accepted candidates divided by all accepted
  candidates
- Eligible-candidate coverage: acceptable candidates accepted divided by acceptable
  candidates
- System transformation coverage: eligible documents actually rewritten divided by
  eligible documents
- Abstention rate by reason
- Exact invariant failure rate
- Structural failure rate
- Semantic severity distribution

Accuracy without coverage can reward a system that abstains on everything. Coverage
without false-acceptance reporting can reward an unsafe system.

Accepted-set semantic error depends on the category mixture. Reports state category
prevalence and never present a curated hard-negative interval as a formal real-world
guarantee.

### Style

- Blind owner pairwise preference on the common accepted subset
- One-sided and two-sided abstention outcomes
- Coverage-adjusted product utility under a pre-registered scoring rule
- Preference over the strongest baseline
- Declared-rule compliance
- Channel fit
- Held-out feature distance
- Topic-held-out style score
- Cross-corpus rare-phrase and unique n-gram leakage
- Novel entity and quantity leakage
- Canary extraction and cross-profile extraction success rates

No authorship classifier is trusted until it demonstrates topic independence and
does not reward copied content.

### Product behavior

- Time to first useful profile
- Percentage of inferred preferences corrected by users
- Evidence exclusion and deletion success
- Rewrite acceptance, editing, and revert behavior
- Frequency and comprehension of abstention reasons
- CLI completion and error-recovery rates
- Desktop accessibility task completion
- Voice transcription correction rate and task completion

### Resource behavior

- Model load time
- First-token and completed-candidate latency
- Validation latency
- End-to-end latency
- Peak resident memory
- Model and profile disk use
- Tokens per second
- Cancellation latency
- CPU, GPU, and energy observations on declared tiers

## Statistical reporting

- Separate development, calibration, and locked test sets.
- Report confidence intervals, not only point estimates.
- Report results per risk category, mode, channel, format, model artifact, and
  hardware tier.
- Use paired comparisons when systems rewrite the same source.
- Account for repeated observations from one owner through participant-clustered
  confidence intervals or an equivalent repeated-measures model.
- Blind human raters to candidate origin.
- Randomize candidate order.
- Resolve ambiguous semantic cases through independent adjudication.
- Use human adjudication for release semantic labels and owner preference. Model
  judges may triage disagreement but are not the sole release authority.
- Publish abstentions and excluded cases.
- Never tune a threshold on the locked release set.
- Predeclare one confirmatory locked-set candidate before opening locked results, or
  predeclare a simultaneous-bound or multiplicity-correction procedure for every
  artifact, strategy, threshold, and subgroup comparison used to make a claim.
  Exploratory locked comparisons cannot qualify a newly selected candidate without a
  fresh sealed confirmatory set.
- Use one-sided exact confidence bounds for critical failure rates and report them by
  risk category rather than hiding a weak category in a pooled total.
- Use one independent cluster outcome for exact binomial bounds or a qualified
  cluster-aware interval when examples share a source, template, topic, or
  participant.
- Predeclare minimum sample sizes for every required stratum. Report insufficient
  evidence with an interval instead of selectively omitting a small stratum.

With zero observed failures in 300 independent examples, the rule-of-three upper 95
percent bound is still approximately 1 percent. A small happy-path suite cannot
support a broad meaning-preservation claim.

## Validation evaluation

Each gate is evaluated independently before it participates in the live cascade.

| Gate | Required evidence |
| --- | --- |
| Encoding and schema | Malformed and boundary input fixtures |
| Sentinels and literals | Exact positive and adversarial mutation cases |
| Structural fingerprint | Supported-format conformance fixtures |
| Novel entity and quantity | Precision and recall by entity class |
| Claim comparison | Hard negatives for roles, scope, polarity, and time |
| Entailment and contradiction | Calibration curve and threshold stability |
| Cross-unit checks | Multi-sentence and multi-block cases |
| Declared constraints | Complete precedence and conflict matrix |

An `Uncertain` result is measured as a first-class outcome. Strict policy rejects it.

## Model qualification

A model is supported only as an exact artifact and runtime combination.

The report records:

- Backend kind, version, and executable digest where available
- Runtime-reported model digest and the qualified manifest projection
- Upstream immutable revision and controlled local file digests where available
- License expression, reviewed source, and license-text digest
- Quantization, tokenizer, prompt-template, and output-schema digests
- Context limit and generation parameters
- Strategy, planner, validator, evaluator, and calibration versions
- Hardware and operating system
- Fidelity, coverage, style, latency, and resource metrics
- Long-input and repeated-run behavior
- Each supported language

Mutable tags and general benchmark rankings do not qualify a model. Any artifact,
template, parameter, runtime, evaluator, strategy, or qualification-suite change
invalidates the affected qualification record.

The same process qualifies an independent semantic evaluator. Generator and evaluator
errors can be correlated, so using one model for both requires explicit evidence.

### Runtime, backend, and quantization comparison

A device capability probe does not grant support. Each CPU, Metal, CUDA, HIP,
Vulkan, or hybrid execution class is compared through the same product cases. The
report records effective context, memory, offload, driver, runtime build, and all
settings that can change behavior.

Lower-precision artifacts receive a predeclared non-inferiority comparison against
Q8 or a higher-precision reference. Cross-runtime and cross-backend differential
tests report changes in critical accept or abstain decisions, fidelity failures,
structured-output validity, latency, and memory. Any critical divergence requires
full independent qualification or a narrower support claim.

### Multilingual and mixed-language qualification

Version 1.0 requires English, at least one additional Latin-script language, and at
least one non-Latin-script language to pass independently. Exact languages are
selected after authorized data and fluent human review are available. Each supported
language has native-authored or professionally curated cases for entities, roles,
quantities, dates, negation, modality, conditions, attribution, quotation,
coreference, formality, agreement, punctuation, Unicode, and prompt injection.

Mixed-language suites cover intra-sentence and inter-sentence switches, quotations,
technical terms, product names, code, URLs, identifiers, and left-to-right plus
right-to-left transitions where advertised. The system may not translate or move a
language boundary without an explicit transformation request. Ambiguous language is
user-declared or causes abstention.

Reports stratify every release measure by language, script, locale, mixed-language
pattern, model, mode, and format. Pooled results cannot qualify a weak stratum.
Machine-translated English cases may support diagnostics but cannot replace fluent
human fidelity and preference review.

Automatic language detection and routing receive calibrated per-language and
per-script error, misrouting, uncertainty, and abstention bounds set before the
locked run. Every qualified mixed-language pair or set must either pass its locked
cases or deterministically abstain with exact original output. It may not translate,
move a language boundary, or emit a partial result when that pair or set is not
qualified.

## Document release gates

### Plain text

- Byte-identical original on document-atomic abstention
- Source newline kind and final-newline state preserved for file rewriting
- Complete protected-span fixtures
- Deterministic output application

Any future newline-normalization behavior is a separate explicit transformation
mode, not an implicit rewrite side effect.

### Markdown

- Byte identity outside approved ranges
- Structural fingerprint equality
- No newly introduced raw HTML, link destination, or executable construct
- CommonMark and extension fixtures for each supported feature
- Fuzz and property suites pass
- Unsupported syntax produces protection or explicit abstention

### DOCX

- Untouched package-part content hashes remain equal
- Relationships and content types remain valid
- Only approved text nodes change
- Output XML and package structure validate
- Compatibility fixtures reopen successfully
- Signed, encrypted, macro-enabled, and ambiguous features are rejected as declared
- ZIP and XML resource-limit tests pass

## Product validation decision

The initial study defines numeric thresholds before architecture freeze. The default
directional requirements are:

- Zero known critical literal or structure regressions in the supported fixture set
- A calibrated upper confidence bound for semantic false acceptance that matches the
  advertised risk level
- Measurable owner preference over the strongest simple baseline
- No material fidelity regression relative to that baseline
- Useful transformation coverage at the selected threshold
- Resource requirements that fit at least one clearly documented laptop tier

If style gain is absent, simplify the profile system. If false acceptance remains
high, narrow supported inputs or raise abstention. If coverage is too low, improve
analysis and generation without weakening the fidelity gate.

## Source-form diagnostics

Source classification and published watermark methods may be studied in an isolated
research suite to understand the effects of rerendering. These diagnostics:

- Use paired prompts and unmarked controls
- Hold topics out between training and test
- Match task and length distributions
- Report receiver operating characteristics or true-positive rate at fixed
  false-positive rate
- Show fidelity and style results beside any signal change
- Include human-written controls
- Use only published reproducible methods and synthetic fixtures
- Never influence candidate selection or a release gate

A lower source-classification score is not proof of human authorship, privacy, or
provenance removal.

The watermark research suite freezes the complete detector procedure, including
normalization, tokenizer, repeated-event policy, eligibility, windows, keys,
payloads, aggregation, thresholds, abstention, and runtime. It calibrates that
complete procedure, controls every searched dimension, reports exact bounds for rare
false positives, and keeps provider production keys outside the study. The full
preregistration and noninterference contract is in
[Text watermark evaluation protocol](research/2026-08-12-watermark-evaluation-protocol.md).

## Editorial-lint evaluation

Editorial lint is evaluated as an explainable quality system, separately from
source classification and watermark research. Its findings may participate in the
live quality loop only after all hard fidelity gates pass.

The versioned corpus and the known-watermark research lane are defined in
[Editorial-quality and watermark research corpora](evaluation-corpora.md). The
checked-in development corpus is synthetic. Licensed public, participant, and locked
data remain gated by their own manifests and governance decisions.

For each rule, report:

- Exact rule and policy version, language, channel, format, and profile scope
- Positive fixtures, protected-context exclusions, and adversarial near-matches
- Precision and recall where a complete labeled set is meaningful
- False positives in human-written, quoted, technical, and accessibility content
- User acceptance, rejection, and manual revision of proposed fixes
- Source findings resolved, retained, introduced, suppressed, and uncertain
- Fidelity, style preference, document-level repetition, latency, and abstention

Compare a transparent user-editable rule baseline with any learned rule. A learned
rule ships only if it improves a predeclared quality outcome without a material
fidelity or false-positive regression. The report names reduced findings and never
converts them into a probability of human or machine authorship.

## Editorial brief and temporal preference evaluation

Compare no clarification, fixed generic questions, adaptive document-derived
questions, and a full user-authored brief. The adaptive system must improve over the
strongest simpler condition without a material fidelity, usability, privacy, or
resource regression.

Report:

- Main-point, audience, stance, requested-action, and protected-commitment adherence
- Blind owner preference and edit distance to the owner's final revision
- Questions asked, answered, skipped, revised, or marked irrelevant
- Answer time, interruption burden, abandonment, and marginal value per question
- Incorrect assumptions and incorrect promotion of situational answers to the profile
- Deterministic active-profile reconstruction at historical timestamps and contexts
- Supersession, conflict, revocation, deletion, and transitive invalidation behavior

The baseline is an explicit append-only preference ledger with time, context,
provenance, confidence, consent, supersession, conflict, and derivation edges. A graph
database, temporal embedding, or learned Temporal Knowledge Graph is justified only
by predeclared incremental value over that baseline. Recommendation or link-prediction
results from unrelated datasets are not product evidence.

## Release report

Every public release includes:

- Supported model artifact matrix
- Supported language, script, locale, mixed-language, runtime, backend, and hardware matrix
- Supported format capability matrix
- Selective-risk report
- Style comparison with baselines
- Cross-platform test results
- Accessibility status
- Known limitations and abstention categories
- Dependency and model license manifest
- Reproducible commands for public fixtures

## Research references

- [Text style transfer evaluation](https://aclanthology.org/N19-1049/)
- [Metric ensembles for style transfer](https://aclanthology.org/2025.naacl-srw.41/)
- [Negation and sentence representations](https://aclanthology.org/2022.blackboxnlp-1.20/)
- [TRUE factual consistency evaluation](https://aclanthology.org/2022.naacl-main.287/)
- [AlignScore](https://aclanthology.org/2023.acl-long.634/)
- [TinyStyler](https://aclanthology.org/2024.findings-emnlp.781/)
- [StyleDistance](https://aclanthology.org/2025.naacl-long.436/)
- [Green-token watermark](https://proceedings.mlr.press/v202/kirchenbauer23a.html)
- [SynthID-Text](https://www.nature.com/articles/s41586-024-08025-4)
