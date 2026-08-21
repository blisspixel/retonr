# Implementation Plan: Single Retained HTTP/1 Session With Kernel Attribution

## Selected Design And Constraints

The selected design gives one attached Ollama preflight an owned transport lifetime:
one direct loopback TCP connection, one HTTP/1 handshake, ordered requests, and no
reconnect capability. The reverse established 4-tuple receives point-in-time kernel
attribution before application traffic and after every complete response.

The implementation must preserve these constraints:

- explicit HTTP IP-literal loopback endpoint;
- no DNS, proxy, redirect, implicit retry, runtime mutation, generation, or model
  load;
- bounded connection attribution, request count, response bytes, aggregate bytes,
  elapsed time, and cancellation latency;
- Windows evidence described as a context-binding PID, not exclusive ownership;
- Linux evidence requires exactly one visible same-UID holder under the current proc
  and namespace policy, fails closed on observed visibility gaps, and does not prove
  that invisible holders are absent;
- deterministic unsupported behavior on macOS;
- no claim of continuous exclusive ownership, application-handler execution, or
  absence of proxying;
- an inert versioned report with `qualified: false`.

The retained session is initially private to attached preflight. Ordinary discovery
and generation remain on the existing client until a separate design review chooses
otherwise.

## Source Revision And Drift Check

- Design source revision: `a58e07473eb558e3e38aa382de59af909f4a647b`
- Evidence collection digest:
  `e2663f4f1db2b548c38226c431caa8ae94f1fee74d364ad7b7f0ec208ea54a37`
- Source drift at plan creation: none

Before implementation, refresh the target revision and compare the following
boundaries with the evidence inventory in `../context.md`:

- request transport in `crates/ollama/src/backend.rs`;
- request sequence in `crates/ollama/src/backend/preflight.rs`;
- attached orchestration in
  `crates/eval/src/local_ollama_attested_preflight.rs`;
- native observer contract and Windows, Linux, and macOS adapters under
  `crates/runtime-attestor/src/`.

Return to design review if the preflight already owns a connection, the listener lease
no longer brackets the operation, the report has acquired authority, or platform
observer semantics changed.

## Affected Components

| Component | Planned responsibility |
| --- | --- |
| `crates/ollama/src/backend.rs` | Expose the selected preflight through a dedicated retained-session path without changing ordinary pooled calls. |
| `crates/ollama/src/backend/preflight.rs` | Run the exact existing logical request sequence on one session and invoke attribution after every response. |
| `crates/ollama/src/` new private session module | Own direct connect, HTTP/1 handshake, sequential request framing, body drain, aggregate limits, cancellation, and terminal close behavior. |
| `crates/runtime-attestor/src/contract.rs` | Add connection endpoint, attribution scope, completeness, evidence, limits, lease operation, and stable redacted errors. |
| `crates/runtime-attestor/src/platform/windows.rs` | Resolve the reverse established TCP row and compare its context PID with the retained process instance. |
| `crates/runtime-attestor/src/platform/linux.rs` | Resolve the reverse established socket inode and require exactly one visible same-UID holder under current proc and namespace policy to name the retained process instance. |
| `crates/runtime-attestor/src/platform/macos.rs` | Return the stable unsupported result for connection attribution. |
| `crates/eval/src/local_ollama_attested_preflight.rs` | Compose process and connection leases, version the report, and retain qualified false. |
| Native and deterministic tests in the affected crates | Prove one connection, no reconnect, owner checks, drift, limits, cancellation, redaction, and platform claim scope. |

The preferred dependency direction keeps platform evidence in
`rewrite-runtime-attestor` and transport mechanics in `rewrite-ollama`. A narrow
callback or port lets the retained session request a connection observation without
moving native types into the general inference contract. The eval layer owns the
adapter that holds the process lease and accumulates final inert evidence.

## Ordered Work Packages

### Work Package A: Freeze The Connection Evidence Contract

Define private invariant-bearing types for:

