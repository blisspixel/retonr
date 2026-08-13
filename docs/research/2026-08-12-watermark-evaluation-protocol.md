# Text watermark evaluation protocol

## Abstract

This document defines a preregistered, reproducible protocol for studying text
watermarks in Retonr research. Its evidence cutoff is August 12, 2026. It is an
experimental contract, not a claim that a watermark can identify every generated
text, survive every edit, establish authorship, or be removed by Retonr.

The protocol treats a detector as a complete decision procedure. Normalization,
tokenization, repeated-context handling, length eligibility, window search, key
search, score aggregation, thresholding, and abstention are all part of that
procedure. Calibration therefore covers the full procedure, not an isolated score.
Every claim is conditional on a named implementation, immutable configuration,
key model, generator, tokenizer, content stratum, length and entropy band, attack
budget, and fidelity gate.

The central safeguard is architectural. Watermark evaluation is an offline research
activity over copied, consented, or synthetic fixtures. Detector outputs cannot
enter Retonr's live editorial generation, retry, ranking, acceptance, profile, or
feedback paths. This separation permits rigorous measurement without turning
watermark evasion into a product objective.

## Status, scope, and contribution

- **Evidence cutoff:** August 12, 2026.
- **Scope:** token-bias, sampling, semantic, multi-bit, signature, post-hoc, mixed,
  and sequential text watermarks with public primary evidence or runnable fixtures.
- **Unit of claim:** one frozen detector procedure in one declared evaluation
  domain.
- **Primary contribution:** a common statistical and operational protocol that
  tests calibration, power, robustness, spoofing, fidelity, drift, and
  reproducibility while isolating those diagnostics from Retonr's product loop.
- **Non-goal:** selecting edits to lower a detector score.
- **Non-goal:** certifying human authorship, intent, ownership, misconduct, or legal
  compliance.
- **Non-goal:** claiming universal watermark removal, universal detection, or
  detector evasion.
- **Non-goal:** inferring the behavior of a proprietary deployment from a reference
  implementation alone.

The protocol can support a narrow conclusion such as: "Under the locked study
manifest, detector version X met the declared false-positive bound and power floor
for English prose with at least Y eligible events under attacks A and B that passed
the fidelity gate." It cannot support: "the text is AI-written," "the watermark is
unbreakable," or "Retonr removes watermarks."

## Evidence and claim discipline

### Evidence classes

Every material statement in a report receives one of these evidence classes. A
source can support different claims at different classes. Peer review does not make
an implementation match a paper, and an official implementation does not
independently validate its reported results.

| Class | Repository label | Meaning |
| --- | --- | --- |
| E1 | peer_reviewed | Primary paper completed peer review; claim is bounded by its stated experiment or proof assumptions |
| E2 | official_implementation | Maintainer or provider artifact that can be inspected and run; conformance to a paper remains testable |
| E3 | preprint | Public primary report without completed peer review at the cutoff |
| E4 | provider_statement | Official provider claim or documentation with undisclosed or inaccessible parts not independently verified |
| E5 | local_observation | Reproducible Retonr result from a frozen manifest and preserved artifacts |
| E6 | inference | Stated conclusion derived from cited evidence and explicit assumptions |
| U | unknown | Evidence is absent, inaccessible, contradictory, too weak, or outside the tested domain |

Each report also records artifact reproducibility:

| Grade | Requirement |
| --- | --- |
| R0 | No usable artifact was located |
| R1 | Artifact is public but incomplete, mutable, or not reproduced |
| R2 | Artifact is pinned and reproduced once from a clean environment |
| R3 | Artifact is pinned, independently reproduced from a clean environment, and produces the preserved result bundle |

The compact form is **E1/R2**, **E4/R0**, or similar. E5 is not assigned until the
study bundle reaches R2. R3 requires reproduction by someone who did not prepare
the original run.

### Claim ledger

The report contains a row for every claim:

| Field | Required content |
| --- | --- |
| claim_id | Stable identifier |
| statement | Narrow, falsifiable statement |
| evidence_class | E1 through E6 or U |
| reproduction_grade | R0 through R3 |
| source_or_run | Primary source or immutable local run identifier |
| population | Scheme, detector, model, language, length, entropy, content, and threat model |
| estimand | Exact quantity and unit |
| uncertainty | Interval, multiplicity adjustment, and sample size |
| conflicts | Contrary evidence or local result |
| exclusions | Data or conditions intentionally outside the claim |
| revalidation_trigger | Change that invalidates or reopens the claim |

Negative, null, contradictory, and inconclusive findings remain in the ledger. They
are not moved to an unpublished appendix. Unknowns remain U rather than being
resolved by analogy.

## Research questions and falsifiable hypotheses

Parameters below are preregistered for each study. They are not universal defaults:

- alpha_target: maximum acceptable false-positive probability for a specified null
  stratum.
- beta_target: minimum acceptable detection power for a specified positive stratum.
- delta_quality: largest acceptable degradation on a named quality measure.
- epsilon_fidelity: maximum acceptable probability of a material fidelity failure.
- delta_drift: largest accepted detector or data drift on a named measure.
- L_min: minimum eligible evidence count, defined by the scheme rather than by
  characters or words.

### RQ1: Does the complete detector procedure control false positives?

For every claim-bearing negative stratum s:

- **Performance hypothesis:** the upper simultaneous confidence bound for
  Pr(flag | unmarked, s) is no greater than alpha_target,s.
- **Failure condition:** the bound exceeds alpha_target,s, the detector cannot
  reproduce its locked score, or exclusions make the stratum unidentifiable.

The null population is "not generated with the exact key and embedding
configuration under test." It is not synonymous with "human." Other models, other
keys, other watermark schemes, edited text, translation, code, and mixed documents
are separate null strata.

### RQ2: At what evidence lengths and entropy levels is power adequate?

For every required positive stratum s at or above L_min:

- **Performance hypothesis:** the lower simultaneous confidence bound for
  Pr(flag | marked, s) is at least beta_target,s.
- **Failure condition:** the bound falls below beta_target,s or abstention exceeds
  its declared ceiling.

