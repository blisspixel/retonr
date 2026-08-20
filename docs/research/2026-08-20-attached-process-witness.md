# Attached Ollama process witness

## Decision summary

The next 0.2 trust-boundary slice is a versioned, read-only attached-process witness
around the existing Ollama preflight. It derives executable evidence from the
kernel-observed listener owner on Windows and Linux. It fails closed on macOS because
the required listener-to-process mapping has no suitable public unprivileged API.

The report is deliberately inert:

- evidence level is `observed_native_listener`;
- `response_bound` is always `false`;
- `qualified` is always `false`;
- no runtime-build, effective-state, package, qualification, activation, or role
  authority is created or persisted; and
- no generation, acquisition, process control, or network-policy mutation occurs.

This is stronger than HTTP observation alone, but it is not exact response
attestation. The implementation decision is recorded in
[ADR 0008](../decisions/0008-attached-process-witness.md).

Evidence was reviewed through August 20, 2026. External spend was $0.

## Research question

Can an unprivileged cross-platform Retonr process prove that a user-managed Ollama
loopback listener, its HTTP responses, and exact executable bytes all belong to one
stable process incarnation?

The answer is no under one uniform mechanism. Windows and Linux can provide useful
point-in-time native ownership evidence with different limits. macOS cannot provide
the required arbitrary listener ownership evidence through a durable public
unprivileged API. None of the reviewed listener-table APIs alone binds a particular
HTTP response to the observed process.

## Platform evidence

| Platform | Admitted evidence | Material limit | Version 1 behavior |
| --- | --- | --- | --- |
| Windows | Public owner-PID TCP table, retained process handle, process creation time, process-reported image path, retained file handle and digest | The table has no socket object identity; the opened disk file is not proof of loaded memory pages | Observe and recheck, remain unqualified |
| Linux | Exact proc TCP row and socket inode, unique same-user descriptor owner, pidfd, process start ticks, same network namespace, retained `/proc/PID/exe` object and digest | Requires same-user proc visibility in the same PID and network namespaces; proc TCP is deprecated | Observe and recheck, remain unqualified |
| macOS | No admitted public unprivileged arbitrary listener-owner mapping | Apple labels the relevant `libproc` interfaces private; stronger system APIs require entitlement and installation | Return `Unsupported` before HTTP |

Windows documents the owner-PID TCP table through
[`GetExtendedTcpTable`](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedtcptable).
A retained handle from
[`OpenProcess`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openprocess)
and creation time from
[`GetProcessTimes`](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)
reduce PID-reuse ambiguity. The process path comes from
[`QueryFullProcessImageNameW`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew).

Linux pidfds are tied to one process and can be polled for exit, as documented by
[`pidfd_open(2)`](https://man7.org/linux/man-pages/man2/pidfd_open.2.html). The
process executable link is documented in
[`proc_pid_exe(5)`](https://man7.org/linux/man-pages/man5/proc_pid_exe.5.html), and
network namespace isolation is documented in
[`network_namespaces(7)`](https://man7.org/linux/man-pages/man7/network_namespaces.7.html).
The kernel calls `/proc/net/tcp` deprecated in favor of TCP diagnostics, so
[`NETLINK_SOCK_DIAG`](https://man7.org/linux/man-pages/man7/sock_diag.7.html) is the
planned replacement.

Apple's own
[`libproc` header](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.h)
states that the interface is private and subject to change. Endpoint Security can
provide stronger execution evidence, but its client entitlement makes it a separate
installed-system-component decision rather than a baseline CLI mechanism.

## Implemented sequence

1. Parse the existing strict loopback-only Ollama endpoint and reject port zero.
2. Apply fixed ceilings for native socket data, rows, processes, descriptors,
   executable bytes, and elapsed time.
3. Find exactly one admitted listener owner and retain a process-incarnation
   capability.
4. Derive the executable object from the observed process and hash the retained
   single-link regular file under cancellation and byte limits.
5. In verify mode, reject an unexpected executable digest before any HTTP request.
6. Run the existing read-only Ollama preflight without generation.
7. Recheck listener ownership, process incarnation and liveness, executable object,
   executable bytes, and relevant platform evidence.
8. Give native drift priority over an HTTP failure and discard the entire report on
   any mismatch.
9. Emit only the redacted inert witness and existing redacted preflight report.

## Threat disposition

| Threat | Version 1 response |
| --- | --- |
| PID reuse | Retain a Windows process handle or Linux pidfd and compare the native start token |
| Process exit | Poll the retained capability and fail with a stable redacted error |
| Listener moves to another PID | Reobserve and fail as listener rebound |
| Linux same-PID listener replacement | Compare the exact socket inode and fail as listener rebound |
| Windows same-PID listener replacement | Not detectable with the admitted owner-PID table; remain point-in-time and unbound |
| Executable path replacement | Hash a retained file object and compare a newly derived object after preflight |
| Executable byte drift | Rehash the retained object and fail |
| Hard-link alias | Require exactly one filesystem link |
| Wildcard or non-loopback binding | Exact loopback row does not match, so observation fails |
| Multiple matching listeners or owners | Fail as ambiguous |
| Permission or visibility loss | Fail closed without PID-only or process-name fallback |
| Proxy, container, VM, or WSL forwarding owner | Witness the forwarding process; an expected Ollama digest then fails |
| Path, argument, environment, or provider text disclosure | Omit these values and map native errors to stable categories |
| Resource exhaustion | Stop at explicit rows, bytes, process, descriptor, time, and cancellation limits |

## What remains unproven

Two snapshots around independent HTTP calls do not prove which process served those
responses. Exact response binding requires Retonr to own one persistent TCP
connection, record its client and server 4-tuple, and verify the server-side accepted
socket owner for that exact connection. The transport must reject reconnection,
migration, listener handoff, and any inability to observe the accepted socket.

The witness also does not establish:

- complete runtime package or native dependency closure;
- actually loaded code or executable memory pages;
- effective runtime configuration or launch mode;
- model artifact-set completeness, tokenizer, template, license, or source revision;
- provider cloud disablement;
- OS-enforced denial of non-loopback outbound traffic; or
- safety against a fully hostile same-privilege process.

Ollama 0.32.14 contains an experimental read-only `/api/status` response for its
effective cloud-disable decision. That version-scoped provider evidence is useful,
but it is not an OS isolation proof and is not part of this slice. Ollama's documented
local-only inputs remain in the [Ollama FAQ](https://docs.ollama.com/faq).

## Next implementation order

1. Bind the complete preflight to one retained TCP connection and verify the exact
   accepted server socket owner. This closes the largest remaining gap in the new
   witness instead of layering stronger identities over an unbound response.
2. Replace Linux proc TCP parsing with bounded socket diagnostics and retain the
   socket identity in the evidence.
3. Reconstruct one selected Ollama installation and model as complete canonical
   runtime and artifact-set manifests, including native dependencies, source,
   transformations, tokenizer, template, and license disposition.
4. Add version-gated provider cloud-disable evidence and OS-enforced outbound denial.
5. Only then construct attached runtime-build, effective-state, and effective-package
   evidence and make a local generation plan eligible for smoke evaluation.

This order makes every later evaluation attributable to a frozen process, transport,
package, configuration, and isolation chain. A useful model score before that chain
would not be reproducible qualification evidence.
