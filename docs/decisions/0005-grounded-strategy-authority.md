# ADR 0005: Grounded strategy has proposal authority only

- Status: proposed
- Decision owners: project maintainers
- Decision checkpoint: roadmap milestone 0.2 implementation
- Last reviewed: 2026-08-12

## Context

A generation strategy needs masked source, bounded style context, an exact artifact,
and a structured candidate contract. Giving the strategy access to final validation,
document writes, profiles, tools, filesystem, or network would let model behavior or
prompt content cross authorization boundaries.

Prompt delimiters do not make prompt injection impossible. The stronger boundary is
to ensure that untrusted prompt data has no authority and that model output remains
an untrusted proposal.

## Decision drivers

- Every generated candidate must reach the common validation cascade.
- Original protected surfaces should not be needed in the model prompt.
- Prompt, output, runtime, and artifact identity must be retained without raw text.
- Cancellation must discard partial work and preserve the exact original.
- The current literal evaluator must not be presented as open-domain equivalence.

## Options considered

### Let the model return an accepted rewrite

This is simple but combines proposal and authorization, cannot enforce deterministic
invariants reliably, and makes interface behavior backend-dependent.

### Give each strategy its own validator

This allows specialized behavior but creates bypasses and makes baseline comparison
unreliable.

### Generate masked candidates, then use the common engine

Serialize a versioned prompt envelope containing untrusted data, request complete
masked candidates, retain only redacted provenance, and pass candidates to the
existing engine with no strategy-specific acceptance path.

## Decision

Select masked proposal generation followed by the common engine.

The application builds the same deterministic protection plan used by the engine.
The prompt contains the masked source, typed sentinel tokens without their original
surfaces, requested mode, bounded style context, and required candidate count in a
structured JSON envelope. The versioned prompt template precedes that envelope and
is bound by digest.

The strategy can call only the backend-neutral inference port. It cannot access
files, network tools, profiles, document mutation, or final validation. It returns
untrusted masked candidates plus a redacted trace containing runtime, artifact,
prompt, input, output-schema, candidate-count, and usage identities.

The application passes those candidates through sentinel, protected-value,
structure, text-safety, semantic, and ranking gates. Backend cancellation becomes an
exact-original cancelled abstention. Other backend failures remain typed operational
errors and never replace source bytes.

The current evaluator accepts only literal-mode token-sequence preservation. Pure,
balanced, strong, and open-domain lexical changes remain uncertain until an
independent evaluator passes its calibration gate.

## Consequences

### Positive

- Prompt injection cannot directly acquire product authority.
- Protected surfaces remain outside the model prompt.
- Candidate lineage is retained without raw content in default traces.
- The same engine decides model-free checks and grounded proposals.

### Negative

- The application deterministically constructs protection before the engine repeats
  and verifies it.
- Useful open-domain transformation remains intentionally limited until calibration.
- Complete candidates are buffered before any output can be applied.

### Follow-up

- Add prompt-injection and sentinel-attack fixtures.
- Add an independently qualified semantic evaluator port and provenance.
- Add generation trace linkage to the durable transaction record before schema
  freeze.
- Add a property test proving application and engine protection plans remain
  identical for the same input and declared terms.

## Validation

The decision passes when accepted, rejected, cancelled, malformed, oversized,
identity-drift, prompt-injection, and sentinel-attack cases all retain exact-original
atomicity, every returned candidate has common-gate evidence, and serialized default
traces contain no source, candidate, style context, or protected surface.

Revisit only if a future strategy needs additional evidence. Additional evidence may
be added to the proposal contract, but final acceptance remains outside the
strategy.

## References

- [Common validation cascade](0001-common-validation-cascade.md)
- [Architecture](../architecture.md)
- [Security and privacy](../security.md)
- [0.2 execution plan](../planning/0.2-grounded-cli.md)
