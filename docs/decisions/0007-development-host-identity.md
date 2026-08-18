# ADR 0007: Development host identity and hardware-probe privacy fields

- Status: proposed
- Decision owners: project maintainers
- Decision gate: milestone 0.2 required decisions
- Last reviewed: 2026-08-17

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

Adopt Option B, with an explicit field policy.

Define a `HostEnvironment` record in the model domain, following the pattern already used
by `EffectiveRuntimeState`: private fields, closed vocabularies, a bounded decoder, a
fixed canonical encoding, and a frozen digest. It becomes the defined preimage of
`platform_digest` and `execution_class_digest`, and of the qualification v2
`hardware_envelope_digest`.

Permitted fields describe capability only:

- Operating-system family and version string
- Architecture and application binary interface
- CPU model string, physical core count, and logical core count
- Total system memory, rounded to a declared granularity
- Accelerator model string, reported memory, and device class
- Memory model, either unified and shared or dedicated
- Compute backend and execution placement, reusing the existing closed vocabularies
- Backends that were present but rejected, with the reported reason
- Driver or runtime library version strings
- Binary profile, because a debug build measured artifact import 14.5 times slower than
  a release build on the same host and the same bytes

Prohibited fields, which must never enter this record or any digest derived from it:

- Hardware serial numbers, board identifiers, or disk identifiers
- Network adapter addresses
- Operating-system installation identifiers or activation identifiers
- Hostnames, machine names, account names, or user directory paths
- Absolute filesystem paths of any kind
- Geolocation, network identity, or organization identity

A model store root is recorded as a declared symbolic name, never as an absolute path,
consistent with the existing rule that portable manifests store normalized relative paths
rather than a machine-specific root.

Add a `development-hosts` register document holding one entry per owner-controlled host.
Each entry declares a stable label matching the lowercase identifier convention already
used for fixture and category names, an entry version, an observation date, and the
permitted fields above. Every artifact-inventory table, hardware tier, bakeoff budget, and
resource claim references an entry label and version instead of asserting an unnamed
current machine.

Add a third `QualificationStatus` variant meaning not attempted on this host, so coverage
is representable rather than inferred from absence. The variant carries no authority: it
can never satisfy an activation check, and existing serialized records remain readable.

Deliberately left open: whether the register is machine-readable in 0.2 or remains
documentation, how tier vocabularies are revised to admit a high-memory integrated-GPU
class, and whether a future consented opt-in permits richer local diagnostics. None of
those block the record or the status variant.

## Consequences

### Positive

- Results from separate owner-controlled machines coexist as strata rather than
  overwriting one another.
- Qualified on host A and not yet run on host B becomes a statement the evidence model can
  make.
- The opaque digests gain a defined, inspectable preimage without changing their role.
- A reader can reconstruct what differed between two hosts from retained records.
- The privacy boundary is stated once, in a place a reviewer can check, rather than being
  decided implicitly by whichever probe is written first.

### Negative

- Host labels are owner-declared and therefore not self-verifying.
- A new canonical encoding is one more frozen format to maintain.
- Rounding total memory to a declared granularity slightly reduces measurement fidelity,
  which is accepted because exact byte counts add fingerprinting surface without changing
  any tier decision.

### Follow-up

- Add the `HostEnvironment` record with round-trip, bounded-decoder, closed-vocabulary,
  and canonical-encoding tests matching the existing identity records.
- Add the third qualification status with a test proving it cannot authorize activation.
- Create the register with entries for both current development hosts.
- Update the readme, roadmap build queue, and the three research documents that assert an
  unnamed current machine.
- Add a revalidation trigger for a change to the set of locally installed artifacts. No
  such trigger exists today, so replacing the installed model set fires no named review.
- Revise the hardware tier vocabularies to admit the observed high-memory
  integrated-accelerator class.

## Validation

The decision is confirmed when two register entries exist, a qualification record on each
host references its entry, a report can state coverage per host without inferring anything
from a missing record, and no prohibited field appears in any canonical encoding.

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
