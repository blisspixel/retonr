# Editorial pattern graph mathematics

## Status

- Review date: 2026-08-13
- Evidence cutoff: 2026-08-13
- Evidence classes: peer-reviewed primary research and inference
- Implementation status: proposed supporting research, not product behavior
- Scope: editorial-pattern evidence, derived relationships, bounded scoring, and
  evaluation

This note refines the
[editorial pattern graph research decision](2026-08-13-editorial-pattern-graph.md).
It specifies candidate mathematics and implementation constraints. It does not
qualify a graph, threshold, scorer, or learned relationship for a Retonr release.
Every numeric threshold below is a preregistration candidate. Calibration may
reject or replace it before a locked evaluation, but a locked-test result must
never be used to tune it.

## Abstract

Retonr should treat an editorial pattern graph as a deterministic projection of an
immutable, typed occurrence ledger. The ledger, not the graph, is the evidence
authority. A direct fixed-point feature baseline should ship before propagation,
higher-order clusters, or covariance-aware distance. Graph features earn live use
only through a predeclared comparison against that baseline and only after hard
fidelity gates pass.

The recommended first graph is a bounded bipartite incidence relation between
eligible document units and independently qualified editorial patterns. Pair,
sequence, dependency, cluster, and hyperedge views are derived from that relation.
The design preserves denominators, opportunity counts, uncertainty, and
provenance. It also prevents correlated features from being counted repeatedly.

Population source signals require a different schema, data set, report, and
application boundary. Detector scores, statistical watermark outcomes,
model-family likelihoods, provider labels, and human-versus-AI classifiers cannot
be nodes, edges, seeds, weights, targets, profile evidence, or training labels in
the live editorial graph. A phrase discovered during source-signal research may
enter the editorial catalog only through an independent, provider-neutral
qualification that shows a concrete editorial defect.

## Decision

The default implementation path is:

1. Build a canonical event ledger and a transparent flat-rule scorer.
2. Derive bounded graph statistics for inspection and evaluation.
3. Qualify clusters or interactions only when they improve a predeclared editorial
   outcome without a material fidelity or false-positive regression.
4. Keep propagation, hypergraph algorithms, and covariance-aware distances as
   challengers until the simpler representation is demonstrably insufficient.

A graph database, graph neural network, mutable temporal knowledge graph, and
floating-point ranking path are not justified for the first implementation.
Sorted Rust vectors, checked integer arithmetic, and a relational store are enough.

## Research questions

The product-facing investigation asks:

1. Do typed relationships improve editorial precision, owner preference, or useful
   abstention over an independently qualified flat rule catalog?
2. Can the improvement be obtained without worse fidelity, more clean-control
   findings, or opaque ranking?
3. Which relationship families remain useful under topic, channel, language, and
   document-level holdouts?
4. Can every accepted score be reconstructed from bounded canonical evidence?

It does not ask whether a document was written by a person or a model. It does not
ask whether editing reduces a watermark or detector score. Those are isolated
research questions with no live rewrite authority.

## Definitions and notation

Let:

- U be the ordered set of eligible analysis units.
- W be the ordered set of eligible relationship windows.
- J be the ordered set of eligible documents.
- D be the ordered set of highest-level dependence clusters used by the declared
  inference policy, such as author, participant, or source bundle.
- document(w) map each relationship window to exactly one document in J.
- cluster_of_document(j) map each document j to exactly one cluster in D.
- P be the ordered set of qualified editorial pattern IDs.
- N be the size of W for a declared relationship scope.
- x[w, i] be 1 when pattern i is present in window w and 0 otherwise.
- n[i] be the number of eligible windows containing pattern i.
- n[i, j] be the number of eligible windows containing both i and j.
- y[j, i] be 1 when pattern i occurs anywhere in the eligible portion of document j.
- S be the fixed-point scale 1,000,000.
- G be the ordered set of feature families.

Presence is counted at most once per pattern per window. Raw occurrence count and
density remain separate fields. Without that separation, one repetitive paragraph
can distort a relationship that is meant to describe document-level prevalence.

Window-level point estimates describe the declared window population, so a long
document can contribute more windows than a short document. Inferential resampling
uses the highest declared dependence cluster in D, not individual windows. Isolated
population-excess estimates use y[j, i] so repeated uses inside one document do not
pretend to be independent documents. Author- or source-bundle prevalence is a
different estimand and must be named explicitly if used.

An eligible unit is not merely a unit with a match. The ledger must retain the
complete denominator and the rule that made each unit eligible. Quotations, code,
citations, protected literals, unsupported languages, and other excluded contexts
must be represented as exclusions, not silently removed after counting.

