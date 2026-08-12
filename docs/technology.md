# Technology stack

## Decision policy

This baseline reflects the ecosystem reviewed on August 11, 2026. Exact versions are
pinned in lockfiles when implementation begins. A version in this document is a
reviewed starting point, not permission for unattended upgrades.

Every dependency must have a clear owner, maintenance signal, compatible license,
cross-platform evidence, and a smaller cost than implementing the required behavior
directly. Model files receive the same license and provenance review as code.

## Recommended baseline

| Layer | Choice | Status |
| --- | --- | --- |
| Language | Rust 1.97.1, edition 2024, resolver 3 | Chosen for prototype |
| Async runtime | Tokio 1.53.x | Chosen |
| Serialization | Serde, serde_json, schemars | Chosen |
| Errors | thiserror in libraries, anyhow at binary boundaries only | Chosen |
| CLI | clap 4.6.x | Chosen |
| HTTP client | reqwest 0.13.x | Chosen |
| Local API | axum 0.8.x, tower 0.5.x, tower-http 0.7.x | Planned after CLI |
| Logging | tracing and tracing-subscriber | Chosen |
| Persistence | rusqlite 0.40.x with bundled SQLite | Chosen for prototype |
| Markdown | pulldown-cmark 0.13.x with source offsets | Chosen for spike |
| DOCX | zip 8.x plus quick-xml 0.41.x | Planned bounded adapter |
| Local generation | Ollama native API | Chosen first backend |
| Second generation backend | Pinned llama-server sidecar over HTTP | Deferred |
| MCP | Official `rmcp` 3.1.x | Planned |
| Desktop shell | Tauri 2.11.x | Planned |
| Desktop frontend | TypeScript, React, React Aria, Vite | Recommended pending UX spike |
| Audio capture | CPAL 0.17.x behind a feature | Planned for voice milestone |
| Speech inference | Backend trait with sherpa-onnx and whisper.cpp evaluated | Decision gate |
| Tests | proptest, insta, assert_cmd, wiremock, tempfile | Chosen by need |

The desktop frontend recommendation favors a mature accessibility and testing
ecosystem. Business rules remain in Rust. The webview layer owns presentation and
calls narrow Tauri commands over versioned types. A Rust-only frontend can be
reconsidered if it meets the same accessibility, testability, and cross-webview bar.

## Rust workspace

The workspace uses a committed `rust-toolchain.toml`, `Cargo.lock`, edition 2024,
resolver 3, and a shared dependency table. Rust 1.97.1 is selected because it is the
current patched stable release at planning time.

Library crates use typed errors and avoid process exit, logging policy, and global
configuration. Application binaries translate stable application errors into CLI,
HTTP, MCP, or desktop presentation.

Pure domain crates use `#![forbid(unsafe_code)]`. Native FFI for audio or inference is
isolated in dedicated infrastructure crates with a documented safety boundary.

## Local inference

### Ollama

Use the native Ollama API instead of assuming its compatibility API is complete.

Discovery precedence:

1. Explicit command option
2. Product-specific environment variable
3. Validated `OLLAMA_HOST` compatibility input
4. `http://127.0.0.1:11434`

The product-specific setting is authoritative. Ollama primarily documents
`OLLAMA_HOST` as a server bind setting, so the client treats it as compatibility
input rather than an unquestioned endpoint.

The first adapter accepts loopback endpoints only, disables system proxies and
redirects, probes `/api/version`, enumerates `/api/tags`, and inspects the selected
artifact through `/api/show`. It checks the selected digest before and after
generation and discards results if identity drifts. Its reqwest build has no TLS
feature because this adapter rejects HTTPS and can contact only IP-literal loopback.
Future HTTPS artifact acquisition uses a separate reviewed client. Version,
capability, identity, and local-policy failures are actionable typed errors. Version
1.0 does not automatically start, install, pull, or manage the Ollama process.

Structured output constrains response syntax, not factuality. Every result is parsed,
bounded, validated, and treated as untrusted input.

Qualified generation uses `stream: false`, `think: false`, an explicit JSON schema,
and explicit context, output-token, sampling, seed, and empty stop settings. Ollama has no
documented exact native preflight token-count endpoint for the recorded tokenizer.
Until the product owns a matching tokenizer, qualification establishes conservative
per-artifact byte and context envelopes and long inputs abstain before generation.

