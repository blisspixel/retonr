# Security Hardening Proposal: Managed Local-Only Isolation

## Decision

We will treat Ollama cloud-disable configuration as provider declaration evidence, never as network enforcement. Local-only qualification will require a managed prelaunch process tree inside an OS-enforced loopback-only boundary. Attached user-managed processes remain observation-only.

## Executive Recommendation

There is no honest portable way for this CLI to retrofit complete isolation onto an arbitrary running process tree. Preexisting connections, inherited handles, unknown child executables, local brokers, and platform privilege models leave gaps. A managed lifecycle lets us establish the boundary before executable code runs, retain the process tree and policy objects together, run local allow and non-loopback deny canaries, and fail closed on drift.

## Evidence

- E10 and E11, Ollama preflight contracts and code: verify a loopback endpoint and provider behavior but do not enforce outbound denial.
- E12, bound eval command: ties one retained transport to process evidence and deliberately remains unqualified.
- E15 through E18, architecture and roadmap: require isolation, package/load closure, and evaluation before qualification.
- P03, Ollama source and documentation: `OLLAMA_NO_CLOUD=1` disables documented cloud features and requires restart.
- P04, Linux network namespaces: can give a managed process tree a loopback-only network view when host policy permits setup.
- P05, Windows Filtering Platform and Job Objects: can enforce outbound filters and process-tree lifecycle with administrator-controlled setup.
- P06, Apple Network Extension: requires a signed, entitled, approved system extension for durable per-process flow control.

The evidence inventory and primary links are recorded in [context.md](../context.md).

## Current Design And Failure Mode

The current bound preflight establishes that a retained local TCP connection was repeatedly attributed to one observed process under platform-specific limitations. It does not control that process's other sockets. A cloud-disable flag narrows provider behavior, but it is not a general network policy and cannot prevent other code paths, dependencies, or compromised runtime code from connecting outward.

Retrofitting a filter after attachment also begins too late. Existing connections can survive policy installation, children can already exist, and platform policy selectors often identify an executable or application rather than one exact PID. The resulting evidence could look local-only while leaving an unobserved route.

## Desired Invariants

1. The network boundary is active before any managed runtime instruction executes.
2. The complete admitted runtime process tree cannot escape the lifecycle or policy boundary.
3. Loopback traffic required for local inference succeeds, while direct IPv4 and IPv6 non-loopback canaries fail.
4. Provider cloud-disable evidence is bound to an exact reviewed runtime version, managed environment, and startup marker.
5. Provider declaration and OS isolation remain separate evidence, and neither alone qualifies a run.
6. Policy, process, version, and package drift fail closed before evaluation.
7. Unsupported privilege or platform conditions return a deterministic unsupported result without weakening the claim.

## Constraints And Non-Goals

The design remains local-first and requires no Internet for validation. It does not retrofit arbitrary attached processes, promise an unprivileged path on every host, or treat a semver floor as a reviewed-version allowlist. It does not acquire models or runtimes automatically. macOS qualification requires a separately shipped signed and entitled component, so the current CLI-only path remains unsupported there.

## Before Architecture

[Before architecture](../diagrams/managed-local-only-isolation-before.mmd)

The current system observes a local listener and process while the host network remains available. This correctly produces an unqualified report.

## Options

### Option 1: Provider Declaration Only

[After architecture](../diagrams/managed-local-only-isolation-provider-declaration-only-after.mmd)

This option sets `OLLAMA_NO_CLOUD=1`, admits only reviewed versions, and verifies the startup marker. It is valuable provider evidence but deliberately leaves `network_isolation_enforced` false and cannot qualify.

| Dimension | Delta from before | Basis | Confidence | Validation |
| --- | --- | --- | --- | --- |
| Security | Narrows documented provider features only | P03 | High | Version and marker fixtures |
| Performance | Negligible | Simple parsing | High | Microbenchmark |
| Operability | Easy managed environment setup | P03 | High | Process fixture |
| Compatibility | Requires reviewed Ollama versions | P03 | Medium | Exact-version matrix |
| Maintainability | Small explicit contract | P03 | High | Unit coverage |
| Delivery risk | Low | No privileged integration | High | Existing CI |

### Option 2: Retrofit Attached Filtering

[After architecture](../diagrams/managed-local-only-isolation-retrofit-attached-filtering-after.mmd)

This option attaches platform filters to an already running listener owner. It cannot reliably close preexisting flows, inherited descriptors, unknown child processes, brokered traffic, or selector gaps. It may reduce risk operationally but cannot satisfy the proposed qualification invariant.

