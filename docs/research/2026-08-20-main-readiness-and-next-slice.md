# Main readiness and next 0.2 slice

## Decision

The highest-value next slice is trustworthy attachment to a user-managed Ollama
runtime. Model acquisition and generative evaluation must wait until Retonr can bind
the listener, runtime build, complete model package, effective configuration, and
local-only controls into one reproducible evidence chain.

This ordering protects the local-first contract and prevents a mutable Ollama tag or
one successful response from being mistaken for qualification. It also keeps the
next work at zero external cost.

Evidence in this review was refreshed on August 20, 2026.

## Public main readiness

| Check | Evidence | Result |
| --- | --- | --- |
| Default branch | `main` | Correct |
| Remote revision | `bd9fb71ad7af09579dde60c4538f13a832db4de6` | Local `origin/main` and GitHub match |
| Latest quality run | [Run 32331755461](https://github.com/blisspixel/retonr/actions/runs/32331755461) | Passed |
| Cross-platform Rust | Ubuntu, Windows, and macOS | Passed |
| Policy and documentation | Three repository-policy jobs and Markdown | Passed |
| Additional gates | Coverage, supply chain, fuzz smoke, and loopback-only Ollama | Passed |
| Main coverage | 90.52 percent line coverage | Above the 80 percent floor |
| Main test execution | 716 nextest tests, with two intentional helpers skipped | Passed |
| Open work on GitHub | No open pull requests or issues | Clean |
| Branch governance | No ruleset or branch protection | Action required |

The public main revision is clean and passing. It is not adequately protected. A
direct push, force push, or deletion is not currently prevented by repository
governance.

## Documentation drift found

| Drift | Correction |
| --- | --- |
| README said Gemma 4 26B and Qwen3.6 27B were installed | Treat the August 13 inventory as expired historical evidence |
| Roadmap queued attestation, extractor contracts, pair extraction, and the shadow join as future work | Those boundaries already exist and must be preserved |
| Roadmap referred to current schema v4 work | Schema v5 is current; schema v4 remains migration history |
| Current state cited an older revision, workflow, test count, and coverage result | Separate the exact public-main result from current unpublished branch verification |
| Current state counted 12 repository model commands | There are 14 repository commands plus repository-free `device-evidence` |

Historical research records remain dated observations. Current public-facing status
must not convert them into claims about the present machine or supported models.

## Live Ollama observation

The existing Ollama service was inspected read-only. Retonr did not start, stop,
restart, load, or acquire anything.

| Property | Observation |
| --- | --- |
| Endpoint | Explicit IPv4 loopback on port 11434 |
| Runtime version | 0.32.14 |
| Current official release | [Ollama v0.32.14](https://github.com/ollama/ollama/releases/tag/v0.32.14) |
| Resident models | None initially; one separate small model became resident during final verification |
| Relevant current generation tag | `qwen3.8:27b` |
| Inventory digest | `22130167c4c20e20c7b71454612966ca8e8171e9b3cc8ab6ce8aa6cbfec79643` |
| Reported family and quantization | `qwen35`, 27.3B, Q4_K_M |
| Reported capabilities | Completion, thinking, tools, and vision |
| Previously named bakeoff tags | Gemma 4 26B, Qwen3.6 27B, and Ministral 3 8B were absent |

The exact upstream source revision, complete local package closure, license binding,
and transformation history for `qwen3.8:27b` were not established. The runtime did
not provide a Retonr runtime-build digest. A cloud-disable setting was not proven.
The tag is therefore a development observation, not an eligible artifact or support
claim.

Ollama documents that local-only mode must be configured explicitly and that context
and model placement depend on effective runtime settings. Its structured-output
capability is transport evidence only. It is not evidence that one exact model,
template, and policy satisfy a Retonr contract. See the
[Ollama FAQ](https://docs.ollama.com/faq),
[`/api/ps` reference](https://docs.ollama.com/api/ps), and
[structured outputs](https://docs.ollama.com/capabilities/structured-outputs).

## Implemented read-only preflight

This branch adds a versioned `rewrite-eval --ollama-preflight` plan and report with
two modes:

- `observe` records bounded, content-redacted runtime and model-description evidence.
- `verify` requires every frozen description field and digest to match.

The adapter reads version, inventory, and residency, captures verbose model
description, then re-reads inventory, version, and residency before success. Drift,
unexpected remote fields, oversized bodies, non-loopback endpoints, residency when
an idle runtime is required, cancellation, and identity mismatches fail closed.
Preflight targets carry only runtime references and inventory digests; they never
construct or claim Retonr artifact identity. License and template text are hashed and
omitted from the report. No generation endpoint is called, and the report always
emits `qualified: false`.

### Plan contract and workflow

Plans use schema version 1 and are limited to 64 KiB. Unknown fields are rejected.
The endpoint must be an explicit IPv4 or IPv6 loopback literal. `plan_id` is a
bounded lowercase machine label. The expected runtime version is exact, and
`require_idle` controls whether any resident model fails the run.

Each plan contains one to eight models ordered by runtime reference. References and
inventory digests are unique. In `observe` mode, each model contains only
`reference` and `inventory_digest`; `expected_details` must be absent. The report
returns bounded format, family, quantization, canonical capabilities, and license,
template, and detailed-metadata digests. To freeze a `verify` plan, retain the same
plan fields, change `mode` to `verify`, and copy each observed `details` object into
that model's `expected_details`. Verify mode rejects any missing, reordered,
duplicated, or changed evidence.

The report binds the canonical parsed plan with `plan_digest`. Preserve the plan and
report together because the report deliberately does not repeat endpoint or policy
fields. Neither mode creates reusable qualification or activation evidence.

The live observe and verify passes succeeded against runtime 0.32.14 without loading
a model. They also exposed and fixed two compatibility defects:

- Current Ollama inventory uses a bare 64-character digest while older fakes used a
  `sha256:` prefix. Both wire forms now normalize to one internal digest.
- Current verbose model metadata exceeds the old 2 MiB discovery ceiling. Discovery
  now has a tested 16 MiB fixed ceiling; caller-configurable response ceilings cannot
  exceed fixed adapter maxima.

During final validation, a separate local client made a small model resident. The
idle-required verification rejected that state as designed. The same frozen plan in
non-idle verification mode matched the selected `qwen3.8:27b` evidence, reported the
resident model, and still emitted `qualified: false`. Retonr did not load or unload
either model.

## Execution order

1. Publish the read-only preflight and stable `required` aggregate gate
   after local review and publication authorization.
2. Extend runtime attestation to bind the loopback listener to the exact operating
   system process, regular entrypoint bytes, version, launch mode, and effective
   configuration. Recheck the binding before and after use.
3. Reconstruct the complete external Ollama package as a canonical artifact-set
   manifest. Bind every blob, manifest, tokenizer, template, license decision, and
   upstream source revision. Keep runtime-build identity separate from package
   identity.
4. Prove local-only operation. Require explicit Ollama cloud disablement and
   OS-enforced denial of non-loopback outbound traffic for Retonr and the runtime.
5. Project the existing eight-case smoke and 39-case editorial protocol into
   versioned local generation plans. Fix seed, reasoning, context, output schema,
   concurrency, residency, timeout, and resource policy.
6. Run the eight-case smoke only after steps 2 through 4 pass. Any identity,
   isolation, malformed-output, fidelity, cancellation, or resource failure stops
   the run.
7. Advance clean smoke results to the locked 39-case projection, repeatability runs,
   and risk-stratified reporting. Keep model output proposal-only and preserve the
   existing hard gates.
8. Create qualification and activation evidence only after exact thresholds pass on
   each declared operating system and hardware tier. A development lead is not a
   support claim.

This order is stronger than running the current tag immediately because every later
measurement becomes attributable to a frozen stack. Without that chain, a useful
score cannot be reproduced or safely activated.

## GitHub governance plan

The workflow should expose one stable `required` job that depends on every Rust,
repository-policy, Markdown, coverage, supply-chain, fuzz, and loopback-only job. A
main ruleset should then:

- require a pull request;
- require the stable quality check;
- require the branch to be current before merge;
- block force pushes and branch deletion;
- dismiss stale approvals after material changes; and
- apply to administrators, with an explicit emergency bypass policy.

GitHub documents these controls in
[About protected branches](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches),
[About rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets),
and
[Available rules for rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets).
Ruleset creation is an external governance change and requires explicit publication
authorization.

## Cost boundary

External spend for this review and implementation is $0. No model was acquired and
no paid service was used. The remaining daily ceiling is $5. The planned identity,
isolation, fixture projection, and local verification work requires no paid action.
Any later acquisition remains separately approved and must fit inside the remaining
ceiling.
