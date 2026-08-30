# ADR 0007: Development host identity and hardware-probe privacy fields

- Status: accepted
- Decision owners: project maintainers
- Decision gate: milestone 0.2 required decisions
- Last reviewed: 2026-08-27

## Context

[Model and runtime support](../model-support.md) opens by stating that Retonr must work
on more than one developer workstation, and the same document later states that neither
code nor documentation assumes a developer drive, cache path, accelerator, or memory
size. The current records do not honor that intent.

Evidence gathered from a second owner-controlled machine, an AMD Ryzen 7 7840U laptop
with an integrated Radeon 780M, exposed four concrete gaps.

Artifact-inventory claims are bound to an unnamed implicit host. Statements that name
installed packages, recorded Ollama digests, an accelerator, or a model store root
appear in the readme, the roadmap build queue, and three research documents without
saying which machine produced them. A reader on a second machine cannot tell whether a
mismatch means artifact drift, a different host, or a stale record.

`QualificationStatus` has exactly two variants, `Qualified` and `Rejected`. A machine
that has never run a qualification is representable only by the absence of a record, and
absence carries no host, no date, and no reason. Never attempted is byte-identical to
attempted and lost. The `unknown` vocabulary that the research policy already reserves
for absent evidence has no place in the type.

`HardwareTier` carries `id`, `memory_mib`, and `accelerator`, validated only as bounded
text. There is no closed vocabulary and no register, so four spellings of one machine are
four distinct tiers and nothing detects the drift. The project applies the principle that
a name is an address rather than an identity rigorously to artifacts and not at all to
hosts.

`platform_digest`, `execution_class_digest`, and the qualification v2
`hardware_envelope_digest` are bare digests with no defined preimage. Nothing in the
workspace defines the canonical bytes they commit to. Two hosts produce different digests,
which correctly prevents conflation, but no reader can reconstruct what differed.

The hardware tier vocabularies also cannot describe the observed machine. One tier
describes a CPU-only laptop with 8 GB of unified memory, and another describes a
constrained integrated device. The measured host has 61.8 GB of system memory behind an
integrated GPU, which fits neither row.

A decision is required now because milestone 0.2 lists local hardware-probe privacy
fields among its required decisions, and because the multi-host evidence work cannot
proceed without settling what may be recorded about a machine.

## Decision drivers

- Multi-host coverage must be representable and queryable, not merely distinguishable.
- A probe must never require network access, telemetry, or an account.
- Recorded hardware facts must describe capability, not the identity of a person or a
  specific device.
- Anything written into a content-addressed evidence record is effectively permanent,
  because the digest is the identity and rewriting it invalidates dependent records.
- Existing v1 and schema-v4 evidence must not be rewritten.
- The decision must not weaken the existing separation between inert evidence and
  authority.

## Options considered

### Option A: a stable per-machine identifier inside the effective runtime state

Add a durable machine identifier, derived from a hardware serial, network adapter
address, installation identifier, or hostname, to `EffectiveRuntimeState`.

This makes host attribution exact and trivially queryable. It also writes a persistent
device or person correlator into a content-addressed record that is designed to be
compared and, under the portable-manifest goal, shared. A digest cannot be redacted after
the fact without invalidating every record that binds it. Retained diagnostics and
rewrite records would gain a stable cross-run correlator that the privacy documentation
does not currently permit. Rejected.

### Option B: an owner-declared host class in a separate register

Define a canonical `HostEnvironment` record describing capability classes only, hash it
into the existing `platform_digest` and `execution_class_digest`, and maintain a
versioned register that maps an owner-chosen label to those digests.

Identity remains content-addressed and portable, the preimage becomes inspectable, and
the human-facing label lives in a layer that can be revised without invalidating
evidence. The register is a documentation artifact rather than an authority, matching the
existing treatment of inert records. The cost is that a label is owner-declared and
therefore not self-verifying, so two machines could be labeled inconsistently by mistake.
That failure is visible and correctable, unlike a permanent identifier.

### Option C: leave the free-text hardware tier as it is

No new type, no register. This preserves the current shape and continues to permit
undetected drift between four spellings of one machine, keeps the digest preimages
undefined, and leaves never-attempted unrepresentable. It does not satisfy the stated
requirement that Retonr work across more than one workstation with retained evidence.
Rejected.

## Decision

Adopt the capability-record portion of Option B as a narrow, additive milestone-0.2
contract. Broader development-host registration and accelerator characterization remain
separate future decisions.

`HostEnvironmentV1` is a model-domain inert record with private fields, closed
vocabularies, bounded canonical JSON decoding, exact re-encoding, and a content-derived
identity. V1 admits only the reviewed Linux, x86-64, GNU-libc native-CPU profile. It has
four independently domain-separated typed projections plus one complete identity:

- `retonr:host-environment-operating-system:v1\0`
- `retonr:host-environment-architecture:v1\0`
- `retonr:host-environment-execution-class:v1\0`
- `retonr:host-environment-hardware-envelope:v1\0`
- `retonr:host-environment:v1\0`

