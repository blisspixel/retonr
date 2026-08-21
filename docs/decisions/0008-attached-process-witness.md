# ADR 0008: Attached process witness

- Status: proposed
- Decision owners: project maintainers
- Decision checkpoint: roadmap milestone 0.2 implementation
- Last reviewed: 2026-08-21

## Context

The read-only Ollama preflight observes runtime, inventory, model-description, and
residency data over loopback HTTP. A loopback URL does not establish which process
owns the listener, which executable object is associated with that process, or which
process served an HTTP response. The existing managed-process attestor hashes a
caller-selected path and constructs caller-supplied runtime-build and effective-state
facts. Reusing it for an attached user-managed process would misstate the evidence.

There is no stable unprivileged operating-system API that provides the same listener
ownership proof on Windows, Linux, and macOS. The evidence boundary therefore needs
an explicit platform capability decision and an exact statement of what remains
unproven.

## Decision drivers

- Derive executable evidence from a kernel-observed listener owner, never from a
  plan-supplied path.
- Retain a process-incarnation capability across the existing read-only preflight.
- Bound socket tables, processes, descriptors, executable bytes, elapsed time, and
  cancellation latency.
- Serialize no executable path, arguments, environment, user name, or raw native
  error.
- Preserve the local-first and no-generation boundary.
- Fail closed when the operating system or current permissions cannot provide the
  admitted evidence.
- Do not promote a point-in-time listener observation into response attestation,
  runtime-build identity, effective-state identity, qualification, or authority.

## Options considered

### Reuse managed-process attestation

This would require a caller path and would emit managed-process identity. It would
not observe a listener, process incarnation, or loaded executable. It is rejected.

### Shell out to process and socket utilities

Tools such as `netstat`, `ss`, `lsof`, and PowerShell expose localized text, differ
by host installation and privilege, and cannot retain a process capability. They are
not an admitted trust boundary.

### Claim one portable attached-process implementation

This would conceal material platform differences. macOS has no durable public
unprivileged listener-to-process API for arbitrary daemons. Linux visibility depends
on the current network and PID namespaces and `/proc` permissions. This option is
rejected.

### Add a bounded platform adapter with an inert report

This keeps native mechanisms behind one safe facade and lets unsupported or
incomplete observations fail deterministically without weakening the claim. This is
the selected option.

## Decision

Add `rewrite-runtime-attestor` as the only first-party crate permitted to contain the
native FFI required by this capability. All existing crates retain
`unsafe_code = "forbid"`. The crate exposes a safe, versioned observer and retained
lease. Every unsafe block has a local safety argument and is exercised by a native
Windows listener test.

On Windows, use the documented owner-PID TCP table, retain a process handle with
query and synchronization access, bind the PID to its creation time, obtain the
process-reported image path, and hash a retained read-only single-link regular file
handle. Recheck listener ownership, process liveness, process creation time, file
identity, and bytes after the HTTP preflight.

On Linux, use a bounded same-network-namespace `NETLINK_SOCK_DIAG` listener dump,
retain the exact socket inode and kernel socket cookie, require a unique visible
same-UID process descriptor owner, retain a pidfd, record process start ticks, open
`/proc/PID/exe`, and hash that single-link regular file object. Recheck the exact
listener through socket diagnostics plus the process, namespace, file identity, and
bytes after the HTTP preflight. Do not fall back to deprecated `/proc/net/tcp*` row
selection when socket diagnostics is unavailable.

On macOS, return the stable `Unsupported` error. Do not use Apple's private
`libproc` listener-ownership interfaces in a qualification boundary. An entitled
system component is a separate future product and distribution decision.

The version 1 report is `observed_native_listener`. It always emits
`response_bound: false` and `qualified: false`. It constructs and persists no
`RuntimeBuildIdentity`, `EffectiveRuntimeState`, package evidence, qualification,
activation, or role authority. Observe mode records executable evidence. Verify mode
also requires one exact executable digest.