Model tags are mutable. Qualification and rewrite records identify the artifact
digest, quantization, tokenizer, backend version, prompt-template digest, context
limit, parameters, and seed. Seeded generation is not described as bitwise portable
across hardware or backend versions.

### Model candidates

Qwen3.5-9B is a real Apache-2.0 candidate and a reasonable quality tier. It is not
declared the universal default until the project benchmark runs on representative
hardware. The Q4_K_M artifact is approximately 6.6 GB before runtime overhead.

The qualification matrix should include:

- A smaller 4B-class laptop tier
- Qwen3.5-9B as the balanced candidate
- At least one larger quality tier
- Exact quantizations for every advertised tier
- Realistic context caps such as 8K, 16K, and 32K

Thinking output is disabled or discarded for normal rewriting. Hidden reasoning is
never stored in traces.

Qwen3-Embedding-0.6B is the initial embedding candidate. The canonical
`embedding_space_id` covers the exact artifact, runtime, dimensions, input
instruction, preprocessing, normalization, truncation, byte and token limits,
distance, quantization, and serialization identity. Requests set `truncate: false`.
Any field change invalidates the corresponding vectors. The adapter checks the
complete artifact and runtime identity before and after each batch, discards the batch
on drift, and requires requalification and reindexing.

### llama.cpp

The second backend should initially use a pinned `llama-server` sidecar over HTTP.
This limits exposure to a changing native API and keeps unsafe FFI out of the core.
Only the supported common schema subset is used, and all generated JSON is validated
again in Rust.

## Persistence and retrieval

SQLite stores schema migrations, immutable profile versions, evidence metadata,
declared rules, preference signals, model manifests, installed-artifact state,
append-only qualification and invalidation records, activation decisions, active
role pointers, and selected rewrite metadata.

Required settings and patterns:

- Foreign keys enabled
- Write-ahead logging where supported
- Busy timeout
- Forward-only migrations with pre-migration backup and verified restore rollback
  tests
- One controlled writer
- Blocking database work kept off async executors
- Atomic profile version creation
- Atomic artifact activation that binds an installed digest to a currently valid
  qualification, appends the activation decision, and updates one role pointer
- Startup and recovery revalidation of every active artifact binding
- Complete export and deletion

The expected personal corpus does not need a vector database. Store versioned `f32`
vectors as blobs, filter by profile and channel, and score cosine similarity in Rust.
Use FTS5 for lexical retrieval. `sqlite-vec` remains an optional experiment because
its Rust integration is pre-1.0 and outside normal semantic-version guarantees.

Sensitive text encryption requires a separate decision. The design must cover both
desktop keychains and headless CLI environments, backup and recovery, migration,
lost keys, and Linux installations without a secret-service daemon.

## Markdown

`pulldown-cmark::Parser::into_offset_iter()` provides source ranges, but those ranges
do not form a lossless mutable syntax tree. The implementation uses them only to
identify a deliberately small set of non-overlapping eligible spans.

The adapter preserves LF or CRLF, byte order mark state, final newline state, and all
bytes outside approved edits. It reparses output and compares a structural
fingerprint. CommonMark plus each enabled extension is versioned as an adapter
capability.

## DOCX

No general Rust DOCX library is assumed to provide lossless package preservation.
The adapter works directly with the OPC ZIP package using stable `zip` and
`quick-xml`.

Untouched package parts retain identical decompressed bytes. Supported XML changes
are narrow and verified. The adapter never extracts package paths to the filesystem,
loads external entities, follows external relationships, or accepts unbounded ZIP or
XML input.

Compatibility qualification includes schema checks, package-part hashes, reopen
tests, and fixtures for the exact supported WordprocessingML matrix.

## First-party API

The planned local API uses axum and tower. It starts with an explicitly preview `/v0`
surface during development and promotes the compatibility-tested contract to `/v1`
at the 0.9 freeze. Version 1.0 binds to loopback only and includes:

- Capability and version discovery
- Request and response JSON schemas
- RFC 9457 transport problems and separate successful domain outcomes
- Cancellation and deadlines
- Body, token, candidate, concurrency, and time limits
- Conditional writes and client operation IDs for mutation requests
- Explicit profile-read and profile-write authorization
- High-entropy local authentication plus Host and Origin validation
- `Cache-Control: no-store` on every authenticated success and error response
- Redacted logs and no content logging by default

The desktop application calls the application service in process. It does not start
a second HTTP server for ordinary use.

## MCP and agent skills

