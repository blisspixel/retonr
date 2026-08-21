# Implementation Plan: Managed Prelaunch Isolation

## Implementation Status

The Linux isolation, retained-handle launch, bounded startup-stream, single
namespace-local channel, process attestation, and native-load seams are implemented.
The development-only `rewrite-eval` library now joins retained runtime-package,
isolation, process, connection, provider-declaration, read-only API, and native-load
evidence into one inert managed preflight. It reobserves the live boundaries, closes
the process tree, and never falls back to attached mode. Its report explicitly does
not prove model use, effective-runtime identity, or qualification, and it has no CLI
surface. Windows and macOS return unsupported. The production reviewed-runtime
allowlist is empty, so the provider disposition remains unreviewed. Exact
v0.32.15 static model binding and a retained-session local-judge transport receipt are
implemented separately, but neither is joined to this managed report. The static
model binding consumes an opaque, nonserializable, single-use receipt from the exact
preflight runner. Retained-session completions enforce an absolute 4 MiB UTF-8 input
ceiling before wire serialization or completion traffic. An opt-in managed outcome
also constructs package-declared typed runtime-build identity after the exact managed
package, process, and native-load join. Only the exact package entrypoint is joined to
live process and load evidence; target, revision, and other package semantics are not
independently live-observed. Cleanup completes before return, and the binding
explicitly lacks generation-bound provider, effective configuration, platform and
driver, compute placement, effective context, and retained-live-runtime evidence. A
separate opt-in v0.32.15 completion receipt proves stable runtime-reported residency
only. A retained managed execution that joins those relationships plus a
candidate-generation receipt remains downstream work.

## Selected Design And Constraints

Local-only eligibility requires managed prelaunch OS isolation plus separate exact-version provider declaration evidence. Attached processes remain observation-only. Linux is the first implementation target; Windows follows after executable-child closure; current macOS CLI builds return unsupported.

## Source Revision And Drift Check

The plan is anchored to Git revision `c3657edd126f164facc311719b831d6926e7c06d` and evidence collection SHA-256 `6a9d8e2195834f5d123add1626048d468b1b8bfd68eedb8bfaae022efa59ec3c`. Recheck Ollama version/config/status behavior for every reviewed version, and revalidate platform privilege and lifecycle APIs before enabling a new platform. A version floor alone never admits a runtime.

## Affected Components

- `crates/ollama`: inert exact-version cloud-disable declaration and startup-marker evidence.
- New `crates/runtime-isolation`: platform-neutral prepare, launch, lease, reobserve, and close contracts.
- Linux adapter: namespace creation, loopback setup, ambient descriptor close-on-exec sealing and stage-two closure, capability reduction, target-inherited seccomp socket policy, launch, and retained lifecycle.
- Later Windows adapter: dynamic WFP policy plus no-breakaway kill-on-close Job.
- `crates/app` and `crates/eval`: later composition with package, process, isolation, and evaluation evidence.

## Ordered Work Packages

1. Implement strict Ollama version parsing, reviewed-version policy injection, managed environment declaration, bounded startup-marker parsing, and redacted inert evidence. Keep the production allowlist empty.
2. Define an isolation policy limited to loopback and a retained lease that owns both policy and process-tree lifecycle.
3. Implement capability probing and deterministic unsupported results.
4. Implement Linux namespace setup before runtime execution, bring up loopback only, seal ambient descriptors as close-on-exec and verify stage-two closure, set no-new-privileges, reduce capabilities, install the target-inherited seccomp policy, and launch the verified package.
5. Run local allow, IPv4 deny, IPv6 deny, host pathname `AF_UNIX`, `AF_VSOCK`, and `io_uring_setup` canaries, then reobserve namespace, seccomp mode, process tree, provider evidence, and policy lifecycle.
6. Bind the results into an inert managed-runtime report without changing attached preflight reports.
7. Add Windows only after all possible runtime child executables are derived from a frozen package manifest.
8. Keep macOS unsupported until a separately approved signed, entitled Network Extension design exists.

## Compatibility And Migration

The provider evidence and isolation interfaces are additive. Existing Ollama backend APIs and v1 preflight reports remain unchanged. No user configuration file is read or changed. Existing attached-process reports stay readable and unqualified. No migration grants authority to historical data.

## Tactical Protections During Migration

The provider evidence type always carries false network-isolation and qualification fields. The production reviewed-version allowlist starts empty and must be changed through reviewed source. Isolation capability failure never falls back to the host network. The runtime process does not resume or enter its main code path before the namespace and seccomp boundaries are active. The seccomp policy permits only `AF_INET` and `AF_INET6` through `socket()`, denies every other socket family and `io_uring_setup`, and must remain in mode 2 during target reobservation.

## Tests And Security Validation

Provider fixtures cover malformed and prerelease versions, pre-feature versions, feature-floor but unreviewed versions, future unreviewed versions, version drift, missing and conflicting environment values, missing, duplicate, conflicting, and oversized markers, and output redaction. Linux native tests verify loopback access, IPv4 and IPv6 non-loopback denial, inability to reach a visible host pathname Unix listener, exact `AF_VSOCK` and `io_uring_setup` denial, inherited-descriptor closure, child and guardian lifecycle, capability reduction, seccomp and namespace reobservation, cancellation, and deterministic unsupported policy. Tests use local canaries and require no Internet.

## Performance And Resource Benchmarks

Measure namespace preparation, managed launch latency, canary completion, reobservation, and teardown. Establish a bounded launch deadline and confirm cleanup after every failed stage. For later Windows work, measure WFP classification overhead and filter installation/readback latency before adoption.

## Rollout And Rollback

Ship provider evidence first with no reviewed production version, then Linux isolation behind explicit capability detection. Admit one frozen runtime version only after exact source, package, marker, and native isolation fixtures pass. Rollback removes that reviewed entry or disables managed isolation; no attached path is promoted as a fallback.

## Acceptance Criteria

- Provider declaration is exact-version, bounded, redacted, and inert by itself.
- Production reviewed-version policy is explicit and source-controlled.
- The OS boundary exists before managed runtime execution.
- The target inherits a seccomp mode 2 socket policy that permits only `AF_INET` and
  `AF_INET6` through `socket()` and denies every other family plus `io_uring_setup`.
- The admitted process tree cannot break away and dies when the lease closes.
- Loopback succeeds and IPv4/IPv6 non-loopback canaries fail without Internet access.
- Policy and process identity reobserve without drift.
- Attached mode remains unqualified on every platform.
- Unsupported host conditions return deterministic unsupported results.
- Focused and workspace formatting, strict clippy, tests, policy, cross-platform CI, and at least 80 percent line coverage pass.

## Open Decisions

Choose whether Linux hosts without unprivileged namespace support require a narrow elevated helper or remain unsupported. Define the Windows executable-child closure and administrator installation flow before WFP implementation. A macOS product and signing decision is required before any qualified path can be planned there.
