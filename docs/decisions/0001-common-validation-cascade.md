# ADR 0001: Common validation cascade

- Status: proposed
- Decision owners: project maintainers
- Decision gate: roadmap milestone 0.1
- Last reviewed: 2026-08-11

## Context

Multiple future generation strategies will have different latency, context, and
surface-divergence behavior. Allowing each strategy to define its own acceptance
rules would make fidelity depend on the route selected and create bypasses that are
hard to evaluate.

Natural-language meaning cannot be proved mechanically in the unrestricted case.
Exact syntax, structure, typed literals, and protected values can be enforced, while
learned semantic assessment remains calibrated evidence with an uncertain state.

## Decision drivers

- No style improvement may compensate for a fidelity failure.
- Every interface must produce the same decision for the same transaction.
- Uncertainty must be observable and rejectable under strict policy.
- The system must return the exact original under document-atomic abstention.

## Options considered

### Strategy-specific pipelines

Each strategy would own generation and validation. This is initially convenient but
duplicates policy, makes comparisons unreliable, and permits fast routes to weaken
safety.

### Shared hard gates with independent semantic evidence

Strategies only propose candidates. A common engine validates candidate ownership,
sentinels, protected values, adapter structure, text safety, and semantic evidence.
Only eligible candidates reach lexicographic ranking.

## Decision

Use one common validation cascade for all candidates. Deterministic gates run before
semantic assessment and ranking. A hard failure or disallowed uncertainty makes a
candidate ineligible. Ranking cannot trade fidelity for style. Document-atomic mode
returns no edits when any required unit has no eligible candidate.

The semantic evaluator remains a replaceable, independently versioned port. The
current literal evaluator is deliberately narrow and is not evidence that
open-domain equivalence is solved.

## Consequences

### Positive

- Generation strategies cannot bypass fidelity policy.
- Model-free fixtures exercise the complete transaction.
- CLI, API, MCP, desktop, and skills can share conformance cases.
- Abstention and failure have stable machine-readable records.

### Negative

- Broader rewriting depends on evaluator qualification and may abstain frequently.
- Adapter and validator contracts require careful versioning.
- Adding a strategy does not avoid the cost of the common cascade.

### Follow-up

- Expand hard-negative cases by semantic risk category.
- Calibrate an independent local semantic evaluator.
- Report accepted-output risk together with coverage and abstention.
- Add document-level cross-reference gates before broader atomicity modes.

## Validation

The decision passes its gate when all strategies use the same engine path, critical
gate suites contain no known critical regression, exact abstention holds across
supported operating systems, and critical-path coverage exceeds the project target.

Revisit only if measured evidence shows that the cascade creates a fidelity defect
or cannot represent a required independently validated format capability.

## References

- [Architecture](../architecture.md)
- [Evaluation strategy](../evaluation.md)
- [Security and privacy](../security.md)
