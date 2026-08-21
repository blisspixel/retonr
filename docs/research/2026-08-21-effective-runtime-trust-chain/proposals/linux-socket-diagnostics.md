# Security Hardening Proposal: Linux Socket Diagnostics

## Decision

We will replace Linux `/proc/net/tcp*` row acquisition with one retained `NETLINK_SOCK_DIAG` session. Listener discovery will use a bounded dump, while retained connections will use exact kernel queries with a retained socket cookie. We will preserve the existing visible same-UID descriptor-holder scan and its explicit visibility limitation.

## Executive Recommendation

The split design is the smallest change that materially narrows socket-reuse and namespace-drift risk without overstating what an unprivileged Linux process can observe. It preserves the current public evidence schema and remains unqualified. It does not prove exclusive ownership or application handler execution.

## Evidence

- E01, current Linux listener adapter: reads `/proc/net/tcp*` and joins rows to visible descriptor links.
- E02, current retained-connection adapter: repeats text-table tuple and inode matching before scanning visible holders.
- E03, public connection evidence: deliberately limits its claim to visible same-UID descriptor holders.
- E04, runtime-attestor lease contract: pins the process incarnation but does not pin the network namespace through the socket-table reader.
- P01, Linux netlink UAPI: defines message framing, dump interruption, acknowledgement, and sender fields.
- P02, Linux inet-diag UAPI and TCP implementation: supports listener dumps, exact tuple lookup, and socket-cookie validation.

Repository evidence E01 through E04 and primary evidence P01 through P02 are indexed in [context.md](../context.md).

## Current Design And Failure Mode

The current adapter opens the process-visible TCP tables for each observation, parses every admitted row, matches a reverse tuple, then bridges the socket inode to visible same-UID descriptor links. Bounds and fail-closed behavior are already useful. The missing control is stable kernel socket identity: a tuple and inode can be observed at different points, while callback execution can occur after namespace drift.

The practical failure mode is not that `/proc` is always wrong. It is that the evidence cannot reject every close-and-reuse event within its observation window, and its socket-table namespace is not retained as an object. A process could therefore satisfy point-in-time tuple checks without preserving the same kernel socket identity across the response sequence.

## Desired Invariants

1. Every listener and connection result comes from the network namespace selected when the lease is created.
2. A retained connection is reobserved by exact reverse tuple and the same kernel socket cookie.
3. Listener dumps are accepted only after a complete `NLMSG_DONE` result with no interruption, overrun, truncation, or malformed frame.
4. Exact queries are accepted only after one matching data record and one successful acknowledgement.
5. Descriptor-holder enumeration remains bracketed by socket observations and fails closed on any drift.
6. Published evidence contains no address, port, inode, UID, cookie, sequence, namespace identifier, errno, or kernel message.

## Constraints And Non-Goals

We retain the current byte, row, process, descriptor, retry, cancellation, and elapsed-time ceilings. We add no dependency beyond enabling the existing rustix network feature. We do not claim visibility into other network namespaces, invisible processes, duplicated descriptors outside the admitted proc view, exclusive socket ownership, or application handler execution.

## Before Architecture

[Before architecture](../diagrams/linux-socket-diagnostics-before.mmd)

The baseline repeatedly acquires a text snapshot and correlates tuple, inode, and visible holders. It has no retained socket-table session or kernel cookie.

## Options

### Option 1: Retain Proc Tables

[After architecture](../diagrams/linux-socket-diagnostics-retain-proc-tables-after.mmd)

This option tightens parsing and bounds but retains the current acquisition mechanism. It has the lowest delivery cost and compatibility risk, but it does not close the socket-reuse or namespace-pinning gaps.

| Dimension | Delta from before | Basis | Confidence | Validation |
| --- | --- | --- | --- | --- |
| Security | Minimal | E01-E03 | High | Existing adversarial parser tests |
| Performance | Unchanged whole-table reads | E01-E02 | High | Fixture and native benchmarks |
| Operability | No new kernel interface | E01 | High | Existing CI |
| Compatibility | Broadest Linux compatibility | E01 | High | Cross-target build |
| Maintainability | Keeps duplicate text parsers | E01-E02 | High | Code review |
| Delivery risk | Low | Current implementation | High | Focused regression suite |

### Option 2: Bounded Dump Only

[After architecture](../diagrams/linux-socket-diagnostics-bounded-dump-only-after.mmd)

