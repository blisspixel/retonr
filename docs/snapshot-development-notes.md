# Snapshot development notes

These notes record development-library behavior that ships beside a snapshot or
lives in the source repository. They are not the hands-on snapshot workflow.
Start with [Testing a development snapshot](testing-snapshot.md).

## What you are testing

Retonr is a local-first editorial engine. The snapshot's primary useful workflow is
**fidelity checking**. It also exposes pre-model source inspection, deterministic
editorial lint, a provisional `rewrite` command that refuses without a selected
qualified local artifact and does not start a runtime, and
offline artifact administration. The experimental read-only TUI presents source
inspection, lint, and supplied-candidate checking together.

You bring two documents. The first is your original. The second is a complete
rewritten version of it, produced by any tool or by hand. Retonr answers one
question: did the rewrite change anything that was supposed to stay fixed?

It checks quantities, URLs, email addresses, identifiers, declared protected
terms, document structure, newline shape, and unsafe control characters. If the
rewrite preserved all of them, Retonr reports `rewritten` and can write the
accepted bytes out. If it did not, Retonr reports `abstained` with a reason and
gives you back the exact original bytes instead.

The snapshot does not generate text. It never contacts the network.

The Linux archive also contains the internal `retonr-isolation` helper so its exact
bytes can participate in runtime-package review. The snapshot CLI does not invoke
that helper, and the helper is not a standalone user command.

Each archive includes `THIRD-PARTY-NOTICES.txt` with complete upstream legal
materials for its locked default shipping dependencies and Rust standard library.
Crate archives are checked against their locked checksums. Retained upstream
materials cover declarations omitted from published crate archives; their exact
source, version, and content digest are pinned in the repository.

The source repository also contains a development-only
`rewrite-eval --ollama-bound-preflight` command. It is not part of this snapshot CLI
workflow. When a developer explicitly supplies a versioned plan, that command may
contact only its IP-literal loopback Ollama endpoint. It sends the complete read-only
preflight over one directly connected retained HTTP/1 transport and checks native
connection attribution before traffic and after every fully drained response. It
does not generate, acquire, activate, qualify, or authorize a model. macOS refuses
the command before HTTP. Successful Windows and Linux reports remain unqualified and
explicitly do not prove exclusive socket ownership or application-handler execution.
Linux selects socket rows through bounded `NETLINK_SOCK_DIAG` and still depends on a
complete visible same-UID descriptor view. Within that view, the holder scanner
retains the proc root, anchors each process with a pidfd, strictly parses the bounded
effective-UID status records, and inspects descriptor links relative to the held
process directory. A second anchored status read rejects effective-UID drift across
descriptor inspection. Access denial or an incomplete view fails closed.

Linux development libraries also implement managed user, network, and PID namespace
isolation, retained-handle launch, loopback-only transport, namespace-local process
attestation, and exact native-load observation when host policy permits. The managed
target inherits a seccomp socket allowlist before launch: `socket()` permits only
`AF_INET` and `AF_INET6`, every other socket family and `io_uring_setup` are denied,
and target reobservation requires seccomp mode 2. A development-only `rewrite-eval`
library API joins those boundaries with a retained runtime package, exact connection
evidence, the cloud-disable declaration and startup marker, and read-only Ollama
observation. Its inert report has no CLI surface and explicitly does not prove model
use, effective-runtime identity, or qualification. Windows managed isolation and
exact native-load binding are unsupported. macOS managed isolation, attached
attribution, and native-load binding are unsupported. The provider cloud-disable
contract has an empty production reviewed-runtime allowlist, so the managed report
remains unreviewed.

The repository's Linux CI does not treat an uncontrolled worker as proof of this
native boundary. Ordinary tests may accept only a typed access-denied compatibility
outcome when host proc policy blocks observation. Linux CI uses three separate native
proof gates. The managed attestor runs networkless as the caller UID with all
capabilities dropped and no-new-privileges set. The statically linked worker fixture
runs networkless with only `CAP_SETPCAP` and `CAP_CHECKPOINT_RESTORE` on its root test
process. The latter permits exact `/proc/<pid>/map_files` observation. The fixture
drops both capabilities before worker execution and requires the exact worker, native closure,
private GGUF mapping, stable reobservation, and post-exit rejection path. The
controlled source-build fixture runs in a privileged, networkless container and requires native success for two
namespace-private quota-bounded tmpfs attempts, descendant drain, helper tree
commitment, verified host export, independent application rehash, portable original
path and ordinary Unix permission-bit commitment, special-bit rejection, durable
publication, reacquisition, and blocked schema-2 review. All three executions contribute to the
workspace LLVM profile before the 80 percent line floor is checked.

Process-holder attestor fixtures run serially in both the ordinary Linux test
scheduler and coverage. Concurrent child creation can temporarily inherit a
listener and interfere with exact ownership observation. Coverage still runs every
attestor target and feature, followed by the required native gates.

An opt-in managed-preflight library call can return the unchanged report with a
separate inert package-declared typed runtime-build binding. Only the exact entrypoint
is joined to managed process and native-load evidence; target, revision, and other
package semantics are not independently live-observed. Cleanup is complete before
return; the process is not retained, no effective state is constructed, and model
use, handler execution, and qualification remain false.

Other development-only libraries can bind one inert installed model package to one
exact verified idle Ollama v0.32.15 inventory and details observation, and can run the
neutral local-judge contract over an already-preflighted retained stream. These are
not snapshot CLI commands. The static binding consumes an opaque, nonserializable,
single-use receipt issued by the exact preflight runner and leaves model loaded, model
used, handler, effective identity, and qualification false. Retained-session input is
limited to an absolute 4 MiB of UTF-8 before wire serialization or completion
traffic. The judge executor emits a separate nonserializable transport receipt, but
that receipt does not prove managed isolation, handler execution, model load or use,
candidate generation, effective identity, semantics, or qualification.

The retained Ollama session also has an opt-in v0.32.15 completion profile that
requires two equal singleton runtime memory reports after generation. Its separate
nonserializable receipt proves stable runtime-reported residency on that transport
only. Runtime memory size is not package inventory size, and the receipt does not
prove handler execution, model use, resident-page identity, effective identity, or
qualification. The legacy completion path is unchanged.

An admission-gated development library operation composes that profile with the
managed Linux process, runtime package lease, native observer, direct connection,
static model binding, and post-generation revalidation. It keeps the immutable model
artifact digest distinct from the mutable Ollama inventory digest and records the
runtime-reported effective context length. The exact runtime must have a reviewed
production cloud-disable disposition before launch. Because the allowlist is empty,
this snapshot blocks the operation before generation. It has no CLI surface and does
not prove model weight use, handler execution, complete effective identity, semantic
correctness, or qualification.

The same snapshot also administers exact local model artifacts offline. These
commands copy, inspect, migrate, or remove local files. They do not download,
qualify, activate, or run a model:

```console
retonr --data-dir <DIRECTORY> model import <ARTIFACT> --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model import-set <SOURCE_ROOT> --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model inventory
retonr --data-dir <DIRECTORY> model inventory-set
retonr --data-dir <DIRECTORY> model pending-operations
retonr --data-dir <DIRECTORY> model migrate --yes
retonr --data-dir <DIRECTORY> model reconcile --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model reconcile-set --manifest <MANIFEST_JSON>
retonr --data-dir <DIRECTORY> model remove --artifact-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model recover-removal --artifact-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model remove-set --artifact-set-id <SHA256> --installation-generation <N> --yes
retonr --data-dir <DIRECTORY> model recover-set-removal --artifact-set-id <SHA256> --installation-generation <N> --yes
```
