# Security Hardening Proposal: Package And Load Closure

## Decision

We will preserve `ArtifactSetManifest` v1 as the canonical byte inventory and add typed, versioned runtime-package, model-package, and native-load contracts. Static package membership will remain distinct from observed runtime use. Existing v1 identity records and canonical bytes will not change.

## Executive Recommendation

Typed semantic overlays provide the missing bridge from verified bytes to defensible runtime identity. A runtime package can state which member is the entrypoint and which members are dependencies or evidence. A separate native observation can state which admitted file-backed components were visible in the retained process at a checkpoint. Neither statement alone becomes qualification evidence.

## Evidence

- E06, generic artifact-set contract: proves a bounded canonical inventory but not semantic completeness.
- E07, effective-package v1: requires output-affecting purpose for every member and cannot represent evidence-only files honestly.
- E08 and E09, runtime identity contracts: accept opaque package, dependency, configuration, and loaded-component digests.
- E13, current app attestation: hashes a caller path and accepts critical digest declarations without a retained process.
- E14 and E15, managed import and lease boundaries: strongly verify a tree but retain no per-member code handles through launch.
- E04, retained process attestor: binds process incarnation and entrypoint object but has no native load closure.

These entries and the platform API references used for later load observation are indexed in [context.md](../context.md).

## Current Design And Failure Mode

The repository already has a strong portable byte-set contract and careful managed import verification. The semantic relationship between those bytes and runtime identity is still caller supplied. The loaded-components field is also caller supplied, while the current app path synthesizes it from an entrypoint digest. That can preserve record shape, but it cannot establish packaged dependency closure or observed native use.

The failure mode is authority by digest resemblance. Two structurally valid opaque digests can be placed into an identity without a typed relationship to the canonical member set or retained process. Static inventory can also be mistaken for actual use, especially when a package contains optional libraries, licenses, provenance, or backend-specific assets.

## Desired Invariants

1. Every semantic manifest entry corresponds path-for-path to exactly one canonical artifact-set member.
2. Exactly one runtime entrypoint is identified, and all derived v1 digests come from typed canonical subsets.
3. Evidence-only files are representable without claiming that they affect output.
4. Static load policy never claims that a member was actually loaded.
5. Native load evidence is bound to a retained process incarnation, package manifest, checkpoint, and complete admitted platform view.
6. Portable identities contain no host path, inode, address, timestamp, ownership, permission, or credential material.
7. Existing v1 records remain readable and inert; no backfill or digest similarity creates authority.

## Constraints And Non-Goals

We preserve all current v1 canonical bytes and public report shapes. JSON remains a bounded transport form, while IDs use deterministic typed binary encoding. We will not claim equality of resident executable pages, detect arbitrary in-memory patching, infer which request a module handled, or infer model loading from an idle Ollama listener.

## Before Architecture

[Before architecture](../diagrams/package-and-load-closure-before.mmd)

The current path joins caller declarations and one point-in-time file hash into inert identity records. It has no typed semantic package or live load relationship.

## Options

### Option 1: Retain Opaque Identity Inputs

[After architecture](../diagrams/package-and-load-closure-retain-opaque-identity-inputs-after.mmd)

This option improves documentation around the existing interfaces and continues treating all opaque inputs as inert. It has minimal compatibility and delivery risk but cannot establish package semantics or observed load closure.

| Dimension | Delta from before | Basis | Confidence | Validation |
| --- | --- | --- | --- | --- |
| Security | No new binding | E08-E09, E13 | High | Negative authority tests |
| Performance | No material change | Current code | High | Existing benchmarks |
| Operability | No migration | Current code | High | Existing CI |
| Compatibility | Full v1 compatibility | E06-E09 | High | Golden record tests |
| Maintainability | Opaque call sites remain easy to misuse | E13 | High | API audit |
| Delivery risk | Low | Current code | High | Existing gates |

### Option 2: Typed Package And Load Contracts

[After architecture](../diagrams/package-and-load-closure-typed-package-and-load-contracts-after.mmd)

This option layers `RuntimePackageManifestV1` and `ModelPackageManifestV1` over the exact artifact-set member order, then records a separate `NativeLoadObservationV1`. Typed builders derive existing runtime identity fields without changing their byte representation. A later model-load observation is required before model use can be claimed.

| Dimension | Delta from before | Basis | Confidence | Validation |
| --- | --- | --- | --- | --- |
| Security | Explicit byte, role, and observed-load bindings | E04, E06-E09, E13-E15 | High | Golden and adversarial contract tests |
| Performance | Bounded canonical encoding plus platform observation cost | E06, platform APIs | Medium | Encoding and mapping benchmarks |
| Operability | New reconstruction and observation steps | E14-E15 | Medium | Fixture-driven CLI tests |
| Compatibility | Additive contracts, v1 bytes preserved | E06-E09 | High | Cross-version golden IDs |
| Maintainability | More types, substantially fewer ambiguous digests | E08-E09 | High | Compile-time API use audit |
| Delivery risk | Medium staged implementation | Platform variance | Medium | Pure contracts before native adapters |

## Comparison

Option 1 accurately labels the current limit but leaves the central trust-chain gap in place. Option 2 is additive, separates static and observed facts, and creates deterministic construction paths for existing identity fields. It costs more implementation effort, but each phase can remain inert until its relationships are complete.

## Recommendation

We select Option 2. The initial slice will implement pure contracts, canonical encodings, decoders, typed builders, and fixtures only. Application leases, platform observers, persistence, model-load evidence, and qualification will follow as separate gates.

## Evidence Coverage And Residual Risk

Repository evidence strongly supports the contract gap and compatibility strategy. Native platform evidence supports file-backed mapping enumeration but cannot prove unmodified resident pages or arbitrary anonymous executable memory. macOS attached observation remains unsupported unless a separately reviewed entitled design is adopted.

## Migration And Rollout

No existing record changes. New types begin in memory and fixtures. After their canonical form stabilizes, an explicit schema migration may add empty immutable tables with recursive relationship checks. Old records remain inert and are not backfilled. Runtime and model reconstruction ships before native observation, and native observation ships before any effective-state authority.

## Validation Plan

Tests cover canonical IDs across hosts, strict decoding, every missing, extra, duplicate, reordered, unknown, and over-limit case, sharded models, embedded components, evidence-only roles, transformed lineage, typed v1 derivation, serialization redaction, migration non-authority, native mapping drift, cancellation, and platform unsupported behavior. New pure contract and codec code targets at least 95 percent line coverage.

## Implementation Work Packages

1. Add shared source and transformation contracts.
2. Add runtime package roles, policies, canonical codec, and v1 runtime identity builder.
3. Add model package roles, shard and embedded-component rules, canonical codec, and fixtures.
4. Add the inert native-load record and typed effective-state builder.
5. Add retained application code-member leases and a Linux object-bound observer.
   Keep Windows unsupported until a public mechanism binds each mapped section to
   the exact file object rather than reopening a reported pathname.
6. Add persistence only after golden contracts are frozen.

The executable handoff is [typed-package-and-load-contracts.md](../implementation/typed-package-and-load-contracts.md).

## Open Questions

The selected Ollama runtime and model package versions must be frozen before production reconstruction fixtures are added. The Linux mapping visibility policy requires its own reviewed verification plan. Windows stays unsupported until an object-bound mechanism is available.