## Authoritative evidence ledger

The graph is rebuildable from an append-only release of bounded records:

| Record | Required content |
| --- | --- |
| Evidence snapshot | Corpus and rights identity, partition, language, channel, document kind, collection window |
| Unit observation | Store-scoped opaque document ID, stable unit ID, unit kind, eligibility decision, context ID |
| Pattern occurrence | Pattern ID and version, matcher identity, unit ID, byte or structural range, exclusion state |
| Sequence opportunity | Boundary policy, source unit, target unit, lag, eligible successor set |
| Dependency observation | Parser identity, parser artifact, relation, direction, confidence state |
| Declared relationship | Relationship ID, type, authority, scope, version |

Raw personal prose does not belong in graph identity or graph serialization.
Reports should retain stable IDs, aggregate counts, and authorized ranges only as
long as the transaction contract requires them.

The editorial store and source-signal research store use different domain-separated
identifier types. They do not expose a common document digest, generic evidence ID,
or automatic conversion that could join private product evidence to a source study.

At minimum, every derived release binds:

- Pattern catalog and matcher artifact identities
- Tokenizer and parser identities where used
- Language, channel, document kind, and context taxonomy
- Unit, window, boundary, lag, and opportunity definitions
- Evidence snapshot and partition identities
- Smoothing, support, shrinkage, clustering, and family-budget policies
- Propagation policy, if any
- Canonical schema version and event-ledger digest

Changing any bound item creates a new derived release. Mutable replacement of a
weight under an old identity is invalid.

## Graph model

### Base incidence graph

The authoritative derived structure is a bipartite graph between eligible units
and pattern definitions. A unit connected to k distinct patterns is also the
original k-way hyperedge. Pairwise and higher-order projections must be derived
from this incidence relation, rather than stored as independent evidence.

This design has three advantages:

- It preserves the observation unit and denominator.
- It permits exact recomputation of pair and set statistics.
- It exposes whether a large clique came from one dense unit or many independent
  observations.

### Typed nodes

The initial closed node set should contain:

- PatternDefinition: an observable, qualified editorial pattern
- ContextSlice: a language, channel, document kind, or rhetorical position
- PreferenceRule: an explicit prefer, avoid, require, or forbid statement
- RedundancyCluster: a versioned group of overlapping measurements
- EvidenceRelease: an immutable provenance target for qualified relationships

Evidence units remain ledger records, not durable product-graph nodes. A
PreferenceRule preserves its authority and must not be learned from a population
correlation. Hard require and forbid rules remain outside soft activation.

### Typed edges

The initial closed edge set should contain:

- CoOccurs: symmetric presence within an exact eligible window
- Precedes: directed presence within a declared lag and boundary
- DependsOn: directed parser relation under an exact parser identity
- MemberOf: declared or qualified cluster membership
- ConflictsWith: declared incompatibility
- Requires: declared dependency
- QualifiedBy: provenance to an EditorialEvidenceReleaseId

Correlation does not create ConflictsWith, Requires, or causal meaning. Those
relations require an explicit authority and independent validation.
EditorialEvidenceReleaseId and SourceSignalResearchReleaseId are distinct,
domain-separated newtypes. Neither can deserialize as the other, and neither has a
From conversion into the other.

## Pairwise co-occurrence

For a pair i, j, the sufficient 2 by 2 table is:

    o11 = n[i, j]
    o10 = n[i] - n[i, j]
    o01 = n[j] - n[i, j]
    o00 = N - n[i] - n[j] + n[i, j]

The decoder must enforce:

    N > 0
    0 <= n[i, j] <= min(n[i], n[j])
    n[i] + n[j] - n[i, j] <= N

Association statistics are unavailable when their declared denominator, eligible
margin, expected cell, alpha, or kappa is not strictly positive. The implementation
returns a typed unavailable result rather than NaN, infinity, or a fabricated edge.

Pointwise mutual information is useful for research inspection:

    PMI(i, j) = ln((n[i, j] / N) / ((n[i] / N) * (n[j] / N)))

