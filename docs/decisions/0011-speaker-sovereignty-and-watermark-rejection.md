# ADR 0011: Speaker sovereignty and rejection of unconsented text watermarking

- Status: proposed
- Decision owners: project maintainers
- Decision gate: roadmap milestone 0.2
- Last reviewed: 2026-09-26

## Context

Upstream model providers and standards discussions increasingly contemplate inserting
statistical watermarks, token-selection biases, invisible Unicode patterns, and
tracking payloads into generated prose. These mechanisms aim to identify the tool or
provider across downstream distribution.

At the same time, users rely on local editorial tools to refine, restructure, and
personalize rough and generated drafts. Questions arise regarding how Retonr should
treat upstream token sequences, statistical provider marks, and detector scores
during the rewrite lifecycle.

## Decision drivers

- Speech belongs to the speaker: the user directing and approving the final text is its
  author and speaker.
- Upstream wording has zero authority: a draft provides raw material, not an editorial
  mandate.
- Watermarking hidden meaning in content is wrong: embedding unconsented tracking
  payloads into prose compromises creative expression and user privacy.
- Rewriting in your voice provides creative agency and directly rejects unconsented
  hidden tracking.
- Avoid detector optimization: tuning output against third-party AI classifiers turns
  an unreliable detector into an optimization objective and violates fidelity
  principles.
- Deterministic preservation: explicit cryptographic bindings (e.g. C2PA) must not be
  falsely presented as valid on derivatives.

## Options considered

### Option A: Passive signal preservation

Attempt to detect and preserve upstream statistical watermarks during editing.

- Pros: Complies with paternalistic provider expectations.
- Cons: Technologically infeasible during significant re-expression; compromises the
  user's voice; accepts the premise that providers retain editorial claims on downstream
  prose.

### Option B: Active detector evasion (cat-and-mouse)

Incorporate watermark detectors and classifier scores into the live candidate ranking
loop to maximize classifier evasion.

- Pros: Directly targets third-party detector scores.
- Cons: Violates INV-P02 (Fidelity dominates style); trades semantic accuracy and
  personal voice for classifier bypass; brittle against updated detector keys and
  schemes; misleads users into expecting guaranteed untraceability.

### Option C: Principled speaker sovereignty and natural re-expression

Affirm that upstream wording has zero authority. Reconstruct eligible prose
exclusively around the speaker's personal voice, document brief, and explicit
constraints under the common validation cascade. Do not protect statistical marks,
and do not optimize against detectors.

- Pros: Codifies clear ethical boundaries; dismantles unconsented tracking through
  legitimate personal re-expression; keeps candidate ranking grounded in fidelity and
  voice; maintains strict fail-closed handling for explicit cryptographic bindings.
- Cons: Requires clear user education that Retonr does not offer a detector-evasion or
  untraceability guarantee.

## Decision

Select Option C.

1. Codify speaker sovereignty as a foundational product tenet: speech belongs to the
   speaker, and upstream wording possesses zero authority over the final text.
2. Formally declare unconsented hidden text watermarking to be an invasive tracking
   mechanism.
3. Reject statistical watermarks and source signals as preservation targets. They
   receive zero retention protection during prose rewriting.
4. Prohibit detector and classifier metrics from participating in live generation,
   candidate ranking, or acceptance. Live candidate selection is driven solely by the
   validation cascade and the speaker's voice profile.
5. Handle explicit cryptographic bindings (e.g., C2PA credentials) via explicit
   derivative invalidation workflows, preventing false attribution.

## Consequences

### Positive

- Core architecture remains focused on fidelity, voice, and document structure.
- Re-expression legitimately eliminates unconsented tracking without participating in
  detector cat-and-mouse dynamics.
- Interface contracts remain clean, rejecting deceptive claims of guaranteed human
  authorship or detector immunity.

### Negative

- Users seeking quick detector-bypass utilities may misunderstand why Retonr refuses
  to optimize for detector scores.
- Documentation must continually clarify the difference between natural re-expression
  and adversarial evasion.

### Follow-up

- Update `docs/governance/editorial-sovereignty.md` and `docs/invariants.md` (INV-P01,
  INV-P06).
- Maintain isolated research fixtures in `crates/eval` for measuring watermark
  behavior without connecting them to live runtime pipelines.

## Validation

- Candidate selection logic contains zero references to detector, watermark, or
  classifier outputs.
- Regression tests verify that candidates are evaluated strictly on fidelity, schema,
  and profile match.
- Documentation and CLI help text clearly communicate speaker sovereignty and the
  absence of detector-evasion claims.

## References

- [Product and engineering invariants](../invariants.md)
- [Editorial sovereignty and legal responsibility](../governance/editorial-sovereignty.md)
- [Provenance, marking, and derivative handling](../provenance.md)
- [Text watermark science and Retonr implications](../research/2026-08-12-text-watermark-science.md)
