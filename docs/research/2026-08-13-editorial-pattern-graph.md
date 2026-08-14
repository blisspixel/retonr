# Editorial pattern graph research decision

## Status

- Review date: 2026-08-13
- Evidence cutoff: 2026-08-13
- Decision: adopt a bounded graph-shaped editorial catalog, but keep population
  source signals in a separate research system with no live rewrite authority
- Implementation status: planned

The detailed estimator, graph, scoring, and determinism rules are in the companion
[editorial pattern mathematics note](2026-08-13-editorial-pattern-mathematics.md).
The locked evidence and promotion design is in the
[editorial pattern evaluation preregistration](2026-08-13-editorial-pattern-evaluation.md).

## Verdict

The proposed graph is useful because editorial observations can interact and
co-occur. Words, phrases, rhetorical templates, punctuation habits, and cadence
patterns can reinforce one another. A graph can represent those relationships more
honestly than a flat word ban list.

Retonr must not build one graph that mixes editorial judgment, personal preference,
and model-source inference. That design would let model-family correlations leak
into generation or ranking even if the public interface called them soft signals.
The safe design has three separately versioned evidence planes:

| Plane | Purpose | Live rewrite authority |
| --- | --- | --- |
| Editorial pattern graph | Named, contextual, explainable writing defects | May guide or rank only after every hard fidelity gate passes |
| Personal style profile | Authorized evidence of the user's preferences | May guide or rank within the profile contract |
| Source-signal research graph | Population excess ratios, model-family correlations, watermark results, and authorship experiments | None |

The first two may be compared in a bounded editorial decision. The source-signal
graph is never an input to live generation, retry, ranking, acceptance, or profile
learning.

## Research grounding

Kobak et al. analyzed 15.1 million English-language PubMed abstracts from 2010
through 2024. They extrapolated expected 2024 word frequencies from pre-LLM trends,
then reported both an excess frequency gap and an excess frequency ratio. Examples
included ratios of 28.0 for `delves`, 13.8 for `underscores`, and 10.7 for
`showcasing`. The method is useful population evidence. The authors release code and
yearly counts that reproduce the main excess-frequency analysis.

It is not an individual-document classifier. The paper states that the method
cannot distinguish direct model use from people adopting model-preferred language,
cannot separate model families, and cannot always separate style change from a new
topic. Its ratios are specific to a biomedical corpus and observation window.

Juzek and Ward independently identified 21 focal words whose increased use in
scientific abstracts is likely related to model use. Their experiments did not find
evidence that architecture, algorithms, or training data alone explained the
overrepresentation. Their model comparisons were consistent with a possible role
for reinforcement learning from human feedback, but the authors describe the human
study as exploratory. Retonr therefore must not encode a causal story such as
"next-token sampling plus preference tuning creates this phrase" as a verified
rule fact.

Community and commercial catalogs can suggest hypotheses. Wikipedia's signs guide
is a community-maintained operational aid, not a scientific authorship test.
Bloomberry reports a June 2026 catalog of 7,622 signal entries under CC BY 4.0, where
regex surface variants and replacement pairs count as entries. It publishes a
141-entry machine-readable sample, not the complete internal enforcement corpus or
an independently reproducible validation package. The page's source tiers,
production status, and false-positive labels remain vendor assertions. Neither
source is an admissible production rule set without an independent Retonr fixture,
rights decision, and qualification review.

Primary sources:

