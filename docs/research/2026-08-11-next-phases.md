# Next-phase research ledger

## Review status

Reviewed: August 11, 2026.

Scope: local model runtime and evaluation, profile evidence and retrieval, CLI and
local API, MCP and Agent Skills, compatibility adaptation, Tauri desktop delivery,
accessibility, local speech, and cross-platform packaging.

This ledger records the primary-source assumptions behind the phase execution plans.
It does not mark a dependency as selected, a model as qualified, or a feature as
implemented. Final choices that affect stored data, public contracts, security,
licenses, or distribution require architecture decision records.

## Research method

- Prefer official specifications, project documentation, standards, model cards, and
  peer-reviewed research.
- Separate external facts from project targets and recommendations.
- Treat mutable package versions, model tags, API behavior, and protocol drafts as
  revalidation triggers.
- Treat structured model output and learned evaluation as fallible.
- Keep every public compatibility or preservation claim narrower than its retained
  conformance evidence.

## Model runtime and artifact identity

### Findings

Four identities must remain separate:

1. Generation strategy
2. Runtime backend
3. Installed model artifact
4. Qualified artifact and runtime combination

An installed model or backend capability does not establish product support.
Support is the intersection of implemented product behavior, backend behavior, model
behavior, and a passing qualification record.

Ollama exposes separate endpoints for runtime version, installed models, model
details, generation, and embeddings. Model tags remain mutable. Discovery metadata
and capability claims are untrusted input and do not replace local qualification.

The native generation API supports non-streaming responses, a `think` control, and
structured output. Structured output constrains syntax, not factual or semantic
correctness. There is no documented exact native token-count preflight that can prove
context fit for every recorded tokenizer. Until the product owns an exact matching
tokenizer, each qualified artifact needs a conservative source and context envelope.

Ollama embedding requests can truncate oversized inputs by default. Product requests
must set `truncate: false` and fail explicitly rather than silently changing profile
evidence.

### Planning consequences

- Define neutral inference and manifest contracts before the Ollama adapter.
- Use a pure scripted backend fake plus an HTTP compatibility fake.
- Probe version, tags, and model details, select an exact returned digest, then check
  identity immediately before and after generation.
- Accept loopback endpoints only in the first local runtime phase.
- Disable system proxies and redirects in the qualified local client.
- Never install, start, pull, create, copy, push, or delete through a rewrite.
- Set explicit context, output, sampling, candidate, byte, time, retry, and
  cancellation bounds.
- Use complete masked candidates only. Keep the sentinel restoration map inside the
  deterministic engine.
- Replace raw provider-string errors before backend errors become external contracts.

### Primary sources

