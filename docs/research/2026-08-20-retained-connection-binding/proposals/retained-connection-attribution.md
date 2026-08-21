# Security Hardening Proposal: Retained Connection Attribution

## Decision

Select Option 2, one retained HTTP/1 session with bounded point-in-time kernel
attribution, for the next attached Ollama preflight slice. Reserve Option 3, a managed
runtime chain, for later package and isolation work. Do not describe Option 2 as
exclusive socket ownership, application-handler proof, or qualification.

## Executive Recommendation

We have three serious choices:

- Option 1, keep pooled reqwest with listener snapshots, preserves the current
  transport and improves temporal drift observation without binding responses.
- Option 2, use one retained HTTP/1 session with kernel attribution, gives the
  preflight an owned client socket and prevents implicit reconnect during the
  operation.
- Option 3, move to a managed runtime chain, would let the application own package,
  launch, process lifetime, listener, and connection capabilities, but expands scope
  well beyond an attached read-only preflight.

I recommend Option 2 under the current constraints. It directly addresses the gap
left by the existing proposed attached-process decision while preserving the user-managed
runtime and an incremental rollback path. What gives me pause is not the dedicated
HTTP/1 transport itself, but the platform attribution ceiling. Windows provides a
context-binding PID, Linux provides conditional visible-holder evidence, and macOS
remains unsupported. We must carry those facts into the report instead of converting
transport continuity into a stronger process claim.

## Evidence

I inspected the source and existing proposed decision at revision
`a58e07473eb558e3e38aa382de59af909f4a647b`. The evidence below is locally defined so
a reader does not need the context registry to understand later references.

| Evidence | Finding or document | What it establishes |
| --- | --- | --- |
| `E01` | Pooled request transport in `crates/ollama/src/backend.rs` | A reusable reqwest client sends each request without exposing the selected TCP connection. |
| `E02` | Multi-request preflight in `crates/ollama/src/backend/preflight.rs` | One operation issues ordered version, tags, residency, show, and confirming requests. |
| `E03` | Bracketed attached-process orchestration in `crates/eval/src/local_ollama_attested_preflight.rs` | The listener witness surrounds the HTTP operation and correctly emits `response_bound: false` and `qualified: false`. |
| `E04` | Windows listener witness in `crates/runtime-attestor/src/platform/windows.rs` | The public TCP table supplies an owner PID and the observer retains a process handle. |
| `E05` | Linux listener witness in `crates/runtime-attestor/src/platform/linux.rs` | A listener inode is mapped to a unique visible same-user descriptor owner and retained pidfd. |
| `E06` | macOS unsupported boundary in `crates/runtime-attestor/src/platform/macos.rs` | The repository refuses to use private listener ownership APIs for attached evidence. |
| `E07` | Witness contract in `crates/runtime-attestor/src/contract.rs` | Stable bounded listener evidence, cancellation, redaction, and drift vocabulary already exist. |
| `E08` | [Proposed attached-process witness decision](../../../decisions/0008-attached-process-witness.md) | The proposed design explicitly defers one persistent connection and exact 4-tuple attribution. |

The observed fact is that listener and HTTP boundaries are separate. The inferred
structural condition is that no caller above reqwest can distinguish reuse of the
first connection from an implicit second connection. A successful listener snapshot
therefore says which process owned the listening context at that instant, not which
server-side connection carried bytes.

The adversarial review adds a second inference. Even after we retain one client
socket, kernel snapshots remain point-in-time attribution. They cannot prove that an
accepted socket was never duplicated between observations, identify the application
handler that generated a response, or show that the immediate peer did not proxy the
request. Those are residual risks, not implementation defects in the proposed scope.

## Current Design And Failure Mode

The eval layer first acquires a native listener and executable witness. It then calls
the existing preflight, which issues several requests through the adapter's reusable
client. Finally, it reobserves the listener and process evidence. Runtime, inventory,
and residency drift fail closed, as do listener, process, and executable drift.

This is a sound point-in-time listener witness. The gap appears when the server closes
a keep-alive connection or the pool otherwise chooses another connection. The client
can complete a later request without exposing that transition to the orchestration
layer. Rechecking the listener can show that the endpoint still names the same
process, but it cannot show that every response used the same accepted socket context.

The gap matters because the next evidence level is about response attribution, not
availability. An implicit reconnect might still reach the same binary and return
coherent data, yet the operation no longer has one retained transport subject. If we
allowed that result, a future process or listener change could be hidden behind the
pool's recovery behavior.

