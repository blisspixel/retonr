# ADR 0009: Retained connection attribution

- Status: proposed
- Decision owners: project maintainers
- Decision checkpoint: roadmap milestone 0.2 implementation
- Last reviewed: 2026-08-20

## Context

The read-only Ollama preflight currently uses independent requests through a pooled
HTTP client. The attached-process witness observes the loopback listener owner,
retains one process incarnation, and rechecks executable evidence before and after
that preflight. Those snapshots do not establish that every response traveled on one
connection or that the operating system attributed the accepted side of that exact
connection to the retained process.

The distinction matters. An HTTP client may reconnect after a server close. A
listener can be replaced while an established connection survives. A process can
share or transfer a socket. Platform socket tables also expose different facts:
Windows reports a context-binding PID for a TCP row, Linux can expose an exact socket
inode and visible descriptor holders, and macOS has no admitted public unprivileged
tuple-to-process API.

## Decision drivers

- Send the complete read-only preflight over one directly connected and retained TCP
  stream.
- Keep the existing preflight and attached-process report contracts unchanged.
- Perform no DNS lookup, proxying, redirect, connection pooling, retry, or reconnect.
- Attribute the exact reverse established 4-tuple before application traffic and
  after every fully drained response.
- Retain process-incarnation and executable evidence across all connection checks.
- Bound connect time, request time, body bytes, aggregate session bytes, native table
  bytes, rows, processes, descriptors, polling, and cancellation latency.
- Serialize no address, port, executable path, argument, environment value, response
  body, or raw native error.
- Distinguish transport attribution from exclusive socket ownership and
  application-handler execution.
- Fail closed when a platform, permission boundary, or observation cannot provide
  the admitted evidence.

## Options considered

### Keep pooled requests with wider process snapshots

Additional listener snapshots would still permit an invisible reconnect between
requests. They also would not bind a response to an exact established TCP row. This
option is rejected.

### Add a dedicated retained HTTP session and native attribution seam

A low-level HTTP/1 client connection over one caller-opened `TcpStream` has no
connector after the initial connect. A synchronous observation seam can inspect the
exact retained tuple before traffic and after each response without giving the HTTP
adapter platform authority. This is the selected option.

### Parse HTTP directly in the native observer

Combining HTTP framing with native process inspection would duplicate protocol code,
blur dependency direction, and enlarge the platform-specific trusted surface. This
option is rejected.

### Require a Retonr-managed runtime chain

Owning process creation, socket inheritance, namespaces, and transfer policy could
support a stronger future claim. It would exclude the first qualification target, a
user-managed Ollama service, and would still require a separate macOS distribution
decision. This remains later work rather than a prerequisite for the bounded
attached-runtime observation.

## Decision

Add a separate versioned bound-preflight surface. Preserve the published read-only
preflight and attached-process witness reports exactly as their existing contracts
describe them.

The Ollama adapter opens one explicit IP-literal loopback `TcpStream`, captures its
client and server endpoints, performs one HTTP/1 handshake, and sends the existing
ordered preflight sequence through that sender. Every response must be successful
JSON, remain within its individual byte ceiling, finish within the operation
deadline, and be fully drained before the next request. The complete session also
has an aggregate byte ceiling. A close declaration, protocol upgrade, truncated
body, completed connection driver, send failure, cancellation, deadline, or native
observation failure ends the operation. There is no second connect or handshake
path.

The runtime attestor remains the only first-party crate with native socket-table and
process authority. It accepts the exact retained client and server tuple without
serializing it and compares every connection attribution with the already-retained
listener process.

On Windows, the attestor uses `GetExtendedTcpTable` with
`TCP_TABLE_OWNER_PID_CONNECTIONS`, requires one exact reverse IPv4 or IPv6 row in
`MIB_TCP_STATE_ESTAB`, and requires its documented context-binding PID to match the
retained process handle and creation time. The report does not call this PID an
exclusive owner or application handler.

On Linux, the first slice selects the exact reverse established row from the bounded
same-network-namespace proc TCP view, retains its socket inode, and requires the
visible same-user descriptor-holder set to contain exactly the retained process.
The inode, visible holder, pidfd, process start token, listener, executable object,
and executable bytes must remain stable. The kernel deprecates the proc TCP table, so
a bounded `NETLINK_SOCK_DIAG` replacement remains the immediate Linux follow-up.

On macOS, the bound preflight returns unsupported before making an HTTP request.
Apple's public APIs do not provide an ordinary unprivileged point query that maps an
arbitrary established tuple to a process. Private `libproc` or socket structures are
not admitted. An entitled Network Extension would be a separate installation,
permission, and product decision.