- [Kobak et al., Science Advances 2025](https://www.science.org/doi/10.1126/sciadv.adt3813)
- [Kobak et al. reproducibility repository](https://github.com/berenslab/llm-excess-vocab)
- [Juzek and Ward, COLING 2025](https://aclanthology.org/2025.coling-main.426/)
- [Wikipedia signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing)
- [Bloomberry AI Sentence DNA catalog description](https://www.bloomberry.ai/research/ai-writing-patterns)
- [Bloomberry machine-readable sample](https://www.bloomberry.ai/research/ai-sentence-dna-corpus-sample.json)

## Product graph contract

The product-facing graph is an editorial rule catalog with explicit relationships.
It does not contain an `ai_probability`, detector score, model-family tag, or
watermark outcome.

### Nodes

Each node represents one bounded editorial observation:

- Word or lemma in a qualified context
- Multi-word phrase or transition
- Rhetorical template such as repeated contrastive reframing
- Paragraph or section template
- Punctuation or formatting density
- Sentence-length or paragraph-length distribution
- Repetition, attribution, qualification, or conclusion behavior

Every node has a stable rule ID and version, matcher identity, language and channel
scope, exclusions, minimum evidence, severity, explanation, proposed actions, and
fixture-set identity. Surface vocabulary alone is never sufficient when a quoted,
technical, literal, cited, or user-preferred use is plausible.

### Edges

The first schema should admit the same small closed set of edge types frozen by the
mathematics contract:

- `co_occurs`: symmetric presence within an exact eligible window
- `precedes`: directed presence within a declared lag and boundary
- `depends_on`: directed parser relation under an exact parser identity
- `member_of`: declared or qualified cluster membership
- `conflicts_with`: declared incompatibility
- `requires`: declared dependency
- `qualified_by`: provenance to an editorial evidence release

Every edge has a stable ID, direction, rule-set version, evidence revision, declared
scope, and qualification state. An edge is not valid merely because two patterns
appeared together in a convenience corpus. The training, calibration, and locked
test partitions must remain separated.

### Output

The scanner returns an interpretable activation vector and named cluster findings,
not a universal slop score. A report may show counts, densities, source ranges,
resolved findings, introduced findings, and profile-relative differences. It must
not say that the passage is human-written, AI-written, undetectable, authentic, or
watermark-free.

An aggregate may exist as a bounded internal diagnostic for calibration. It cannot
become the only user-facing result, erase named evidence, or trade one severe defect
for many weak improvements.

## Source-signal research contract

Population excess ratios, model-family labels, detector results, and statistical
watermark observations belong in an isolated research record. This record may use
nodes and edges for experiments, but it has no callable edge into the live rewrite
service.

A source-signal record binds at least:

- Corpus manifest, rights decision, language, domain, channel, and time window
- Unit of analysis, tokenization, normalization, and minimum occurrence policy
- Expected-frequency model, confidence interval, and multiple-testing policy
- Exact model, runtime, prompt, sampling, and artifact identities where applicable
- Pattern and relationship estimates with uncertainty
- Calibration, locked-test, and revalidation identities

The record never supplies a product ranking weight. If a word or structure first
found in source-signal research appears to be a real editorial defect, it must enter
the product graph through an independent contextual rule, clean counterexamples,
user-value evidence, and qualification process.

## Personal profile relationship

The user profile is not a human-authorship detector. It records authorized evidence
of how one user chooses to write in a declared channel and context. Profile distance
is evaluated separately from editorial findings.

For example, a repeated three-item construction might be a qualified lint finding
when it is dense, redundant, and contrary to the document brief. It might be a
positive profile match when the user deliberately uses that device. The policy
engine resolves that conflict from explicit preference, context, and rule severity.
It does not infer that either form is more human.

## Baselines and updates

There is no universal clean-human baseline. Baselines must be versioned by language,
domain, channel, population, collection period, and rights decision. User writing
requires authorization, retention, revocation, deletion, and cross-profile isolation.

Frequency ratios need a minimum support threshold and uncertainty estimate. Rare
terms can produce unstable ratios, while common terms can produce a meaningful
absolute excess with a modest ratio. Retonr research should retain both frequency
gap and ratio where the method applies rather than selecting whichever number looks
more dramatic.

Graph updates are immutable releases, not silently decaying mutable weights. A new
observation window produces a new graph version and a change report. Release notes
identify added, removed, reweighted, split, and merged rules. Old rewrite records
retain the exact graph and policy identities they used.

## Ranking boundary

The eventual ranking order remains lexicographic:

1. Reject every candidate that fails a hard fidelity, structure, literal, policy,
   or resource gate.
2. Apply explicit user rules and document constraints.
3. Evaluate personal-profile fit within the qualified profile scope.
4. Evaluate fluency and named editorial findings.
5. Prefer a justified editorial improvement only among otherwise eligible results.

The engine never retries or resamples solely to lower a source-signal statistic. It
does not hard-ban a word because that word was overrepresented in a population. It
does not maximize distance from the source or from a model-family centroid.

## Evaluation

Each product rule and relationship requires:

- Positive fixtures and neighboring clean controls
- Quoted, cited, technical, literal, accessibility, and protected-context exclusions
- Authorized user corpora and independently adjudicated clean controls without
  AI/human authorship inference
- Language, domain, channel, and document-length strata
- Introduced-finding, fidelity, abstention, and user-acceptance outcomes
- A comparison against the simpler flat-rule baseline

The graph earns product use only if relationships improve a predeclared editorial
outcome over the flat catalog without a material fidelity or false-positive
regression. A visually elegant graph is not itself evidence of value.

## Rust implementation direction

The first implementation should use small explicit Rust types, sorted vectors, and
bounded adjacency lists. A graph database is unnecessary, and adding `petgraph` is
premature until an algorithm requires it and benchmarks justify the dependency.
NetworkX can remain a research notebook tool but must not become a shipping runtime
dependency or a second source of product truth.

The eventual serialized catalog needs a byte ceiling before decoding, private
invariant-bearing fields, checked constructors, unknown-field rejection, canonical
ordering, a content digest, and exact schema compatibility tests. Matchers and user
content stay outside the graph identity unless their bounded contract explicitly
requires them. Reports retain rule IDs and counts, not raw personal samples.

## Ordered next steps

1. Freeze a minimal editorial pattern and relationship schema with no source-signal
   fields.
2. Port the existing synthetic lint fixtures into stable rule IDs and prove the flat
   deterministic baseline.
3. Add a small qualified cluster fixture for repetition and formulaic structure.
4. Measure whether graph relationships improve precision or user acceptance over
   the flat baseline.
5. Add authorized personal-profile comparison as a separate evidence input.
6. Keep all population and model-family analysis in the isolated research harness.
7. Permit graph-informed live ranking only in the planned 0.5 qualification slice.

This sequence uses the useful part of the proposal: relationships and transparent
evidence. It rejects the dangerous shortcut: optimizing live prose against a graph
of model fingerprints.
