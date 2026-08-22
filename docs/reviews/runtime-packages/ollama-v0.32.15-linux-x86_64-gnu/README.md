# Ollama v0.32.15 Linux x86-64 GNU review

## Disposition

This exact CPU x64 package candidate is not admitted. It has no production
execution, cloud-disable, model-support, or qualification authority.

The review freezes the official `ollama-linux-amd64.tar.zst` release asset at
1,422,416,084 bytes and SHA-256
`50539c5fe9bf85887733355098dcdb266b433cb8c73fa180713417e9ed6e42bb`.
It selects one baseline CPU backend, accounts for all 51 non-directory archive
entries, replaces required symlink aliases with exact regular-file copies, excludes
unused accelerator and utility code, and binds the exact Retonr isolation helper
built by snapshot run 32588606381.

The transformation and license controls pass. Admission remains blocked because:

- The official build publishes an exact checksum but disables provenance and SBOM
  output. Mutable builder image and compiler package inputs prevent an independently
  verified source-to-binary lineage claim.
- External GNU libc platform components have not been captured by exact retained
  identity on the qualification host.
- The assembled candidate has not completed managed startup and retained teardown.
- The exact `OLLAMA_NO_CLOUD=1` startup marker has not been observed under that
  managed execution boundary.

These are evidence gaps, not inferred failures in Ollama. Retonr keeps the production
reviewed-runtime allowlist empty until every required control passes.

## Evidence index

| Evidence | Purpose |
| --- | --- |
| [Machine disposition](review.json) | Bounded typed control results and exact blockers |
| [Source lineage](source-lineage.json) | Release asset, source revisions, build inputs, and provenance limit |
| [Transformation](transformation.json) | Complete source-tree accounting and exact selected member bytes |
| [License disposition](license-disposition.json) | Selected code licenses and review method |
| [Go module inventory](go-module-inventory.tsv) | All 67 modules embedded in the exact Ollama entrypoint |
| [Native closure](native-closure.json) | Packaged ELF closure and deferred external platform identities |
| [Execution](execution.json) | Unrun managed-startup and cloud-disable requirements |

`review.json` is parsed in the Rust test suite. Its evidence digests are checked
against these files, so a report edit cannot silently preserve the old disposition.
An admitted review would additionally require the exact reconstructed layout digest
and `RuntimePackageManifestId`; a non-admitted review cannot carry either value.

The local-first
[`assemble-ollama-runtime-candidate.sh`](../../../../scripts/assemble-ollama-runtime-candidate.sh)
script accepts an already-downloaded archive and helper, verifies every frozen input,
rejects source-tree drift, and creates only the exact ten-file code candidate. The
manual `runtime package review` GitHub workflow performs the separately authorized
download, builds the helper, runs that assembly, checks dynamic linking and version
output, and retains short-lived verification evidence. The workflow cannot change
the checked-in disposition or production policy.
