# Editorial pattern evaluation preregistration

## Status

- Review date: 2026-08-13
- Evidence cutoff: 2026-08-13
- Decision status: proposed evaluation protocol
- Implementation status: research design only, not release policy or product behavior
- Scope: qualification of editorial matchers, graph relationships, and
  profile-relative ranking

This note operationalizes the evaluation boundary in the
[editorial pattern graph decision](2026-08-13-editorial-pattern-graph.md) and the
[supporting mathematics](2026-08-13-editorial-pattern-mathematics.md). It does not
qualify any rule, graph component, threshold, corpus, or scorer. Numeric values are
proposed preregistration candidates. They may be changed using development and
calibration evidence before a locked evaluation. After locking, a changed value
requires a new protocol identity and a new locked set.

## Decision

Retonr should evaluate four different claims separately:

1. A matcher found the bounded textual or structural feature it declares.
2. The resulting editorial finding is useful and actionable in its stated context.
3. A graph component improves a named editorial outcome over the strongest simpler
   baseline.
4. An authorized owner prefers one eligible revision in the declared context.

None of these claims identifies human or model authorship. None establishes that a
text contains or lacks a watermark. A model-source label, detector score, provider
identity, or watermark result is inadmissible as a product-graph target, feature,
weight, calibration label, or promotion outcome.

## Preregistered claim

The confirmatory claim for one candidate graph component is:

> On a manifest-bound locked set, the candidate improves its named primary
> editorial outcome over the strongest already qualified simpler baseline, while
> remaining within separately declared clean-control, introduced-finding, and
> fidelity noninferiority limits.

The component does not qualify if only an aggregate score improves. It must also
pass every hard invariant, required stratum, uncertainty bound, and evidence-rights
gate. A negative or inconclusive result retains the simpler baseline.

## Evaluation unit and estimands

The independent sampling unit is normally a document. If several documents come
from one authorized profile, author, session, source package, or derived variant
family, that higher-level group is the resampling unit. Sentences, paragraphs,
matches, and graph edges within a document are correlated observations, not
independent samples.

For rule r, define:

    TP_r = emitted findings adjudicated correct and in scope
    FP_r = emitted findings adjudicated incorrect or out of scope
    FN_r = adjudicated in-scope findings not emitted

Then:

    matcher_precision_r = TP_r / (TP_r + FP_r)
    matcher_recall_r = TP_r / (TP_r + FN_r)

Recall is reported only for a set that received exhaustive labels for that rule.
Positive-enriched fixtures cannot estimate deployment prevalence without declared
sampling weights.

A correct match need not be actionable. Let A_r be emitted findings for which an
adjudicator would recommend a bounded change under the brief, and E_r be all
emitted findings:

    actionability_precision_r = A_r / E_r

The report keeps matcher precision, matcher recall, and actionability precision in
separate fields. It must not relabel any of them as an AI probability or a universal
quality score.

### Cluster outcomes

A graph cluster is evaluated at the document-cluster level after deterministic
de-duplication. Repeated nodes that explain the same editorial issue count as one
cluster finding. The gold record states which spans and nodes support that finding.

The cluster comparison reports:

- Cluster actionability precision
- Cluster recall on exhaustively labeled cases
- Duplicate suggestion rate
- Severe issue recall
- Findings emitted per eligible thousand words or structural units
- The incremental result after removing each edge family

Raw node count is not a success outcome. A graph that increases counts by naming the
same problem several times has regressed.

### Paired source-output outcomes

Every rewrite comparison uses the same source, brief, protected literals, eligible
candidate pool, model artifact, runtime build, prompt, sampling controls, and
resource ceilings. When the graph only ranks candidates, candidate generation is
performed once and replayed across ranking variants. This isolates ranking from
generation variance.

For document d and rule r:

    resolved[d, r] = 1 if present in source and absent from output
    introduced[d, r] = 1 if absent from source and present in output

Report resolved and introduced findings separately. Do not use a net subtraction
that lets many new weak problems offset one severe fidelity failure.

