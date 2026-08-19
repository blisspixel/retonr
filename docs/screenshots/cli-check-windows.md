# CLI candidate-check capture

## Scope

This image documents the implemented model-free `check` command. It does not imply
that model-backed rewrite, profile, download, qualification, activation, runtime
execution, service, or desktop commands are implemented. Offline artifact
administration exists as a separate `model` command family.

## Capture metadata

| Field | Value |
| --- | --- |
| Source commit | `8278e5413e2f7ee436fa35c664d331994f697173` |
| Binary | `retonr 0.1.0`, release profile |
| Binary SHA-256 | `ed1f92efdfc8ed561e8aeeefa3f0926604608cc87731a8bc35cd8dde16de1bfb` |
| Platform | Windows 11 Pro, build 26200, x86-64 |
| Shell | PowerShell 7.6.3 |
| Theme | Documentation dark terminal theme |
| Viewport | 1440 by 860 CSS pixels |
| Scale | 1 |
| Fixture | `fixtures/cli/source.txt` and `fixtures/cli/candidate.txt` |

## Commands

The release-optimized binary directory was added to the current process `PATH`, then
these commands were run from the repository root:

```powershell
retonr --help
retonr check fixtures/cli/source.txt fixtures/cli/candidate.txt --format text
```

The SVG source contains the verbatim standard output. Prompt lines separate the
commands, and the final two gray lines label the implemented slice and fixture as
presentation annotations. The PNG is a headless browser rendering of that SVG at the
recorded viewport. No product output, status, digest, count, or implemented command
was edited for presentation.

## Reproduction

1. Check out the source commit.
2. Run `cargo build --release --locked -p retonr-cli`.
3. Verify the binary digest shown above.
4. Run the two commands against the checked-in fixture.
5. Compare the output with `cli-check-windows.svg`.
6. Render the SVG at 1440 by 860 pixels to replace `cli-check-windows.png`.
