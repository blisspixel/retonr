# Effective Runtime Trust Chain: Evidence Context

## Analysis identity

- Target revision: `c3657edd126f164facc311719b831d6926e7c06d`
- Source drift at analysis start: none
- Evidence mode: inspected repository source, existing decisions, roadmap contracts,
  and current primary platform and provider documentation
- Repository collection digest:
  `6a9d8e2195834f5d123add1626048d468b1b8bfd68eedb8bfaae022efa59ec3c`
- Collection digest method: SHA-256 over each ordered repository-relative path,
  NUL, Git blob ID, and newline record listed below
- Repository artifact count: 18

This directory is a derived security design review. It is not a vulnerability
finding and it is not proof that proposed controls have been implemented. Observed
claims describe the target revision. Inferred claims explain consequences of those
facts. Proposed claims describe selected work that still requires implementation and
verification.

## Post-analysis status

This evidence inventory is intentionally frozen to the target revision above. Later
work in the same development slice implemented the selected Linux SOCK_DIAG path,
typed runtime and model package contracts, schema-6 persistence, retained package
objects, Linux native-load observation, strict offline Ollama model reconstruction,
Linux managed namespace isolation and attestation, inert cloud-disable evidence, and
a Linux-only read-only managed preflight that joins the runtime-side evidence. The
same slice later added a v0.32.15-only static installed-model-to-idle-inventory binding
and a retained-session local-judge executor with a separate limited transport receipt.
The static binding consumes an opaque, nonserializable, single-use receipt from the
exact preflight runner. Retained sessions reject UTF-8 input above the absolute 4 MiB
ceiling before wire serialization or completion traffic. An opt-in managed outcome
now constructs package-declared typed runtime-build identity after the exact managed
package, process, and native-load join. Only the exact entrypoint is joined to live
evidence; other package semantics are not independently live-observed, cleanup
completes before return, and effective runtime state remains false. The managed target
inherits a seccomp socket allowlist that admits only `AF_INET` and `AF_INET6` through
`socket()`, denies every other socket family and `io_uring_setup`, and requires mode 2
on target reobservation. A separate opt-in v0.32.15 retained completion binds two
equal runtime-reported post-generation residency observations while model use and
resident-page identity remain false. Those additions do not retroactively change the
baseline evidence IDs or collection digest. The managed report and build binding,
static model binding, residency receipt, and judge receipt remain separate, inert,
and outside the CLI. The production cloud-disable allowlist is empty, attached
evidence remains observation-only, Windows managed isolation and exact native-load
binding are unsupported, and macOS is unsupported. See
[Current state](../../current-state.md) for the live implementation boundary.

## Repository evidence inventory

| Evidence | Title | Repository-relative source | Git blob ID | What it establishes |
| --- | --- | --- | --- | --- |
| `E01` | Linux listener lookup | `crates/runtime-attestor/src/platform/linux.rs` | `379804967fd6de8cc50bbadbf07502243fd2ebbf` | Listener discovery reads `/proc/net/tcp*`, resolves one visible same-UID descriptor holder, and retains a pidfd. |
| `E02` | Linux retained-connection lookup | `crates/runtime-attestor/src/platform/linux_connection.rs` | `6858bbfaf1e7ff1bc7766b94fe8f11e594108403` | Exact reverse-tuple selection currently reads `/proc/net/tcp*`, retains an inode, and checks visible same-UID holders. |
| `E03` | Connection evidence contract | `crates/runtime-attestor/src/connection.rs` | `d452d7848600823ef42aa98be32fbe54ff77c518` | Published evidence deliberately disclaims exclusive socket ownership and handler execution. |
| `E04` | Native witness contract | `crates/runtime-attestor/src/contract.rs` | `340aab433d4dd2e3902ab1f0167e75131bb95f55` | Native observation is byte, row, process, descriptor, time, and cancellation bounded. |
| `E05` | Retained witness orchestration | `crates/runtime-attestor/src/lib.rs` | `2223ef5eb7ee76432514b2ded3feb5b017a8b505` | Initial connection publication is retried under fixed limits; later absence and drift fail closed. |
| `E06` | Generic artifact-set manifest | `crates/model/src/artifact_set.rs` | `d3a8a7c4e1629acbf316b2386b6dfe98aa44e18a` | The canonical path and byte set is strong structural identity, but it does not prove semantic completeness or runtime use. |
| `E07` | Effective-package evidence | `crates/model/src/effective_package.rs` | `6798acb5e3e922801738dfc103e65099df2b04e0` | Version 1 binds output-affecting member purposes, but cannot honestly classify evidence-only license and provenance members. |
| `E08` | Runtime-build identity | `crates/model/src/runtime_identity.rs` | `fa70b527ab549d500cb77b150c144a9561d1849d` | Package, dependency, and build-configuration digests are accepted as structurally valid opaque inputs. |
| `E09` | Effective-runtime state | `crates/model/src/runtime_identity/state.rs` | `3828c4b783072e9a17f5152a4e55cac883d42c9a` | Loaded components, configuration, platform, execution, and isolation are accepted as structurally valid opaque digests. |
| `E10` | Ollama preflight contract | `crates/ollama/src/contract.rs` | `78e8ac61240ffb61f185cd43177bfb4fc5b84164` | An Ollama inventory digest is explicitly insufficient to identify a complete artifact set. |
| `E11` | Read-only Ollama preflight | `crates/ollama/src/backend/preflight.rs` | `4f5ca3942fd69cd0b83ec7d237e819f8c4a12c96` | The adapter brackets version, inventory, residency, and model-description observations without generation. |
| `E12` | Bound preflight orchestration | `crates/eval/src/local_ollama_bound_preflight.rs` | `d8c738cb3a24e0c0c32cfa545d8f659593d5dd3a` | One retained transport is repeatedly attributed, but the report remains inert and unqualified. |
| `E13` | Managed attestation service | `crates/app/src/runtime_attestation.rs` | `135f2c2c1144c7d418fe390456395356b58d44c8` | The service hashes a caller path, retains no process, and accepts critical identity digests from the caller. |
| `E14` | Runtime artifact-set lease | `crates/app/src/runtime_artifact_set_lease.rs` | `e0a0d913e896bf8758b990925501dd562966afe0` | The lease revalidates a complete tree under lifecycle locks but retains no handle for every code member through use. |
| `E15` | Import verification boundary | `crates/app/src/artifact_set_import/verify.rs` | `05a2721b2db04005cc05734bf6b1dbeeac7b0161` | Import verification already brackets exact tree snapshots and opened-file hashing while rejecting unsupported objects and drift. |
| `E16` | Retained-connection decision | `docs/decisions/0009-retained-connection-attribution.md` | `f435cc896b376aaca8bc0b5ca8c96fd3f9ca5cf9` | The decision names Linux socket diagnostics, complete package identity, cloud disablement, and OS isolation as prerequisites for effective identity. |
| `E17` | Roadmap dependency order | `docs/roadmap.md` | `c61235bd9d1f896726b63d4416648cd9d0838b5d` | Milestone 0.2 requires OS-enforced non-loopback denial and exact runtime and artifact identity before qualification. |
| `E18` | Grounded CLI plan | `docs/planning/0.2-grounded-cli.md` | `1c2f0fb077eaa98b6e7c08c5e0bcaeca36f1fca6` | The execution plan keeps qualification downstream of package, runtime, isolation, and evaluation evidence. |