Primary paired editorial outcomes may include:

- Actionable findings resolved per document
- Documents improved without an introduced finding
- Document-level repetition or redundancy reduction
- Owner preference among candidates that passed every hard gate

The exact primary outcome, direction, unit, and minimum effect are fixed before the
locked evaluation.

## Evidence admission

### Admissible evidence classes

| Class | Permitted use | Required record |
| --- | --- | --- |
| Synthetic positive fixture | Matcher development and regression | Generator or hand-authored fixture identity, expected range, rule version |
| Synthetic clean control | False-positive development and regression | Context and exclusion rationale |
| Authorized user material | Context-bound preference and clean-control evaluation | Consent scope, retention, revocation, deletion, and profile isolation |
| Licensed public material | Independent evaluation when rights and sampling are adequate | Source, license, retrieval date, digest, sampling frame, and redistribution decision |
| Commissioned adjudication set | Locked editorial evaluation | Protocol, annotator qualifications, agreement, adjudication, and access policy |

An authorized user corpus is not a human-authorship corpus. Authorization answers
whether Retonr may use the material for a declared purpose. It does not label who or
what drafted each sentence. A clean control means that independent editorial review
did not find the targeted defect in the declared context.

Personal material must remain local or enter a separately approved study. It must
not be pooled into a population baseline by default. Revocation removes future
profile authority and follows the declared deletion policy.

### Discovery-only evidence

Wikipedia's signs guide and Bloomberry's Sentence DNA catalog may propose hypotheses
for independently worded editorial rules. They do not qualify a matcher, graph
edge, replacement, score, or release threshold.

The Wikipedia page is a community-maintained guide with changing content and a
ShareAlike license. Its examples are not a controlled sample and do not provide
population false-positive estimates. Retonr should cite it for discovery. Copying
its catalog requires a specific attribution and license-compatibility decision;
the repository license cannot be assumed to cover that use.

Bloomberry reports 7,622 entries under CC BY 4.0 but publicly exposes a 141-entry
machine-readable sample. The full enforcement corpus, sampling frame, production
scoring method, and independent validation data are unavailable. Its status and
false-positive fields are vendor assertions. The sample may be studied only after
the exact artifact, page-level license statement, site terms, retrieval date, and
rights decision are recorded. The public sample cannot support a claim about the
unreleased catalog.

For either source, a proposed rule enters product research only after:

1. It is restated as a provider-neutral editorial property.
2. A concrete harm or owner preference explains why a change may help.
3. Literal, quoted, cited, technical, accessibility, and protected uses become
   explicit exclusions.
4. Retonr supplies independent positive fixtures and neighboring clean controls.
5. A rights review permits the intended storage, transformation, and distribution.
6. The rule passes the same locked qualification as every other product rule.

## Corpus construction

### Partitioning

Create immutable development, calibration, and locked-test manifests. Group before
splitting by every known leakage relation:

- Exact and near duplicate
- Source document and every transformed variant
- Authorized profile or author where known
- Editing session or collection batch
- Prompt and template family for synthetic material
- Upstream dataset or publication package

The candidate split is 50 percent development, 25 percent calibration, and 25
percent locked test by groups, adjusted only before manifest freeze to meet stratum
and power requirements. The proportions are a planning default, not a scientific
constant. Content-addressed manifests record group assignment and prove that a
derived variant did not cross partitions.

Development data may shape matchers and exclusions. Calibration data may select
thresholds, support minima, family budgets, and the final sample size. The locked
set is opened once for one protocol and release candidate. Failure produces a new
version and a new locked set. It does not authorize tuning on the failed test.

### Required strata

Each release declares its supported cells across:

- Language and locale
- Domain and topic
- Channel and document kind
- Document length band
- Plain text and supported structured format
- Quoted, cited, technical, literal, and accessibility-sensitive context
- Authorized profile coverage band
- Source age or collection window

Overall success cannot hide a failed supported stratum. A cell without sufficient
evidence is unqualified or explicitly unsupported. Exploratory partial pooling may
estimate trends, but it cannot replace the observed bound for a required product
cell.

