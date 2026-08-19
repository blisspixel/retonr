# Editorial-quality and watermark research corpora

## Purpose

Retonr needs test material for two different questions:

1. Does a passage contain a named, explainable editorial defect?
2. Is a passage compatible with one known watermark procedure under one exact
   detector configuration?

Those questions require separate corpora, schemas, reports, and release authority.
Neither corpus is a general collection of text labeled human or AI.

## Editorial-quality corpus

The editorial-quality corpus supports the anti-slop quality loop. Each case records:

- A stable case ID, schema version, language, and channel
- Synthetic or separately authorized content provenance
- The exact lint rules under test
- Expected findings with rule IDs and unambiguous source evidence
- A clean counterexample or neighboring context exclusion
- An optional reference revision when a bounded correction is justified
- Protected terms that must survive any later rewrite evaluation

The checked-in development groups are synthetic and redistributable under the
repository license:

- `editorial_quality_v1.json` covers basic residue, filler, attribution, repetition,
  and punctuation findings.
- `editorial_slop_v1.json` covers 12 denser current pattern families with one paired
  clean control per rule.
- `editorial_prose_v1.json` covers 20 structural, rhetorical, and evidential families
  with one paired clean control per rule, across five channels.
- `editorial_model_impressions_v1.json` covers 8 assistant-residue families
  (sycophantic affirmation, hedge openings, capability disclaimers, takeaway
  packaging, empty both-sides framing, casual address, certainty theater, and
  offered follow-ups) with one paired clean control per rule.
- `editorial_assistant_residue_v1.json` covers 10 later assistant-residue families
  (meta breakdown openers, knowledge-cutoff disclaimers, source-gap speculation,
  canned notability blurbs, leftover template placeholders, scientific-register
  padding, roast asides, decorative bold header lists, leftover role disclaimers,
  and empty it-depends scaffolds) with one paired clean control per rule.

Every group pairs each targeted rule with a clean control, and a single test enforces
that invariant for all five. Longer labeled impressions and licensed pre-2000
human excerpts live in the
[writing-sample library](evaluation-style-library.md). They are not lint
authority and not authorship labels.

They do not claim that the patterns identify model authorship. Their reference
revisions are examples of acceptable editing rather than the only correct answer.
Every pattern is included because it can describe a concrete editorial defect in a
declared context, not merely because it correlates with a model family.

### Pattern evidence and limits

The current slop group combines three evidence classes:

- Population evidence that em dash use and other lexical markers changed in some
  scientific corpora during widespread model adoption, while explicitly rejecting
  per-document inference from that trend
- Peer-reviewed evidence that model and hybrid text differs across sequence, phrase,
  and lexical levels, without assuming one permanent universal model style
- Repeated community reports of prefabricated scene setting, ornamental vocabulary,
  contrastive reframing, promotional puffery, and stacked rhetorical structures

Primary research also reports meaningful differences between model generations and
individual model idiolects. The catalog is therefore versioned and revisable. A
phrase never becomes a finding by vocabulary alone when a literal, quoted, cited,
technical, or otherwise legitimate use is plausible. Each rule must state its
applicability, density or context condition, exclusions, and neighboring clean case.

Kobak et al. provide strong population evidence for excess vocabulary in a specific
biomedical corpus, not an individual-document authorship test. Their 2024 ratios of
28.0 for `delves`, 13.8 for `underscores`, and 10.7 for `showcasing` are useful
research observations, but they do not become product weights or word bans. Juzek
and Ward identify a related set of focal words while leaving the causal mechanism
unresolved. Community and commercial pattern lists are discovery inputs only until
their methods, rights, fixtures, and false-positive behavior pass Retonr's admission
and qualification process.

Research basis:

- [Excess vocabulary in 15.1 million PubMed abstracts](https://www.science.org/doi/10.1126/sciadv.adt3813)
- [Reproducibility materials for the excess-vocabulary study](https://github.com/berenslab/llm-excess-vocab)
- [Lexical overrepresentation study](https://aclanthology.org/2025.coling-main.426/)
- [Em dash population study](https://arxiv.org/abs/2606.29540)
- [Multi-level style preference optimization](https://ojs.aaai.org/index.php/AAAI/article/view/40665)
- [Idiolectal model-output study](https://arxiv.org/abs/2608.06589)
- [Human use of model writing styles](https://aclanthology.org/2025.acl-long.267/)

The architecture decision that separates qualified editorial relationships from
source-signal research is documented in the
[editorial pattern graph review](research/2026-08-13-editorial-pattern-graph.md).

Non-synthetic writing is not added to the repository merely because it is publicly
visible. Licensed public data requires a source and redistribution decision.
Participant writing requires approved consent, retention, revocation, and deletion
controls. Locked release cases remain unavailable to rule development and prompt
construction.

## Watermark research corpus

Known-watermark fixtures must be generated, not guessed from style. Each fixture set
uses a public, reproducible marking implementation or an explicitly documented
provider detector and records:

- Scheme, paper, implementation commit, and local patch digest
- Model, tokenizer, runtime, precision, prompt set, and decoding configuration
- Synthetic research key or public verification material where publication is safe
- Marked and unmarked paired controls with matched generation settings
- Text length, entropy, language, task, code, and mixed-source strata
- Complete detector procedure, normalization, window search, threshold, and
  calibration identity
- Raw score, bounded outcome, uncertainty, and known invalidation conditions
- License, redistribution, retention, and access decision for every artifact

Private provider keys, stolen detector material, personal drafts, and adaptive
evasion recipes are never corpus dependencies. Closed provider output is a separate
black-box condition and is never represented as equivalent to a public reference
implementation.

Until a pinned public embedder runs, the checked-in research file
`crates/eval/fixtures/watermark_research/style_is_not_a_watermark_v1.json`
only refuses style-as-mark folklore and inventories literal carriers. It contains
no generated marks and no detector scores.

Initial reproducible candidates include KGW, Unigram, the Kuditipudi keyed sampler,
DiPMark or STA-1, and the public SynthID Text reference implementation. Selection
occurs during the isolated 0.5 research harness and follows the
[watermark evaluation protocol](research/2026-08-12-watermark-evaluation-protocol.md).

## Separation invariant

Editorial findings may guide or rank candidates only after fidelity gates pass.
Watermark and source-classification results never guide generation, retry, ranking,
or acceptance. Reports keep these fields separate:

- Fidelity and structure outcomes
- Editorial lint findings
- Personal-style preference outcomes
- Literal metadata and Unicode observations
- Scheme-specific watermark observations
- Passive source-classifier observations

There is no universal AI score and no `watermark_free` result.

## Logical order

1. Land and validate the synthetic editorial corpus schema and development groups.
2. Implement transparent deterministic lint rules against positive and clean-control
   cases.
3. Add adversarial near-matches, protected contexts, languages, and channels one
   qualified rule family at a time.
4. Approve governance before collecting or admitting any non-synthetic writing.
5. Freeze calibration and locked splits before measuring learned lint components.
6. Build the known-watermark corpus in the isolated 0.5 research harness from pinned
   public implementations and matched unmarked controls.
7. Publish corpus manifests, failures, and uncertainty beside any paper-style claim.

The corpus can grow continuously. Product authority grows only when a named rule or
watermark procedure passes its own declared qualification boundary.
