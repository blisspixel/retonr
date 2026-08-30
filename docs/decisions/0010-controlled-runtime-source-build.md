# ADR 0010: Controlled source build for first runtime admission

- Status: proposed
- Decision owners: project maintainers
- Decision checkpoint: roadmap milestone 0.2 runtime admission
- Last reviewed: 2026-08-25

## Context

Milestone 0.2 requires one exact local runtime package to pass source lineage,
native closure, managed startup, cloud-disable, isolation, and qualification gates
before production generation is enabled.

The frozen official Ollama v0.32.15 Linux x86-64 archive is useful transformation
and license evidence, but it cannot satisfy the current source-to-binary lineage
control. Its tagged release workflow disables provenance and SBOM output. Its build
uses mutable image tags and package resolution that do not independently bind the
published archive to the tagged source. Exact startup tests cannot repair that gap.

The implementation already contains a bounded Ollama transport, package model,
managed Linux isolation, native observation, and an admission-gated retained
generation operation. Replacing those proven contracts without first testing a
controlled source build would discard substantial relevant evidence.

## Decision drivers

- Do not weaken an admission claim to preserve a preferred runtime.
- Bind executable bytes to reviewed source and a retained build closure.
- Preserve local-first and offline-after-setup operation.
- Keep opaque vendor artifacts useful as inert evidence without granting authority.
- Avoid duplicating runtime policy between review records and production code.
- Retain a proportionate fallback if the Ollama build closure cannot be frozen.

## Options considered

### Admit the official archive from its checksum

The release asset and checksum are exact, and the signed source tag is identifiable.
Those facts do not bind the binary to that source or identify every effective build
input. Selecting this option would require a lower-assurance policy and would weaken
the current source-lineage claim.

### Build an exact CPU-only Ollama package under Retonr control

Fetch and verify a frozen input bundle, then build with network access disabled. Bind
the result to the tagged source, embedded llama.cpp revision, Go toolchain, complete
Go module content, OCI base, native packages, compiler and linker tools, build flags,
environment, licenses, transformation, and final member bytes. Retain provenance,
an SBOM, and independent rebuild comparison evidence.

This preserves the implemented Ollama trust chain while replacing the opaque binary
root with evidence Retonr can review directly. It adds build and maintenance cost.

### Select the pinned llama.cpp sidecar fallback

Build a smaller sidecar around a pinned llama.cpp revision and qualify it through the
same package, isolation, transport, and evidence requirements. This may simplify the
native closure, but it requires a new adapter and discards part of the completed
Ollama-specific integration.

## Decision

Proceed with a Retonr-controlled, CPU-only Ollama source build for the first Linux
x86-64 GNU libc admission candidate.

Preserve the official v0.32.15 archive review as a non-admitted historical record.
Do not rewrite its blocked controls or treat its exact checksum as source lineage.

Add an additive runtime-package review schema for controlled source builds. The
existing schema remains readable and retains its current semantics. The new review
must consume the evidence bundle, validate every referenced file digest, reconstruct
the final layout, derive its exact `RuntimePackageManifestId`, and emit one canonical
content-free admission record only when every required control passes.

The frozen input closure must include:

- the exact Ollama source revision and source archive;
- the exact embedded llama.cpp revision;
- the Go toolchain archive and complete module content and checksum set;
- every OCI base by digest;
- every native package by exact identity and payload digest;
- CMake, Ninja, the retained POSIX shell, compiler, assembler, linker, and
  relevant standard-library inputs;
- build arguments, target triple, CPU feature policy, locale, timezone, and source
  epoch;
- the exact Retonr isolation helper and its build evidence;
- license and source dispositions for every selected code component.

The build has a network-enabled fetch phase that produces the frozen input bundle
and a separate network-disabled build phase that may read only that bundle. The
build records whether independent rebuilds have identical runtime bytes, structure,
and portable ordinary Unix permission bits. The canonical app-owned output-tree
sidecar binds those original facts to file sizes and digests independently of durable
container permissions. Set-user-ID, set-group-ID, and sticky bits fail closed before
output commitment or export. Reproducibility is an observed result, not an assumed
guarantee.

The offline build uses controlled-build capability ABI 2. Each attempt creates a
private mount namespace with recursively private propagation and a fresh 16 GiB,
262,144-inode tmpfs at `/tmp`. The byte quota reserves 4 GiB of bounded build scratch
beyond the 4 GiB private-input and 8 GiB committed-output ceilings. It transfers a
bounded exact path-to-retained-file map,
constructs a normalized private input tree, copies every member at its declared path,
verifies exact byte count, SHA-256, trailing EOF, file type, link count, and retained
identity, and remounts the complete private input root read-only. The aggregate input
is capped at 4 GiB before private-file creation. It creates the fixed output alias as
a distinct initially empty directory in that private tmpfs. The caller's input root
is not passed to the helper or target, and the target cannot access the distinct
retained host output. Before the first probe, the parent also copies the exact
reviewed helper into an executable anonymous file, verifies its declared size and
SHA-256, and applies immutable write and size seals. The parent independently observes
the mount namespace before execution, while the helper and static source builder
perform separate checks. The helper revalidates the retained program, mapped input
files, private aliases, and host-output object. The builder revalidates the capability
ABI, input and private-output descriptor-to-alias identities, and output working
directory. Landlock ABI 3 permits exact read and write access to `/dev/null`, input
reads and execution, and private-output writes while denying ambient host reads and
other device files.