- [Ollama version endpoint](https://docs.ollama.com/api-reference/get-version)
- [Ollama installed model listing](https://docs.ollama.com/api/tags)
- [Ollama model details](https://docs.ollama.com/api-reference/show-model-details)
- [Ollama generation API](https://docs.ollama.com/api/generate)
- [Ollama structured outputs](https://docs.ollama.com/capabilities/structured-outputs)
- [Ollama embedding API](https://docs.ollama.com/api/embed)
- [Ollama embedding behavior](https://docs.ollama.com/capabilities/embeddings)
- [llama.cpp server interfaces](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [Hugging Face immutable revisions](https://huggingface.co/docs/huggingface_hub/en/guides/download)
- [SPDX 3.0.1 AI profile](https://spdx.github.io/spdx-spec/v3.0.1/model/AI/AI/)
- [reqwest client controls](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html)

## Evaluation governance and calibration

### Findings

The checked-in prototype suite is useful for smoke and regression behavior. It is
not large or isolated enough to support a release-level semantic claim.

Selective systems must report accepted-output error and useful coverage together.
An apparently safe system can abstain on everything. An aggregate can also hide weak
performance in a critical category. Calibration and locked release evidence must use
separate data.

Model-reported confidence is not calibrated evidence. A usable confidence signal
needs a defined raw score, independent labeled calibration data, a versioned mapping,
risk-coverage reporting, and distribution-shift checks. Generator and evaluator
failures can be correlated.

Model judges have documented ordering and self-preference risks. They may assist
triage, but human adjudication remains the release authority for semantic labels and
owner preference.

### Planning consequences

- Version suite kinds as smoke, development, calibration, locked, and red team.
- Record immutable suite, rubric, split, threshold, artifact, hardware, and build
  identities.
- Never tune prompts or thresholds on the locked set.
- Create a new suite version and invalidation record when a locked label is wrong.
- Report corruption acceptance, accepted-set semantic error, eligible-candidate
  coverage, system transformation coverage, abstention, and resource behavior
  separately.
- Use paired comparisons and one-sided exact confidence bounds for critical failure
  categories.
- Evaluate typed claims, roles, polarity, modality, conditions, time, alignment,
  entailment, and contradiction as distinct evidence.
- Keep the common deterministic validation cascade authoritative for every baseline
  and strategy.

### Primary sources

- [Selective classification and risk coverage](https://proceedings.neurips.cc/paper/2017/hash/4a8423d5e91fda00bb7e46540e2b0cf1-Abstract.html)
- [Neural confidence calibration](https://proceedings.mlr.press/v70/guo17a.html)
- [Behavioral NLP testing](https://aclanthology.org/2020.acl-main.442/)
- [PAWS semantic adversaries](https://aclanthology.org/N19-1131/)
- [TRUE factual consistency evaluation](https://aclanthology.org/2022.naacl-main.287/)
- [Style-transfer metric failure analysis](https://aclanthology.org/2025.findings-emnlp.1175/)
- [LLM judge bias evaluation](https://proceedings.neurips.cc/paper_files/paper/2023/hash/91f18a1287b398d378ef22505bf41832-Abstract-Datasets_and_Benchmarks.html)
- [NIST exact binomial confidence limits](https://itl.nist.gov/div898/software/dataplot/refman2/auxillar/exacbici.htm)
- [Machine-learning reproducibility program](https://www.jmlr.org/papers/v22/20-303.html)

## Profile evidence, embeddings, and retrieval

### Findings

Style evidence needs explicit provenance, derivation, consent, revocation, and
version identity. W3C PROV offers useful entity, activity, derivation, and revision
concepts, but the product needs only a small typed internal subset.

Embedding quality is task-specific. A strong general semantic retrieval model may
prefer topic over style. Authorship representations can contain topic information.
Profile evaluation therefore needs topic-held-out comparisons and metadata-only or
interpretable-feature baselines.

Retrieved evidence can be copied or extracted. Retrieval security needs cross-profile
isolation, canary testing, rare phrase and unique n-gram checks, entity and quantity
checks, and contribution caps. Topic similarity should not be the primary retrieval
objective for a style profile.

### Planning consequences

- Make evidence, observations, rules, and compiled profile versions immutable.
- Preserve derivation and explicit confirmation for user-edited generated text.
- Never admit a raw or merely accepted candidate as evidence.
- Identify embedding space by exact artifact, runtime, dimensions, input instruction,
  normalization, truncation, limits, distance, quantization, and serialization.
- Invalidate and rebuild vectors when any identity field changes.
- Filter consent, profile, channel, communicative act, register, length, source cap,
  and topic diversity before style-oriented ranking.
- Track retrieved evidence IDs and run post-generation copying and novel-entity
  checks.
- Use forward-only schema migrations with pre-migration backup and verified restore
  rollback. Make profile entity derivations append-only, deterministic, and
  deletion-aware.

### Primary sources

- [W3C provenance data model](https://www.w3.org/TR/2013/REC-prov-dm-20130430/)
- [MTEB embedding task variation](https://aclanthology.org/2023.eacl-main.148/)
- [StyleDistance](https://aclanthology.org/2025.naacl-long.436/)
- [Authorship representation and topic confounding](https://aclanthology.org/2023.tacl-1.80/)
- [LeakDojo retrieval corpus leakage](https://aclanthology.org/2026.findings-acl.287/)

## CLI and local API

### Findings

Standard input, output, and error can have different terminal states. Interactive
formatting and progress must not leak into machine output. Preserved source controls,
file names, and diff content can become active terminal instructions if written
without neutralization.

Loopback is still an authority boundary. A browser page or local process may attempt
to call a local service. Origin, Host, authentication, scope, resource limits, and
redaction remain necessary.

RFC 9457 defines HTTP problem details. Fidelity outcomes such as abstention remain
successful domain envelopes; problem details are appropriate for malformed request,
authorization, transport, resource, and operational failures. The HTTP
`Idempotency-Key` draft expired in April 2026 and is not a stable normative basis for
1.0. Conditional writes and client operation IDs are safer first-party choices.

### Planning consequences

- Evaluate configuration with a non-overridable security ceiling and deny-wins
  restrictive policy.
- Require explicit trust before project configuration can influence behavior.
- Keep raw terminal output unavailable or behind a double opt-in.
- Buffer until validation completes and discard partial output on cancellation.
- Start HTTP only through an explicit command and keep desktop calls in process.
- Bind to an actual loopback address only for 1.0.
- Require high-entropy local authentication and separate rewrite, profile-read,
  profile-write, learning, and administration scopes.
- Validate Host and Origin, deny permissive CORS, set sensitive responses to
  `no-store`, and propagate disconnect into cancellation.
- Stream progress only, never unvalidated candidate text.

### Primary sources

- [RFC 9457 Problem Details](https://www.rfc-editor.org/rfc/rfc9457.html)
- [RFC 9110 HTTP semantics](https://www.rfc-editor.org/rfc/rfc9110.html)
- [RFC 9111 HTTP caching](https://www.rfc-editor.org/rfc/rfc9111.html)
- [JSON Schema 2020-12](https://json-schema.org/draft/2020-12)
- [OpenAPI specification](https://spec.openapis.org/oas/latest.html)
- [Idempotency-Key draft status](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/)
- [Rust `IsTerminal`](https://doc.rust-lang.org/stable/std/io/trait.IsTerminal.html)
- [NO_COLOR](https://no-color.org/)
- [XDG base directories](https://specifications.freedesktop.org/basedir/)
- [Unicode bidirectional algorithm](https://www.unicode.org/reports/tr9/)

## MCP and Agent Skills

### Findings

MCP 2026-07-28 changes the modern lifecycle. It has no initialize exchange or
protocol session. Requests carry required protocol version and client capabilities
plus optional `clientInfo` in request metadata. Servers expose discovery through
`server/discover`.

Modern Streamable HTTP is POST-only, validates method metadata in headers, supports
JSON and request-scoped server-sent events, and treats stream closure as
cancellation. It does not use the removed modern GET endpoint or require sticky
session IDs.

Skills over MCP remains an in-review working-group proposal as of this review. The
stable Agent Skills format is a `SKILL.md` package. Product skills can use that format
and call stable MCP tools or the local API without making the proposal a release
dependency.

### Planning consequences

- Implement standard input first, then modern Streamable HTTP.
- Pin the official Rust SDK patch and run applicable official conformance tests.
- Keep standard output protocol-only and diagnostics on standard error.
- Bound frames, queues, calls, writers, responses, event streams, and cancellation.
- Expose rewrite and check by default, with separate authority for profile read,
  mutation, learning, and administration.
- Keep multi-step learning in opaque, scoped, expiring, revocable handles bound to
  the authenticated principal.
- Support an older MCP revision only for a named client and complete compatibility
  fixtures.
- Keep first-party skills thin, separately privilege profile mutation, and avoid
  bundled scripts without a measured need.
- Use stable Agent Skills frontmatter, enforce package path containment, and never
  rely on experimental `allowed-tools` metadata for authority.
- Isolate any Skills over MCP experiment and exclude it from the 1.0 gate.

### Primary sources

- [MCP 2026-07-28 specification](https://modelcontextprotocol.io/specification/2026-07-28)
- [MCP standard input transport](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio)
- [MCP Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
- [MCP versioning](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
- [MCP cancellation](https://modelcontextprotocol.io/specification/2026-07-28/basic/patterns/cancellation)
- [MCP authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)
- [MCP security guidance](https://modelcontextprotocol.io/docs/2026-07-28/tutorials/security/security_best_practices)
- [Official MCP conformance suite](https://github.com/modelcontextprotocol/conformance)
- [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Agent Skills specification](https://agentskills.io/specification)
- [Skills over MCP status](https://modelcontextprotocol.io/community/working-groups/skills-over-mcp)

## Compatibility adaptation

### Findings

A completed upstream response can be locally post-processed for a narrow schema
subset. That is shape compatibility, not full upstream semantic or streaming
conformance. Rewriting can make original usage, IDs, fingerprints, and provenance
metadata misleading if the local transformation is not represented separately.

### Planning consequences

- Select one exact upstream schema revision and eligible JSON string paths in a
  decision record.
- Accept only bounded complete non-streaming payloads.
- Classify unsupported tool, structured output, reasoning, refusal, citation,
  annotation, signature, image, audio, and event forms without attempting a rewrite.
- Use byte-range JSON string splicing and verify non-target byte identity.
- Apply document atomicity across all eligible strings.
- Return no payload for malformed or oversized input. Return exact original bytes
  with distinct unsupported, abstained, or verification-failed status for valid input
  that cannot produce a verified rewrite.
- Preserve original upstream metadata but state that it describes the original
  response.
- Provide local status and rewrite provenance through an explicit sidecar or envelope.
- Record original and rewritten digests, target schema, eligible paths, adapter and
  validator versions, and a label stating that retained upstream metadata describes
  the original response.
- Include no outbound request or credential path.

## Tauri security and desktop architecture

### Findings

Tauri capabilities can combine when a window appears in multiple files, and enabled
capability selection requires deliberate configuration. Custom commands also need an
application manifest and application-level scope checks. Tauri content security
policy is not protective unless it is configured.

Tauri update signatures are mandatory in production. Updater key loss can strand
installed clients, so signing-key backup, rotation, loss, and recovery are product
requirements.

The platform WebViews differ: WebView2 on Windows, WKWebView on macOS, and WebKitGTK
on Linux. Functional, visual, keyboard, accessibility, and security tests must run on
each supported platform.

### Planning consequences

- Explicitly list capability identifiers and register every custom command.
- Use one least-privileged capability per window and avoid broad defaults, wildcards,
  shell, process, general HTTP, and general filesystem plugins.
- Keep file, model, profile, update, network, and later audio authority in Rust.
- Bundle all assets, configure strict CSP, avoid remote frames, inline script, eval,
  CDN assets, and external fonts, and render user content as text.
- Use operation IDs and monotonic event sequence numbers so stale events cannot
  replace completed state.
- Generate frontend DTOs from one reviewed Rust-owned contract.
- Keep update checks explicit and off by default under the local-first policy.
- Build, sign, and test packages on their target operating systems.

### Primary sources

- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri command scopes](https://v2.tauri.app/security/scope/)
- [Tauri permissions](https://v2.tauri.app/security/permissions/)
- [Tauri content security policy](https://v2.tauri.app/security/csp/)
- [Tauri isolation pattern](https://v2.tauri.app/concept/inter-process-communication/isolation/)
- [Tauri updater](https://v2.tauri.app/plugin/updater/)
- [Tauri Windows installer options](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri macOS signing and notarization](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri WebdriverIO testing](https://v2.tauri.app/develop/tests/webdriver/)

## Desktop interaction and accessibility

### Findings

React Aria provides accessible interaction primitives while allowing a product-owned
visual system. WCAG 2.2 AA applies to complete workflows, not isolated controls.
Manual assistive-technology review remains necessary on each platform.

The interface needs a linear accessible diff in addition to any side-by-side visual
diff. It must respect keyboard composite-widget conventions, reduced motion, forced
colors, zoom, and logical DOM and focus order. GNOME adaptive guidance provides a
useful 1024 by 600 minimum desktop layout target.

### Planning consequences

- Use a small semantic token system for color, type, spacing, focus, targets, motion,
  protected content, and outcomes.
- Use system fonts and avoid state communicated by color, motion, icon, or position
  alone.
- Support wide and linear layouts at 1024 by 600 and 200 percent zoom.
- Make every workflow complete with keyboard and screen reader alone.
- Test Narrator, Magnifier, On-Screen Keyboard, high contrast, and DPI scaling on
  Windows.
- Test VoiceOver and Reduced Motion on macOS.
- Test Orca, large text, high contrast, and keyboard-only use on Linux.
- Announce meaningful stages, not streaming tokens, audio levels, or every progress
  tick.

### Primary sources

- [React Aria](https://react-aria.adobe.com/getting-started)
- [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- [ARIA keyboard interface guidance](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/)
- [Media Queries Level 5](https://www.w3.org/TR/mediaqueries-5/)
- [GNOME adaptive design](https://developer.gnome.org/hig/guidelines/adaptive.html)
- [GNOME accessibility guidance](https://developer.gnome.org/hig/guidelines/accessibility.html)
- [Microsoft accessibility testing](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessibility-testing)
- [Apple VoiceOver evaluation](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/voiceover-evaluation-criteria/)
- [Apple Reduced Motion evaluation](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/reduced-motion-evaluation-criteria/)
- [Playwright visual comparisons](https://playwright.dev/docs/test-snapshots)
- [Playwright ARIA snapshots](https://playwright.dev/docs/aria-snapshots)

## Local voice

### Findings

A feasible first scope is push-to-talk, local speech recognition, editable transcript
confirmation, deterministic interview control, and local speech output with captions.

CPAL is a cross-platform Rust audio candidate. sherpa-onnx offers local speech
recognition, speech output, and voice activity detection across the target operating
systems and architectures. whisper.cpp is a strong recognition comparison. Each
native runtime, model, voice, and phonemizer still requires independent packaging and
license review.

Speech recognition can hallucinate complete phrases, particularly around pauses.
Transcript confirmation is therefore a fidelity and profile-integrity requirement,
not an optional convenience.

Current Piper distribution is GPL-3.0 and individual voice licenses vary. A model
such as Kokoro may have a permissive model license while its runtime, phonemizer, and
voices still require separate review.

### Planning consequences

- Keep FFI behind safe Rust traits and out of domain crates.
- Send no PCM through WebView IPC.
- Use preallocated bounded audio buffers and perform no allocation, blocking,
  logging, file I/O, IPC, or inference in the real-time callback.
- Delete raw audio immediately by default.
- Require user-edited transcript confirmation before evidence admission.
- Keep the interview controller deterministic and deny the conversational model
  profile-mutation authority.
- Stop speech output before opening the microphone.
- Keep the complete typed path available throughout.
- Defer always listening, wake words, simultaneous capture and playback, voice
  cloning, and general dictation.

### Primary sources

- [CPAL](https://github.com/RustAudio/cpal)
- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx)
- [whisper.cpp](https://github.com/ggml-org/whisper.cpp)
- [Silero VAD](https://github.com/snakers4/silero-vad)
- [Careless Whisper transcription analysis](https://arxiv.org/abs/2402.08021)
- [Kokoro-82M model card](https://huggingface.co/hexgrad/Kokoro-82M)
- [Piper runtime](https://github.com/OHF-Voice/piper1-gpl)
- [Piper voice license guidance](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/VOICES.md)

## Internal performance targets

The following values are proposed project budgets, not external standards. They must
be confirmed or revised through an architecture decision using representative
hardware before contract freeze:

- Desktop response to input: 100 ms p95
- Frontend handler work: below 50 ms
- Meaningful cold desktop window: 2.5 seconds p95
- Meaningful warm desktop window: 1.5 seconds p95
- Idle UI CPU after stabilization: below 1 percent
- UI memory excluding loaded models: below 250 MB
- Visible cancellation state: 100 ms
- Capture stop after cancellation: 250 ms
- Speech inference cancellation: 1 second
- No callback allocation, blocking, or underruns in a 30-minute audio soak
- Speech recognition real-time factor: at most 1.0 on minimum hardware
- Voice activity finalization target: 700 ms after speech end
- Speech output first audio target: 750 ms

The general responsiveness direction is consistent with the
[RAIL response guidance](https://web.dev/articles/rail). Product targets remain
subject to measured cross-platform evidence.

## Revalidation triggers

Recheck this ledger when:

- MCP, Agent Skills, Tauri, Ollama, a speech runtime, or a selected model changes
  revision.
- A dependency, model, voice, or runtime license changes.
- A selected artifact, tokenizer, quantization, prompt, or backend version changes.
- A platform changes signing, notarization, WebView, microphone, accessibility, or
  package requirements.
- A security advisory affects a trust boundary described here.
- A phase begins after the reviewed ecosystem baseline is no longer current.

Update the affected plan and decision record whenever revalidation changes the
recommendation.