### Sample size

Sample size is selected from calibration variance before lock, not from a universal
document count. The manifest records the desired confidence width, noninferiority
margin, expected event rate, clustering assumption, attrition allowance, and power
calculation.

Proposed evidence floors are:

- At least 100 independent document groups in each required locked stratum
- At least 100 exhaustively labeled positive opportunities for each promoted rule
- At least 300 independent clean-control document groups for a rule claiming an
  observed zero false-positive count
- At least 100 authorized participants for a population owner-preference claim

These floors do not override the power calculation. For example, zero events in
300 independent observations has an approximate 95 percent upper bound near 1
percent, but dependence reduces the effective sample size. When documents are
grouped, collapse the outcome to whether each independent group contains any event
and calculate the declared binomial bound at that level. A zero-event bootstrap
cannot establish a useful upper bound.

## Annotation protocol

The label schema is fixed before annotation:

- `present_actionable`: the named issue is present and a bounded change is useful
- `present_not_actionable`: the matcher is correct but context does not justify a
  suggestion
- `absent`: the named issue is not present
- `excluded_context`: a declared exclusion applies
- `ambiguous`: evidence is insufficient for a stable judgment

At least two independent qualified reviewers label each confirmatory case. They do
not see the system variant, source label, provider, detector result, or other
reviewer's decision. A third reviewer adjudicates disagreements from the written
rubric. The unadjudicated labels and rationale remain in the evidence record.

Before production annotation, reviewers complete an anchor set. The proposed gate
is a lower 95 percent confidence bound of at least 0.60 for Cohen's kappa between
the first two reviewers on in-scope versus not-in-scope decisions, plus at least
0.80 raw agreement on severity. These are calibration candidates, not universal
meanings of acceptable agreement. A low base rate can make one agreement statistic
misleading, so the report includes the confusion table, raw agreement, prevalence,
and disagreement reasons.

Ambiguous cases are not silently converted to negatives. Report them separately.
The promotion rule predeclares whether they count as abstentions or conservative
errors for each estimand.

## Baselines and ablations

The strongest qualified simpler variant is the control. Evaluate the following
dependency order on the same paired material:

1. Flat deterministic rule catalog
2. Flat catalog plus direct authorized-profile distance
3. Redundancy-cluster aggregation
4. Qualified pair interactions
5. Sequence edges
6. Dependency edges
7. One fixed propagation step
8. Higher-order challenger, if still justified

For each component C, compare the full candidate with an otherwise identical
variant in which C is removed. Also compare it with the strongest earlier variant.
Report both contrasts. A component does not receive credit for an improvement that
comes from a different prompt, candidate pool, runtime, or threshold.

Required ablations include:

- Flat rules versus graph
- Graph with and without cluster de-duplication
- Every edge family removed separately
- Interactions with their budget set to zero
- Direct profile distance versus graph-mediated profile evidence
- No propagation versus each proposed fixed step count
- Complete supported strata versus each domain, language, and channel held out

A graph-wide positive result cannot qualify an edge family whose own ablation is
negative or inconclusive.

## Statistical analysis

### Paired estimates and intervals

Compute document-paired differences whenever the same source is evaluated under
both variants. For owner studies, resample participants and retain all comparisons
from a selected participant. For multi-document source packages, resample the
highest declared dependency group.

The proposed primary interval for a non-sparse paired count, rate, or preference
effect is a two-sided 95 percent percentile cluster bootstrap with 10,000
deterministic resamples. The implementation records the algorithm, seed derivation,
resampling unit, and tie handling. Report a Wilson interval for an ordinary
standalone binary proportion at the independent-group level, but use the exact
one-sided Clopper-Pearson bound for a zero-event promotion claim. McNemar's exact
paired test may be a secondary diagnostic for paired binary outcomes. Sparse paired
binary noninferiority uses the separate conservative contract below.