The exact private snapshot and read-only remount close pathname substitution,
same-inode mutation, and target-side write access for bytes consumed by the build.
Pre-run and post-run retained-boundary revalidation still rejects later stable drift.
These mechanisms prove exact byte and boundary behavior, not source lineage,
transformation correctness, license disposition, reproducibility, or semantics.

Stage two reports completion only after the coordinator is reaped and descendant-held
diagnostic streams drain. The guardian then commits a private tree bounded to 4,096
entries and 8 GiB of regular-file bytes, exports it from retained file handles into the
initially empty host output, rehashes during copy, and requires the host tree to match
the commitment exactly. The application independently enumerates and rehashes that
host tree and retains its own handles and snapshots. This evidence seal detects drift;
it is not an operating-system write lock. Export is not atomic, so any partial host
tree left by a failed export remains inert.

External platform components are discovered only to propose a frozen set. A second
verification run consumes that reviewed set and fails on every added, removed, or
changed executable mapping. Discovery output never admits itself.

Production policy is generated or validated from the admitted record. A manually
duplicated version or package identifier cannot independently grant execution.

If a bounded spike cannot freeze the CPU build closure or produce a reviewable
rebuild result, stop Ollama admission work and bring the pinned llama.cpp sidecar
option back for decision. Do not lower the source-lineage control.

## Consequences

### Positive

- Runtime admission remains consistent with the repository's exact-identity claims.
- The existing Ollama isolation and retained-transport work remains useful.
- Vendor release archives remain usable as research evidence without becoming an
  authority shortcut.
- Production admission can be reproduced from one canonical evidence bundle.

### Negative

- Retonr owns a native build pipeline and its supply-chain review burden.
- Source builds and independent comparisons require more CI time and retained
  storage than archive reconstruction.
- Dynamic GNU libc components still require an exact qualification-host policy.
- A later Ollama upgrade repeats the complete closure and execution review.

### Follow-up

- Retain the implemented additive review, bundle, two-attempt build, and report
  contracts around the real frozen input closure completed on 2026-08-26.
- Retain the implemented static Ollama offline source builder and developer-only
  archive-normalization and manifest-compilation tools. The independently reacquired
  bundle with historical artifact-set ID
  `54c861ef10e4e4f912bfa1759c697a62937b69b79c9922407112dd9ef6ddf440`
  was removed by a later workspace build-cache cleanup and is not current evidence.
- Produce and independently reacquire a fresh typed-receipt root, then compile the
  implemented blocked schema-2 build-stage review from that retained root,
  then separately adjudicate source lineage, transformation, license, frozen native
  closure, managed startup, and cloud disable from typed retained evidence.
- Retain the implemented non-authoritative external-component discovery and separate
  frozen-set verification contracts. Run both operations for the real candidate.
- Retain the implemented non-authoritative managed review runner for startup,
  cloud-disable, isolation, native load, retained loopback traffic, and final
  reobservation. Add top-level launch, teardown, and durable evidence composition.
- Place the resulting durable evidence outside short-lived workflow artifacts.
- Exercise the composed managed authorization paths above their critical coverage
  target before admitting the package.

## Validation

This decision is validated when:

- malformed, missing, duplicate, reordered, oversized, and digest-drifting bundle
  inputs fail before build or admission;
- a capability ABI mismatch, fixed-alias substitution, descriptor, path, or
  working-directory identity mismatch, parent or helper mount-namespace mismatch,
  program or root replacement, a nonempty output root, or target access to the
  retained host output fail before execution;
- the private output cannot exceed its tmpfs byte or inode quota, and a committed tree
  cannot exceed 4,096 entries or 8 GiB of regular-file bytes;
- the coordinator is reaped and descendant-held diagnostic streams drain before the
  guardian commits output;
- the guardian commitment, verified host export, and independent application rehash
  agree exactly, while substitution or later drift fails closed;
- the build phase completes with outbound networking unavailable;
- two isolated builds produce a retained comparison with every difference
  dispositioned;
- sustained child output is drained without deadlock, successful child output is
  discarded, failures retain only 16 KiB tails per stream, and the outer boundary
  enforces its 32 KiB prefix per stream;
- the final layout and runtime-package identities are derived from verified bytes;
- an unexpected external object, helper, child executable, native backend, or
  platform component fails closed;
- managed startup observes the exact cloud-disabled marker, retained loopback
  request, isolation state, native closure, and clean teardown;
- production policy cannot diverge from the admitted record unnoticed.

Revisit this decision if the controlled Ollama build cannot meet these gates within
the bounded spike, if upstream publishes independently adequate provenance, or if a
pinned llama.cpp sidecar provides equivalent evidence with materially less risk.

## References

- [Current v0.32.15 review](../reviews/runtime-packages/ollama-v0.32.15-linux-x86_64-gnu/README.md)
- [0.2 execution plan](../planning/0.2-grounded-cli.md)
- [Ollama v0.32.15 release workflow](https://github.com/ollama/ollama/blob/v0.32.15/.github/workflows/release.yaml)
- [Ollama v0.32.15 Dockerfile](https://github.com/ollama/ollama/blob/v0.32.15/Dockerfile)
- [Ollama cloud-disable configuration](https://github.com/ollama/ollama/blob/v0.32.15/envconfig/config.go)