| Dimension | Delta from before | Basis | Confidence | Validation |
| --- | --- | --- | --- | --- |
| Security | Partial, race-prone containment | P04-P06 | High | Preexisting-flow negative tests |
| Performance | Platform-specific filter overhead | P05-P06 | Medium | Native benchmarks |
| Operability | Privilege and policy conflicts | P05-P06 | High | Clean-host install tests |
| Compatibility | Lowest parity across platforms | P04-P06 | High | Platform matrix |
| Maintainability | Three unrelated retrofit paths | P04-P06 | High | Architecture review |
| Delivery risk | High with weak claim | Platform constraints | High | Threat-model review |

### Option 3: Managed Prelaunch Isolation

[After architecture](../diagrams/managed-local-only-isolation-managed-prelaunch-isolation-after.mmd)

This option prepares an OS boundary, launches the verified runtime inside it, retains policy and process-tree lifecycle together, verifies the provider declaration, and runs offline canaries. Linux begins with a new loopback-only network namespace. Windows follows with dynamic WFP filters plus a no-breakaway kill-on-close Job. macOS remains unsupported until an entitled Network Extension product path exists.

| Dimension | Delta from before | Basis | Confidence | Validation |
| --- | --- | --- | --- | --- |
| Security | Boundary exists before runtime execution | P04-P06 | High | Escape, inherited-handle, and canary tests |
| Performance | Namespace negligible; filter cost measured per platform | P04-P06 | Medium | Native latency and launch benchmarks |
| Operability | Host policy or privilege required | P04-P06 | High | Capability probe and clear unsupported result |
| Compatibility | Linux first, Windows next, macOS gated | P04-P06 | High | Explicit support matrix |
| Maintainability | One narrow interface with platform adapters | Proposed contract | Medium | Interface and lifecycle tests |
| Delivery risk | Medium-high, staged by platform | Platform constraints | High | Linux pilot before broader rollout |

## Comparison

Option 1 is necessary but insufficient. Option 2 creates a costly platform surface without a defensible complete-boundary claim. Option 3 is the only design that can make prelaunch and process-tree lifecycle explicit. Its lack of universal unprivileged support is a product limitation to publish, not a reason to weaken the evidence class.

## Recommendation

We select Option 3 and retain Option 1 as one required evidence input. The first production slice is Linux managed network-namespace isolation. Windows is gated on a complete executable-child closure and administrator-controlled WFP lifecycle. macOS remains unsupported for qualification.

## Evidence Coverage And Residual Risk

Primary platform documentation establishes the control surfaces and privilege constraints. Residual risk includes kernel or privileged-service compromise, indirect host services that are intentionally admitted, unknown executable children on Windows, and deployment policy that disables namespace creation. The first policy admits loopback only and closes inherited descriptors to avoid broker exceptions.

## Migration And Rollout

We first ship inert provider declaration evidence with an empty production reviewed-version allowlist. We then add a new narrow runtime-isolation crate and Linux adapter behind explicit capability detection. No attached report changes authority. Qualification remains impossible until package/load closure, isolation reobservation, and locked evaluation are all present. Rollback disables the managed isolation feature and leaves existing evidence readable and inert.

## Validation Plan

Provider tests cover exact version parsing, prerelease and unsupported versions, missing or conflicting environment declarations, startup-marker drift and bounds, and redaction. Isolation tests close inherited descriptors, attempt IPv4 and IPv6 non-loopback connections, verify loopback service access, test child escape and guardian death, reobserve namespace and process identity, exercise cancellation and time limits, and run without Internet. Cross-platform compile gates must preserve deterministic unsupported behavior.

## Implementation Work Packages

1. Add inert version-gated provider declaration evidence with an empty reviewed allowlist.
2. Define `prepare`, `launch`, retained-lease, `reobserve`, and close semantics in a platform-neutral crate.
3. Implement Linux managed loopback-only namespace setup and lifecycle retention.
4. Bind exact package, process tree, provider declaration, isolation checks, and canary results into an inert report.
5. Implement Windows only after executable-child closure is frozen; keep macOS unsupported.
6. Promote authority only with effective identity and locked evaluation evidence.

The executable handoff is [managed-prelaunch-isolation.md](../implementation/managed-prelaunch-isolation.md).

## Open Questions

The product must choose whether Linux hosts that disable unprivileged user namespaces will require a narrow elevated helper or return unsupported. Windows needs an approved child-executable closure before WFP application-identity filters can support the intended process-tree claim.