## Primary external references

These sources inform proposed mechanisms. They are linked directly so a reviewer can
recheck the current platform or provider contract.

- [Linux Netlink userspace API](https://docs.kernel.org/userspace-api/netlink/intro.html)
- [Linux internet socket diagnostics UAPI](https://github.com/torvalds/linux/blob/master/include/uapi/linux/inet_diag.h)
- [Linux TCP socket diagnostics implementation](https://github.com/torvalds/linux/blob/master/net/ipv4/tcp_diag.c)
- [Linux process mapping documentation](https://www.kernel.org/doc/html/latest/filesystems/proc.html)
- [Linux user namespace setup](https://docs.kernel.org/userspace-api/unshare.html)
- [Linux no-new-privileges contract](https://docs.kernel.org/userspace-api/no_new_privs.html)
- [Microsoft WFP ALE connect fields](https://learn.microsoft.com/en-us/windows/win32/api/fwpsu/ne-fwpsu-fwps_fields_ale_auth_connect_v4)
- [Microsoft WFP object management](https://learn.microsoft.com/en-us/windows/win32/fwp/object-management)
- [Microsoft Job objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [Apple content filter providers](https://developer.apple.com/documentation/networkextension/content-filter-providers)
- [Apple Network Extension deployment](https://developer.apple.com/documentation/technotes/tn3134-network-extension-provider-deployment)
- [Ollama cloud-disable documentation](https://docs.ollama.com/faq)
- [Ollama v0.32.14 cloud status type](https://github.com/ollama/ollama/blob/v0.32.14/api/types.go)
- [Ollama v0.32.14 configuration source](https://github.com/ollama/ollama/blob/v0.32.14/envconfig/config.go)

## Falsifiable trust-chain invariants

- A retained Linux TCP connection is accepted only when the kernel returns the exact
  server-side tuple, established state, expected UID, nonzero inode, and stable socket
  cookie, and the visible same-UID holder set contains only the retained process.
- A static package manifest identifies exact bytes and declared roles. It never claims
  that a process loaded those bytes.
- Loaded-component identity comes only from a bounded native observation tied to the
  retained process incarnation and a reviewed static runtime package.
- Provider cloud-disable evidence remains separate from kernel network enforcement.
- A qualified local-only operation launches only after isolation policy installation
  and readback, retains that policy for the complete process tree lifetime, and fails
  closed on drift or guardian loss.
- Effective runtime identity and evaluation authority are constructed only after all
  required evidence joins match exact versions and identities without drift.

## Non-goals and limitations

- Linux proc descriptor enumeration cannot exclude holders hidden by UID, ptrace,
  proc-mount, PID namespace, or security policy.
- Windows application identity is not exact PID identity, and public connection tables
  do not enumerate every duplicated socket handle.
- Socket attribution does not prove application-handler execution.
- Static package identity does not prove resident-page integrity or prevent arbitrary
  in-memory modification.
- Provider configuration does not prove network isolation.
- Attached, already-running runtimes are not eligible for local-only qualification.
- macOS qualification remains unsupported until a signed and approved system-extension
  design passes dedicated physical-hardware validation.