Every identity hashes its domain, the big-endian `u64` byte length, and the exact compact
canonical JSON bytes. Projection types are distinct so an operating-system,
architecture, execution, or hardware digest cannot be silently transposed at a typed
boundary.

V1 permits only these capability fields:

- Linux family and the normalized kernel release
- x86-64 instruction set and GNU-libc ABI
- The closed managed-Linux native-CPU execution profile, `NativeCpu` backend, and
  `CpuOnly` placement
- Whether the observing binary included debug assertions
- An explicit statement that accelerator capability was not assessed for this profile
- A normalized CPU model class, physical core count, and logical core count
- Total system memory rounded down to a fixed 1,024 MiB granularity

The app-owned current-host observer uses bounded local reads, `uname`, process affinity,
and cgroup-v2 CPU, memory, and cpuset state. The cgroup observation requires the Linux
UAPI initial cgroup-namespace inode, an exact cgroup2 mount at `/sys/fs/cgroup` whose
filesystem root is `/`, unlimited CPU and memory controls at every membership ancestor,
an exact domain cgroup type, and a leaf effective cpuset equal to the online CPU set.
Every sensitive proc, sysfs, and cgroup file is opened relative to a retained verified
filesystem root and must report that root's kernel mount ID. The namespace link must
remain on the verified proc mount before it is followed to the expected nsfs object.
Namespace and mount scope are rechecked after the control reads. It starts no external
process, performs no network access, and fails closed when required evidence is
unavailable, ambiguous, or resource constrained. Construction requires two equal fresh
observations. Revalidation requires another two equal observations and equality to the
retained record. The opaque authority is noncloneable and nonserializable. A
debug-assertion observer may produce portable rejected evidence but can never produce
the reviewed supported result.

This record is additive. It supplies the exact preimages used by the generation
qualification system's four platform fields. It does not reinterpret or replace the
existing `EffectiveRuntimeState.platform_digest`, the Ollama execution digest, or any
legacy serialized identity.

Prohibited fields, which must never enter this record or any digest derived from it:

- Hardware serial numbers, board identifiers, or disk identifiers
- Network adapter addresses
- Operating-system installation identifiers or activation identifiers
- Hostnames, machine names, account names, or user directory paths
- Absolute filesystem paths of any kind
- Geolocation, network identity, or organization identity

A model store root remains a declared symbolic name, never an absolute path. V1 records
no model-store root at all.

Deliberately left open: a development-host register, a host-specific not-attempted
qualification state, accelerator and driver characterization, revised hardware tiers,
and any future consented richer local diagnostics. None is implied by `HostEnvironmentV1`
or required to validate its exact native-CPU qualification preimage.

## Consequences

### Positive

- Qualification platform digests gain defined, inspectable preimages without changing
  the role or bytes of older runtime digests.
- Separate typed projections prevent category transposition at app and model boundaries.
- A retained current-host capability can be freshly revalidated before and after
  pretraffic assessment.
- A reader can reconstruct what differed between two hosts from retained records.
- The privacy boundary is stated once, in a place a reviewer can check, rather than being
  decided implicitly by whichever probe is written first.

### Negative

- A new canonical encoding is one more frozen format to maintain.
- Rounding total memory to a declared granularity slightly reduces measurement fidelity,
  which is accepted because exact byte counts add fingerprinting surface without changing
  any tier decision.
- The deliberately narrow V1 profile rejects constrained, non-Linux, non-x86-64,
  non-GNU-libc, and accelerator-characterized environments instead of partially describing
  them.

### Follow-up

- Persist the canonical host preimage beside schema-7 qualification evidence without
  treating it as authority.
- Add a third qualification status only if host-stratified coverage is implemented; it
  remains outside this decision's V1 contract.
- Create a development-host register only after its authority, lifecycle, and privacy
  semantics are separately reviewed.
- Update current-state and qualification planning documents when the prepared operation
  starts retaining this host preimage.
- Add a revalidation trigger for a change to the set of locally installed artifacts. No
  such trigger exists today, so replacing the installed model set fires no named review.
- Revise the hardware tier vocabularies to admit the observed high-memory
  integrated-accelerator class.

## Validation

The accepted V1 decision is confirmed by stable identity vectors, strict canonical
round-trip and malformed-input tests, projection-isolation tests, compile-fail typed
transposition coverage, content-redacted debug output, and double-sampled current-host
revalidation. No prohibited field may appear in any canonical encoding.

Revisit the decision if a register label proves insufficient to distinguish two materially
different machines, if a permitted field is shown to identify a device or person more
precisely than intended, or if a machine-readable register becomes necessary before 0.9.

## References

- [0.2 grounded engine and CLI execution plan](../planning/0.2-grounded-cli.md)
- [Model and runtime support](../model-support.md)
- [Evaluation](../evaluation.md)
- [Local model evaluation protocol](../research/2026-08-13-local-model-evaluation.md)
- [Local runtime matrix](../research/2026-08-13-local-runtime-matrix.md)
- [External change watch and revalidation](../external-change-watch.md)
- [ADR 0003: Separate artifact, qualification, and activation identity](0003-artifact-qualification-activation.md)