Use the official Rust SDK and target MCP 2026-07-28. Modern requests carry required
protocol version and client capabilities plus optional `clientInfo` in request
metadata. The server implements `server/discover`; there is no initialize exchange or
protocol session. Implement standard input first, then POST-only Streamable HTTP.
Support an older revision only for a named client with complete compatibility
fixtures.

Streamable HTTP uses a documented custom loopback bearer profile for 1.0 rather than
standard MCP OAuth authorization. It returns a bearer challenge for missing or
invalid credentials, enforces server-side scopes, and is excluded from standard
authorization conformance. Standard input is preferred for clients that cannot
inject the token.

The MCP surface remains small:

- Rewrite content
- Check content and constraints
- Read profile metadata
- Apply explicit profile changes
- Run typed or voice-assisted acquisition through explicit handles

Agent skill packages use the stable `SKILL.md` format as thin clients. They include
stable frontmatter, instructions, schemas, examples, explanatory authority
requirements, offline behavior, and a compatible protocol or API range, and pass
reference validation and smoke tests against the release binary. Real authority is
enforced by the server. Experimental `allowed-tools` metadata is never a security
control. The packages contain no profile or rewrite logic. Skills over MCP remains an
experimental working-group proposal and is not a 1.0 dependency.

## Compatibility proxy

The product first exposes its own API. Version 1.0 compatibility with an upstream
response schema is an offline local post-processing adapter with a published
conformance matrix. It accepts a completed response payload from the caller. It does
not contact an upstream service, store upstream credentials, or act as a generation
backend.

The first supported mode is non-streaming, text-only final assistant content. Tool
calls, structured output, reasoning, refusal events, citations, images, audio,
logprobs, and unsupported event types make an otherwise valid payload unsupported.
Malformed or oversized input returns no payload and a typed error. Unsupported valid
input, rewrite abstention, and final verification failure return the exact original
bytes with distinct machine status. They are never silently or partially rewritten.

The adapter uses byte-range JSON string splicing and verifies non-target byte
identity against a pinned upstream schema. Original upstream IDs, usage, and
fingerprints remain original-response metadata. A separate approved sidecar or
envelope carries the local rewrite status and record.

Streaming input is not supported by the 1.0 adapter. A future outbound reverse proxy
would have to buffer a complete candidate until validation succeeds and could not
claim to preserve upstream token boundaries, timing, logprobs, usage accounting, IDs,
or fingerprints.

## Desktop

Tauri 2.11.x provides the Rust application shell and platform packaging. Its webview
differs by platform: WebView2 on Windows, WKWebView on macOS, and WebKitGTK on Linux.
Functional and visual tests therefore run on all three systems.

Security defaults:

- Explicit enabled capabilities and custom-command manifest
- Least-privileged capabilities per window label
- Opaque, expiring application authority grants bound to the exact window session,
  resource, operation, and action for sensitive work
- Application-level scope checks and negative authorization tests; frontend routes
  are never authorization inputs
- Narrow filesystem and dialog scopes
- Strict content security policy
- No broad shell, process, HTTP, opener, or filesystem plugin
- No remote frame, script, style, font, image, inline script, or eval
- Imported and generated prose rendered as text, never untrusted HTML
- Just-in-time permissions with clear purpose

The frontend uses semantic HTML, accessible primitives, a small design-token system,
and no business logic that is unavailable to the CLI or API. A long-running command
creates a suspended, window-owned operation, then installs a per-invocation Tauri
channel before work begins. Targeted events use contiguous monotonic sequences. A
sequence gap triggers an authoritative snapshot query; close or reload revokes
ownership and cancels the desktop-owned operation. Global privileged event broadcasts
are not used.

## Local voice

Voice is optional at runtime but required as a working 1.0 acquisition interface.
The typed interview remains complete and equivalent.

```rust
trait AudioCapture { /* device discovery and PCM stream */ }
trait Transcriber { /* local speech to editable text */ }
trait Speaker { /* optional local spoken prompts */ }
```

CPAL is the capture candidate. Whisper.cpp and sherpa-onnx are evaluated for local
speech recognition, with sherpa-onnx also evaluated for local speech output and voice
activity detection. The decision depends on accuracy, hallucination behavior, CPU
latency, memory, binary distribution, accessibility, and runtime, model, voice, and
phonemizer licenses across all target platforms.

The 1.0 voice gate requires local speech input and local spoken prompts or responses
using an audited runtime and voice model. Model installation is explicit, resumable,
checksummed, removable, and supports offline import. Each model has a manifest with
source, license, size, checksum, and declared language metadata. A separate
qualification record owns tested runtime, platforms, languages, accuracy, latency,
and resource results.