## Consequences

### Positive

- Windows and Linux gain bounded native evidence derived from the listener owner.
- PID reuse, process exit, owner drift, executable-object drift, byte drift, limits,
  cancellation, and redaction have deterministic failure contracts.
- Linux detects same-process listener replacement through the socket inode and
  retained kernel socket cookie.
- macOS does not receive an overstated or private-API-based support claim.
- The existing Ollama preflight remains a separate published version 1 contract.

### Negative

- Windows owner-PID tables do not expose a socket object identity, so a listener can
  close and reopen under the same process without detection.
- A process-reported image path plus an opened disk file is not proof of every loaded
  executable page or native dependency.
- Linux depends on same-user `/proc` visibility in the same PID and network
  namespaces. Linux ptrace policy can deny even a same-user sibling or parent
  process, in which case the command fails closed. Cross-user descriptor passing and
  forwarding topologies are outside this witness.
- macOS attached-process observation is unavailable through this command.
- The Windows adapter contains reviewed first-party unsafe FFI.

### Follow-up

- The separate retained-connection preflight in
  [ADR 0009](0009-retained-connection-attribution.md) now sends one read-only
  operation over one direct HTTP/1 connection and repeats exact native attribution.
  This does not change the attached report's `response_bound: false` contract.
- Retain bounded SOCK_DIAG parsing, sender and sequence validation, exact tuple and
  state checks, and fail-closed behavior without a proc TCP fallback.
- Decide whether an entitled macOS helper is justified before claiming parity.
- Keep this attached report observation-only. The separate Linux managed-isolation
  and managed-attestation path must not promote or silently replace attached evidence.
- Extend the managed operation so the process remains live after its implemented
  package-declared runtime-build binding. Only its exact entrypoint is joined to live
  process and native-load evidence; other package semantics are not independently
  live-observed. Join exact model-package, runtime-reported residency, local-judge
  receipt, direct effective-state, and candidate-generation evidence before local
  generation becomes eligible. The model-package binding consumes an opaque,
  nonserializable receipt from the exact preflight runner. Retained-session input has
  an absolute 4 MiB UTF-8 ceiling before wire serialization or completion traffic.
  The separate receipts do not upgrade attached evidence or prove managed isolation
  by themselves.

## Validation

The decision passes when formatting, warnings-as-errors Clippy, tests, documentation,
policy, dependency, audit, release build, and line-coverage gates pass, including:

- native Windows and Linux attach and reobserve tests;
- same-listener success and closed-listener failure;
- exact executable-digest mismatch before HTTP work;
- process drift taking priority over an HTTP failure;
- redacted serialization with no path, arguments, environment, license, or template;
- deterministic macOS unsupported behavior; and
- cross-platform continuous integration through the stable `required` aggregate.

Miri cannot execute the Windows kernel FFI used here. The unsafe surface is
target-only and receives native Windows tests plus explicit table-length validation
before every unaligned row read. Revisit this decision if a maintained safe public-API
wrapper removes the first-party unsafe code, if Windows exposes socket object
identity, if Linux changes the admitted SOCK_DIAG or proc descriptor-visibility
interfaces, or if macOS adds an appropriate public API.

## References

- [Microsoft GetExtendedTcpTable](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedtcptable)
- [Microsoft OpenProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocess)
- [Microsoft GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)
- [Microsoft QueryFullProcessImageNameW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew)
- [Linux pidfd_open](https://man7.org/linux/man-pages/man2/pidfd_open.2.html)
- [Linux proc process executable](https://man7.org/linux/man-pages/man5/proc_pid_exe.5.html)
- [Linux network namespaces](https://man7.org/linux/man-pages/man7/network_namespaces.7.html)
- [Linux internet socket diagnostics UAPI](https://github.com/torvalds/linux/blob/master/include/uapi/linux/inet_diag.h)
- [Apple XNU libproc header](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.h)