## Desired Invariants

- One attached preflight performs exactly one direct TCP connect and exactly one
  HTTP/1 handshake.
- All `6 + N` ordered requests for `N` target models use the retained socket.
- No transport, protocol, ownership, timeout, or cancellation failure can reach a
  second connect attempt.
- Before application traffic and after every complete response, the reverse
  established 4-tuple has the same admissible kernel-attributed process instance.
- A missing, closed, non-established, ambiguous, incomplete, or changed observation
  fails the operation and discards partial evidence.
- Each response body is bounded and completely drained before the next request.
- Listener, process, executable, runtime, inventory, model details, and residency
  remain stable through the final check.
- The report states the platform attribution scope, remains inert, and sets
  `qualified: false`.

## Constraints And Non-Goals

The endpoint stays an HTTP IP-literal loopback address. The operation performs no
generation, model load, acquisition, activation, process launch, process stop, or
configuration mutation. It follows no redirect and uses no ambient proxy.

Windows TCP tables report a context-binding PID for a row. We cannot infer exclusive
accepted-socket ownership from that value. Linux can search same-UID descriptors
visible under the current proc, PID namespace, and network namespace policy for the
established socket inode. The first slice requires exactly one visible holder and
fails closed on observed permission or enumeration gaps. Unprivileged code cannot
prove that no invisible holder exists. macOS remains unsupported because the repository has
no reviewed public unprivileged ownership API for this boundary.

We do not claim continuous ownership between snapshots, application-handler
execution, absence of an in-process or immediate-peer proxy, runtime package closure,
effective configuration, cloud disablement, network isolation, qualification, or
activation authority.

## Before Architecture

The [before diagram](../diagrams/retained-connection-attribution-before.mmd) shows the
important split: process evidence is derived from the listener table, while requests
cross an opaque reusable pool. Both paths reach the same endpoint, but there is no
owned object that joins one response to one connection observation.

That separation is intentional in the current revision and explains why the report
sets `response_bound` to false. The proposal changes only this missing lifetime join.

## Options

### Option 1: Keep Pooled Reqwest With Listener Snapshots

The strongest case for Option 1 is compatibility. We can retain the mature client,
its existing body handling, and the current adapter API. Additional listener checks
after response groups would narrow drift windows and could catch a process or listener
replacement sooner than the existing outer bracket.

The [Option 1 diagram](../diagrams/retained-connection-attribution-pooled-snapshots-after.mmd)
keeps the pool as the transport owner. The dashed observation edge becomes more
frequent, but it still terminates at the listener view rather than an accepted
connection. This is useful defense in depth, not response binding.

| Change | Before | After | Security consequence | Cost |
| --- | --- | --- | --- | --- |
| Observation frequency | Listener witness before and after the complete HTTP preflight | Listener witness also runs after response groups | Shorter listener-drift detection interval | More native table and process observations |
| Connection ownership | Hidden inside the reusable client | Still hidden inside the reusable client | No exact response subject exists | No transport migration |
| Reconnect behavior | Client may select another connection | Unchanged | An operation can cross connections without a stable reason | Existing availability behavior remains |
| Report authority | `response_bound: false`, `qualified: false` | Unchanged | Avoids overclaim | No schema authority migration |

The attractive operational property is that a transient server close may remain
recoverable. That same recovery is the security limitation: the operation cannot
distinguish continuity from replacement. We could roll this option out or back with a
small orchestration change, but it would leave the proposed follow-up in E08
incomplete. We should choose it only if delivery time or compatibility explicitly
outweighs response attribution.

### Option 2: Use One Retained HTTP/1 Session With Kernel Attribution

Option 2 creates a private attached-preflight transport that directly connects to the
already validated loopback socket address. One HTTP/1 handshake yields one sender and
connection driver. The preflight serializes every request on that sender, drains each
bounded body, and never owns a connector it could call a second time.

Before sending application traffic, a connection observer resolves the retained
socket's local and peer addresses and attributes the reverse established 4-tuple to
the pinned listener process. After every response, the observer repeats that check.
The final stage also repeats the existing listener, process, executable, runtime,
inventory, and residency evidence. A graceful close, reset, EOF, `Connection: close`,
owner-view gap, process change, or deadline is terminal even if a new connection could
have recovered availability.

