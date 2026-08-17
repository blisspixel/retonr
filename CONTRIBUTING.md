# Contributing to Retonr

Retonr is an early, fidelity-sensitive project. Contributions are welcome when they
preserve its local-first constraints, explicit abstention behavior, and common
validation authority.

## Before changing code

1. Read [the current implementation state](docs/current-state.md).
2. Read [the architecture](docs/architecture.md) and the relevant decision records.
3. Check [the roadmap](docs/roadmap.md) for sequencing and phase gates.
4. Open an issue before making a large interface, storage, model, packaging, or
   security decision.

Public APIs, stored schemas, supported formats, and package identities remain
provisional until their roadmap gates pass.

## Development requirements

- Rust 1.97.1, as pinned in `rust-toolchain.toml`
- Node.js 24 or later for documentation tooling
- PowerShell 7, or the in-box Windows PowerShell 5.1, for repository policy checks
- A supported Windows, macOS, or Linux development environment

Install the pinned documentation dependencies with:

```console
npm ci --ignore-scripts
```

## Required validation

Run these checks before submitting a change:

```console
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo test --locked --workspace --all-features --doc
cargo llvm-cov --locked --workspace --all-features --fail-under-lines 80
cargo doc --locked --workspace --all-features --no-deps
cargo deny check
cargo audit
npm run lint:markdown
pwsh -NoProfile -File scripts/check-repository.ps1
cargo check --locked --manifest-path fuzz/Cargo.toml --all-targets
cargo build --locked --workspace --all-features --release
```

The repository policy script also runs under the in-box Windows PowerShell 5.1 when
PowerShell 7 is unavailable:

```console
powershell -NoProfile -File scripts/check-repository.ps1
```

Continuous integration repeats the applicable gates on Windows, macOS, and Linux.

## Change expectations

- Add regression fixtures for fidelity, structure, or protected-value defects.
- Test accepted output, rejected output, abstention, cancellation, and bounded input
  where the changed boundary can produce them.
- Keep raw document content out of logs, diagnostics, traces, and test snapshots.
- Route every generative strategy through the common validation cascade.
- Describe probabilistic semantic assessment as calibrated evidence, not proof.
- Keep modules cohesive and within the repository size limits.
- Update current-state documentation only when implementation and retained evidence
  support the claim.

Do not add generated authorship attribution, emojis, en dash characters, or em dash
characters to tracked repository content.

## Commits and pull requests

Keep commits focused and use direct commit subjects. Pull requests should explain:

- What changed
- Why the change is needed
- Which public or internal contracts are affected
- How fidelity, privacy, accessibility, and cross-platform behavior were evaluated
- Which validation commands passed

Do not include private writing samples, model credentials, access tokens, or other
sensitive data in issues, commits, fixtures, or pull requests.

## Reporting security issues

Do not report vulnerabilities in a public issue. Follow
[the security policy](SECURITY.md).