Piper is not selected casually because the current runtime is GPL-3 and individual
voice licenses vary. Other speech models receive the same artifact-level review.

Audio defaults:

- Push-to-talk
- Visible microphone state
- Explicit input-device selection
- Local processing
- Transcript preview and correction before ingestion
- Raw audio removed immediately from application-controlled buffers and storage after
  transcription unless the user opts in
- Captions and full keyboard operation
- No PCM transfer through WebView IPC
- Preallocated bounded callback buffers with no allocation, blocking, logging, file
  I/O, IPC, or inference in the callback
- No always-listening, wake-word, simultaneous capture and speech output, speaker
  identity, or voice cloning in 1.0

## Cross-platform delivery

Initial executable targets:

- Windows x86_64
- macOS universal application with qualified aarch64 and x86_64 slices
- Linux x86_64

Additional architectures can graduate through the same test and packaging gates.

Update signatures establish artifact provenance and integrity, not semantic
permission safety. Release qualification retains a reviewed diff of Tauri
capabilities, custom commands, native permissions, network destinations, filesystem
scopes, and application authority rules. Any broader network, privacy, or
external-system policy is disclosed and requires explicit consent before enablement.

Build installers on their target operating system. Windows packages are signed.
macOS packages are signed and notarized. Linux ships only formats that pass install,
upgrade, removal, and desktop-integration tests on declared distributions. Every
Linux format has verifiable signed artifacts or signed repository metadata and a
clean-install verification test.

All code uses platform path and process APIs. No feature relies on shell-string
construction, `/tmp`, Unix-only signals, case-sensitive paths, or UTF-8 filesystem
paths.

## Tooling

| Concern | Tooling |
| --- | --- |
| Format | rustfmt |
| Lint | Clippy with warnings denied |
| Test runner | cargo-nextest plus separate documentation tests |
| Coverage | cargo-llvm-cov with an 80 percent repository floor |
| Property tests | proptest |
| Golden output | insta where semantic assertions also exist |
| CLI tests | assert_cmd |
| HTTP simulation | wiremock |
| Fuzzing | cargo-fuzz on Linux |
| Mutation testing | cargo-mutants on critical crates |
| Frontend unit and component tests | Vitest plus Testing Library |
| Frontend coverage | `@vitest/coverage-v8` with general and critical threshold configs |
| Frontend accessibility | axe-core plus keyboard and accessibility-tree tests |
| Instrumented desktop end-to-end | WebdriverIO Tauri service in a dedicated non-release feature |
| Signed desktop black box | External native or accessibility harness selected per platform; XCTest or equivalent is qualified for macOS |
| Visual and ARIA regression | Playwright in controlled per-platform environments |
| Dependency and license policy | cargo-deny |
| Vulnerability audit | cargo-audit |
| Release packaging | cargo-dist for CLI where qualified, Tauri bundler for desktop |

## Primary references

- [Rust releases](https://blog.rust-lang.org/releases/)
- [Cargo workspaces](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Ollama API](https://docs.ollama.com/api/introduction)
- [Ollama structured output](https://docs.ollama.com/capabilities/structured-outputs)
- [Ollama embeddings](https://docs.ollama.com/capabilities/embeddings)
- [Ollama installed models](https://docs.ollama.com/api/tags)
- [Ollama model details](https://docs.ollama.com/api-reference/show-model-details)
- [Qwen3.5-9B](https://huggingface.co/Qwen/Qwen3.5-9B)
- [Qwen3 Embedding](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B)
- [llama.cpp server](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [pulldown-cmark source offsets](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.Parser.html)
- [MCP specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/index)
- [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [MCP conformance](https://github.com/modelcontextprotocol/conformance)
- [Agent Skills specification](https://agentskills.io/specification)
- [Skills over MCP status](https://modelcontextprotocol.io/community/working-groups/skills-over-mcp)
- [Tauri releases](https://v2.tauri.app/release/)
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/)
- [Tauri command scopes](https://v2.tauri.app/security/scope/)
- [Tauri updater](https://v2.tauri.app/plugin/updater/)
- [Tauri WebDriver testing](https://v2.tauri.app/develop/tests/webdriver/)
- [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- [CPAL](https://github.com/RustAudio/cpal)
- [whisper.cpp](https://github.com/ggml-org/whisper.cpp)
- [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx)
