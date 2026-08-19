# CLI candidate-check capture

## Scope

This image documents the implemented model-free `check` command. It does not imply
that model-backed rewrite, profile, download, qualification, activation, runtime
execution, service, or desktop commands are implemented. Offline artifact
administration exists as a separate `model` command family.

## Capture metadata

| Field | Value |
| --- | --- |
| Source commit | `8c156632dd2464ee6561e42ec7e0e09293fdb066` |
| Binary | `retonr 0.1.0`, release profile |
| Binary SHA-256 | `1448885e8e1170e3e265f0267e6f96640434d50ad3584df767ac58c77b847c74` |
| Platform | Linux 6.6.114.1-microsoft-standard-WSL2, x86-64 |
| Shell | GNU Bash 5.0.17 |
| Rust toolchain | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| Theme | Documentation dark terminal theme |
| Viewport | 1440 by 860 CSS pixels |
| Scale | 1 |
| Fixture | `fixtures/cli/source.txt` and `fixtures/cli/candidate.txt` |

## Commands

The release-optimized binary directory was added to `PATH`, then these commands were
run from the repository root:

```console
retonr --help
retonr check fixtures/cli/source.txt fixtures/cli/candidate.txt --format text
```

The SVG source contains the verbatim standard output. The neutral username, host,
prompt lines, line continuation, title bar, and final two gray lines are presentation
annotations. The PNG is a headless browser rendering of that SVG at the recorded
viewport. No product output, status, digest, count, or implemented command was
edited for presentation.

## Reproduction

1. Check out the source commit on Linux.
2. Run `cargo build --release --locked -p retonr-cli`.
3. Verify the binary digest shown above.
4. Run the two commands against the checked-in fixture.
5. Compare the output with `cli-check-linux.svg`.
6. Render the SVG at 1440 by 860 pixels to replace `cli-check-linux.png`.
