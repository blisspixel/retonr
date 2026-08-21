# Implementation Plan: Split Dump And Exact Query

## Implementation Status

Implemented for Linux attached observation: listener acquisition uses a complete
bounded SOCK_DIAG dump, and every retained connection checkpoint uses exact
cookie-bearing queries around the visible same-UID descriptor-holder scan. The path
has no proc TCP row-selection fallback. Public attached report semantics remain
observation-only and unchanged.

## Selected Design And Constraints

Linux listener discovery will use a complete bounded `SOCK_DIAG` dump. Every retained established connection checkpoint will use an exact tuple query with a retained socket cookie before and after the visible same-UID descriptor-holder scan. Public evidence schema and limitation enums remain unchanged. The implementation must not serialize raw network or kernel identifiers.

## Source Revision And Drift Check

The plan is anchored to Git revision `c3657edd126f164facc311719b831d6926e7c06d` and evidence collection SHA-256 `6a9d8e2195834f5d123add1626048d468b1b8bfd68eedb8bfaae022efa59ec3c`. Before merge, re-read changed Linux attestor files and rerun the relationship tests. Any change to lease creation, holder enumeration, response checkpoints, or evidence serialization requires this plan to be revalidated.

## Affected Components

- `crates/runtime-attestor/src/platform/linux_sock_diag.rs`: safe session, byte codec, parser, and state machines.
- `crates/runtime-attestor/src/platform/linux.rs`: retained session, expected effective UID, listener discovery, and lease identity.
- `crates/runtime-attestor/src/platform/linux_connection.rs`: exact connection queries and cookie-bracketed holder scans.
- `crates/runtime-attestor/src/platform/mod.rs`: Linux module routing.
- `crates/runtime-attestor/Cargo.toml`: enable the existing rustix network feature.

## Ordered Work Packages

1. Define checked UAPI constants and fixed-width byte encoders without unsafe casts.
2. Implement one nonblocking, close-on-exec session bound at lease creation, with assigned port ID and monotonic nonzero sequences.
3. Implement strict datagram and message parsing, sender validation, acknowledgement validation, attribute walking, error mapping, bounds, deadlines, and cancellation.
4. Add listener dump construction and state machine, including one whole-dump retry after interruption.
5. Add exact established-socket query construction and data-plus-ack state machine.
6. Retain cookie, inode, UID, interface index, and tuple privately, then bracket the existing holder scan at every checkpoint.
7. Remove `/proc/net/tcp*` row acquisition from listener and connection paths.
8. Run deterministic, native, cross-target, policy, and coverage gates.

## Compatibility And Migration

This is a Linux-internal replacement with no persistence migration. Windows, macOS, unsupported-platform behavior, serialized enums, and public report fields remain byte-compatible. Linux systems that reject `SOCK_DIAG` return the existing redacted platform observation failure or access-denied class rather than falling back to weaker evidence.

## Tactical Protections During Migration

No production path will choose between proc tables and netlink at runtime. The new parser is completed and tested before routing the listener path. Exact-query integration follows under the same lease. A temporary dead proc parser may remain during development, but the exit gate requires no `/proc/net/tcp*` row reads in the two acquisition paths.

## Tests And Security Validation

Golden request bytes cover IPv4 and IPv6, no-cookie and retained-cookie exact requests, and listener dumps. State tests cover data and acknowledgement ordering, missing and duplicate messages, wrong tuple/state/UID/inode/cookie/interface, incomplete and interrupted dumps, overruns, truncation, malformed lengths and attributes, sender/port/sequence mismatch, every mapped errno class, cancellation, timeout, and ceilings. Native tests cover loopback IPv4, optional IPv6, close-and-stale-cookie failure, and visible duplicate-holder ambiguity where the host permits it.

## Performance And Resource Benchmarks

Measure listener discovery with 1, 1,000, and the configured maximum synthetic records. Measure exact point-query latency independently. Assert allocation and received-byte ceilings and confirm cancellation latency remains bounded by the poll slice. Regressions beyond the existing preflight deadline budget block rollout.

## Rollout And Rollback

The change ships as part of the Linux runtime-attestor build after all platform CI passes. There is no feature flag that silently downgrades the evidence. Rollback is a release revert and returns Linux attached connection evidence to its earlier unqualified implementation.

## Acceptance Criteria

- One retained close-on-exec nonblocking `SOCK_DIAG` session exists per Linux lease.
- Listener discovery accepts only one exact complete LISTEN candidate.
- Every connection checkpoint uses the retained kernel cookie and exact tuple.
- Holder scans are bracketed and drift fails closed.
- No raw network or kernel identifier is serialized or logged.
- No `/proc/net/tcp*` reads remain in socket-row acquisition.
- Focused tests, strict clippy, cross-target checks, repository policy, diff check, and workspace CI pass.
- Implemented Rust line coverage remains at least 80 percent.

## Open Decisions

Record the minimum supported Linux kernel from project policy before release. Decide whether a future privileged observer is warranted for hosts whose proc visibility is insufficient; the current slice must not imply that capability.
