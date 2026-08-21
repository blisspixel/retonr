# Retained Connection Binding: Evidence Context

## Analysis identity

- Target revision: `a58e07473eb558e3e38aa382de59af909f4a647b`
- Source drift at analysis time: none
- Evidence mode: inspected repository source and existing proposed architecture decision
- Collection digest: `e2663f4f1db2b548c38226c431caa8ae94f1fee74d364ad7b7f0ec208ea54a37`
- Collection digest method: SHA-256 over the ordered repository-relative path, NUL,
  Git blob ID, and newline records listed below
- Artifact count: 8

This is a derived security design review, not a vulnerability finding and not proof
that the selected hardening has been implemented. The evidence establishes the
current transport and witness boundaries. The response-attribution properties and
tradeoffs are proposed or inferred unless explicitly marked observed.

## Evidence inventory

| Evidence | Title | Repository-relative source | Git blob ID | What it establishes |
| --- | --- | --- | --- | --- |
| `E01` | Pooled request transport | `crates/ollama/src/backend.rs` | `c0cee27e3bb08949558d76552b527d97a7c76bce` | The adapter owns a reusable `reqwest::Client`; each helper sends a request through the client without exposing the selected TCP connection. |
| `E02` | Multi-request read-only preflight | `crates/ollama/src/backend/preflight.rs` | `4f5ca3942fd69cd0b83ec7d237e819f8c4a12c96` | One preflight performs ordered version, inventory, residency, model-detail, and confirming observations, but the operation has no connection-level lease. |
| `E03` | Bracketed attached-process orchestration | `crates/eval/src/local_ollama_attested_preflight.rs` | `ede26a574187cb0bebb032dbcb337864aae51b2d` | The native listener witness brackets the existing HTTP preflight and deliberately reports `response_bound: false` and `qualified: false`. |
| `E04` | Windows listener witness | `crates/runtime-attestor/src/platform/windows.rs` | `c1e214ea309f1c677af31453f7fdb673c7ce5df9` | Windows observation uses the public TCP owner-PID table and retains a process handle, but it does not expose an exclusive accepted-socket owner. |
| `E05` | Linux listener witness | `crates/runtime-attestor/src/platform/linux.rs` | `4159a43e061d259aad5f920b06493628a85c0fa8` | Linux observation maps a listener socket inode to a unique visible same-user descriptor owner and retains a pidfd; the view is conditional on proc and namespace policy and does not exclude invisible holders. |
| `E06` | macOS unsupported boundary | `crates/runtime-attestor/src/platform/macos.rs` | `02a6c571728dc4b74b409b3ed3de0c823c39d766` | macOS returns the stable unsupported result rather than using private listener-ownership APIs. |
| `E07` | Stable witness contract | `crates/runtime-attestor/src/contract.rs` | `b1e92b5ee49f9e15275eb928c806d89aba684952` | The current public evidence and error vocabulary describe point-in-time listener ownership, executable evidence, bounded observation, cancellation, and drift. |
| `E08` | Proposed attached-process witness decision | `docs/decisions/0008-attached-process-witness.md` | `e737f540aa24a88e337584d676f6ff79dec2d5ac` | The proposed decision explicitly leaves persistent connection ownership and exact 4-tuple attribution as follow-up work and forbids response-attestation overstatement. |

## Claim discipline

Observed claims are facts visible in the listed revision. Inferred claims explain the
security consequences of those facts. Proposed claims describe behavior that does not
exist at the target revision.

The platform evidence has three important ceilings:

- Windows reports a context-binding PID for a TCP row. It does not prove that no
  accepted socket handle was duplicated or handed to another process.
- Linux can map a socket inode to visible same-UID process descriptors under the
  current proc, PID namespace, and network namespace policy. The first slice requires
  exactly one such visible holder and fails closed on observed permission or
  enumeration gaps. Unprivileged enumeration cannot prove that no invisible holder
  exists.
- macOS has no supported public unprivileged listener-to-process ownership API used by
  this repository. It remains unsupported for attached response attribution.

Even a successful before-and-after kernel observation is point-in-time evidence. It
does not prove exclusive socket ownership between observations, prove which thread or
application handler produced bytes, or prove that the immediate peer did not proxy a
request elsewhere.