PMI can assign extreme values to rare pairs. This is a known consequence of using
a ratio of observed joint probability to independent probability, not evidence
that a rare pair is editorially important. Church and Hanks introduced mutual
information for lexical association and also discussed its frequency behavior
([Church and Hanks 1990](https://aclanthology.org/J90-1003/)).

A smoothed four-cell research estimate is:

    D = N + 4 * alpha
    p11 = (n[i, j] + alpha) / D
    p1x = (n[i] + 2 * alpha) / D
    px1 = (n[j] + 2 * alpha) / D
    PMI_alpha = ln(p11 / (p1x * px1))
    NPMI_alpha = PMI_alpha / -ln(p11)

Alpha, minimum joint support, and the treatment of degenerate p11 values must be
predeclared. NPMI remains a research statistic. It is not a durable identity field
unless the exact logarithm implementation and arithmetic contract are frozen.

The likelihood-ratio statistic provides a separate measure of evidence against an
independence model:

    G2 = 2 * sum over cells a,b of O[a,b] * ln(O[a,b] / E[a,b])

Terms with observed count zero contribute zero. G2 is evidence strength, not an
effect size or editorial weight. Dunning showed why likelihood ratios can be more
appropriate than normal approximations for sparse text counts
([Dunning 1993](https://aclanthology.org/J93-1003/)). If many pairs are tested,
the tested family and procedure must be declared before calibration. Use Holm
family-wise error control for a small frozen confirmatory family. For broad
discovery under unproved dependence, use Benjamini-Yekutieli or a document-cluster
resampling procedure whose null calibration is demonstrated. Ordinary
Benjamini-Hochberg is permitted only when its dependence assumptions are
established. G2 p-values come from the declared document-cluster null, not naive
window-level chi-square asymptotics.

For an early bounded positive co-activation edge, shrunken Dice is simpler:

    Dice(i, j) = 2 * n[i, j] / (n[i] + n[j])
    w(i, j) = Dice(i, j) * n[i, j] / (n[i, j] + kappa)

Both terms are in [0, 1], so w is in [0, 1]. This is a measure of overlap, not
residual association and not causality. Kappa and minimum support are
preregistration candidates. The durable implementation should evaluate the same
formula as checked rational arithmetic and round once to an integer weight in
[0, S].

Weighted log odds with an informative Dirichlet prior is appropriate for
discovering features that distinguish a bounded research corpus from a declared
baseline. In product research, its labels must be independently adjudicated
editorial outcomes. Comparisons labeled by provider, model family, watermark, or
source belong only in isolated source-signal research. It must not become an
unbounded candidate score or a source label.
Monroe, Colaresi, and Quinn give the primary method and uncertainty treatment
([Monroe et al. 2008](https://doi.org/10.1093/pan/mpn018)).

Negative association should remain a separate diagnostic. A negative pair must not
be inserted into a nonnegative random-walk matrix, because cancellation can conceal
which named evidence produced an activation.

## Sequential relationships

A sequence edge requires an explicit opportunity denominator. Counting only
observed transitions makes the absence of an eligible successor indistinguishable
from a zero transition.

For row i, let c[i, j] count eligible transitions from i to j, and let q[j] be a
declared background successor distribution over the exact eligible-successor
universe. q must be nonnegative and sum exactly to one under the declared fixed-point
contract. A smoothed row is:

    T[i, j] =
        (c[i, j] + alpha * q[j]) /
        (sum over k of c[i, k] + alpha)

A directional diagnostic is:

    Delta[i, j] = P(j follows i) - P(j)

Delta is bounded by [-1, 1]. The graph must bind exact lag, sentence and paragraph
boundary rules, skipped-unit treatment, and whether multiple target occurrences
within one opportunity collapse to presence.

A row with zero observed eligible transitions is unknown. It must not emit a
prior-only edge even when q and alpha are available. Alpha must be strictly positive.

The initial candidate boundary policy permits no sequence edge to cross a sentence
or paragraph boundary unless a separately named relationship explicitly qualifies
that scope. The initial candidate maximum lag is two eligible units. Both are
preregistration candidates, not implemented behavior.

## Dependency relationships

A dependency observation is keyed by:

    (head_pattern, relation, dependent_pattern, direction,
     parser_build, parser_artifact, language, confidence_policy)

If parsing is unsupported, ambiguous beyond the declared policy, or produced by an
unqualified artifact, the relationship is unknown. It is never guessed from token
distance. Dependency contexts can capture functional similarity that ordinary
linear contexts miss
([Levy and Goldberg 2014](https://aclanthology.org/P14-2050/)), but that result
does not qualify a parser or a dependency edge for Retonr.

## Higher-order evidence

The original pattern set for an eligible unit is a hyperedge. Converting a set of
k patterns to a clique creates k * (k - 1) / 2 pair edges and can make one dense
unit appear to provide many independent observations. The report must preserve
hyperedge support separately from projected pair support.

Hypergraph methods model group relations directly
([Zhou, Huang, and Scholkopf 2006](https://proceedings.neurips.cc/paper_files/paper/2006/hash/dff8e9c2ac33381546d96deea9922999-Abstract.html)).
Motif-based clustering is another principled higher-order view
([Benson, Gleich, and Leskovec 2016](https://doi.org/10.1126/science.aad9029)).
Neither is justified in the shipping path merely because it is mathematically
available.

For research inspection, one explicit co-information convention is:

    I(X; Y; Z) =
        sum over x,y,z of p(x,y,z) *
        ln((p(x,y) * p(x,z) * p(y,z)) /
           (p(x) * p(y) * p(z) * p(x,y,z)))

Under this convention, positive values indicate redundant shared information and
negative values indicate synergy. Every term ranges over the full joint state space;
the all-present cell alone is not three-way interaction information. Co-information
is signed and unstable under sparse support. It must remain a diagnostic until a
locked evaluation establishes an editorial benefit. If frequent itemset mining later
becomes necessary, FP-growth is a suitable reference algorithm
([Han, Pei, and Yin 2000](https://doi.org/10.1145/342009.335372)).

Initial preregistration candidates are:

- At most 32 distinct pattern IDs per eligible unit after deterministic
  de-duplication
- Higher-order enumeration limited to order three
- No automatically mined higher-order set in live ranking
- Excess items reported as a bounded overflow finding rather than silently sampled

These values are computational guardrails proposed for calibration. They are not
implemented product limits.

## Correlation and double-counting

Adding every activated node, pair, cluster, and propagated neighbor would count the
same evidence several times. The first scorer must therefore use these rules:

1. De-duplicate overlapping or nested occurrences according to each rule's
   predeclared span policy.
2. Assign every soft feature to one feature family with a fixed family budget.
3. Place strongly redundant measurements in a versioned RedundancyCluster.
4. Aggregate a redundancy cluster by its maximum weighted deviation or by one
   predeclared representative, not by a sum.
5. Give interaction features a separate capped budget.
6. Never add both an unadjusted joint feature and all of its marginal evidence.

For cluster C:

    D[C] = max over i in C of c[i] * d[i]

For marginal and interaction components:

    D_combined =
        (1 - beta) * D_marginal + beta * D_interaction

The candidate interaction budget is beta <= 0.20. This is a preregistration
candidate. A calibrated value can advance only before the locked test and only if
the interaction ablation supports it.

Mahalanobis distance can account for covariance:

    D_M(x) = sqrt((x - mu)^T * Sigma_inverse * (x - mu))

In a small-sample, high-dimensional profile, the empirical covariance matrix is
unstable or singular. Linear shrinkage is a principled research challenger
([Ledoit and Wolf 2004](https://doi.org/10.1016/S0047-259X(03)00096-4)), but a
global covariance inverse is not recommended for the initial product. If later
qualified, it should be block-diagonal by declared feature family and computed by a
pinned deterministic implementation.

## Bounded activation vectors

The proposed durable representation uses:

- SignedPpm: an i32 value in [-S, S]
- UnitPpm: a u32 value in [0, S]
- Separate positive and negative activation channels
- An L1 budget no greater than S within each feature family
- Stable PatternId ordering

Seeds may come only from explicit soft preferences, qualified profile evidence, and
the active document brief. Hard require and forbid rules stay outside the vector.
A population association, detector result, provider label, or watermark
observation cannot create a seed.

The direct seed vector is the baseline. If propagation later qualifies, a bounded
personalized random-walk form is:

    a_positive[t + 1] =
        rho * s_positive + (1 - rho) * transpose(W) * a_positive[t]

    a_negative[t + 1] =
        rho * s_negative + (1 - rho) * transpose(W) * a_negative[t]

W is nonnegative and row-stochastic. Personalized PageRank supplies the relevant
restart construction
([Haveliwala 2002](https://doi.org/10.1145/511446.511513)). Its success in web
ranking is not evidence that propagation helps editorial decisions.

Initial preregistration candidates are one propagation step, with two steps as a
challenger, and rho = 0.80. The implementation must execute a fixed number of steps,
not stop on a platform-dependent floating-point convergence test.

Each row of W is normalized to sum exactly to S. Use u128 products and checked
addition. Allocate integer remainders by descending remainder, then ascending
PatternId. Apply the same rule after each restart mixture. An overflow,
out-of-range weight, duplicate edge, or incorrect row sum is a hard decode or build
error.

## Personal-profile distance

The profile is an authorized, context-bound statement about one user's preferences.
It is not a human-authorship reference distribution.

For scalar feature i, let [l[i], h[i]] be an acceptable interval, r[i] a positive
normalization range, and x[i] the candidate measurement:

    gap[i] = max(l[i] - x[i], 0, x[i] - h[i])
    d[i] = min(1, gap[i] / r[i])

The fixed-point form computes:

    d_ppm[i] =
        min(S, round_half_up(S * gap[i] / r[i]))

Use u128 for the product. A zero range is invalid.

For a categorical distribution, total variation is an exact bounded initial
distance:

    TV(P, Q) = 0.5 * sum over k of abs(P[k] - Q[k])

When both distributions use integer ppm and sum exactly to S, TV can be computed
with integers. Jensen-Shannon divergence is a symmetric research challenger:

    M = (P + Q) / 2
    JSD(P, Q) = (KL(P || M) + KL(Q || M)) / 2
    d_JS = JSD(P, Q) / ln(2)

d_JS is in [0, 1]. The square root of JSD is a metric
([Endres and Schindelin 2003](https://doi.org/10.1109/TIT.2003.813506)).
JSD should not enter a cross-platform identity or rank until its logarithm and
rounding implementation are pinned and verified.

A sample-derived confidence factor may use:

    c[i] = n_effective[i] / (n_effective[i] + kappa)

Kappa and the definition of effective sample size are preregistration candidates.
Raw sentence count is not an adequate effective sample size when many observations
come from the same document or session. Explicit user declarations retain their own
authority and are not weakened by a population confidence formula.

For feature family g:

    D[g] =
        sum over i in g of w[i] * c[i] * d[i] /
        sum over i in g of w[i] * c[i]

A zero denominator means unknown, not a perfect match. Across known families:

    D_total =
        sum over g of W[g] * D[g] /
        sum over known g of W[g]

    style_fit_ppm = S - round(S * D_total)

The report must include the known-weight fraction, excluded features, uncertainty,
and leading named contributions. An aggregate score cannot replace those details.

## Lexicographic candidate ranking

Soft graph evidence cannot offset a fidelity failure. Candidate selection proceeds
in this order:

1. Reject candidates that fail a literal, structural, semantic, policy, or resource
   gate.
2. Enforce explicit require and forbid constraints.
3. Compare qualified personal-profile distance.
4. Compare qualified fluency evidence.
5. Compare named editorial penalties and introduced findings.
6. Prefer lower authorized edit cost.
7. Break a remaining tie by stable candidate ID.

Unknown profile or editorial coverage produces an abstention or an unknown field,
not zero penalty.

Durable ranking should use predeclared integer bands rather than f32 or f64 totals.
A proposed tuple for already eligible candidates is:

    (
      profile_distance_band,
      reverse(fluency_band),
      editorial_penalty_band,
      edit_cost,
      candidate_id
    )

Band boundaries are preregistration candidates and must be fixed on calibration
data. Pairwise rules such as "tie when confidence intervals overlap" are not a
valid sort comparator because overlap need not be transitive.

## Canonical computation and identity

The durable identity should hash canonical raw sufficient statistics and policy
identities, not platform-produced logarithms, square roots, or unordered sums.
Cross-platform reconstruction requires:

- Fixed enum tags and field order
- Length-prefixed bounded byte fields
- ASCII stable IDs where practical
- Sorted unique vectors, never map iteration order
- u64 observation counters and u128 intermediate products
- One documented division and tie-breaking rule
- Rejection of duplicate IDs, impossible contingency tables, inconsistent totals,
  invalid UTF-8, trailing bytes, overflow, and values above declared caps
- Golden byte and digest fixtures on Windows, macOS, and Linux

A BTreeMap is suitable during construction. A HashMap may be used for temporary
integer accumulation only when its contents are sorted before validation,
serialization, scoring, or reporting.

No durable rank or identity may depend on an unpinned BLAS library, parallel
reduction order, hardware-specific fused operation, or standard-library hash seed.
Research notebooks may use floating point if they record the exact environment and
never become a second product authority.

## Rust-oriented structures

The first API should remain small and invariant-bearing:

    struct PatternGraph {
        schema_version: GraphSchemaVersion,
        identity: PatternGraphId,
        nodes: Vec<PatternNode>,
        edges: Vec<PatternEdge>,
        hyperedges: Vec<PatternSet>,
    }

    struct CoOccurrenceStats {
        windows: u64,
        left: u64,
        right: u64,
        both: u64,
    }

    struct ActivationVector(Vec<(PatternId, SignedPpm)>);

    struct CsrProjection {
        row_offsets: Vec<u32>,
        targets: Vec<u32>,
        weights_ppm: Vec<u32>,
    }

Fields should be private. Checked constructors enforce sort order, uniqueness,
bounds, row sums, count consistency, and capacity limits. Decoders enforce a byte
ceiling before allocation and reject unknown schema versions and trailing data.

A compressed sparse row projection supports bounded propagation without a graph
dependency. The relational store remains authoritative for event and release
persistence. Adding a graph crate should require an algorithmic need, a dependency
review, and a benchmark against the explicit representation.

## Complexity and resource bounds

Let M be the number of occurrences, k[w] the number of distinct patterns in
relationship window w, K the configured window cap, L the sequence lag, E the
number of retained graph edges, H the number of retained hyperedges, and R the
fixed propagation step count.

| Operation | Time | Additional storage |
| --- | --- | --- |
| Extraction and ledger validation | O(tokens + M) | O(M) |
| Pair enumeration | O(sum over w of k[w]^2), bounded by O(N * K^2) | O(E) |
| Sequence enumeration | O(M * L) | O(E) |
| Dependency projection | O(parser arcs) | O(E) |
| Triple enumeration | O(sum over w of k[w]^3), bounded by O(N * K^3) | bounded retained sets |
| Fixed-step propagation | O(R * (P + E)) | O(P + E) |
| Direct profile distance | O(observed profile features) | O(P) |

Every builder must receive explicit maxima for decoded bytes, documents, units,
occurrences, patterns per unit, edges per node, total edges, hyperedges, and report
contributions. Reaching a cap returns a typed error or bounded overflow result. It
must not silently truncate, randomly sample, or continue with a partial identity.

## Statistical qualification

### Split discipline

Development, calibration, and locked-test partitions must be manifest-bound and
disjoint. Documents, near duplicates, authors where known and authorized, sessions,
and derived variants stay in one partition. Topic, language, channel, document
kind, and length require explicit strata. Calibration fixes all thresholds before a
locked test.

Windows within one document are correlated. Confidence intervals and tests should
resample or cluster at the document level, and at the profile level for owner
preference studies. Treating every sentence or pair as independent would
underestimate uncertainty.

### Baselines and ablations

The minimum comparison set is:

1. Flat deterministic rule catalog
2. Flat catalog plus direct personal-profile distance
3. Qualified cluster aggregation
4. Qualified pair interactions
5. Sequence edges
6. Dependency edges
7. One-step propagation
8. Higher-order challenger, if retained after calibration

Each addition receives an ablation. A full graph score without component ablations
cannot establish which relationship helped or whether duplicated evidence caused
the result.

### Outcomes

Report at least:

- Per-rule precision and recall where a complete label set is meaningful
- Clean-control document rate with any false finding
- False findings per 1,000 eligible documents, split by severity
- Introduced-finding rate
- Document-level repetition and redundancy outcomes
- Owner preference with abstention and disagreement
- Literal, structure, semantic, and protected-context fidelity outcomes
- Coverage, known-weight fraction, and abstention by stratum
- Effect estimates, uncertainty intervals, sample sizes, and exclusions
- Runtime, peak memory, and cap failures

The primary hypothesis is that a named graph component improves a predeclared
editorial outcome over the strongest simpler baseline. The null is no improvement.
Fidelity and clean-control non-regression limits must be predeclared as separate
constraints. An improvement in a soft average does not compensate for crossing
either limit.

Let FPR_graph and FPR_flat be the paired clean-control document rates for the graph
and strongest flat baseline. Before opening the locked set, define a noninferiority
margin epsilon_FP. Promotion requires:

    upper_95(FPR_graph - FPR_flat) <= epsilon_FP

The same one-sided requirement applies overall and in every release-critical
stratum. Introduced findings, severity, and abstention remain separate outcomes.
Insufficient independent cluster support is inconclusive, never a pass. The
evaluation protocol must adjust the complete family of component and stratum tests.

Multiple tests are grouped into predeclared families. Exploratory results are
labeled exploratory and cannot qualify a live weight. Negative and inconclusive
results remain in the release evidence.

### Preregistration candidate table

All entries in this table are proposed calibration starting points, not current
product behavior.

| Item | Candidate | Required challenge |
| --- | --- | --- |
| Fixed-point scale | S = 1,000,000 | Golden cross-platform arithmetic vectors |
| Patterns per eligible unit | K = 32 | Overflow frequency and adversarial dense units |
| Higher-order depth | At most 3 | Pair-only and flat-rule ablations |
| Sequence lag | At most 2 eligible units | Lag 0, 1, and 2 comparison by context |
| Interaction budget | beta <= 0.20 | beta = 0 baseline and cluster ablation |
| Propagation steps | 1, with 2 as challenger | Direct seed baseline |
| Restart share | rho = 0.80 | No-propagation and sensitivity analysis |
| Minimum support | Selected on calibration, then frozen | Rare-pattern stability curve |
| Shrinkage kappa | Selected on calibration, then frozen | Unsmoothed and sensitivity curves |
| Rank bands | Selected on calibration, then frozen | Boundary and tie-order fixtures |

## Population excess boundary

Population excess analysis belongs only to isolated source-signal research. Let
y[j, i] be document-level presence for pattern i in document j. For the eligible
document set J_T in target population T:

    p[i] = sum over j in J_T of y[j, i] / |J_T|

Let q[i] be a declared reference expectation estimated from a separately identified
reference population or a frozen expected-frequency model. Unless a causal design
supports stronger language, q is a reference expectation, not the counterfactual
frequency that would have occurred without model use.

The primary effect is the absolute gap:

    Delta[i] = p[i] - q[i]

The ratio p[i] / q[i] and log ratio log(p[i] / q[i]) are secondary and unavailable
when q is zero or support is below the predeclared floor. Rare patterns require a
frozen shrinkage model, such as a beta-binomial hierarchy, and must be suppressed
when the posterior remains too sensitive to its prior. Reporting whichever of gap
or ratio looks larger after inspection is invalid.

Language, domain, channel, time, and document-length composition can confound a
pooled estimate. With target deployment stratum weights omega[s]:

    p_standardized[i] = sum over s of omega[s] * p[i, s]
    q_standardized[i] = sum over s of omega[s] * q[i, s]

The weights and common-support rule are fixed before evaluation. A missing reference
stratum yields unsupported scope or abstention, not an imputed pooled pass.
Uncertainty resamples the highest dependence cluster, such as author, document, or
source bundle. It never treats word occurrences, sentences, or overlapping windows
as independent samples.

Population excess cannot identify an individual document's source. For source class
A and observed feature X, Bayes' rule is:

    P(A | X) =
        P(X | A) * P(A) /
        (P(X | A) * P(A) + P(X | not A) * P(not A))

Suppose P(X) = 0.1 and P(A) = 0.5. One compatible population has P(X | A) = 0.2
and P(X | not A) = 0, which gives P(A | X) = 1. Another has both conditionals equal
to 0.1, which gives P(A | X) = 0.5. The same aggregate prevalence supports
incompatible individual attributions. Population-excess graph outputs are therefore
aggregate records only and expose no per-document authorship or provider posterior.

## Source-signal isolation

Source-signal isolation is structural, not a weight of zero in the same graph.
Separate code modules, schemas, stores, manifests, reports, and application ports
must ensure that source research has no callable path into generation or ranking.

The live editorial system must reject fields for:

- Detector or authorship probability
- Statistical or keyed watermark outcome
- Model-family likelihood or provider identity
- Human, AI, edited-AI, or mixed-source label
- Population excess ratio used as an editorial weight

These values cannot become a node, edge, activation seed, cluster membership,
profile feature, target, training label, retry condition, acceptance condition, or
ranking term.

Machine-learning systems can exploit predictive shortcuts that do not reflect the
intended concept
([Geirhos et al. 2020](https://doi.org/10.1038/s42256-020-00257-z)). A historical
study also found demographic disparities in several GPT detectors under its tested
data and detector set
([Liang et al. 2023](https://doi.org/10.1016/j.patter.2023.100779)). These results
do not establish universal behavior for current detectors. They support the
architectural decision not to make any detector a live editorial authority.

A source-signal observation may nominate a human-reviewed editorial research
hypothesis. There is no automated import. Every nominated hypothesis enters one
declared multiplicity family and must be restated as a provider-neutral, observable
editorial property. Its matcher, threshold, weight, and qualification use only
source-neutral editorial outcomes and locked clean controls. Source scores cannot
select any of those values. Product weight may arise from editorial outcomes and
authorized user preference, never from how well a pattern predicts a model or
provider.

The live editorial crate cannot depend on the source-research crate. Source
diagnostics run only after the accepted artifact is frozen and have no CandidateSet,
RankingContext, retry, acceptance, or profile handle.

### Isolation regression requirements

- A source-research record cannot decode as PatternGraph or
  EditorialEvidenceReleaseId.
- Ranking output is byte-identical when source reports are absent, present, or
  adversarially permuted.
- Product graph identity is invariant to every source-report field and byte.
- QualifiedBy rejects a SourceSignalResearchReleaseId at the type and decode
  boundaries.
- The source diagnostic route cannot receive CandidateSet or RankingContext and is
  callable only after accepted-artifact freeze.
- Cargo dependency policy forbids an editorial-to-source-research dependency.

## Failure modes and required responses

| Failure | Required response |
| --- | --- |
| Unsupported language or context | Return unknown or abstain |
| Missing eligibility denominator | Reject the derived statistic |
| Rare pair with unstable association | Report support and keep out of live score |
| Parser drift or unknown artifact | Invalidate dependency release |
| Excess patterns in one unit | Return bounded overflow finding |
| Duplicate or inconsistent counts | Reject build or decode |
| Fixed-point overflow | Return typed arithmetic error |
| Unknown graph or policy identity | Refuse activation or ranking |
| Profile coverage denominator is zero | Report unknown, never perfect match |
| Source-signal field or ID reaches editorial port | Reject schema or application call |
| Graph challenger misses non-regression limit | Retain simpler baseline |

## Ordered implementation and research sequence

1. Freeze typed pattern IDs, eligibility records, event ordering, bounds, and
   canonical fixtures.
2. Implement the flat deterministic rule baseline and named fixed-point family
   distance.
3. Derive co-occurrence sufficient statistics for inspection only.
4. Add sequence and dependency observations behind separately qualified artifacts.
5. Evaluate redundancy clusters and capped pair residuals against the flat
   baseline.
6. Evaluate one fixed propagation step only if direct graph components advance.
7. Evaluate hyperedges, motifs, or covariance-aware distance only as challengers.
8. Permit a component into live ranking only through an immutable qualification
   record bound to its exact graph, policy, corpus, and evaluation identities.

At every stage, a simpler qualified component remains the fallback only when that
fallback is explicit and independently qualified. Runtime or artifact drift cannot
silently select a different scorer.

## Limitations

- Co-occurrence does not establish editorial importance, causality, or preference.
- A bounded graph cannot represent every rhetorical dependency.
- Parser and matcher errors propagate into derived counts.
- Personal evidence may be sparse, context-confounded, or stale.
- Fixed-point arithmetic improves reproducibility but does not make a statistical
  assumption correct.
- A held-out gain in one language or channel does not transfer automatically.
- Named lint findings remain editorial judgments, not formal guarantees.
- No score proves human authorship, model authorship, authenticity, ownership,
  watermark absence, or legal compliance.

## Primary references

- [Benjamini and Yekutieli, False discovery rate under dependency, 2001](https://doi.org/10.1214/aos/1013699998)
- [Benson, Gleich, and Leskovec, Higher-order organization of complex networks, 2016](https://doi.org/10.1126/science.aad9029)
- [Church and Hanks, Word Association Norms, Mutual Information, and Lexicography, 1990](https://aclanthology.org/J90-1003/)
- [Dunning, Accurate Methods for the Statistics of Surprise and Coincidence, 1993](https://aclanthology.org/J93-1003/)
- [Endres and Schindelin, A new metric for probability distributions, 2003](https://doi.org/10.1109/TIT.2003.813506)
- [Geirhos et al., Shortcut Learning in Deep Neural Networks, 2020](https://doi.org/10.1038/s42256-020-00257-z)
- [Han, Pei, and Yin, Mining frequent patterns without candidate generation, 2000](https://doi.org/10.1145/342009.335372)
- [Haveliwala, Topic-Sensitive PageRank, 2002](https://doi.org/10.1145/511446.511513)
- [Holm, A simple sequentially rejective multiple test procedure, 1979](https://doi.org/10.2307/4615733)
- [Ledoit and Wolf, A well-conditioned estimator for large-dimensional covariance matrices, 2004](https://doi.org/10.1016/S0047-259X(03)00096-4)
- [Levy and Goldberg, Dependency-Based Word Embeddings, 2014](https://aclanthology.org/P14-2050/)
- [Liang et al., GPT detectors are biased against non-native English writers, 2023](https://doi.org/10.1016/j.patter.2023.100779)
- [Monroe, Colaresi, and Quinn, Fightin' Words, 2008](https://doi.org/10.1093/pan/mpn018)
- [Zhou, Huang, and Scholkopf, Learning with Hypergraphs, 2006](https://proceedings.neurips.cc/paper_files/paper/2006/hash/dff8e9c2ac33381546d96deea9922999-Abstract.html)