The [Option 2 diagram](../diagrams/retained-connection-attribution-single-retained-session-after.mmd)
adds one security-relevant component: the retained session. The kernel connection
view and process witness meet at that owned socket lifetime. We can now say every byte
handled by the client operation crossed the same client socket and that the
kernel-attributed peer process matched at each observation point.

| Change | Before | After | Security consequence | Cost |
| --- | --- | --- | --- | --- |
| Client connection | Selected inside a reusable pool | One direct retained socket and one HTTP/1 handshake | Implicit reconnect is impossible | Dedicated transport code and tests |
| Attribution subject | Listener endpoint | Reverse established 4-tuple plus retained client socket | Responses share one transport subject | Native owner check after each response |
| Failure recovery | Pool may reconnect | Any close or attribution gap is terminal | Evidence cannot silently cross connections | Lower availability under unstable runtimes |
| Windows claim | Listener context PID | Listener and established-row context PID matched to one process instance | Narrows the connection context | Still not exclusive accepted-socket proof |
| Linux claim | Visible listener inode holder | Exactly one visible same-UID established inode holder under current proc and namespace policy | Can detect visible cross-process handoff or duplication | Cannot exclude invisible holders; descriptor enumeration and permission sensitivity |
| macOS claim | Unsupported | Unsupported | No private-API overstatement | No attached feature parity |
| Report authority | Listener evidence, response bound false | Point-in-time response-attribution evidence | Better audit subject | Still inert and `qualified: false` |

Security improves because continuity becomes a property of an owned capability rather
than an inference from endpoint stability. The residual boundary remains important.
On Windows, the table names a context PID and may not expose every process holding a
duplicated socket. On Linux, bounded descriptor enumeration can find visible multiple
holders, but unprivileged code cannot establish a complete holder set or exclude
hidden proc entries. In either case, snapshots can miss
a transient handoff that begins and ends between checks. We should encode the evidence
class and completeness in the report rather than collapse both platforms into a
Boolean.

Performance cost comes from kernel attribution after every response. A one-model
preflight makes seven requests, and eight models make fourteen. Windows table reads
are bounded by the existing byte and row ceilings. Linux may scan bounded process and
descriptor sets, so the host process count and proc policy can dominate latency. The
single connection saves repeated handshakes within the operation, but no measurement
yet shows whether that offsets observation cost. We need separate per-stage timing,
not one aggregate benchmark.

Memory remains bounded by one socket, one HTTP driver, the existing response bodies,
and one platform observation buffer. Reliability intentionally becomes stricter. A
server that returns a valid response with `Connection: close` still cannot satisfy the
final owner check, so the complete result must be discarded. This is the right default
for evidence collection, but it can make user-managed runtime instability more
visible.

Migration can stay narrow. The ordinary discovery and generation adapter can remain
on reqwest while the attached preflight uses the retained session. We preserve the
published listener-witness path as rollback and version the new report instead of
reinterpreting old evidence. I would be comfortable expanding the session to
generation only after this read-only slice establishes native fixture stability and a
measured observation budget.

### Option 3: Move To A Managed Runtime Chain

The strongest architectural answer is to stop attaching to an independently managed
listener. If the application verifies the runtime package, launches the process,
retains its lifecycle handle, owns the listener policy, and opens the one retained
connection, it no longer needs to reconstruct as much authority from global socket
tables. This could also provide a clean path for readiness, shutdown, restart, and
isolation policy.

The [Option 3 diagram](../diagrams/retained-connection-attribution-managed-runtime-chain-after.mmd)
replaces the user-managed runtime with an application-owned package and lifecycle
chain. The connection session remains, but process identity comes from the retained
child capability and verified launch rather than only post-hoc listener discovery.

| Change | Before | After | Security consequence | Cost |
| --- | --- | --- | --- | --- |
| Runtime authority | User-managed external process | Application verifies, launches, supervises, and stops the runtime | Reduces attachment ambiguity | Expands product authority and support scope |
| Package identity | Separate future evidence | Package verification precedes launch | Stronger entrypoint and dependency chain | Requires package-manifest completion |
| Process identity | Reconstructed from listener owner | Retained from process creation | Avoids PID discovery as the primary join | Requires cross-platform child lifecycle design |
| Listener policy | External runtime configuration | Application-owned launch and readiness policy | Narrows unexpected topology | Adds configuration and recovery responsibility |
| Connection | Opaque pooled client | Retained session tied to managed process | Stronger lifetime composition | Still needs handler and proxy threat analysis |
| User workflow | Existing user-managed Ollama | New managed mode and artifact lifecycle | Clearer authority | Significant compatibility and migration work |

