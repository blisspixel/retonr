# Initial user research and adjudication protocol

## Status

Status: proposed. Recruitment and non-synthetic collection remain blocked until the
project owner approves this protocol, the data policy, consent materials, study
questions, and analysis registration.

## Purpose

The initial program tests whether the product improves owner-perceived style and
task usefulness over simpler baselines without hiding fidelity errors, abstention,
or accessibility failures. It does not attempt to validate a universal writing
style detector or prove unrestricted semantic equivalence.

## Required preregistration

Before recruitment, the study record freezes:

- Primary and secondary questions
- Eligible population and recruitment channel
- Inclusion, exclusion, and withdrawal rules
- Tasks, channels, languages, and device requirements
- Baseline and product artifact identities
- Minimum practically important effect
- Power analysis, participant count, task count, and stopping rule
- Randomization, blinding, ordering, and repeated-measures analysis
- Fidelity, preference, coverage, abstention, and resource metrics
- Handling of missing, tied, excluded, and one-sided outputs
- Rater training, disagreement, adjudication, and escalation policy
- Data policy, consent revision, retention, and deletion procedure

Exploratory analyses are labeled and cannot replace the registered primary result.

## Participant safeguards

- Consent uses plain language and separates required study data from optional data.
- Participants can inspect submitted writing categories before upload.
- Collection rejects credentials and warns against private third-party,
  employer-confidential, privileged, regulated, or identifying content.
- Withdrawal remains available through the study-specific cutoff and triggers the
  data-policy revocation workflow.
- Compensation does not depend on preferring the product or accepting a rewrite.
- Accessibility needs can be reported without disclosing a diagnosis.

## Study structure

Each participant contributes only the minimum approved evidence and completes a
balanced subset of comparable communication scenarios. Training evidence, held-out
natural writing, scenario responses, and topic-held-out material remain distinct.

Candidate comparisons are randomized and blinded to system origin. The study records
no-rewrite, direct-prompt, style-description, retrieved-example, and product outcomes
where the registered comparison requires them. A candidate shown to an owner has
already passed the applicable deterministic gates, but adjudicators may still mark
semantic or pragmatic defects.

For each task, collect separately:

- Meaning and factual acceptability
- Owner voice preference
- Channel and audience fit
- Fluency and clarity
- Important omissions or additions
- Whether abstention was understandable and useful
- Final action: accept, edit, reject, revert, or keep original

Acceptance is not automatically treated as evidence for profile learning.

## Fidelity annotation

The rubric assigns source spans and candidate spans where practical and classifies:

- Entity, role, quantity, time, negation, modality, condition, attribution, scope,
  ordering, coreference, and cross-reference changes
- Exact literal and protected-value changes
- Structure, control-character, citation, link, and code changes
- Severity, confidence, and whether evidence is insufficient

At least two trained reviewers independently label release-semantic cases. They do
not see system identity or the other reviewer's label. Agreement is reported by risk
category. Disagreement is resolved by a third qualified adjudicator who records the
reason and applicable rubric clause.

The source owner remains authoritative for personal preference, intended tone, and
private contextual meaning. A model may help cluster or prioritize disagreements but
is never the sole release adjudicator.

## Exclusion and missingness

Cases are never silently dropped. The report lists counts and safe reasons for:

- Invalid consent or ownership
- Corrupt or unsupported input
- Language or task outside the registered scope
- Participant withdrawal
- Insufficient source context
- Rater conflict that remains unresolved
- Technical failure
- Product abstention or baseline non-output

Exclusions decided after outcomes are visible require an append-only amendment and a
sensitivity analysis including the original classification where possible.

## Locked evidence

Development, calibration, and locked sets use different participants or a registered
cluster-safe partition. Locked cases are unavailable to prompt construction,
threshold tuning, regression repair, and artifact selection. Only the predeclared
confirmatory candidate is opened for release qualification.

A label defect produces a new corpus version and invalidation record. A neighboring
synthetic regression may be created without copying the locked wording.

## Accessibility research

CLI and desktop tasks include keyboard-only use, screen-reader workflows, zoom,
high-contrast or forced-color modes, reduced motion, understandable errors, safe
diffs, cancellation, and recovery. Results state operating system, assistive
technology, versions, and workflow rather than treating automated checks as complete
accessibility evidence.

## Required outputs

The retained study report contains:

- Approved protocol and manifest digests
- Recruitment and attrition flow
- Participant and task counts without small identifying cells
- Fidelity and preference results with confidence intervals
- Coverage and abstention alongside error rates
- Per-category and per-platform results
- Adjudication agreement and amendment history
- Exclusions, missingness, deviations, and adverse events
- Deletion and revocation verification summary
- Exact product, model, runtime, prompt, profile, and build identities

Raw writing, candidate text, contact details, and free-text notes are not included in
the default report.
