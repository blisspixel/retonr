# Security Hardening Review: Retained Ollama Connection Binding

## Evidence Basis

This review is derived from eight source and decision artifacts at revision
`a58e07473eb558e3e38aa382de59af909f4a647b`. The complete registry and collection
digest are in [context.md](context.md).

I inspected the current transport, preflight sequence, bracketed attached-process
orchestration, and native listener observers. The decisive evidence is that the
preflight sends several requests through a reusable client while the listener witness
exists outside that transport. The current report correctly records that this is not
response binding and remains unqualified.

## Constraints

We must preserve the explicit IP-literal loopback endpoint, bounded responses,
cancellation, no proxies, no redirects, no implicit retries, and read-only operation.
The attached report remains inert with `qualified: false`.

Platform claims stay deliberately narrow. Windows supplies a context-binding PID for
a TCP row, not exclusive socket ownership. Linux accepted-socket holder discovery is
limited to same-UID descriptors visible under the current proc and namespace policy.
The first slice requires exactly one visible holder and fails closed on observed
permission or enumeration gaps, but cannot prove that no invisible holder exists.
macOS has no supported public mechanism in the current design and remains
unsupported. Point-in-time observations
do not prove continuous exclusive ownership, application-handler execution, or lack
of proxying by the immediate peer.

No measured latency or memory budget was supplied, so resource effects remain
source-derived or hypothetical and have explicit benchmark plans.

## Opportunity Portfolio

| Opportunity | Evidence | Options | Recommendation | Proposal |
| --- | --- | --- | --- | --- |
| Bind read-only preflight traffic to one retained connection | Pooled transport, multi-request preflight, inert process witness, Windows and Linux native observers, macOS unsupported boundary (`E01` through `E08`) | Option 1: pooled snapshots; Option 2: single retained session; Option 3: managed runtime chain | Select Option 2 for the next bounded slice | [Retained connection attribution](proposals/retained-connection-attribution.md) |

## Recommendation Summary

I recommend Option 2, one retained HTTP/1 session with point-in-time kernel
attribution before traffic and after every complete response. It gives the preflight
an owned connection capability and removes implicit reconnect ambiguity without
turning the product into a runtime manager. The cost is stricter availability and
more native observation work: a server close, incomplete owner view, or attribution
deadline becomes a terminal failure.

Option 1 is reasonable only if compatibility and delivery speed matter more than
response attribution. It can narrow listener-drift windows, but it cannot identify
which pooled connection carried a response. Option 3 becomes preferable only after
the project chooses to own runtime packages, launch, lifetime, recovery, and
isolation. That authority expansion is not justified for the current read-only
user-managed preflight.

The selected option improves evidence quality but does not establish qualification.
Its successful report must continue to explain the platform attribution scope and set
`qualified` to false.

## Next Decisions

- Set an owner-observation latency budget for one-model and eight-model preflights.
- Confirm the supported Linux proc visibility profiles.
- Decide whether IPv4 and IPv6 enter the first native slice together.
- Keep macOS unsupported until a reviewed public mechanism exists.
- Review the [implementation plan](implementation/single-retained-session.md) before
  source changes begin.
