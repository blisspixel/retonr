# Installation and distribution

## Objective

Retonr should be easy to install without hiding what is downloaded or changing the
machine outside a documented per-user directory. The primary CLI path will provide
one PowerShell command for Windows and one POSIX shell command for macOS and Linux.
Package-manager and inspect-before-run paths remain available for users who do not
pipe a bootstrap script into a shell.

The bootstrap installers are planned release artifacts. They are not published yet,
and commands in this document must not be presented as working installation commands
until the corresponding signed release assets exist.

## Planned stable entry points

After the distribution gate passes, the convenience commands will use repository
owned release assets with stable names:

```powershell
irm https://install.retonr.dev/stable/retonr-installer.ps1 | iex
```

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://install.retonr.dev/stable/retonr-installer.sh | sh
```

Each bootstrap script must also support an inspect-first workflow, an exact version,
a non-interactive mode, a custom install root, and a no-path-change mode. The release
page publishes the script, its digest, signature, provenance, and the exact binary
assets it may install.

The convenience command is not the only trust path. Release documentation must show
how to download the script, inspect it, verify its signature, and run it separately.
The pipe form trusts DNS, TLS, the stable redirect host, and the delivered bootstrap
script before that script can verify a payload. It is a convenience boundary, not an
independently verified bootstrap. The inspect-first path is the end-to-end
verification path.

## Installer contract

Both bootstrap installers perform the same ordered work:

1. Parse only documented arguments and reject unknown input.
2. Detect the operating system, architecture, and required binary format.
3. Resolve an exact requested version, never an unbounded development build.
4. Download one matching release manifest and artifact over HTTPS.
5. Verify the manifest signature, artifact digest, artifact signature or attestation,
   expected filename, and expected byte length before execution.
6. Inspect the archive and reject absolute paths, parent traversal, unsafe links, and
   unexpected members.
7. Extract into a new versioned staging directory beneath the selected install root.
8. Run the packaged self-check without network access.
9. Atomically switch the active launcher only after every check passes.
10. Add the per-user binary directory to `PATH` only with an explicit documented
   choice.
11. Print the installed version, location, verification result, and next command.

Failure leaves the previously active version untouched. Re-running the same version
is idempotent. No installer requires administrator access for the default path,
starts a background service, enables telemetry, installs a model runtime, or
downloads a model.

The manifest-signing decision must define the signature scheme, embedded trust root,
expected publisher or issuer plus repository and workflow identity, key rotation,
revocation, recovery, and a verifier that ships with the bootstrap or exists on every
declared bare host. Missing verification support fails closed. GitHub attestation
availability depends on repository visibility and account plan; independent signing
remains required whenever the selected plan cannot produce public attestations.

## Install layout and lifecycle

The default layout separates immutable program versions from user data:

```text
<per-user data>/retonr/
  bin/
  versions/<version>/
  manifests/
  installer-state.json
```

Profiles, models, caches, and configuration use platform-native application data and
cache directories. They are not stored beside the executable. Update and uninstall
operations therefore have explicit independent scopes:

- Re-running a version-pinned installer is the initial update path. A dedicated
  `retonr update` command ships only after its signed metadata, interrupted-update,
  rollback, and package-manager ownership behavior are independently qualified.
  Automatic update checks are off by default for the CLI.
- `retonr uninstall` removes application binaries and installer state.
- `retonr uninstall --purge` separately previews and removes application-controlled
  profiles, models, caches, and configuration after confirmation.
- Downgrade refuses incompatible stored schemas and never edits user data without a
  tested migration or recovery path.

The installer and updater never delete an old version until the new version has
passed startup and rollback checks. The retained-version count is bounded and
configurable.

## Release target matrix

The minimum 1.0 matrix is evidence based rather than a generic platform claim:

| Platform | Initial required artifact | Installation evidence |
| --- | --- | --- |
| Windows | Signed x86-64 and Arm64 CLI archives and installers | Standard user, clean install, update, recovery, uninstall, long paths, locked files |
| macOS | Signed and notarized Apple silicon and Intel CLI artifacts | Gatekeeper, clean install, update, rollback, uninstall, offline operation |
| Linux | Signed x86-64 and Arm64 glibc archives | Declared distributions, glibc floor, clean install, update, rollback, uninstall |

The six listed architecture targets require native or appropriately isolated test
evidence. Musl and package-manager formats graduate separately. macOS may ship one
universal binary or two architecture-specific CLI artifacts, but each slice is
qualified separately.

The selected `dist` version must prove support for every target it generates.
Windows Arm64 uses a reviewed custom native build and package job unless that exact
future `dist` version has qualified `aarch64-pc-windows-msvc` support.

## Package-manager paths

The one-line installers are the first supported path because they can deliver a
consistent signed artifact. Additional channels are useful only when they preserve
the same identity and verification rules:

- Homebrew for macOS and supported Linux environments
- WinGet or Scoop for Windows
- `cargo install --locked` for developers when native sidecars are not required
- Distribution-native Linux packages after install, upgrade, removal, dependency,
  and repository-signing behavior is qualified

Every channel reports the same Retonr version and build identity. A channel may not
silently substitute a different model runtime, feature set, update policy, or data
location.

## Release pipeline

The CLI pipeline may use `dist`, the renamed `cargo-dist` project, after a recorded
qualification decision. The package remains `cargo-dist`, while the executable and
project name are `dist`. Generated workflows are reviewed as source, pinned, and
tested in clean virtual machines. Version 0.32.0 is a research baseline, not an
automatic selection. Its stock installers and experimental updater do not satisfy
this contract without audited wrappers and negative verification tests. Desktop
packaging remains owned by the Tauri pipeline.

Release evidence includes:

- Reproducible build inputs and committed lockfiles
- Software bill of materials and dependency license report
- Per-artifact SHA-256 digests
- Platform signatures and notarization where applicable
- A signed schema-versioned release manifest with target, URL, length, digest,
  platform floor, and expected signer identity
- Build provenance and release attestations
- Installer transcript and negative verification tests
- Clean install, offline startup, update, interrupted update, rollback, and removal
- Proof that bootstrap and release archives contain no model or hidden service

The release is staged as a draft and verified through CI or authenticated draft
asset access. It is then published as an immutable release. Post-publication tests
use exact versioned public URLs, and only after they pass does the separate
product-owned stable redirect or signed channel manifest move. GitHub
`releases/latest` is not the stable bootstrap pointer. Assets under a published
version are never replaced.

## Setup after installation

Installation ends with a small functioning binary. `retonr setup` performs a local
hardware and runtime probe, explains model choices, and asks before any networked
download. `retonr doctor` remains non-mutating. Offline users can import previously
verified runtime and model artifacts.

Model acquisition is deliberately separate from application installation because
model size, license, hardware compatibility, and network policy require an informed
choice. The complete selection contract is in [Model and runtime support](model-support.md).

## Primary references

- [Grok CLI installation pattern](https://github.com/xai-org/grok-build)
- [GitHub artifact attestations](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
- [Sigstore Cosign verification](https://docs.sigstore.dev/cosign/verifying/verify/)
- [dist project and documentation](https://github.com/axodotdev/cargo-dist)
- [GitHub immutable releases](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases)
- [Tauri distribution documentation](https://v2.tauri.app/distribute/)