This option pins a `SOCK_DIAG` session and uses strict bounded dumps for listeners and connections. It improves namespace stability and obtains a kernel cookie, but re-dumping the table is unnecessary for an already known connection and leaves a wider matching window.

| Dimension | Delta from before | Basis | Confidence | Validation |
| --- | --- | --- | --- | --- |
| Security | Better namespace and cookie evidence | P01-P02 | High | Cookie-reuse native test |
| Performance | Still scales with table size | P02 | High | Large synthetic dump benchmark |
| Operability | Requires SOCK_DIAG access | P01 | High | Native Linux probe |
| Compatibility | Modern Linux UAPI | P01-P02 | High | Minimum-kernel matrix |
| Maintainability | One parser, less proc text code | E01-E02 | High | Source audit |
| Delivery risk | Medium parser complexity | P01 | Medium | Adversarial framing matrix |

### Option 3: Split Dump And Exact Query

[After architecture](../diagrams/linux-socket-diagnostics-split-dump-and-exact-query-after.mmd)

This option uses the operation matched to each uncertainty. A listener can have multiple candidates, so discovery consumes a complete bounded dump. A retained connection already has an exact tuple, so every checkpoint uses an exact query and the retained cookie. The descriptor-holder scan is bracketed by these exact observations.

| Dimension | Delta from before | Basis | Confidence | Validation |
| --- | --- | --- | --- | --- |
| Security | Strongest point-in-time socket identity | P01-P02 | High | Close-and-reuse and stale-cookie tests |
| Performance | Exact connection lookup, bounded listener dump | P02 | High | Point-query and dump benchmarks |
| Operability | Same access requirement as dump-only | P01 | High | Native Linux probe |
| Compatibility | Stable Linux UAPI, proc visibility remains | P01-P02, E03 | High | Cross-target and policy tests |
| Maintainability | Shared strict codec and explicit state machines | P01-P02 | Medium | Parser coverage at least 90 percent |
| Delivery risk | Highest initial implementation effort | P01-P02 | Medium | Staged focused CI and native tests |

## Comparison

Option 1 is cheapest but does not meet invariants 1 or 2. Option 2 meets namespace pinning but performs a broad operation where the kernel supports a narrower exact lookup. Option 3 meets all achievable invariants while preserving the existing visibility caveat. Its parser complexity is bounded by a fixed UAPI subset and an adversarial deterministic test matrix.

## Recommendation

We select Option 3. The public evidence remains inert and unqualified, but the internal observation is stronger: the same retained kernel cookie must survive before and after each visible-holder scan and every response checkpoint.

## Evidence Coverage And Residual Risk

The recommendation is directly supported by current source behavior and Linux UAPI/kernel behavior. Residual risk remains where unprivileged proc visibility is incomplete, where another namespace contains a relevant process, or where a visible process transfers a duplicated descriptor beyond the admitted view. These conditions fail closed when visible, but absence outside the view cannot be proved.

## Migration And Rollout

We will keep non-Linux adapters and public serialized evidence unchanged. Linux will first add the isolated codec and fake transport tests, then route listener discovery, then route retained connection checks. Removal of `/proc/net/tcp*` row acquisition is the final Linux migration gate. Rollback is a code revert, not a data migration.

## Validation Plan

Validation includes golden IPv4 and IPv6 requests, combined and split response datagrams, malformed headers and attributes, incorrect sender and sequence, incomplete and interrupted dumps, error mapping, byte/message/time/cancellation ceilings, native loopback listener and connection tests, stale-cookie close tests, strict clippy, cross-target checks, repository policy, and at least 80 percent line coverage.

## Implementation Work Packages

1. Add a safe retained `SockDiagSession`, byte codec, parser, and deterministic transport seam.
2. Replace listener table acquisition with a complete bounded LISTEN dump.
3. Replace connection table acquisition with exact tuple and cookie queries bracketing the existing holder scan.
4. Remove obsolete text-table row readers, confirm redaction, and run native and cross-target gates.

The executable handoff is [split-dump-and-exact-query.md](../implementation/split-dump-and-exact-query.md).

## Open Questions

The minimum supported Linux kernel should be recorded from the existing project policy before release. Native network-namespace pinning tests may skip where the host disables unprivileged namespace creation, but deterministic codec coverage must never skip.