- normalized client and server socket addresses;
- one connection observation stage;
- attribution scope, including `windows_context_pid` and
  `linux_single_visible_same_uid_holder`;
- completeness and unsupported outcomes;
- a content-derived observation digest that does not expose the ephemeral client port
  in ordinary diagnostics;
- connection limits for owner-resolution attempts, socket-table bytes and rows,
  process and descriptor entries, aggregate response bytes, request count, and
  elapsed time.

Add stable reasons:

- `bound_connection_unavailable`;
- `bound_connection_owner_unavailable`;
- `bound_connection_owner_ambiguous`;
- `bound_connection_snapshot_incomplete`;
- `bound_connection_owner_mismatch`;
- `bound_connection_owner_changed`;
- `bound_connection_closed`;
- `bound_connection_replaced`;
- `bound_connection_protocol_violation`;
- `bound_connection_resource_limit`;
- `bound_connection_cancelled`;
- `bound_connection_deadline_exceeded`.

Reason priority is cancellation, deadline, invalid policy or resources, incomplete or
ambiguous ownership, process or owner change, connection close or replacement, HTTP
protocol failure, and existing runtime drift.

### Work Package B: Implement The One-Shot Retained Session

Use the already selected direct HTTP/1 stack to build a private session over
`tokio::net::TcpStream`:

- connect directly to `OllamaEndpoint::socket_addr()`;
- record `local_addr()` and `peer_addr()` once;
- perform one HTTP/1 client handshake;
- retain the sender and connection driver for the operation;
- send one request at a time with an exact IP-literal Host value;
- require HTTP/1.1 persistence and reject upgrade and `Connection: close`;
- stream and bound each response and the aggregate session bytes;
- fully drain a response before the next send;
- expose no connector, clone, retry, or second-handshake operation after successful
  construction;
- close the socket and abort or join the driver on every failure or cancellation.

Keep response status, media type, schema, and redaction policy equivalent to the
existing decoder. Factor shared pure validation where practical rather than allowing
the retained path and reqwest path to drift.

### Work Package C: Attribute The Established Connection

Immediately after connect, poll the native connection observer under both an attempt
ceiling and the operation deadline. A completed TCP connect can precede the server's
user-space `accept`, so an initially absent accepted owner is retryable only inside
this bounded pre-traffic stage. No application request may be sent until attribution
succeeds.

After every fully decoded response, observe once without reacquiring the connection.
Require the reverse 4-tuple to remain established and attributed to the pinned process
instance. Once an observation is absent, closed, ambiguous, incomplete, or changed,
the session is terminal. A later row with the same numeric tuple cannot revive it.

On Windows, query the documented established owner-PID table for the exact reverse
tuple and compare the row PID with the retained process handle and creation identity.
Record the attribution scope as context PID. Do not enumerate the PID and call it an
exclusive socket-holder set.

On Linux, identify the reverse established socket inode, enumerate bounded same-UID
descriptors visible under the current proc, PID namespace, and network namespace
policy, and require exactly one process instance matching the retained pidfd subject.
An observed permission, namespace, or enumeration gap returns snapshot incomplete.
Multiple visible process holders return owner ambiguous. Multiple descriptors inside
the same process remain one visible process holder. Success does not prove that no
invisible holder exists.

On macOS, return unsupported before connection work begins.

### Work Package D: Move The Logical Preflight Onto The Session

Preserve the existing sequence exactly. For `N` target models, send `6 + N` requests:

1. runtime version;
2. tags;
3. residency;
4. one show request per target, in canonical plan order;
5. confirming tags;
6. confirming runtime version;
7. confirming residency.

The list describes request order, not design alternatives. Every response is followed
by connection attribution before the next request. Preserve all existing runtime,
inventory, target-digest, details, residency, body, content-type, cancellation, and
deadline checks.

After the final connection observation, reobserve the listener, process,
entrypoint, and executable bytes through the existing lease. Only an identical final
witness may produce a report.

### Work Package E: Version The Inert Report

Add a new report version that distinguishes:

- listener and process witness evidence;
- retained client-socket continuity;
- platform connection-attribution scope;
- point-in-time observation count and digest;
- `qualified: false`.

Do not reinterpret an existing report version or change the meaning of
`response_bound: false`. If a Boolean response field is retained for compatibility,
pair it with a precise evidence class so `true` cannot be read as exclusive process or
handler proof. Prefer a descriptive enum over an authority-sounding Boolean.

### Work Package F: Complete Native And Deterministic Verification

Add deterministic transport fixtures first, followed by native Windows and Linux
owner fixtures. Run formatting, warnings-denied check and Clippy, unit and process
tests, documentation, policy, dependency review, audit, fuzz smoke, and coverage.

Do not enable the new path as the documented default until the benchmark record and
supported Linux visibility profiles are reviewed.

## Compatibility And Migration

- Preserve the existing read-only preflight plan and report decoders.
- Add a new command or explicit plan schema for retained response attribution.
- Keep current attached listener witness behavior as a published rollback path.
- Do not route ordinary discovery or generation through the new transport in this
  slice.
- Keep macOS deterministic unsupported behavior. Do not silently fall back to pooled
  response claims.
- Version connection evidence so future stronger mechanisms do not reinterpret
  Windows context PID or Linux conditional visibility.

No data migration is required because the evidence is inert and is not persisted as a
runtime-build, qualification, or activation record.

## Tactical Protections During Migration

- Preserve existing endpoint validation, no-proxy, no-redirect, no-retry, HTTP/1,
  body limits, content-type checks, and content-free errors.
- Preserve the outer listener, process, executable, runtime, inventory, and residency
  bracket until the retained session includes every equivalent check.
- Keep old reports at `response_bound: false` and every new report at
  `qualified: false`.
- Reject fallback to the pooled client after the retained session begins.
- Treat a complete HTTP response followed by a failed attribution check as discarded
  evidence.
- Keep the Windows and Linux platform scope explicit in both schema and prose.

## Tests And Security Validation

### Transport continuity

- Happy path accepts exactly one server connection, one client handshake, and
  `6 + N` ordered requests.
- All requests arrive on the same accepted fixture socket ID.
- The next request is not observed until the prior response body is released and
  fully drained.
- A counting connector or factory is invoked exactly once.
- A retry-canary second listener is never contacted after the first connection
  closes, resets, times out, or fails attribution.
- HTTP/1.0 without persistence, `Connection: close`, upgrade, mid-header EOF,
  mid-body EOF, reset, FIN, and idle expiry all discard the complete operation.

### Connection identity and drift

- The retained socket's local and peer addresses remain exact.
- An absent reverse row is terminal after initial attribution.
- Reappearance of the same numeric tuple does not revive a failed session.
- Same PID with different process creation or start identity fails.
- Same process with a replaced listener fails final listener reobservation.
- Same listener owner with a changed executable fails final process reobservation.
- A forwarding proxy that returns correct Ollama bytes fails an expected Ollama
  executable match at the immediate peer.

### Ownership ambiguity

- Linux accepted-socket transfer to a child produces changed-owner failure.
- Linux accepted-socket duplication across two processes produces ambiguous-owner
  failure when both are visible.
- Multiple descriptors inside one process remain one process owner.
- Proc permission, PID namespace, network namespace, or enumeration-limit gaps return
  snapshot incomplete.
- Windows process-incarnation tests prove PID reuse does not satisfy a retained
  process handle.
- Windows report serialization names context-PID attribution and contains no
  exclusivity claim.

### Cancellation and deadlines

Inject cancellation and paused-time deadline expiry:

- before connect;
- while connect is pending;
- during initial owner polling;
- during HTTP handshake;
- while sending;
- while waiting for response headers;
- while streaming the body;
- between responses;
- during post-response attribution;
- during final listener and executable reobservation.

Every case closes the socket, stops later requests, performs no reconnect, drops the
process lease, emits no partial report, and returns the stable highest-priority reason.

