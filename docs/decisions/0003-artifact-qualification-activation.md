# ADR 0003: Artifact, qualification, and activation identity

- Status: accepted
- Decision owners: project maintainers
- Decision checkpoint: roadmap milestone 0.2 implementation
- Last reviewed: 2026-08-13

## Context

A mutable runtime reference such as a model tag cannot establish which bytes were
tested, licensed, installed, or used. Upstream metadata can describe an artifact,
but it cannot grant product support. Combining installation and qualification into
one record would also make license review, invalidation, removal, and recovery
ambiguous.

The product needs to support several future artifact roles without allowing a model
download, runtime inventory response, or user-visible label to become an implicit
authorization decision.

## Decision drivers

- Every qualified operation must bind to immutable artifact bytes.
- Upstream capability claims remain untrusted until measured.
- Installation, qualification, invalidation, and activation have different owners
  and lifecycles.
- Interrupted activation must not create a partially authorized state.
- Model and voice license decisions must remain independently reviewable.

## Options considered

### Runtime tag as product identity

Store the runtime-local model name and trust runtime inventory. This is simple, but
tags can move and runtime metadata does not prove product thresholds, license review,
or the bytes used during a prior run.

### One mutable model record

Store installation facts, test results, and active state together. This reduces the
number of types but makes historical evidence mutable and complicates invalidation,
rollback, and concurrent use.

### Separate immutable evidence and explicit active state

Use content-derived artifact identity, immutable qualification evidence, explicit
invalidation, append-only activation decisions, and a small current binding. Recheck
the installed digest at activation and use boundaries.

## Decision

Select separate artifact, qualification, invalidation, activation-decision, and
active-binding concepts.

An artifact manifest records intrinsic facts, approved origin and revision,
content-derived identity, size, format, family, tokenizer identity when applicable,
and reviewed license evidence. Declared roles, languages, and context remain
untrusted metadata.

A qualification record binds the exact artifact to an exact runtime, operating
system, hardware tier, source and context envelopes, prompt and request policy,
threshold policy, supported roles, result, and license decision. Qualification does
not activate the artifact.

Activation verifies installed identity, current qualification, invalidation state,
role, and safe storage key before changing the active binding. A rewrite never
downloads, pulls, qualifies, or activates an artifact implicitly.

Qualification v1 remains the frozen single-file authority contract. Claim extraction
uses a separate inert qualification-v2 evidence type because it requires a complete
artifact set, an attested runtime build, an effective runtime state, and
effective-package evidence. SQLite schema v3 persists those records in separate
immutable tables and recursively revalidates canonical bytes, indexed identities, and
relationships. It does not project v2 evidence into v1 or create an active binding.

## Consequences

### Positive

- Mutable runtime labels cannot silently change a qualified dependency.
- Support claims can cite exact retained evidence.
- Removal, revocation, requalification, and rollback have explicit semantics.
- Generation, embedding, speech, and voice artifacts can reuse one lifecycle.

### Negative

- Artifact lifecycle code and storage require several related record types.
- Runtime-native digests must be normalized and mapped carefully to product identity.
- Upgrades require new evidence instead of modifying an old qualification.

### Follow-up

- Decide acquisition and staged-download policy in a separate record.
- The repository-owned artifact-set lease is implemented; it verifies bytes and
  holds the shared lifecycle boundary but grants no role authority.
- Application-owned managed-process attestation now produces inert
  runtime-build and effective-state records. Keep those records out of
  claim-extraction activation.
- Add invalidation inputs for runtime, prompt, threshold, license, and platform
  changes.
- Prove that removal cannot orphan an active or in-use binding.

## Validation

The decision passes when corrupt, truncated, wrong-size, wrong-role, unknown-license,
invalidated, mutable-tag, duplicate-binding, interrupted-activation, and recovery
fixtures all fail closed, while an exact qualified binding activates atomically.

Revisit if a selected runtime cannot expose an identity that can be related to
verified product-managed bytes or equally strong retained evidence.

## References

- [Architecture](../architecture.md)
- [Technology choices](../technology.md)
- [0.2 execution plan](../planning/0.2-grounded-cli.md)
- [SPDX 3.0.1 AI profile](https://spdx.github.io/spdx-spec/v3.0.1/model/AI/AI/)
