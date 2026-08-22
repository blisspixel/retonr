# Implementation Plan: Typed Package And Load Contracts

## Implementation Status

The runtime-package, model-package, source, transformation, native-load, typed
runtime-identity, retained package-lease, schema-6 persistence, strict Ollama model
reconstruction, and offline inert import contracts are implemented. Linux has the
object-bound native-load adapter. Windows exact native-load binding and ordinary
macOS observation are unsupported. Imported or persisted evidence grants no
qualification, activation, or execution authority. A Linux-only read-only managed
preflight joins runtime-package, isolation, process, connection, provider-declaration,
and native-load evidence but remains inert. A separate v0.32.15-only static binding
now relates the installed model package to one exact verified idle inventory and
details observation while keeping model load, use, handler, effective identity, and
qualification false. A retained-session judge receipt is also implemented separately.
The managed preflight can derive typed runtime-build identity from its exact package,
process, and native-load join. An admission-gated one-shot managed operation is
implemented, but the empty reviewed-runtime allowlist blocks it before launch. Once
admitted, it retains that process, runtime package lease, native observer, and direct
connection through one structured completion and two equal post-generation residency
observations. It keeps the model artifact digest separate from the Ollama inventory
digest and records effective context before cleanup. Generation-bound provider,
effective configuration, platform and driver, and compute-placement relationships
remain absent, so no effective runtime state is constructed. One admitted runtime
package, the remaining direct effective-state evidence, exact model-package and judge
joins, and a distinct candidate-generation receipt remain downstream.

## Selected Design And Constraints

Add `RuntimePackageManifestV1`, `ModelPackageManifestV1`, and `NativeLoadObservationV1` as additive typed contracts over the existing canonical artifact set and retained process evidence. Static package roles and observed load state remain separate. Existing v1 identity and report bytes must not change, and new records grant no authority by themselves.

## Source Revision And Drift Check

The plan is anchored to Git revision `c3657edd126f164facc311719b831d6926e7c06d` and evidence collection SHA-256 `6a9d8e2195834f5d123add1626048d468b1b8bfd68eedb8bfaae022efa59ec3c`. Before each phase, recheck `artifact_set`, `effective_package`, `runtime_identity`, import verification, runtime leases, and platform attestor contracts. Canonical-byte changes require new golden IDs and an explicit compatibility decision.

## Affected Components

- `crates/model`: source, transformation, runtime-package, model-package, native-load contracts, codecs, IDs, builders, and fixtures.
- `crates/app`: retained runtime-package lease and typed package attestation boundary.
- `crates/runtime-attestor`: default-unsupported native-load observation and a Linux object-bound adapter.
- `crates/model-store`: later explicit schema migration and recursive relationship validation.
- `crates/eval`: later separate versioned reconstruction and effective-preflight command.

## Ordered Work Packages

1. Implement pure source and transformation values with bounded strict decoding.
2. Implement runtime roles, static load policies, complete path-for-path validation, canonical encoding, and derived v1 runtime-build fields.
3. Implement model roles, evidence-only distinctions, shard closure, embedded components, transformation lineage, canonical encoding, and fixtures.
4. Implement the inert native-load record and typed effective-state derivation.
5. Add a runtime-package lease that retains the base lease and opened code-member handles through launch and observation.
6. Add a bounded double-snapshot Linux native observer. Keep Windows unsupported
   because `VirtualQueryEx` plus `GetMappedFileNameW` exposes a pathname, not a file
   handle or file identity bound to the mapped section. Keep ordinary attached macOS
   unsupported.
7. Freeze golden contracts, then add empty schema-v6 tables with no backfill.
8. Reconstruct one frozen Ollama runtime and one frozen model offline.
9. Add runner process-graph and model-load evidence before any effective-package authority.

## Compatibility And Migration

All new contracts are additive. `ArtifactSetManifest` v1,
`RuntimeBuildIdentity` v1, `EffectiveRuntimeState` v1,
`EffectivePackageEvidence` v1, current preflight reports, and qualification v2 remain
unchanged. Schema v6 adds empty immutable tables under a backup-backed explicit
migration. Prior rows remain byte-identical and inert.

## Tactical Protections During Migration

Production code must not promote arbitrary existing constructors. Typed builders are introduced beside them and are the only path used by new package-attestation code. Persistence waits until contract fixtures and IDs are frozen. Native observation waits until static manifests and retained code-member lifetimes are complete. Evaluation waits until runtime and model load evidence can be joined without inference.

## Tests And Security Validation

Golden fixtures cover a runtime package, an Ollama-like model, sharded Safetensors, and GGUF embedded tokenizer/template descriptors. Negative fixtures cover missing, extra, duplicate, reordered, unknown, and over-limit members and roles; incomplete shards; missing evidence; transformation errors; and host-metadata leakage. App tests cover symlink, reparse point, hard link, replacement, extra file, drift, limits, and cancellation. Native tests cover duplicate, deleted, unopenable, anonymous, changed, and replaced mappings, wrong process, bounds, redaction, and deterministic macOS unsupported behavior.

## Performance And Resource Benchmarks

Benchmark canonical encoding and decoding at representative and maximum member counts. Enforce the existing 1 MiB manifest transport ceiling and artifact-set limits. Native observation enforces explicit mapping, component, input-byte, hashed-byte, elapsed-time, and cancellation bounds, with per-platform benchmarks recorded before enabling production reconstruction.

## Rollout And Rollback

Roll out pure contracts first, then application leases, then platform observation, then persistence, then offline reconstruction. Each layer remains inert until the next relationship is available. Rollback removes unused additive readers or disables the new command; it never rewrites prior records.

## Acceptance Criteria

- Every semantic manifest is a complete ordered overlay of one admitted artifact set.
- Exactly one runtime entrypoint exists, and v1 fields are derived through typed builders.
- Evidence-only model files are represented without output-affecting claims.
- Static policy contains no actual-load claim.
- Native observations bind one retained process, package, checkpoint, and admitted complete platform view.
- Portable identities contain no host metadata or secrets.
- All existing v1 golden bytes remain unchanged.
- Contract and codec coverage is at least 95 percent; application boundaries at least 90 percent; native adapters at least 85 percent; workspace line coverage remains at least 80 percent.
- Formatting, strict clippy, all tests, policy, migrations, cross-target CI, and diff checks pass.

## Open Decisions

Freeze exact runtime and model package versions before production fixtures. Approve platform-specific external-library policies before native observation can verify a package. Define model runner graph and model-load evidence in a separate ADR before effective-package v2.

The Windows boundary follows the public API contracts for
[`VirtualQueryEx`](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-virtualqueryex)
and
[`GetMappedFileNameW`](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getmappedfilenamew).
The former reports virtual-region attributes and the latter returns a mapped file
name. Neither API returns a file handle or file ID bound to the mapped section.