Intervals describe sampling uncertainty under the declared design. They do not
cover annotation error, corpus bias, rights failures, model drift, or unsupported
deployment populations. Overlapping confidence intervals do not by themselves
define a tie.

### Hierarchical reporting

Report the effect for every required stratum and the macro-average across strata.
Also report the micro-average with its observed mixture, but do not use it as the
sole promotion statistic. The macro-average prevents a large easy domain from
dominating the decision.

An exploratory hierarchical model may estimate partial-pooling effects with random
intercepts for document group and authorized participant. Its prior, likelihood,
diagnostics, and code identity must be declared. The locked promotion gate remains
the predeclared frequentist bound unless a different inferential contract was
frozen before data access.

### Multiple comparisons

Define confirmatory families before opening the locked set:

- Per-rule matcher and actionability claims
- Graph component improvements
- Required-stratum non-regression claims
- Owner-preference claims

Use Holm's sequential procedure at family-wise alpha 0.05 for claims that authorize
a product component. Under unproved dependence, use Benjamini-Yekutieli at false
discovery rate q = 0.05 for labeled discovery analyses. Ordinary
Benjamini-Hochberg is permitted only when its dependence assumptions are
established. A discovery result generates a new hypothesis and evaluation version;
it cannot qualify a live rule on the same data.

Effect sizes and confidence bounds remain mandatory after multiplicity correction.
A corrected p-value alone is not a promotion result.

## Owner-preference study

Owner preference means the preference of a person authorized to direct the
document, not an inference about who wrote the source. The study presents eligible
outputs in randomized, blinded left-right order. The owner may choose left, right,
tie, neither, or abstain and may provide a bounded reason code.

The primary paired preference estimand is:

    delta_preference =
        (graph_wins - baseline_wins) / all_presented_pairs

Ties contribute zero and remain in the denominator. Also report:

    decisive_graph_rate = graph_wins / (graph_wins + baseline_wins)
    tie_rate = ties / all_presented_pairs
    neither_rate = neither / all_presented_pairs
    abstention_rate = abstentions / all_presented_pairs

Do not split ties into half-wins. Use participant-clustered intervals. If an
incomplete comparison design is necessary, a Bradley-Terry model may be a secondary
analysis, with participant effects and connectivity diagnostics. The raw paired
counts remain authoritative.

The proposed population gate is a lower 95 percent bound for delta_preference of at
least 0.05, no material increase in `neither`, and no required subgroup below its
noninferiority margin. A personal profile can instead qualify from that owner's
repeated authorized decisions. It must not be generalized to a population claim.

## Fidelity noninferiority

Fidelity is a separate constraint, not a term in a weighted quality average.
Evaluate exact and adjudicated outcomes independently:

- Protected literal preservation
- Format and structural preservation
- Formula, code, citation, and identifier preservation
- Bounded semantic entailment or contradiction review
- Requested length and change-budget conformance
- New editorial finding rate

For a binary harm outcome, define:

    delta_harm = p_graph_harm - p_baseline_harm

Let M be the predeclared maximum acceptable increase. Noninferiority requires the
one-sided 95 percent upper confidence bound for `delta_harm` to be no greater than
M. This reverses the burden of proof: failure to detect a difference is not enough.

For a paired binary risk difference, use a preregistered matched-risk-difference
score or exact interval with validated coverage. An ordinary percentile bootstrap is
inadmissible when harms or discordant clusters are sparse or zero because it can
degenerate at zero. A conservative fallback is a simultaneous adjusted upper bound
for graph risk minus the corresponding lower bound for baseline risk. If no
predeclared method produces a nondegenerate bound with validated coverage, the gate
is inconclusive and cannot pass.

Proposed gates are:

| Outcome | Candidate gate |
| --- | --- |
| Protected literal, formula, code, identifier, or required structure violation | Zero observed violations; each violation blocks promotion |
| Independently adjudicated material meaning error | Upper bound for graph-minus-baseline increase no greater than 0.005 |
| Introduced actionable finding | Upper bound for graph-minus-baseline increase no greater than 0.005 |
| Unsupported-context activation | Upper bound no greater than the rule's clean-control limit |
| Change-budget overflow or hard resource failure | Zero accepted outputs; failures remain visible |