The report exposes only redacted process evidence, redacted connection-attribution
evidence, the existing redacted API observation, an opaque domain-separated binding
digest, and explicit limitation fields. It states that all response bytes used one
retained client transport and that platform attribution matched at every checkpoint.
It also states that exclusive socket ownership and application-handler execution are
not proven. The report remains read-only, inert, and `qualified: false`. It creates no
runtime-build, effective-state, package, qualification, activation, or role record.

The version 1 command surface is
`rewrite-eval --ollama-bound-preflight <PLAN_JSON_FILE>`. The bound plan nests one
existing preflight plan and adds executable and aggregate retained-session byte ceilings.
Observe mode forbids an expected executable digest. Verify mode requires it. The
command is mutually exclusive with the base and attached preflight commands and with
generation or evaluation-suite inputs.

## Consequences

### Positive

- Reconnection and connection-pool substitution become structurally unavailable in
  the bound preflight.
- Every accepted API observation is tied to one retained client transport and a
  repeated exact reverse-tuple attribution check.
- Listener, process, executable, connection, runtime, inventory, and residency drift
  fail the complete operation.
- Platform differences and residual sharing risk remain machine-readable instead of
  being hidden behind one portable ownership claim.
- Existing versioned reports retain their original semantics.

### Negative

- The bound preflight adds a second HTTP implementation path for read-only discovery,
  although response parsing and domain validation remain shared where practical.
- HTTP/2, HTTP/1 pipelining, reconnect, and transparent retry are unavailable by
  design.
- Native attribution checks after every response add socket-table and process
  inspection latency.
- Windows cannot enumerate every duplicated socket holder through the admitted API.
- Linux proc visibility can be incomplete because of UID, ptrace, proc-mount, PID
  namespace, or security policy. The operation fails when an admitted required view
  is unavailable, but it cannot elevate an unprivileged snapshot into proof of all
  possible holders.
- macOS cannot run this command without a future separately approved system
  component.

## Follow-up

- Replace Linux proc TCP row selection with bounded `NETLINK_SOCK_DIAG` evidence,
  retaining the kernel socket cookie, inode, UID, tuple, and namespace scope.
- Reconstruct one selected Ollama installation and model as complete canonical
  runtime and artifact-set manifests, including native dependencies, source,
  transformations, tokenizer, template, and license disposition.
- Add version-gated provider cloud-disable evidence and OS-enforced non-loopback
  denial for every participating process.
- Only after transport, package, configuration, and isolation evidence are complete,
  construct attached runtime-build and effective-state identity or admit local
  generation to smoke evaluation.

## Validation

The decision passes when formatting, warnings-as-errors Clippy, tests,
documentation, policy, dependency, advisory, release, and line-coverage gates pass,
including:

- exactly one accepted client connection and one HTTP handshake for all ordered
  preflight requests;
- no reconnect when the server closes between responses;
- complete response draining before the next request;
- exact reverse IPv4 and IPv6 tuple matching with correct network byte order;
- stable native attribution before traffic and after every response;
- deterministic rejection of missing, ambiguous, incomplete, changed,
  non-established, cancelled, expired, oversized, closed, and upgraded sessions;
- listener, process-incarnation, executable-object, executable-byte, runtime,
  inventory, model-description, and residency drift fixtures;
- redaction canaries covering paths, ports, provider text, arguments, environment,
  and native errors;
- deterministic macOS refusal before HTTP work; and
- cross-platform continuous integration through the stable `required` aggregate.

## References

- [Microsoft GetExtendedTcpTable](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-getextendedtcptable)
- [Microsoft MIB_TCPROW_OWNER_PID](https://learn.microsoft.com/en-us/windows/win32/api/tcpmib/ns-tcpmib-mib_tcprow_owner_pid)
- [Microsoft WSADuplicateSocket](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-wsaduplicatesocketw)
- [Linux Netlink protocol](https://docs.kernel.org/userspace-api/netlink/intro.html)
- [Linux Netlink UAPI family identifiers](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/netlink.h)
- [Linux internet socket diagnostics UAPI](https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git/tree/include/uapi/linux/inet_diag.h)
- [Linux proc process descriptors and visibility](https://docs.kernel.org/filesystems/proc.html)
- [Linux proc TCP tables](https://docs.kernel.org/networking/proc_net_tcp.html)
- [Apple libproc header](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.h)
- [Apple Network Extension flow audit token](https://developer.apple.com/documentation/networkextension/nefilterflow/sourceprocessaudittoken)