### Limits and redaction

- Test exact-limit success and one-over failure for attribution attempts, socket table
  bytes and rows, process and descriptor entries, request count, per-response bytes,
  aggregate bytes, and elapsed stages.
- Put secret-like canaries in executable paths, environment, raw OS errors, response
  bodies, model license, template, and proxy destinations.
- Assert that Display, Debug, JSON failures, traces, and cancellation errors contain
  no canary.
- Successful reports expose only bounded public facts and opaque digests.

Target at least 95 percent line coverage for retained-session orchestration and at
least 90 percent for native connection observers, with every material failure branch
represented. Workspace line coverage remains at least 80 percent.

## Performance And Resource Benchmarks

Compare the published pooled listener-witness path with the retained session using
the same deterministic one-model and eight-model fake-server plans. Record:

- total operation time;
- connect and HTTP handshake time;
- initial owner-resolution time and attempts;
- each post-response attribution time;
- response decode time;
- request and accepted-connection count;
- process and descriptor entries inspected;
- peak resident memory;
- maximum response and aggregate buffered bytes;
- cancellation time at connection, body, and post-response stages.

Run native Windows and Linux measurements separately because their attribution cost
mechanisms differ. Report distributions and outliers rather than one mean. The first
benchmark establishes the baseline. Project maintainers must approve explicit
owner-check and total-operation thresholds before the retained path becomes the
documented default.

## Rollout And Rollback

Roll out in this order:

- land contracts and deterministic fake tests without changing the documented path;
- land the private retained session and retry-canary tests;
- land Windows established-row attribution and native fixtures;
- land Linux established-inode attribution and visibility fixtures;
- land the new eval report and process-level command tests;
- publish benchmark and supported-environment evidence;
- enable the path only after all cross-platform gates pass.

Rollback is an explicit command-path change. Disable the retained response-attribution
path and retain the published listener-witness preflight, whose report already states
`response_bound: false` and `qualified: false`. Because no authority or durable state
is created, rollback requires no database migration or evidence revocation.

## Acceptance Criteria

- Source drift is reviewed against revision
  `a58e07473eb558e3e38aa382de59af909f4a647b` before coding.
- One preflight creates exactly one TCP connection and exactly one HTTP/1 handshake.
- Exactly `6 + N` requests use that retained socket in canonical order.
- No failure path invokes a second connect, connector, handshake, or pooled request.
- Initial attribution succeeds before application traffic.
- The reverse established 4-tuple matches the retained process after every complete
  response.
- Any close, absence, non-established state, ambiguity, incomplete visibility, owner
  drift, process drift, listener drift, executable drift, cancellation, deadline, or
  resource exhaustion fails closed and discards all evidence.
- Windows output says context-binding PID and never exclusive socket ownership.
- Linux succeeds only with exactly one visible same-UID holder in the supported namespace
  profile.
- macOS returns stable unsupported before connection work.
- Proxy tests prove the evidence names only the immediate peer.
- Errors, traces, Debug, and JSON failures pass all redaction canaries.
- The report is versioned, inert, and always sets `qualified: false`.
- Existing preflight reports retain their prior meaning.
- Formatting, warnings-denied check and Clippy, tests, documentation, policy, audits,
  fuzz smoke, and coverage pass on every supported CI platform.
- Native latency and memory results are recorded before default enablement. If no
  budget is approved, the path remains explicit and non-default.

## Open Decisions

- Choose the narrow callback or port shape joining the Ollama session to the retained
  platform process lease without moving native evidence into the general inference
  contract.
- Set owner-resolution attempt, per-observation time, and total-operation budgets.
- Define the supported Linux proc mount and namespace profiles.
- Decide whether IPv4 and IPv6 ship together in the first slice.
- Decide whether a response evidence enum replaces or supplements the existing
  `response_bound` Boolean.
- Determine whether a stronger supported Windows connection identity is available
  without increasing privilege.
- Keep macOS unsupported or begin a separate managed-runtime design review.