The 0.005 margins are proposed policy tolerances, not claims that a half-percentage
point is universally harmless. Domain owners must tighten them for high-consequence
material. Zero observed failures is an empirical release gate, not proof that a
failure is impossible.

## Proposed promotion thresholds

This table is a candidate policy for calibration. It is not implemented release
policy.

| Gate | Proposed locked-test requirement |
| --- | --- |
| Matcher precision | Lower 95 percent bound at least 0.98 per promoted rule |
| Matcher recall | Lower 95 percent bound at least 0.90 where labels are exhaustive |
| Actionability precision | Lower 95 percent bound at least 0.90 per promoted rule or cluster |
| Clean-control document finding rate | Upper 95 percent bound at most 0.01 overall and at most 0.02 in each required stratum |
| Graph primary effect | Lower 95 percent bound above zero and, for a bounded proportion, point improvement at least 0.02; other estimands freeze a unit-specific minimum |
| Duplicate suggestion rate | Upper 95 percent bound no worse than the flat baseline |
| Owner preference when style-affecting | Meets the owner-preference gate above |
| Fidelity | Meets every noninferiority and zero-observed-failure gate above |
| Required strata | No unsupported pooling; every required cell passes or remains unqualified |
| Multiplicity | Confirmatory family passes its frozen Holm-adjusted decision |

Thresholds cannot be relaxed after a locked result. If calibration shows that one
is infeasible or meaningless, revise the protocol and explain why before locking.
Passing does not compel release. Failing any hard gate prohibits promotion under
that protocol.

## Calibration

Matcher or graph scores are not probabilities unless a calibration experiment
binds score values to an exact event. If a future component emits a probability of
an actionable finding, fit its calibrator only on the calibration partition and
freeze it before lock.

Report reliability by predeclared bins, the Brier score, bin counts, and the
observed event rate. Empty and sparse bins remain visible. Expected calibration
error alone is insufficient because bin choice can change it. Compare the
uncalibrated and calibrated component and include a constant-prevalence baseline.

Candidate promotion thresholds for a displayed probability are positive Brier skill
against both the constant-prevalence and locked flat baselines, an overall absolute
calibration-gap upper bound at most 0.02, and a simultaneous worst-supported-bin gap
upper bound at most 0.05. These are proposed calibration values, not implemented
policy. If they do not pass, expose named evidence and severity rather than a
probability.

Calibration is specific to rule version, corpus, language, channel, graph identity,
and observation window. A rank or fixed-point activation that lacks this evidence
must be labeled a score, not confidence.

## Temporal drift and revalidation

Every qualified graph release binds its evidence window. Re-run the locked-shaped
evaluation when any bound matcher, graph, parser, tokenizer, profile policy, corpus
mixture, or supported runtime contract changes. Also evaluate a new time-window
slice before carrying a qualification into a materially later evidence release.

Monitor by required stratum:

- Pattern and exclusion prevalence
- Actionability precision and clean-control rate
- Edge support and direction
- Calibration error when probabilities exist
- Owner preference, tie, neither, and abstention rates
- Fidelity and introduced-finding rates
- Unknown, unsupported, and cap-failure rates

Proposed drift triggers are an absolute clean-control increase above 0.005, an
actionability lower bound below 0.90, reversal of a qualified edge direction, or a
required-stratum fidelity bound crossing its margin. A trigger suspends the affected
component or retains the qualified older baseline until a new version passes. It
does not silently mutate a weight under an existing identity.

Distribution-distance statistics may help diagnose drift, but no universal
distance threshold is presumed. A changed topic mixture is not proof that a rule
changed, and a stable aggregate can hide a failed subgroup.

## Qualification record

The immutable result binds at least:

- Protocol, implementation, graph, rule, matcher, and policy identities
- Corpus, rights, split, adjudication, and exclusion manifests
- Model artifact, runtime build, prompt, sampling, and candidate-pool identities
  where generation is involved
- Primary and secondary estimands, thresholds, margins, test families, and seeds
- Counts, exclusions, missingness, abstentions, effect sizes, intervals, and
  adjusted decisions
- Per-stratum results and every ablation
- Runtime, peak memory, cap failures, and unsupported cases
- Negative, inconclusive, and failed-gate results

The report includes enough aggregate evidence to reproduce each statistic without
publishing personal prose. Raw authorized content remains subject to its consent,
retention, and deletion contract.

## Ordered execution

1. Freeze rule semantics, exclusions, match ranges, and annotation rubric.
2. Admit evidence only after rights and privacy review.
3. Create leakage groups, strata, and content-addressed split manifests.
4. Establish matcher correctness and the flat deterministic baseline.
5. Complete development and calibration; select thresholds and sample size.
6. Freeze the protocol, implementation, graph, model, runtime, prompt, and policy
   identities.
7. Run flat-versus-graph pairs and component ablations on the locked set.
8. Compute clustered intervals, multiplicity decisions, noninferiority gates, and
   required-stratum results without tuning.
9. Publish the complete qualification record, including failures.
10. Promote only the exact component identities that passed every required gate.

## Limitations

- Editorial labels contain judgment even with a written rubric and adjudication.
- Authorized owner preference can be sparse, situational, and internally
  inconsistent.
- A locked corpus represents only its declared sampling frame and time window.
- Bootstrap intervals do not repair a biased corpus or an invalid independence
  unit.
- A high matcher precision does not establish actionability.
- A preference win does not establish fidelity, authorship, authenticity, or legal
  compliance.
- A graph can pass one domain and fail another.
- No evaluation here proves that text is human-written, AI-written, authentic,
  owner-authored, watermark-free, or undetectable.

## Primary sources

- [Efron, Bootstrap Methods: Another Look at the Jackknife, 1979](https://doi.org/10.1214/aos/1176344552)
- [Wilson, Probable Inference, the Law of Succession, and Statistical Inference, 1927](https://doi.org/10.1080/01621459.1927.10502953)
- [McNemar, correlated proportions, 1947](https://doi.org/10.1007/BF02295996)
- [Cohen, A Coefficient of Agreement for Nominal Scales, 1960](https://doi.org/10.1177/001316446002000104)
- [Holm, A Simple Sequentially Rejective Multiple Test Procedure, 1979](https://www.jstor.org/stable/4615733)
- [Benjamini and Yekutieli, False discovery rate under dependency, 2001](https://doi.org/10.1214/aos/1013699998)
- [Clopper and Pearson, Exact confidence limits, 1934](https://doi.org/10.1093/biomet/26.4.404)
- [Schuirmann, Two One-Sided Tests for Equivalence, 1987](https://doi.org/10.1007/BF01068419)
- [Bradley and Terry, The Method of Paired Comparisons, 1952](https://doi.org/10.1093/biomet/39.3-4.324)
- [Brier, Verification of Forecasts Expressed in Terms of Probability, 1950](https://journals.ametsoc.org/view/journals/mwre/78/1/1520-0493_1950_078_0001_vofeit_2_0_co_2.xml)
- [Guo et al., On Calibration of Modern Neural Networks, 2017](https://proceedings.mlr.press/v70/guo17a.html)
- [NIST AI Risk Management Framework](https://airc.nist.gov/airmf-resources/airmf/5-sec-core/)
- [NIST Generative AI Profile, NIST AI 600-1](https://nvlpubs.nist.gov/nistpubs/ai/NIST.AI.600-1.pdf)
- [Wikipedia signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing)
- [Bloomberry catalog description](https://www.bloomberry.ai/research/ai-writing-patterns)
- [Bloomberry machine-readable sample](https://www.bloomberry.ai/research/ai-sentence-dna-corpus-sample.json)
- [Bloomberry terms](https://www.bloomberry.ai/terms)
