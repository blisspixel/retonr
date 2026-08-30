# Controlled runtime source-build support

This directory contains reviewed source transformations needed only to prepare the
frozen tool bundle used by the separately authorized Ollama runtime qualification
workflow.

`ninja-1.12.1-retonr-shell.patch` removes Ninja's ambient `/bin/sh` and `/dev/null`
opens. The patched binary requires `RETONR_NINJA_SHELL` to name one inherited
`/proc/self/fd/<number>` capability. The static runtime source-build coordinator
opens the frozen BusyBox executable, clears close-on-exec for that one descriptor,
and supplies it to Ninja. The patch does not grant network or filesystem authority.
Landlock still limits the shell and every child to the verified input tree and the
fresh output tree.

Controlled-build capability ABI 2 creates a private mount namespace, makes mount
propagation recursively private, and mounts a 16 GiB, 262,144-inode tmpfs at `/tmp`.
The quota reserves 4 GiB beyond the 4 GiB private-input and 8 GiB output ceilings
for extraction, compiler targets, Cargo state, and other bounded build scratch.
Stage one constructs the fixed `/tmp/retonr-controlled-build/input` alias inside that
tmpfs from the exact retained descriptor, path, byte count, and SHA-256 of every
frozen member. It rejects more than 4 GiB before creating any private file, copies
each member from offset zero, verifies exact length, trailing EOF, digest, file type,
link count, and retained identity, normalizes its mode, and remounts the complete input
root read-only. The caller's mutable input root is neither bound nor exposed to the
target. Before the first probe, the parent separately copies the exact reviewed helper
into an executable anonymous file, verifies its byte count and SHA-256, and applies
immutable write and size seals. The fixed
`/tmp/retonr-controlled-build/output` alias is a distinct empty directory in the same
private tmpfs. The target never receives or mounts the retained host output. Landlock
grants read and execute access only to the constructed input tree, exact read and
write access to `/dev/null`, and write access only to the private output while denying
other device files. The parent independently observes and retains the mount
namespace before execution, and the helper and coordinator separately verify the
descriptor, alias, program, and output working-directory relationships. After the PID
namespace drains, the guardian commits at most 4,096 entries and 8 GiB of logical
regular-file bytes, exports them without replacement through retained descriptors,
and rehashes the retained host tree. The application independently reopens, rehashes,
and seals that exact commitment before accepting build evidence.

The digest-verified private snapshot and read-only remount prevent target writes,
pathname substitution, and same-inode mutation from changing bytes consumed after the
snapshot. Pre-run and post-run retained-boundary revalidation still detects later
stable drift. These checks establish exact byte and filesystem-boundary behavior, not
source lineage, license, transformation correctness, reproducibility, or semantics.

Child-tool output is drained concurrently to prevent pipe deadlock. Successful tool
output is discarded. On failure, the coordinator relays only the final 16 KiB of each
stream with a stage label; the outer controlled-build boundary retains at most a
32 KiB prefix per stream. Serialized evidence contains only bounded identities and
digests, never those raw bytes.

The prepared Ninja archive must also contain this exact marker at
`ninja/retonr-shell-contract.json`:

```json
{"environment":"RETONR_NINJA_SHELL","schema_version":1}
```

The marker file includes one final LF byte. The coordinator compares every byte,
including that terminator, before it grants Ninja any build authority.

The checked-in `ollama-v0.32.15-parameters.json` is the canonical transformation
parameter record for the first controlled build. Its digest is a required frozen
input and must match the digest declared by the final runtime layout. The build
uses Go `GOAMD64=v2` and passes Zig's `-march=x86_64_v2` spelling to the native
compiler so the `x86-64-v2` input policy and produced baseline agree. The parameter
record is compact canonical JSON followed by exactly one LF byte.
Native compilation is capped at one job because the retained Zig 0.15.1 compiler
can require about 14 GiB for one AVX-512 repack translation unit. The
manifest compiler reads this ceiling from the canonical parameter record rather
than duplicating it as an unrelated launch default.

The coordinator rejects upstream or locally modified Ninja payloads that lack the
marker. Artifact digests, source revision, patch bytes, compiler identity, build
logs, and license disposition belong in the frozen bundle evidence. This directory
does not itself qualify a compiled tool.

