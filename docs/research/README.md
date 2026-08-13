# Research program

## Purpose

This directory holds dated technical records that inform Retonr's active product,
architecture, evaluation, and roadmap documents. The records aim for paper-level
care, but their presence does not make a claim true. Active product commitments live
in the parent documentation and must cite reproducible evidence or state uncertainty.

## Evidence vocabulary

Research records and release reports use these labels:

| Label | Meaning |
| --- | --- |
| `standard` | A published normative specification or enacted primary legal text |
| `peer_reviewed` | A peer-reviewed paper under its named threat model and experiment |
| `official_implementation` | Maintainer or provider code or documentation that can be inspected or exercised |
| `provider_statement` | A dated provider claim whose undisclosed parts have not been independently verified |
| `preprint` | A public research report without completed peer review at the cutoff |
| `local_observation` | A reproducible Retonr experiment with exact artifacts and environment |
| `inference` | A conclusion drawn from stated evidence and assumptions |
| `unknown` | Evidence is absent, inaccessible, contradictory, or insufficient |

A stronger-looking label does not make a result universal. A peer-reviewed attack
applies to its schemes, artifacts, distributions, and threat model. A standard
defines behavior but does not prove that implementations conform. A provider
statement is useful primary evidence about public policy but may leave the deployed
mechanism, key, calibration, or coverage undisclosed.

## Research integrity rules

- Date every review and state its evidence cutoff.
- Prefer primary papers, standards, official implementations, and official provider
  statements over summaries.
- Record exact versions, immutable revisions, configurations, data, hardware, and
  operating systems needed to reproduce a result.
- Separate a reported result from a Retonr observation and from an inference.
- State the hypothesis, null, operating point, exclusions, and failure conditions.
- Preserve negative and inconclusive results.
- Do not generalize from one language, content class, length band, model, tokenizer,
  detector, watermark, runtime, or attack to all others.
- Predeclare locked release experiments and never tune against their outcomes.
- Report uncertainty, false positives, false negatives, abstention, and sample size,
  not only accuracy or area under a curve.
- Measure fidelity and task quality independently from source-signal behavior.
- Treat 2026 frontier results as provisional until independently reproduced.
- Never convert a classifier or detector result into proof of human authorship,
  misconduct, ownership, or legal compliance.
- Never let watermark or source-classification diagnostics guide live generation,
  retry, ranking, or acceptance.
- Record conflicts between sources instead of averaging them into false certainty.
- Revalidate a finding when its standard, provider behavior, implementation, model,
  runtime, law, or operating environment changes.

## Paper-style synthesis contract

The watermark and provenance synthesis should contain:

1. Abstract and precise contribution statement
2. Research questions and non-goals
3. Mechanism and evidence taxonomy
4. Threat models and trust boundaries
5. Literature and provider-practice review through the stated cutoff
6. Falsifiable Retonr hypotheses
7. Experimental design, calibration, and statistical analysis plan
8. Runtime and artifact assurance case
9. Product and architecture implications
10. Limitations, failure modes, ethics, legal boundary, and potential misuse
11. Reproducibility manifest and artifact plan
12. Open questions and revalidation triggers

The document must identify what Retonr contributes beyond synthesizing prior work.
A local experiment is not complete until an independent person can reconstruct the
tested system from its manifest and verify the reported outputs.

## Publication threshold

An arXiv-style technical report is a useful target for clarity and review, not a
release badge. Before external submission, require:

- Complete primary-source citation audit
- Clear separation between literature results and new Retonr experiments
- Public or access-controlled artifact manifests with legal data rights
- Reproduction on a clean machine from frozen inputs
- Independent technical review, statistical review, and security review
- Qualified legal review of jurisdiction-specific statements
- Disclosure of null, negative, contradictory, and out-of-scope results
- Removal of product-marketing claims not directly supported by the paper

External publication must not expose private watermark keys, provider secrets,
adaptive evasion recipes, personal writing corpora, or sensitive provider outputs.