This option offers the greatest long-term leverage, especially if the product later
needs deterministic package, launch, and isolation evidence. It does not eliminate all
response questions. A managed process can still delegate accepted sockets, proxy
requests, or route work through helpers unless the launch and topology policy forbids
those behaviors. Application-handler execution remains a separate claim.

The operational cost is much larger than a transport change. We would own cold start,
readiness, shutdown, crashes, orphan recovery, updates, disk and memory policy,
platform packaging, and user migration. Runtime residency may improve repeated-call
latency but increases idle memory. Rollback must keep managed mode separate from
attached evidence so authority records are not silently reinterpreted.

Option 3 should win when the package manifest, effective configuration, isolation,
and lifecycle milestones are ready and the project deliberately chooses managed
runtime authority. It should not be pulled forward merely to avoid implementing one
bounded connection session.

## Comparison

The table summarizes direction, not a composite score. Every unmeasured effect has a
validation plan below.

| Dimension | Option 1: pooled snapshots | Option 2: retained session | Option 3: managed runtime |
| --- | --- | --- | --- |
| Security | Slightly improves listener drift detection; response connection remains unknown | Removes implicit reconnect and joins responses to one retained socket with point-in-time attribution | Adds package, launch, process, listener, and connection ownership, subject to topology policy |
| Performance | Preserves pooling; adds snapshot cost | Adds owner checks after each response; one handshake per operation | Adds verification and lifecycle costs; warm reuse may help repeated operations |
| Memory | Existing pool and bounded snapshot buffers | One retained socket, HTTP driver, process lease, and bounded observer buffers | Managed runtime and model residency can materially increase memory |
| Reliability | Keeps client recovery behavior | Close, ambiguity, or visibility gaps are terminal | Lifecycle becomes explicit but adds supervision and recovery failure modes |
| Operability | Lowest change and weakest diagnosis | New transport stages, native fixtures, and stable attribution reasons | Highest packaging, update, diagnostics, and support burden |
| Migration | Minimal | Incremental attached-preflight path with published rollback | Foundational user workflow and authority change |
| Reversibility | Simple | Disable the new path and retain listener-only evidence | Requires separately versioned managed mode and authority records |

Option 2 has the most proportionate boundary. It spends performance and reliability
budget exactly where evidence needs stronger lifetime control, while preserving the
rest of the adapter during rollout. Option 1 is easier but does not meet the desired
invariant. Option 3 is stronger only after much broader prerequisites are complete.

## Recommendation

I recommend Option 2 for the next implementation slice. The evidence in E01, E02, and
E03 shows that the missing control is connection lifetime ownership, not another
listener snapshot. E04 through E06 also show why the output must be platform-scoped
and remain unqualified.

We should revisit the choice if native owner checks exceed the accepted latency
budget, Linux visibility fails on intended user configurations, or a public macOS
mechanism becomes mandatory for the milestone. In the first two cases, Option 1 may be
the honest temporary result, with `response_bound: false`. Option 3 becomes preferable
only when managed runtime authority is an explicit product decision supported by
package, configuration, isolation, and lifecycle designs.

## Evidence Coverage And Residual Risk

| Evidence | Option 1 | Option 2 | Option 3 | Tactical protection during migration |
| --- | --- | --- | --- | --- |
| `E01` - Pooled request transport | Unaffected | Addressed for attached preflight | Addressed | Never label pooled requests response-bound |
| `E02` - Multi-request preflight | Listener drift interval narrows | All requests share one retained socket | Requests join a managed lifecycle | Preserve exact request order and drift checks |
| `E03` - Bracketed attached process | Remains response-bound false | Gains point-in-time response evidence | Superseded only by a separate managed mode | Keep `qualified: false` in every mode until full closure |
| `E04` - Windows listener witness | Process drift only | Context PID matched for listener and established row | Child handle strengthens process identity | State that Windows is not exclusive socket proof |
| `E05` - Linux listener witness | Listener inode only | Established inode maps to exactly one visible same-UID holder under current policy | Child ownership reduces discovery dependence | Fail on observed permission, namespace, or enumeration gaps and do not claim invisible holders are absent |
| `E06` - macOS unsupported boundary | Unchanged | Unchanged | Potentially mitigated by managed launch, pending public design | Return stable unsupported result |
| `E07` - Witness contract | No connection lease | Extended with separate connection evidence and reasons | Requires separate managed capabilities | Preserve stable redacted errors and bounded observation |
| `E08` - Attached-process decision | Follow-up remains | Persistent-connection follow-up implemented narrowly | Folded into later managed design | Preserve the decision's non-qualification language |