`llama-cpp-zig-clang20-evex512.patch` is a separate retained source input. It adds
Clang 20's explicit `-mevex512` permission only to llama.cpp CPU variants that
already request AVX-512. Without it, Zig 0.15.1 rejects AVX-512 intrinsics because
its Clang 20 frontend keeps EVEX-512 disabled. The patch does not raise the portable
baseline or enable AVX-512 in the baseline backend. It also compiles only
`ggml-cpu/arch/x86/repack.cpp` at `-O0` with LLVM vectorization disabled; Zig's
`-O3` compilation of that source otherwise exceeds 20 GiB on the qualification
host. Every other native translation unit remains at `-O3`.

llama.cpp also compiles `llama-ui-embed` as a build-host helper during a cross
build. The coordinator assigns a distinct frozen compiler shim for this one tool.
It targets `x86_64-linux-musl` with static linkage so the helper can run without
an ambient glibc loader or host libraries. Runtime artifacts continue to target
the separately recorded `x86_64-linux-gnu.2.28` baseline.
Native executables, shared libraries, and module libraries are stripped by the
linker so a strip failure is a link failure. The install phase does not rely on
Zig `objcopy`, whose 0.15.1 strip operation is not implemented for these outputs.
The Cgo-enabled Go executable also passes `-Wl,--strip-all` to its external
linker because Go's `-s -w` does not remove debug sections contributed by Cgo
objects.

CMake installs ten same-directory shared-library aliases as symbolic links. The
coordinator accepts only the exact alias names and link targets recorded in the
canonical parameter record, requires the remaining 21 installed members to be
nonempty direct files with one link, and rejects extra or missing entries. It then
copies each approved alias target into the final package as an ordinary file. The
managed output therefore preserves the loader-visible names without retaining
indirect filesystem entries or following an unreviewed link.

`ollama-mlx-reproducible-errors.patch` removes `__DATE__` and `__TIME__` from
the MLX dynamic-loader error prefix. Those macros are nondeterministic source
inputs and Zig rejects them under the controlled reproducible-build settings.
The retained error still reports its stable source file and line number.

The preparation binaries require absolute paths and create outputs without
replacement:

```text
cargo run -p rewrite-runtime-source-builder --bin rewrite-runtime-source-archive -- <kind> <absolute-source-directory> <absolute-archive>
cargo run -p rewrite-runtime-source-builder --bin rewrite-runtime-source-manifest -- evidence <absolute-component-root>
cargo run -p rewrite-runtime-source-builder --bin rewrite-runtime-source-manifest -- license-review-template <absolute-component-root> <absolute-review-json>
cargo run -p rewrite-runtime-source-builder --bin rewrite-runtime-source-manifest -- licenses <absolute-component-root> <absolute-review-root>
cargo run -p rewrite-runtime-source-builder --bin rewrite-runtime-source-manifest -- manifest <absolute-component-root> <absolute-manifest>
```

Run the manifest preparation operations in the listed order. `evidence` writes the
non-license provenance records first. `license-review-template` then enumerates every
exact non-license component and every package in the retained `Cargo.lock`. It writes
a compact canonical review template with a final LF and one domain-separated exact
subject identity per entry. The template destination must be outside the component
root and must not already exist.

The completed review input is a separate directory containing `review.json` and a
`materials` tree. A reviewer must keep the generated subject order and identities,
enter one reviewer-declared SPDX expression for every subject, and select at least
one actual UTF-8 legal material for every subject. Each material selection records
its evidence kind, its reviewer-facing source-relative path, and a `source_path`
relative to the review directory's `materials` tree. Applicable license texts,
notices, attribution records, exceptions, and other legal text must all be retained.
The review file remains `pending_review`; the generator makes no legal or admission
decision.

`licenses` rejects changed subject identities, missing or reordered subjects,
unsorted or empty material selections, noncanonical or ambiguous JSON, indirect
files, non-UTF-8 material, and fixed quota excess. It preserves each selected
material's exact UTF-8 bytes in canonical schema-2 `legal/licenses.json`, computes
its byte size and SHA-256 digest, and writes the inventory without replacement. It
does not fetch or infer material. The fully populated review directory is therefore
an offline setup input and requires human review before compilation. `manifest`
then measures the finished schema-2 inventory with the other exact components.

Treat a component root as immutable once it has been measured or executed. Create
a new candidate root for any changed member and compile a new manifest. This is
required for evidence hygiene and also avoids stale executable page mappings when a
Linux proof runs through WSL over DrvFS. Each controlled-build output directory must
already exist, be direct, and be empty before launch.
