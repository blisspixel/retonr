# Security Hardening Review: Effective Runtime Trust Chain

## Evidence Basis

This review is derived from 18 repository artifacts at revision
`c3657edd126f164facc311719b831d6926e7c06d` and current primary Linux, Windows,
macOS, and Ollama documentation. The complete registry and integrity record are in
[context.md](context.md).

We already have a useful retained-connection preflight, canonical generic byte sets,
and inert runtime identity contracts. The decisive gaps occur where those pieces are
joined. Linux still obtains socket rows from a deprecated proc table. Static package
membership is not separated from observed load state. Provider cloud configuration
cannot enforce network denial, and a running attached service cannot be moved into a
race-free cross-platform isolation boundary.

## Constraints

The selected design must preserve local-first and offline-after-setup behavior,
versioned evidence, deterministic validation, bounded native inputs, explicit
cancellation, and current redaction. It must not rewrite existing version 1 identity
bytes or promote an existing inert preflight into qualification.

The design also has to be honest about platform capability. Linux can support the
first managed isolation slice when namespace policy permits. Windows needs an
administrator-controlled WFP guardian. macOS needs a signed, entitled, approved
Network Extension and dedicated physical validation. No paid cloud inference,
artifact acquisition, or hosted testing is authorized by this review.

## Opportunity Portfolio

| Opportunity | Evidence | Options | Recommendation | Proposal |
| --- | --- | --- | --- | --- |
| Replace deprecated Linux TCP row acquisition | Linux listener and retained-connection proc readers, bounded witness contract, kernel socket diagnostics (`E01` through `E05`) | Option 1: retain proc tables; Option 2: bounded dump only; Option 3: split listener dump and exact connection query | Select Option 3 | [Linux socket diagnostics](proposals/linux-socket-diagnostics.md) |
| Separate static package identity from observed runtime use | Generic byte set, opaque identity inputs, managed attestation and lease gaps (`E06` through `E10`, `E13` through `E15`) | Option 1: keep opaque digests; Option 2: typed package manifests plus native load observation | Select Option 2 | [Package and load closure](proposals/package-and-load-closure.md) |
| Make local-only qualification enforceable | Bound preflight limitations and milestone isolation requirement (`E11`, `E12`, `E16` through `E18`) | Option 1: provider declaration only; Option 2: retrofit attached-process filtering; Option 3: managed prelaunch isolation | Select Option 3 | [Managed local-only isolation](proposals/managed-local-only-isolation.md) |

## Recommendation Summary

I recommend the three selected options as one dependency-ordered trust chain. First,
we should replace only the Linux kernel row acquisition layer while preserving its
visible-holder limitation. Second, we should keep the generic artifact set and add
typed runtime and model package overlays plus a separate native load observation.
Third, we should permit local-only qualification only through a managed prelaunch
isolation lease.

The attractive part of this sequence is that each claim gains one clear owner. The
kernel observer owns socket and process facts. Static manifests own portable byte and
role identity. The native load observer owns point-in-time loaded-file evidence. The
isolation guardian owns outbound policy and process-tree lifetime. Evaluation only
consumes their exact joined identities.

What gives us pause is cross-platform isolation parity. Linux is implementable first
at no paid cost, but host user-namespace policy can still make it unsupported. Windows
requires elevated policy authority. macOS requires a separately distributed system
extension. We should preserve these as explicit capability results and never fall
back to an unisolated generation path.

## Next Decisions

- Complete and verify the selected Linux socket-diagnostics implementation.
- Freeze the typed package contracts and golden identities before adding persistence.
- Select one exact runtime artifact before reviewing an Ollama version allowlist or
  Windows child-executable closure.
- Treat Linux managed isolation as the first enforcement implementation; keep Windows
  and macOS qualification disabled until their native prerequisites are proven.
- Do not reacquire a model or run generation without the separate approval already
  required by the roadmap.