Residual risk remains after Option 2:

- Windows supplies a context-binding PID rather than an exclusive set of holders.
- Linux sees only descriptors permitted by proc and namespace policy.
- A transient accepted-socket handoff can occur between snapshots.
- The immediate peer can proxy work elsewhere.
- Socket ownership does not identify the application handler that produced the bytes.
- The current executable file evidence is not proof of every loaded memory page.
- macOS attached response attribution remains unsupported.

These residuals mean Option 2 mitigates attribution ambiguity but does not create a
formal provenance guarantee.

## Migration And Rollout

We can introduce the selected option without changing existing report meaning:

- Add a new versioned connection-attribution evidence contract and report version.
- Keep the current attached listener-witness command available as the rollback path.
- Route only the new attached response-attribution path through the retained session.
- Leave ordinary discovery and generation on the existing adapter until separately
  reviewed.
- Ship Windows and Linux behind native deterministic fixtures. Keep macOS explicitly
  unsupported.
- Compare both paths on the same frozen local preflight plan before selecting the new
  path as the documented default.

Rollback disables the new command path and returns to the published listener-only
report. No qualification or activation record migration is required because both
reports are inert.

## Validation Plan

Security validation must prove the absence of a reconnect path, not merely count one
connection during a happy test.

- Use a retry canary: the first server closes and a second listener offers valid
  responses. The operation must fail and the second accept count must remain zero.
- Assert one connect, one HTTP handshake, `6 + N` ordered requests, one in-flight
  request, and complete body drain before the next request.
- Fail on close before headers, close mid-body, reset, FIN, idle expiry,
  `Connection: close`, tuple absence, changed owner, process exit, listener
  replacement, and non-established state.
- On Linux, transfer or duplicate the accepted descriptor across processes and require
  changed-owner or ambiguous-owner failure when the complete view exposes it.
- On Windows, test process incarnation and row attribution while asserting that the
  report names context PID scope rather than exclusive ownership.
- Put a forwarding proxy at the endpoint and require expected Ollama executable
  evidence to fail on the immediate peer.
- Inject cancellation and virtual-time deadlines at connect, owner resolution,
  handshake, send, response headers, body, between responses, post-response
  attribution, and final witness stages.
- Test exact and one-over limits for connection observation attempts, process and
  descriptor entries, aggregate response bytes, and request count.
- Place secret-like canaries in executable paths, environment, raw OS errors,
  response bodies, license, template, and proxy destinations. None may appear in
  Display, Debug, JSON failures, or traces.

Performance validation compares the current pooled listener witness with the retained
session for one and eight model targets. Record total wall time, connect and handshake
time, each owner-check duration, response decoding time, request count, peak resident
memory, and aggregate buffered bytes. No threshold should be invented before the
baseline is measured. The implementation review must set an explicit owner-check and
total-preflight budget before enabling the path by default.

## Implementation Work Packages

- Define typed retained-session identity, attribution scope, completeness, limits,
  and stable redacted failures.
- Implement one direct HTTP/1 preflight session with no connector or retry capability
  after construction.
- Extend Windows observation from listener rows to the reverse established row while
  preserving the context-PID claim.
- Extend Linux observation to the established socket inode and require exactly one
  visible same-UID holder under the retained pidfd and current namespace policy.
- Join post-response attribution to the existing final listener, process, executable,
  runtime, inventory, and residency checks.
- Version the inert report and preserve `qualified: false`.
- Add deterministic transport, platform, cancellation, resource, redaction, process,
  and CLI tests.
- Measure latency and memory before changing documented defaults.

The selected option has a complete handoff in
[implementation/single-retained-session.md](../implementation/single-retained-session.md).

## Open Questions

- What per-response owner-check and total-preflight latency budgets should gate
  rollout?
- Which Linux proc mount, ptrace, PID namespace, and network namespace profiles are
  supported?
- Should IPv4 and IPv6 enter the first native slice together?
- Can Windows expose a stronger supported connection object identity without added
  privilege?
- Is macOS attached response attribution required, or is deterministic unsupported
  behavior acceptable until managed runtime work?
- Should a successful immediate-peer proxy observation have a separate evidence class
  even though it cannot identify the upstream handler?
