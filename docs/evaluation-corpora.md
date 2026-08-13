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

The first checked-in corpus is synthetic and redistributable under the repository
license. It contains both positive findings and clean controls. It does not claim
that the patterns identify model authorship, and its reference revisions are examples
of acceptable editing rather than the only correct answer.

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

1. Land and validate the synthetic editorial corpus schema.
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