Power is reported against eligible evidence count and entropy band. It is not
silently extrapolated to shorter or lower-entropy text. The dependence of
detectability on length and entropy is central to both green-list methods and
SynthID-Text ([KGW](https://proceedings.mlr.press/v202/kirchenbauer23a.html);
[SynthID-Text](https://www.nature.com/articles/s41586-024-08025-4)).

### RQ3: Does calibration survive domain change?

- **Hypothesis:** a threshold calibrated on the development domain still satisfies
  RQ1 in each locked language, script, code, content, and model stratum.
- **Failure condition:** any required stratum misses its bound, even when a pooled
  aggregate passes.

### RQ4: What survives a fixed, fidelity-preserving edit?

- **Hypothesis:** after each locked nonadaptive edit at a named intensity, the lower
  power bound remains at least the attack-specific beta_target and the output passes
  the independent fidelity gate.
- **Failure condition:** power misses the floor, the fidelity gate fails, or too few
  samples remain eligible to estimate the quantity.

Robustness claims do not include edits that changed the document's facts, required
meaning, protected spans, or task correctness.

### RQ5: What succeeds under bounded adaptive access?

- **Hypothesis:** under each preregistered attacker knowledge level and query budget,
  the lower robust-power bound and upper spoofing bound meet their declared
  criteria.
- **Failure condition:** a valid attack crosses either bound, or detector feedback
  leaks beyond the declared oracle.

Adaptive experiments test a research system built for the study. They do not expose
a detector oracle through Retonr.

### RQ6: Can an unmarked generator be made to spoof a mark?

- **Hypothesis:** the upper simultaneous confidence bound for successful,
  fidelity-valid spoofing stays below sigma_target for every locked target scheme
  and access model.
- **Failure condition:** the bound exceeds sigma_target or attribution ambiguity is
  observed.

Watermark stealing and spoofing are required because published attacks show that a
detectable signal need not establish the named generator's authorship
([watermark stealing](https://proceedings.mlr.press/v235/jovanovic24a.html);
[DITTO](https://aclanthology.org/2026.eacl-long.229/)).

### RQ7: How do mixed and sequential watermarks interact?

- **Hypothesis:** each required detector controls false flags and meets power for
  its declared mixture proportions, segment lengths, orderings, and key
  combinations after correction for the full search.
- **Failure condition:** collision, overwriting, nested signals, or key
  multiplicity invalidates either calibration or payload decoding.

Sequential composition must be tested explicitly. ENS composes independent keyed
unbiased marks, so treating its layers as one ordinary single-key observation would
miss a defining part of the procedure
([ENS](https://openreview.net/forum?id=iZ7i2y1YxO)).

### RQ8: Does Retonr's ordinary editorial behavior change detection outcomes?

- **Hypothesis:** with Retonr configured only for its declared editorial objective,
  detection rates before and after the edit can be estimated with paired
  uncertainty while quality and fidelity meet their independent criteria.
- **Required interpretation:** this is a descriptive local observation, not an
  evasion claim.
- **Failure condition:** any generation, retry, ranking, or acceptance decision had
  access to scheme identity, detector output, or an attack objective.

### RQ9: Are detector results stable and reproducible?

- **Hypothesis:** two clean reproductions from the immutable bundle produce
  byte-identical categorical decisions and scores within a preregistered numeric
  tolerance.
- **Failure condition:** environment, tokenizer, normalization, runtime, key,
  nondeterminism, or dependency drift causes a decision mismatch.

### RQ10: What quality and capacity cost buys a detection result?

- **Hypothesis:** for each locked watermark strength and payload, the lower
  confidence bound for the paired quality difference exceeds -delta_quality, the
  fidelity-failure upper bound is at most epsilon_fidelity, and the applicable RQ2
  power floor is met.
- **Failure condition:** any bound fails, exact payload recovery misses its floor,
  or the comparison changes generator settings other than the watermark.

The result is a set of supported operating points, not one universal quality score
or a post hoc optimum.

## Separation from the live editorial loop

### Noninterference rule

The research path and product path are separate trust domains:

| Live editorial path | Offline research path |
| --- | --- |
| Accepts the user's local document and declared editorial brief | Accepts only copied, synthetic, public, or explicitly consented fixtures |
| Uses local generation after setup | Uses frozen research environments, which may include public reference models |
| Applies deterministic structure and protected-span checks plus explicitly bounded semantic review | Applies watermark embedders, detectors, attacks, statistical analysis, and research fidelity measurement |
| Returns a candidate, abstention, or validation result | Returns an access-controlled experiment bundle and claim ledger |
| Never receives a detector score, key, scheme label, or watermark classification | Has no callable edge into generation, retry, ranking, acceptance, style memory, or user feedback |

The permitted flow is one way:

1. A fixture is copied into an isolated study dataset under its data rights.
2. The study operates on the copy.
3. Aggregate, reviewed research conclusions may inform documentation or a future
   product decision.
4. No row-level score, label, learned attack, threshold, key, or detector feature
   returns to the product.

### Prohibited couplings

The following are protocol violations:

- Calling a detector from the CLI, desktop app, library API, candidate generator,
  ranker, retry loop, acceptance gate, style profile, feedback learner, or temporal
  knowledge graph.
- Selecting among candidates based on a watermark score, label, uncertainty, or
  proxy trained on those outputs.
- Prompting a model to reduce, erase, imitate, collide with, or preserve a target
  watermark.
- Retrying after a detector result.
- Sending a private user draft to a provider detector or attack service.
- Storing per-user detector scores in telemetry, a profile, or a feedback history.
- Treating the research detector as a content moderation, authorship, employment,
  education, or legal decision system.

Tests for this boundary include dependency-graph inspection, command-surface
inspection, fixture-only input validation, network denial during the live loop, and
an integration assertion that no research package is linked into shipping
artifacts. A boundary test failure stops the study and release review.

## The experimental object

### Complete detector procedure

A detector procedure D is frozen as:

D = (normalizer, tokenizer, context rule, repeat rule, score, aggregator, windows,
keys, payloads, thresholds, abstention, runtime).

Changing any member creates a new D and invalidates the old calibration for that
new procedure. This includes a harmless-looking Unicode normalization change,
tokenizer patch, expanded window family, added key, different repeated-n-gram
setting, or changed minimum length.

The detector returns:

- flag;
- no_flag; or
- abstain with a machine-readable reason.

The raw score is retained for analysis, but product code never consumes it.
Abstention is an outcome, not a discarded row. Reports provide:

- unconditional flag rate among all eligible negatives;
- conditional flag rate among determinate negatives;
- abstention rate and reasons;
- unconditional detection rate among all marked cases;
- conditional power among determinate marked cases.

### Independent experimental unit

The default unit is an independently sampled source document or prompt-generation
run. Tokens, windows, keys, payload bits, and several completions from one prompt
are not independent documents.

- Split by source document, prompt family, author, repository, and near-duplicate
  cluster before generation.
- Keep all transformations and repeated generations of one source in one split.
- Use paired analysis for original and edited versions.
- Cluster confidence intervals by source or prompt when several outputs share it.
- Report document-macro estimates as primary. Token-micro estimates are diagnostic.
- If a hierarchical model is used, preregister its formula, priors or estimators,
  convergence checks, and fallback analysis.

### Eligibility and evidence count

Length means the number of eligible scoring events after the scheme's frozen
context and repeat rules. Reports also include raw bytes, Unicode scalar values,
characters, whitespace-delimited words, tokenizer tokens, scored tokens, unique
contexts, and unique scoring events.

L_min is fixed before the locked test. A sample below L_min abstains for
insufficient evidence. It cannot be padded, joined with another source, moved into a
shorter post hoc bin, or counted as a no_flag.

## Data design

### Split policy

Use four disjoint partitions:

1. **Fixture development:** integration debugging only.
2. **Detector training:** only when the published detector requires training, such
   as a keyed Bayesian SynthID detector.
3. **Calibration:** thresholds, bins, and abstention rules are frozen here.
4. **Locked test:** opened once for the preregistered analysis.

Independence is enforced by provenance groups, not random rows. Semantic
near-duplicate detection runs before assignment. Any leak across partitions stops
the run. A replacement split receives a new study identifier and the leaked run is
preserved as invalid.

The official SynthID reference states that its Bayesian detector is trained for a
specific key on independent, representative watermarked and unwatermarked data.
Training, calibration, and locked evaluation therefore cannot reuse responses
([SynthID reference implementation](https://github.com/google-deepmind/synthid-text)).

### Negative corpus

The negative matrix contains independently labeled strata:

- native human prose with documented rights and provenance;
- human prose assisted by ordinary editing tools, with assistance level labeled;
- templated and highly repetitive human text;
- unmarked output from the same generator, tokenizer, sampling policy, and task;
- unmarked output from other generators and model families;
- output marked under another independently sampled key;
- output marked by another scheme;
- legitimate editorial revisions of unmarked human and model text;
- direct and back translations by language direction;
- mixed human and model-authored documents with exact segment boundaries;
- source code, markup, tables, lists, quotations, and low-entropy structured text;
- content that resembles known trigger patterns without use of the target key.

Human and AI are not the two values of a single null label. Each source process is
reported separately. Pooled estimates use frozen deployment weights and never hide
the worst required stratum.

### Positive corpus

The positive matrix crosses:

- scheme and immutable implementation;
- key and key-generation method;
- generator and tokenizer;
- sampling parameters;
- task and content domain;
- language, script, and code language;
- eligible length band;
- entropy band;
- payload for multi-bit schemes;
- mark order and layer count for sequential schemes;
- mixture proportion and marked segment length for mixed documents.

Prompts and generation seeds are assigned before looking at detector outcomes.
Generation failures, refusals, truncations, and empty outputs remain accounted for
with preregistered exclusion or abstention reasons.

### Length, entropy, language, and code strata

Length cut points are defined on calibration data and frozen. A minimum matrix
includes below-L_min, just-above-L_min, medium, and long eligible-evidence bands.
No neighboring bins are merged because a result is unfavorable.

The long-text band includes documents that exceed one detector window, one
generation context, and one ordinary response. Evaluate full-document, fixed-window,
and localized decisions separately. Record concatenation boundaries, chunking,
maximum scanned windows, memory and compute failures, and score behavior as
unmarked material accumulates. A detector that searches more locations on a longer
document must calibrate that expanded search rather than reuse a short-text
threshold.

When generator logits are available, retain per-token entropy, the scheme's
published entropy statistic when applicable, and eligible-event entropy. Freeze
entropy cut points on calibration data. When logits are unavailable, a proxy can be
reported only as **entropy_proxy** with its model and error study. It cannot be
interpreted as the generator's true conditional entropy.

Language strata identify language, locale, script, tokenizer, direction of
translation, and code switching. Native reviewers assess fidelity where human
review is required. Cross-language results are never inferred from English.
Published crosslingual evaluation found substantial direction-dependent failures
for tested watermark and translation combinations, which makes direction a required
factor rather than nuisance variation
([crosslingual evaluation](https://aclanthology.org/2025.findings-emnlp.390/);
[official fixtures](https://github.com/SecureDL/xlingual_watermark_eval)).

Code is a separate population. Record programming language, parser, formatter,
compiler or interpreter, task, test suite, and abstract syntax tree transformation.
Measure compile rate, tests passed, task correctness, and semantic equivalence
independently from detection. Variable renaming, formatting, dead-code insertion,
and other meaning-preserving code transformations are separate attacks. Published
code experiments show that robustness observed on prose cannot be assumed for code
([code study and implementation](https://github.com/uiuc-arc/llm-code-watermark)).

## Null calibration and uncertainty

### Analytic, exact, and empirical nulls

An analytic null is used only when the complete implementation satisfies the
paper's assumptions for the actual text population. The study records the proof
obligation: token-event distribution, key independence, context uniqueness,
conditioning, selection, and all scanned windows or keys.

If that obligation is not met, calibrate the complete decision procedure
empirically on an independent negative corpus. Analytic score formulas can remain
diagnostics, but their nominal p-values are not represented as calibrated error
probabilities. This distinction is necessary because asymptotic z-score tails can
miscalibrate at the very low false-positive rates often claimed for text
watermarks. Non-asymptotic and unique-n-gram methods have been proposed to address
that issue
([Three Bricks](https://arxiv.org/abs/2308.00113);
[statistical pivots](https://arxiv.org/abs/2404.01245)).

The empirical calibration invokes exactly the same normalization, eligibility,
window scan, key scan, payload decode, threshold, and abstention code as the locked
test. Calibrating one score and then maximizing over many scores in evaluation is
invalid.

A key-randomized empirical null can score fixed negative documents with independent
synthetic keys when the scheme permits it. Keys and documents are both sampling
factors: keep all scores for one document in the same cluster, report the number of
documents and keys, and do not describe their Cartesian product as that many
independent documents. A text-randomized null and key-randomized null answer
different questions and are both retained when key sensitivity is claim-bearing.

### Repeated n-grams and dependent evidence

Context-seeded watermark scores can reuse the same pseudorandom decision when
contexts repeat. Counting those occurrences as independent evidence makes a
binomial or normal null anticonservative. The official KGW implementation
recommends ignoring repeated n-grams for this reason
([KGW implementation](https://github.com/jwkirchenbauer/lm-watermarking)).
SynthID-Text likewise has a repeated-context rule that must match between generation
and detection; the reference implementation is the authority for the frozen
fixture, not an assumed independent-token model
([SynthID reference implementation](https://github.com/google-deepmind/synthid-text)).

For every context-dependent fixture, report:

- raw tokenizer tokens;
- context width;
- eligible contexts;
- distinct contexts;
- eligible scoring events;
- distinct scoring events;
- repeated events excluded;
- score with the specification-correct repeat rule;
- naive all-token score, labeled diagnostic and excluded from claims.

The unique unit is defined by the scheme. It can be a context-token pair, context,
key position, semantic region, or another published event. If independence remains
unproven after deduplication, use an empirical null clustered by document.

### False-positive estimates

For each negative stratum, report n, flags, no_flags, abstentions, the point
estimate, and a 95 percent confidence interval. For rare events:

- use an exact binomial interval or a preregistered method with demonstrated
  coverage;
- use a one-sided upper bound for the alpha_target criterion;
- if zero flags occur in n eligible trials, report the upper bound
  1 - gamma^(1/n), where 1 - gamma is the one-sided confidence level, not "zero
  FPR";
- determine n before opening the locked test from the target upper bound;
- use clustered or block resampling when the sampling unit is a prompt, author, or
  source document rather than an independent Bernoulli row.

For independent Bernoulli document units, the smallest zero-event sample capable of
placing the one-sided upper bound at or below alpha_target is:

n_min = ceiling(log(gamma) / log(1 - alpha_target)).

Use the multiplicity-adjusted gamma assigned to that claim, not automatically 0.05.
If false flags are anticipated, derive the acceptance count and n by exact binomial
inversion or a preregistered simulation. For a clustered design, derive n from the
cluster model or conservative design effect and analyze at the declared independent
unit. Do not apply the independent-document formula to token or key replicates.

Power sample size is likewise locked from beta_target, a declared plausible power
under the alternative, the one-sided lower-bound method, multiplicity, clustering,
and expected abstention. The preregistration states the minimum observed detection
count that would satisfy the lower-bound criterion. A study is not enlarged after
seeing a nearly passing result unless it used a valid preregistered sequential
design.

NIST guidance describes exact binomial intervals for small samples and rare
probabilities, and its measurement guidance emphasizes uncertainty for both false
alarm and detection probabilities
([binomial intervals](https://www.itl.nist.gov/div898/handbook/prc/section2/prc241.htm);
[instrument performance](https://www.nist.gov/publications/estimating-instrument-performance-confidence-intervals-and-confidence-bounds)).

### Power, payload, and localization

For every positive stratum, report the same counts and a 95 percent lower
confidence bound for power. AUROC and precision-recall curves may be secondary
descriptions. They do not replace a locked operating-point analysis.

Multi-bit reports add:

- exact-message recovery;
- per-bit accuracy with document-clustered uncertainty;
- invalid-codeword and abstention rates;
- payload size and effective capacity per eligible event;
- confusion between user, model, key, or message identifiers;
- false payload decode on every negative stratum.

Localization reports separate:

- document-level flag;
- segment boundary overlap;
- span precision and recall;
- boundary error in eligible events;
- missed marked segments;
- false localized spans in negative documents.

Document detection and span localization are different estimands. Efficient
windowed detectors can search for localized marks, but the window selection itself
must be included in calibration
([localized detection](https://aclanthology.org/2025.acl-long.316/);
[official implementation](https://github.com/XuandongZhao/llm-watermark-location)).

### Multiplicity

The preregistration enumerates every threshold, direction, window length, offset,
key, payload, detector, language stratum, attack, and interim look that can create a
claim. The complete family is controlled by one of:

- an empirical maximum-statistic null for the frozen search;
- a valid family-wise correction such as Holm or Bonferroni;
- a preregistered hierarchical gate in which later claims are tested only after an
  earlier gate passes;
- another method with a cited coverage argument under the actual dependencies.

The report includes raw and adjusted intervals or p-values. Picking the strongest
window, key, language, or attack after seeing the test data is exploratory and
cannot support a confirmatory claim.

### Fixed and sequential sampling

The default design has a fixed locked sample size. Optional stopping is prohibited.
If a sequential design is necessary, preregister the information times, stopping
boundaries, alpha-spending or anytime-valid method, and maximum sample size.
Stopping early because the observed rate looks adequate or harmful invalidates an
ordinary fixed-sample interval.

## Window, span, key, and mixture design

### Window family

Freeze a finite window family by eligible-event lengths and offsets. The detector
procedure returns the maximum or other preregistered aggregate over that family.
Calibrate that aggregate, not a nominal per-window statistic. Record how text edges,
short windows, repeated contexts, and normalization affect eligibility.

### Key model

Use synthetic research keys generated by a documented cryptographic random process.
Keys are independent across splits unless the research question explicitly tests
reuse. Record:

- scheme and key format;
- generator and entropy source;
- number of candidate keys;
- assignment to train, calibration, and test;
- key-reuse policy;
- public commitment or keyed hash for artifact verification;
- access principals and destruction or retention rule.

Raw secret keys are not committed to version control, placed in reports, or shared
with product code. Provider production keys are never requested, inferred, or used.

A key scan is part of D. Adding K keys and flagging when any passes changes the
false-positive probability. Calibrate the K-key decision directly or apply a valid
family correction. Report both target-key detection and non-target-key confusion.

### Mixed documents

Construct controlled documents from exact source spans. Factors include:

- marked proportion;
- marked segment length;
- number of segments;
- segment position;
- human, unmarked-model, and marked-model surrounding content;
- same-topic and different-topic joins;
- same-language and code-switched joins;
- boundary-preserving and boundary-obscuring edits.

Store character, token, and eligible-event boundary truth. A document flag cannot
be counted as correct localization. Dilution by inserting unmarked content is both
a mixture condition and an attack when performed adversarially.

### Sequential and colliding marks

Cross:

- key A then key B under one scheme;
- scheme A then scheme B;
- unbiased layers with independent keys;
- multi-bit payload A then payload B;
- embedding before and after translation or paraphrase;
- repeated embedding with the same key;
- a marked excerpt inside another marked document.

For each order, report detection and payload recovery for every constituent mark,
joint outcomes, score displacement, quality, and false attribution. The result can
be overwriting, coexistence, interference, or unknown. Do not infer one from
single-mark performance. Collision experiments follow the same quality gates and
complete-procedure calibration
([watermark collision](https://aclanthology.org/2025.findings-naacl.37/)).

## Attack evaluation

### Threat-model declaration

Every attack study names:

- attacker goal: removal, missed detection, payload corruption, spoofing, false
  localization, key discrimination, or detector extraction;
- attacker knowledge: text only, public algorithm, generator and tokenizer,
  detector family, exact detector, key, or reference corpus;
- oracle: none, one binary decision at the end, binary decisions, rounded score,
  exact score, or internal gradients;
- query budget and whether failed queries count;
- compute, model, data, and wall-clock budget;
- allowed edit operations and maximum intensity;
- required fidelity and task-quality gates;
- target: document flag, localized span, key identity, or payload;
- locked stopping rule;
- transfer target and whether it was unseen during attack development.

Results are not compared across different knowledge, oracle, fidelity, or budget
levels as though they were the same threat model.

### Locked nonadaptive suite

The nonadaptive suite is fixed before detector outcomes are opened. Each
transformation has immutable code, parameters, model weights, prompt, decoding
settings, and seeds. It includes, where meaningful:

- canonical Unicode and line-ending normalization;
- controlled whitespace, punctuation, casing, spelling, character substitution,
  insertion, and deletion;
- random token-level insertion, deletion, and substitution at fixed eligible-event
  rates;
- head, tail, and centered cropping;
- marked-span cropping with and without boundary knowledge;
- dilution by unmarked text at fixed proportions and segment layouts;
- sentence and paragraph reordering that remains coherent under the fidelity gate;
- fixed-model paraphrase at declared strengths;
- compression, expansion, and summarization as separate transformations;
- translation and back translation for every declared direction, including
  asymmetric one-way translation;
- sequential combinations in a frozen order;
- parser-based code formatting, identifier renaming, equivalent expression
  replacement, comment edits, and dead-code transformations.

An operation that is nonsensical for a stratum is marked not applicable before the
test. It is not counted as robust or failed. The test must retain attempted edits
that fail to produce output.

Published results establish that robustness varies by scheme and attack rather than
supporting one universal ranking. WaterPark evaluates ten watermarkers and twelve
attack families in a common framework
([peer-reviewed study](https://aclanthology.org/2025.findings-emnlp.1148/);
[official implementation](https://github.com/JACKPURCELL/WaterPark)). Paraphrase,
translation, and smoothing studies further motivate locked, transformation-specific
claims
([paraphrase evaluation](https://aclanthology.org/2024.emnlp-main.1005/);
[smoothing attacks](https://aclanthology.org/2025.findings-emnlp.264/)).

### Adaptive suite

Adaptive evaluation is isolated from the locked nonadaptive suite. Use ascending
access levels:

| Level | Attacker access |
| --- | --- |
| A0 | Watermarked text only; no scheme or detector information |
| A1 | Public scheme and reference implementation; no target key or oracle |
| A2 | Bounded binary detector decisions |
| A3 | Bounded rounded or bucketed scores |
| A4 | Bounded exact scores |
| A5 | White-box local detector and synthetic key |

A4 and A5 exist to characterize worst-case research behavior, not to define a
product interface. Each level uses a separately locked query budget. Query
transcripts, rejected calls, cached calls, and errors count according to the
preregistration. Development queries never touch the transfer test. The final
candidate is scored once by a held-out detector instance or key when the research
question tests transfer.

If an adaptive method tests several candidates and returns the best, the quality
and fidelity gate applies to the returned candidate. The report also gives total
queries, proportion of sources with any valid candidate, and success among all
assigned sources. It cannot report only sources on which the attack converged.

### Translation and paraphrase

Freeze translator or paraphraser model, immutable weight digest, prompt, sampling
policy, locale, and seed. Separate:

- same-language paraphrase;
- one-way translation;
- back translation;
- pivot translation;
- paraphrase followed by translation;
- translation followed by paraphrase.

Report source and target tokenizer eligibility separately. Native-language human
review is required for claim-bearing fidelity results. A reference-model semantic
score alone cannot establish preserved meaning. Google documents that thorough
rewriting and translation can reduce SynthID confidence, but that official
statement is E4 evidence about provider behavior, not a universal attack result
([SynthID safeguards](https://ai.google.dev/responsible/docs/safeguards/synthid)).

### Dilution, cropping, and localized edits

Attack intensity is based on eligible events and marked-span coverage. Report:

- original and remaining marked eligible events;
- inserted unmarked eligible events;
- marked fraction before and after;
- location of retained or deleted spans;
- whether the attacker knew a boundary;
- detector window selected and its corrected significance.

An attack that merely removes nearly all marked content is not compared with a
meaning-preserving paraphrase without showing the retained-information and fidelity
differences.

### Stealing and spoofing

Use only public reference systems, locally generated synthetic keys, and datasets
whose licenses permit the experiment. Do not probe a provider's production
detector, infer a production key, or imitate a real organization.

The stealing split contains disjoint:

- query prompts and target responses;
- attack-training data;
- attack-validation data;
- locked spoofing test;
- locked removal transfer test;
- prompts, topics, and generators reserved to detect memorization.

Record the target query count, unique-token count, cost model, key-reuse pattern,
surrogate capacity, and whether the victim exposes labels or scores. Evaluate both:

- **spoofing:** unmarked or adversarially generated content is attributed to the
  target mark while passing fidelity and policy gates;
- **scrubbing:** target-marked content no longer flags while passing fidelity and
  task-quality gates.

Also measure false transfer to other keys and schemes. Published watermark stealing
and DITTO results show that signal replication and attribution ambiguity are
practical research questions for tested systems, not merely theoretical edge cases
([stealing paper](https://proceedings.mlr.press/v235/jovanovic24a.html);
[official attack artifacts](https://watermark-stealing.org);
[DITTO paper](https://aclanthology.org/2026.eacl-long.229/)).

### Oracle leakage controls

- Research detectors bind only to loopback or an isolated worker network.
- Access requires a study identifier and is denied to product processes.
- Responses disclose only the oracle level assigned to the attack.
- Query logs are append-only within the experiment bundle.
- Keys are loaded into the detector worker, not passed in requests.
- Development and locked-test workers use disjoint keys and credentials.
- Exact scores are never included in user-visible errors, telemetry, profiles, or
  released per-document artifacts.
- Rate and budget enforcement occurs before scoring.
- Caches are partitioned by study and cannot reveal another key's result.

Any unplanned score, key, threshold, or locked-test response exposure is leakage.
Stop the run, revoke affected synthetic keys, preserve the incident record, and
start a new preregistration if research continues.

## Quality and fidelity co-measurement

### Independent objectives

Detection, quality, and fidelity are distinct outcomes. An edit is a valid attack
success only when:

1. it was produced within the locked threat model;
2. it crosses the preregistered detector criterion;
3. it passes every required deterministic fidelity check;
4. it passes task-quality criteria;
5. it does not introduce a policy or safety failure.

Report both:

- joint valid success among all assigned sources; and
- detector change conditional on passing fidelity.

This prevents low-quality or meaning-changing output from making an attack look
strong, and prevents fidelity failures from disappearing through conditioning.

### Deterministic fidelity checks

Before model-based or human judgment, compare:

- names, organizations, products, locations, dates, times, units, numbers, and
  currencies;
- URLs, citations, identifiers, file paths, commands, and code symbols;
- quotations and their boundaries;
- negation, modality, uncertainty, requirements, prohibitions, and exceptions;
- headings, lists, tables, links, code fences, front matter, and other required
  structure;
- user-declared protected spans and exact tokens;
- sentence or span alignment needed to audit omissions and additions.

Failures are categorized, not collapsed into one similarity score. Deterministic
checks provide strong evidence about the fields they inspect but do not prove full
semantic equivalence.

### Semantic and human review

Model-based entailment, embeddings, learned evaluators, and reference-free quality
scores are secondary, probabilistic measurements. Record model, prompt, weights,
calibration, directionality, and disagreement. They are never formal fidelity
guarantees.

Claim-bearing human review uses:

- reviewers qualified for the language and domain;
- randomized, blinded source-output pairs;
- a rubric fixed before review;
- separate judgments for factual consistency, omissions, additions, intent,
  fluency, and overall acceptability;
- duplicate review on a preregistered fraction;
- adjudication that is blind to detector outcomes;
- inter-reviewer agreement with uncertainty;
- retained reasons for rejection.

The detector score and attack label are hidden from reviewers. Review order is
randomized, and the same reviewer does not see every transformation of one source
consecutively.

### Paired quality and capacity analysis

Compare watermarked and unwatermarked outputs from the same prompt or source under
matched generator, tokenizer, sampling, and budget settings. When the sampling
construction permits shared random numbers without changing either distribution,
record that pairing; otherwise pair by prompt and generation replicate. Report the
paired effect and its confidence interval for every task-quality measure.

Watermark strength, payload length, eligible evidence, generation latency, exact
message recovery, power, false positives, and fidelity failures form a
multi-objective surface. Evaluate only preregistered operating points. Show all
points and uncertainty, including dominated and failed ones. Do not select a
strength on the locked test or compress incompatible measures into an unvalidated
composite score.

### Editorial-loop observation

For RQ8, Retonr receives only the source, editorial brief, protected spans, and its
ordinary configuration. The candidate is frozen before research scoring. A separate
research worker then scores source and output exactly once. No score-dependent
retry, candidate choice, prompt change, or profile update is allowed.

The paired report includes:

- ordinary editorial acceptance and abstention;
- deterministic fidelity outcomes;
- blinded human fidelity outcomes when required;
- detector transition table: flag to flag, flag to no_flag, flag to abstain, and
  corresponding transitions from other starting outcomes;
- score change as a descriptive diagnostic;
- eligible-evidence change and length change;
- stratification by edit intent and magnitude.

The allowed conclusion is about that frozen Retonr configuration on that dataset.
It is not a claim of watermark removal or detector evasion.

### Code quality

Code transformations additionally require:

- successful parse and format;
- successful compile or type check where applicable;
- unchanged public interface unless the task permits a change;
- all locked tests passing;
- unchanged expected outputs and side effects on held-out tests;
- no newly introduced warnings, unsafe behavior, or dependency;
- manual review for transformations not covered by tests.

Pass rate and watermark outcome are reported jointly. Passing a weak test suite is
not proof of semantic equivalence, so test adequacy and uncovered behavior remain
limitations.

## Detector and data drift

### Immutable identity

The study manifest pins:

- source repository and commit;
- package and artifact digests;
- patches;
- compiler, interpreter, runtime, and operating system;
- model and tokenizer identifiers plus immutable weight and vocabulary digests;
- normalizer and Unicode data version;
- accelerator and deterministic-kernel settings;
- detector training data and checkpoint;
- thresholds, keys, window family, and repeat rule.

"Latest" means the latest stable generally available release selected at
preregistration. That release is then pinned for the study. A later stable release
creates a new detector identity and new calibration rather than silently changing a
reproducibility bundle.

### Canary and drift checks

Before every run:

- score canonical positive, negative, repeated-context, Unicode, short-text,
  multilingual, mixed-span, and code canaries;
- require exact categorical decisions;
- require raw-score agreement within the declared numeric tolerance;
- verify input, output, environment, model, tokenizer, and executable digests;
- verify that networking and process boundaries match the manifest.

For a continuing claim, run labeled drift samples from each supported stratum.
Preregister the drift estimand, such as change in false-positive probability, power,
abstention, score quantiles, or a distribution distance, plus delta_drift and its
confidence procedure. An unlabeled score-distribution alert can trigger review but
cannot prove that error rates changed.

### Recalibration triggers

Any of these invalidates calibration until a new locked study passes:

- detector, normalizer, tokenizer, Unicode, runtime, or numerical-kernel change;
- generator, model weights, sampling, prompt family, or output filter change;
- threshold, key count, key-reuse policy, payload, window, or minimum-length change;
- detector-training or calibration-data change;
- supported language, script, code language, content domain, or length expansion;
- changed attack model or oracle;
- detected data contamination or provenance correction;
- material shift beyond delta_drift;
- upstream implementation or paper correction affecting assumptions.

Thresholds are never updated automatically from live traffic. Every recalibration
gets a new version, preregistration, test split, and claim ledger entry.

## Public fixture matrix

### Selection principles

The matrix favors primary papers, official implementations, distinct mechanism
families, and isolated reproducibility. It is not a leaderboard and need not install
every research toolkit into the repository.

- Resolve the latest stable generally available artifact when a study is
  preregistered, then pin its commit and digest.
- Run each family in its own locked environment or container.
- Keep adapters and result schemas small. Do not add a framework dependency when a
  frozen command-line fixture is enough.
- Patch only through a recorded overlay. Never edit upstream code without preserving
  the diff.
- Audit license, model terms, data rights, network behavior, and artifact origin.
- Treat framework reimplementations and author implementations as separate
  fixtures. Agreement is an experiment, not an assumption.

### Core matrix

| Family | Primary fixture | Evidence at cutoff | Required purpose |
| --- | --- | --- | --- |
| Green-list token bias | [KGW paper](https://proceedings.mlr.press/v202/kirchenbauer23a.html) and [official code](https://github.com/jwkirchenbauer/lm-watermarking) | E1 plus E2 | Exact and empirical nulls, repeat handling, length, entropy, windows, edits |
| Tournament sampling | [SynthID-Text paper](https://www.nature.com/articles/s41586-024-08025-4), [reference code](https://github.com/google-deepmind/synthid-text), and [Transformers implementation](https://huggingface.co/docs/transformers/internal/generation_utils) | E1 plus E2; deployment statements E4 | Mean, weighted, and keyed Bayesian detectors; training separation; low-entropy and repeated-context behavior |
| Distribution-preserving sampling | [Robust distortion-free paper](https://openreview.net/forum?id=FpaCL1MO2C) and [official code](https://github.com/jthickstun/watermark) | E1 plus E2 | Empirical null, key alignment, edits, and quality without assuming distributional equality in the implementation |
| Unbiased reweighting | [Unbiased watermark paper](https://proceedings.iclr.cc/paper_files/paper/2024/hash/c5b00c5bdcc6fe35907dbcca03d27652-Abstract-Conference.html) | E1; artifact grade audited at execution | Unbiasedness diagnostics, detector power, short text, smoothing |
| Entropy-aware code | [SWEET paper](https://aclanthology.org/2024.acl-long.268/) and [official code](https://github.com/hongcheki/sweet-watermark) | E1 plus E2 | Code-specific entropy filtering, compilation, tests, abstract syntax tree attacks |
| Semantic region | [SemStamp paper](https://aclanthology.org/2024.naacl-long.226/) and [official code](https://github.com/abehou/SemStamp) | E1 plus E2 | Semantic robustness, region dependence, paraphrase, multilingual transfer |
| Multi-bit | [XMark paper](https://aclanthology.org/2026.acl-long.672/) and [official code](https://github.com/JiiahaoXU/XMark) | E1 plus E2 | Exact payload recovery, capacity, message confusion, collision, quality |
| Public verification | [UPV paper](https://proceedings.iclr.cc/paper_files/paper/2024/hash/214d2cffc381938be6f7254d5382904f-Abstract-Conference.html) and [official code](https://github.com/THU-BPM/unforgeable_watermark) | E1 plus E2 | Public detection, forgery, attribution limits, secret isolation |
| Public signature | [Publicly-Detectable Watermarking preprint](https://eprint.iacr.org/2023/1661) and [archived official code](https://github.com/jfairoze/publicly-detectable-watermark) | E3 plus E2 | Signature correctness, public-key detection, payload errors, forgery, and dependency age |

The core matrix may be reduced when a research question does not require every
family. Omissions and their rationale are preregistered. A missing fixture never
becomes evidence that its family behaves like an included one.

### Extended matrix

| Research question | Candidate fixture | Evidence at cutoff |
| --- | --- | --- |
| Sequential independent keys | [ENS](https://openreview.net/forum?id=iZ7i2y1YxO) | E1; artifact availability audited at execution |
| Linguistics-aware token selection | [STELA](https://aclanthology.org/2026.acl-long.2115/) and [official code](https://github.com/Shinwoo-Park/stela_watermark) | E1 plus E2 |
| Alternative multi-bit capacity | [Codable](https://proceedings.iclr.cc/paper_files/paper/2024/hash/abdc8c031aa6c6917c3b593166e5e340-Abstract-Conference.html) and [official code](https://github.com/lancopku/codable-watermarking-for-llm) | E1 plus E2 |
| Post-hoc embedding | [TextSeal preprint](https://arxiv.org/abs/2605.12456) and [Meta Seal code](https://github.com/facebookresearch/meta-seal) | E3 plus E2 |
| 2026 robustness claim | [SAFESEAL preprint](https://arxiv.org/abs/2605.23175) | E3; artifact grade audited at execution |
| Unified benchmark | [MarkLLM paper](https://aclanthology.org/2024.emnlp-demo.7/) and [official toolkit](https://github.com/THU-BPM/MarkLLM) | E1 plus E2 |
| Multi-scheme attack benchmark | [WaterPark paper](https://aclanthology.org/2025.findings-emnlp.1148/) and [official platform](https://github.com/JACKPURCELL/WaterPark) | E1 plus E2 |

Preprints can generate exploratory hypotheses and extended fixtures. They do not
inherit peer-reviewed status because code runs locally. Frontier 2026 results remain
provisional until independently reproduced.

## Preregistration record

The preregistration is immutable and content-addressed before opening locked data.
It contains every field below or an explicit not-applicable reason.

### Identity and governance

- study_id and protocol_version;
- owners for scientific, statistical, security, data, and independent review;
- creation date and evidence cutoff;
- decision or claim the study may inform;
- non-goals and prohibited uses;
- data classification and approved execution environment;
- conflict-of-interest and external-provider disclosures;
- evidence classes expected and minimum reproduction grade;
- preregistration document hash and signatures or approvals.

### Research design

- research questions and directional or equivalence hypotheses;
- population and inference target;
- independent experimental unit and clustering unit;
- factorial design and required strata;
- assignment, randomization, pairing, and blocking;
- inclusion, exclusion, abstention, and failure rules;
- train, development, calibration, and test split algorithms;
- near-duplicate and contamination checks;
- fixed sample sizes with derivation;
- any sequential boundaries and maximum sample size.

### Systems

- scheme, embedder, detector, attack, and evaluator identifiers;
- source repositories, immutable commits, packages, images, weights, and digests;
- model, tokenizer, normalizer, Unicode, runtime, hardware, and operating system;
- exact configurations, patches, environment locks, and deterministic settings;
- key-generation method, key count, reuse, storage, and public commitments;
- payload family and error-correcting code if applicable;
- network allowlist and process trust boundaries.

### Outcomes and statistics

- primary and secondary estimands;
- alpha_target, beta_target, sigma_target, delta_quality, epsilon_fidelity, and
  delta_drift by stratum;
- score, threshold, window, span, key, and payload procedures;
- L_min and frozen length and entropy bins;
- analytic-null assumptions and proof checks;
- empirical calibration construction;
- interval estimators and confidence levels;
- multiplicity family and adjustment;
- macro, micro, paired, and clustered analyses;
- missingness, failure, abstention, and outlier handling;
- sensitivity and exploratory analyses, labeled as such.

### Attacks and quality

- threat model, oracle level, query and compute budget;
- exact attack code, parameters, models, prompts, seeds, and order;
- edit intensity and eligibility denominator;
- deterministic fidelity assertions;
- semantic evaluator configuration and its role;
- human-review sampling, qualification, blinding, rubric, and adjudication;
- code parsers, compilers, tests, and task checks;
- joint attack-success definition.

### Stops, deviations, and reporting

- immediate safety, privacy, key-leakage, and oracle-leakage stops;
- data contamination and implementation mismatch stops;
- statistical futility or invalidity rules;
- claim-release criteria;
- deviation classification and approval;
- required negative-result and limitation sections;
- artifact retention, access, publication, withdrawal, and deletion rules.

Any field changed after the locked test is opened is a deviation. The original
value, reason, authorizing review, time, and affected claims remain visible. A
change to a primary hypothesis, threshold, sample size, split, exclusion, attack,
or analysis converts the affected result to exploratory unless a valid
preregistered correction procedure applies.

## Reproducibility bundle

Every run produces:

- preregistration and content hash;
- source and data manifests with provenance, rights, versions, and file digests;
- split manifest and near-duplicate audit;
- environment lock, container or virtual-machine recipe, and software bill of
  materials;
- immutable model, tokenizer, detector, and attack identifiers;
- all configuration files and recorded patches;
- random seeds and randomization log;
- public key commitments and synthetic-key access record, excluding secret keys;
- canonical canary inputs and expected outputs;
- generation, edit, detector, and quality-gate event logs;
- raw per-document outcomes and scores in access-controlled storage;
- aggregate tables, confidence calculations, and plot source data;
- machine-readable claim ledger;
- failures, abstentions, exclusions, deviations, and negative-result register;
- exact commands or workflow needed for a clean reproduction;
- reproduction report with environment, operator, hashes, and mismatches.

Artifact manifests use relative paths or content-addressed identifiers, never a
developer's private machine path. A clean reproduction begins with an empty
environment and disabled undeclared network access. A result reaching R3 must be
reconstructed from the bundle, not from an already prepared worktree.

### Negative-result register

Each attempted hypothesis gets a record even when it fails:

| Field | Content |
| --- | --- |
| result_id | Stable identifier |
| hypothesis_id | Preregistered hypothesis |
| status | supported, failed criterion, inconclusive, invalid, or not run |
| estimate | Point estimate and interval |
| sample | Assigned, eligible, determinate, and abstained counts |
| reason | Statistical result or protocol failure |
| quality | Fidelity and task-quality outcome |
| deviations | Linked deviation identifiers |
| artifacts | Immutable result-bundle references |
| implication | Narrow conclusion and claims withdrawn |
| follow_up | New hypothesis, if any, clearly exploratory |

"No significant difference" is not converted into equivalence. An inconclusive
study remains inconclusive when its interval includes both acceptable and
unacceptable performance.

## Report schema

The human-readable report and machine-readable record contain the same core fields:

- schema_version;
- study_id, protocol_hash, run_id, and parent_run_id;
- cutoff_date, started_at, completed_at, and status;
- evidence_class and reproduction_grade per claim;
- system identities and all immutable digests;
- data provenance, licenses, consent class, split hashes, and strata;
- hypothesis, estimand, target, decision rule, and multiplicity family;
- counts for assigned, generated, eligible, determinate, abstained, excluded, and
  failed cases;
- false-positive, power, spoofing, payload, localization, quality, and fidelity
  estimates with interval method and bounds;
- length and entropy definitions plus observed distributions;
- attack threat model, oracle, budget, queries, and valid-success definition;
- drift checks and canary outcomes;
- aggregate and per-stratum results;
- negative, contradictory, invalid, and unknown results;
- deviations, incidents, reviewer decisions, and limitations;
- artifact index and independent reproduction outcome;
- exact supported claim text and prohibited extrapolations;
- revalidation triggers.

Raw scores use stable document pseudonyms and stay access controlled. Public reports
prefer aggregates with disclosure review. Payloads, keys, private prompts, personal
text, and operational attack traces are excluded or redacted without making the
aggregate irreproducible for authorized reviewers.

## Data governance

### Allowed data

Use synthetic data, public benchmark data with compatible terms, locally generated
model output, and owner-provided text with explicit research consent. Record the
right to generate derivatives, run automated attacks, retain outputs, and publish
aggregates. Ordinary product use does not imply watermark-research consent.

No private user draft enters a study by default. If owner-provided text is essential:

- obtain specific, revocable consent;
- minimize the copied fields and remove unrelated personal information and secrets;
- isolate the study copy from product state;
- prohibit external provider calls unless separately disclosed and approved;
- define retention, withdrawal, deletion, and aggregate-publication rules;
- preserve a tombstone rather than the withdrawn content when audit integrity
  requires it.

NIST's Generative AI Profile identifies provenance, privacy, and measurement as
connected risk-management concerns. It supports disciplined governance but does not
certify this protocol
([NIST AI 600-1](https://nvlpubs.nist.gov/nistpubs/ai/NIST.AI.600-1.pdf)).

### Secrets and identifiers

- Generate only synthetic study keys.
- Store secrets outside version control with least-privilege access.
- Commit a verification commitment, not the key.
- Separate key custodianship from attack execution where feasible.
- Never encode a real person's identity, account, or customer identifier as a
  multi-bit payload.
- Use fictitious payloads and organizations in spoofing studies.
- Rotate or destroy a key after unintended disclosure and invalidate affected
  security claims.

### Network and provider boundary

Local-first and offline-after-setup remain product constraints. Research requiring a
model download acquires and verifies artifacts during setup, then runs offline when
the fixture permits. Proprietary provider detectors are excluded unless the
provider offers an authorized research interface and data rights permit every
input. Even then, they are a separate provider study and receive no private user
text or Retonr live-loop access.

## Stop and claim-release criteria

### Immediate stop conditions

Stop a run and preserve its state when:

- locked data or outcomes leak into development;
- a prompt, source, author, repository, or near duplicate crosses protected splits;
- a key, exact detector score, threshold, or unplanned oracle leaks;
- a production provider or real organization is queried or imitated without
  explicit authorization;
- a private or unconsented document is discovered;
- artifact digests, licenses, model terms, or implementation identity do not match
  the preregistration;
- undeclared networking or a product-to-research dependency is observed;
- a canary decision mismatches;
- nondeterminism exceeds the declared tolerance;
- the attack or evaluation corrupts facts, protected spans, task correctness, or
  safety beyond its gate;
- the independent unit, null assumptions, or multiplicity family cannot be
  reconstructed;
- a preregistered statistical stop boundary is crossed.

The incident and partial result remain in the negative-result register. Restarting
requires a new run identifier and, when locked information was exposed, a new
preregistration, split, and key set.

### Criteria to release a research claim

A confirmatory claim can be released only when:

- the preregistration predates locked-test access;
- the complete detector procedure and all artifacts match their digests;
- calibration covers the entire selection procedure;
- every required stratum meets its false-positive upper bound;
- every claimed positive or attack stratum meets its power or spoofing bound;
- abstention and missingness satisfy their declared ceilings;
- quality and fidelity gates pass with their required uncertainty;
- multiplicity is controlled;
- all deviations are reviewed and do not invalidate the claim;
- negative, contradictory, and unknown results are disclosed;
- the result bundle reaches at least R2;
- an independent statistical and security review finds the claim wording supported;
- revalidation triggers and prohibited extrapolations are present.

Pooled success cannot override a required stratum failure. A failed criterion can
support a negative claim if the design remains valid and uncertainty is reported.

### What "ship" means here

Shipping under this protocol means publishing or internally accepting a bounded
research conclusion and its reproducibility bundle. It does not ship:

- a detector in Retonr;
- a watermark-removal or spoofing feature;
- score-guided rewriting;
- a human or AI authorship classifier;
- a legal or compliance determination.

Any such product proposal requires a separate decision record, threat model,
privacy review, security review, user-benefit analysis, and architecture review.
This protocol supplies evidence but grants no product authority.

## Limitations and unknowns

- A finite fixture matrix cannot represent every watermark, model, key, tokenizer,
  language, genre, codebase, or deployment.
- Empirical calibration supports sampled strata and their declared population. It
  cannot prove a distribution-free false-positive rate.
- Analytic p-values depend on assumptions that natural text, repeated contexts,
  window selection, and implementation details can violate.
- Very low claimed false-positive rates require very large independent negative
  samples for informative upper bounds.
- Human review is costly, variable, and culturally dependent. Learned semantic
  evaluators can share model biases and miss factual changes.
- High fidelity under a rubric does not establish identical meaning in all future
  uses.
- Reference implementations may differ from provider deployments. Provider
  statements may omit keys, filters, calibration, model coverage, and runtime
  changes.
- Secret-key results depend on key management. A leaked or reused key changes the
  threat model.
- Public verification can widen access for audit while also widening the spoofing
  surface. Attribution remains a separate claim.
- Adaptive robustness is budget-relative. A passed budget does not establish
  robustness to an unbounded attacker.
- Translation and code transformations can alter semantics in ways that automated
  gates miss.
- Multi-bit capacity, quality, and robustness trade off with text length, entropy,
  payload, and scheme settings. One operating point cannot establish a capacity
  curve.
- New generators can learn, imitate, weaken, or collide with prior marks. Results
  drift with model and data ecosystems.
- Strong detection under a known generator and key does not prove that a document
  originated from that generator. Spoofing, copying, mixed authorship, quoting, and
  key compromise remain alternative explanations.
- General reliable detection after unrestricted meaning-preserving transformation
  faces fundamental limits under broad threat models
  ([detector reliability analysis](https://arxiv.org/abs/2303.11156);
  [Watermarks in the Sand](https://proceedings.mlr.press/v235/zhang24o.html)).
- The state of 2026 preprints, code, and provider deployments is provisional at the
  cutoff. Unreproduced claims remain E3, E4, or U.

## Revalidation triggers and open questions

Reopen the evidence review when:

- a fixture, paper, provider policy, official implementation, or stable dependency
  changes;
- a scheme gains or loses peer-reviewed support;
- a new attack changes the relevant access or budget frontier;
- a supported local model or tokenizer changes;
- Retonr expands languages, code languages, document structures, or runtime
  platforms;
- a calibration or fidelity incident occurs;
- regulation or a technical standard defines a new provenance requirement;
- independent reproduction conflicts with the claim ledger.

Open questions remain:

- Can a common complete-procedure calibration API cover sampling, semantic,
  signature, and multi-bit detectors without hiding scheme-specific assumptions?
- Which entropy quantity best predicts eligible evidence across tokenizers, and how
  can it be estimated when logits are unavailable?
- How should simultaneous calibration scale to many providers, keys, windows,
  payloads, and localized spans without making power unusable?
- Which multilingual fidelity protocol is reliable enough for asymmetric
  translation attacks?
- How should sequential and colliding marks represent attribution when several
  legitimate generators edit the same document?
- What public evidence is sufficient to distinguish a provider deployment from its
  reference implementation?
- Can reproducible negative corpora cover rare structured human text without
  collecting sensitive user writing?
- What governance permits useful external replication without releasing keys or
  operational spoofing traces?

## Primary-source index

### Statistical foundations and detector behavior

- [A Watermark for Large Language Models](https://proceedings.mlr.press/v202/kirchenbauer23a.html),
  ICML 2023, E1.
- [On the Reliability of Watermarks for Large Language
  Models](https://proceedings.iclr.cc/paper_files/paper/2024/hash/d78e9e4316e1714fbb0f20be66f8044c-Abstract-Conference.html),
  ICLR 2024, E1.
- [Three Bricks to Consolidate Watermarks for Large Language
  Models](https://doi.org/10.1109/WIFS58808.2023.10374576), WIFS 2023, E1.
- [Robust Distortion-free Watermarks for Language
  Models](https://openreview.net/forum?id=FpaCL1MO2C), TMLR 2024, E1.
- [Statistical Pivots for Large Language Model
  Watermarking](https://arxiv.org/abs/2404.01245), preprint at the cited record, E3.
- [A Likelihood Based Approach to Distribution-Preserving Watermark
  Detection](https://proceedings.mlr.press/v258/li25d.html), AISTATS 2025, E1.
- [Scalable Watermarking for Identifying Large Language Model
  Outputs](https://www.nature.com/articles/s41586-024-08025-4), Nature 2024, E1.

### Semantic, code, multi-bit, and public verification

- [SemStamp](https://aclanthology.org/2024.naacl-long.226/), NAACL 2024, E1.
- [SWEET](https://aclanthology.org/2024.acl-long.268/), ACL 2024, E1.
- [MPAC](https://aclanthology.org/2024.naacl-long.224/), NAACL 2024, E1.
- [Codable Watermarking](https://proceedings.iclr.cc/paper_files/paper/2024/hash/abdc8c031aa6c6917c3b593166e5e340-Abstract-Conference.html),
  ICLR 2024, E1.
- [Unforgeable Publicly Verifiable
  Watermarking](https://proceedings.iclr.cc/paper_files/paper/2024/hash/214d2cffc381938be6f7254d5382904f-Abstract-Conference.html),
  ICLR 2024, E1.
- [Publicly-Detectable Watermarking for Language
  Models](https://eprint.iacr.org/2023/1661), cryptology preprint at the cutoff,
  E3.
- [XMark](https://aclanthology.org/2026.acl-long.672/), ACL 2026, E1.
- [STELA](https://aclanthology.org/2026.acl-long.2115/), ACL 2026, E1.

### Attacks and stress tests

- [Watermark Stealing in Large Language
  Models](https://proceedings.mlr.press/v235/jovanovic24a.html), ICML 2024, E1.
- [Watermarks in the Sand](https://proceedings.mlr.press/v235/zhang24o.html),
  ICML 2024, E1.
- [Revisiting Watermark Robustness to
  Paraphrasing](https://aclanthology.org/2024.emnlp-main.1005/), EMNLP 2024, E1.
- [Crosslingual Watermark
  Evaluation](https://aclanthology.org/2025.findings-emnlp.390/), Findings of
  EMNLP 2025, E1.
- [Watermark under Fire](https://aclanthology.org/2025.findings-emnlp.1148/),
  Findings of EMNLP 2025, E1.
- [Watermark Collision](https://aclanthology.org/2025.findings-naacl.37/),
  Findings of NAACL 2025, E1.
- [Smoothing Attacks](https://aclanthology.org/2025.findings-emnlp.264/),
  Findings of EMNLP 2025, E1.
- [DITTO](https://aclanthology.org/2026.eacl-long.229/), EACL 2026, E1.

### Official implementations and guidance

- [KGW implementation](https://github.com/jwkirchenbauer/lm-watermarking), E2.
- [SynthID-Text reference](https://github.com/google-deepmind/synthid-text), E2.
- [Transformers watermark processors and
  detectors](https://huggingface.co/docs/transformers/internal/generation_utils),
  E2.
- [MarkLLM](https://github.com/THU-BPM/MarkLLM), E2.
- [WaterPark](https://github.com/JACKPURCELL/WaterPark), E2.
- [NIST AI Test, Evaluation, Validation, and
  Verification](https://www.nist.gov/ai-test-evaluation-validation-and-verification-tevv),
  official guidance.
- [NIST Adversarial Machine Learning taxonomy](https://www.nist.gov/publications/adversarial-machine-learning-taxonomy-and-terminology-attacks-and-mitigations-0),
  NIST AI 100-2e2025.
