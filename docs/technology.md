# Technology stack

## Decision policy

This baseline reflects primary-source review through August 12, 2026. Exact versions
are pinned in lockfiles, manifests, and release inputs. A version in this document is
a reviewed starting point, not permission for an unattended upgrade.

Shipping code prefers the newest stable or generally available release that passes
the complete affected matrix. Alpha, beta, release-candidate, nightly, and
working-draft inputs are allowed only for isolated research or an explicit pinned
compatibility requirement. `Latest` never means a floating tag.

Every dependency owns one necessary capability. Admission considers source,
maintenance, license, advisories, unsafe code, native libraries, build scripts,
procedural macros, transitive size, platform cost, offline behavior, and removal
path. A popular package or compatible API shape is not enough.

## Current baseline

| Layer | Choice | Status |
| --- | --- | --- |
| Language | Rust 1.97.1, edition 2024, resolver 3 | Exact current pin |
| Async runtime | Tokio 1.53.x | Chosen |
| Serialization | Serde, serde_json, schemars | Chosen by contract need |
| Errors | thiserror in libraries, anyhow at binary boundaries | Chosen |
| CLI | clap 4.6.x | Chosen |
| HTTP client | reqwest 0.13.x | Chosen for bounded loopback adapters |
| Logging | tracing and tracing-subscriber | Chosen with content redaction |
| Persistence | rusqlite 0.40.2 with bundled SQLite | Artifact state implemented; profiles planned |
| Markdown | pulldown-cmark 0.13.x with source offsets | Planned bounded adapter |
| DOCX | zip 8.x plus quick-xml 0.41.x | Planned bounded adapter |
| First attached runtime | Ollama native API | Implemented candidate |
| Portable runtime | Pinned `llama-server` sidecar | Second qualification target |
| Local API | axum 0.8.x, tower 0.5.x, tower-http 0.7.x | After stdio agents |
| MCP | Official `rmcp` 3.1.2 candidate | Planned exact pin and revalidation |
| Agent package | Agent Plugins 1.0.0 schemas | Working-draft compatibility target |
| Knowledge exchange | Open Knowledge Format 0.2 | Experimental import and export target |
| Desktop | Native Rust toolkit selected by cross-platform spike | Decision gate |
| Tests | proptest, insta, assert_cmd, wiremock, tempfile | Added only by need |

Versions are rechecked at implementation and release gates. A newer stable patch is
preferred only after it passes compatibility, supply-chain, platform, and product
qualification. The exact Rust release and broader engineering baseline are recorded
in [Rust engineering and release research](research/2026-08-12-rust-engineering.md).

## Rust workspace

The workspace uses a committed `rust-toolchain.toml`, `Cargo.lock`, edition 2024,
resolver 3, shared dependency declarations, warnings denied, and strict workspace
lints. Rust 1.97.1 is both the repository pin and current support floor until an
explicit older minimum is adopted and tested separately.

All-features validation and shipping builds are different jobs. A shipping feature
manifest excludes test support, fuzz hooks, instrumented presentation hooks, and
experimental adapters. Default, no-default, shipping, and all-feature configurations
receive explicit coverage.

Domain and application crates forbid unsafe code. A native or FFI dependency is
preferred behind a process boundary. Any unavoidable first-party unsafe code requires
an architecture decision and one quarantined adapter crate with a safe bounded
facade, safety arguments, Miri where applicable, sanitizers on supported targets,
fuzzing, and native platform tests.

Release builds use stable Rust. Date-pinned nightly exists only for isolated Miri,
fuzz, sanitizer, or diagnostic lanes. Moving stable and beta jobs are non-publishing
canaries.

## Local inference

### Adapter architecture

The core uses small inference and embedding ports. Each runtime implementation has
three separate responsibilities:

1. A transport dialect sends and decodes bounded messages.
2. An identity driver establishes the runtime, artifact set, template, tokenizer,
   effective parameters, output policy, and execution class.
3. An acquisition driver describes explicit download, offline import, runtime import,
   or attachment to user-managed state.

An OpenAI-compatible transport can be reused without treating every compatible
server as the same runtime. Unknown identity, hidden defaults, silent fallback, or an
undisclosed output postprocessor prevents qualification.

### Ollama

Use the native Ollama API because it exposes version, inventory, model details, and
running state that the compatibility surface does not establish.

The adapter accepts only validated IP-literal loopback endpoints, disables proxies
and redirects, bounds every response, and checks identity before and after
generation. It never starts, installs, updates, pulls, or reconfigures Ollama as a
side effect of rewriting.

Qualification still requires stronger effective identity than a mutable tag or
reported version. It binds the source artifact set, Ollama inventory digest, complete
effective model-description digest, runtime package identity where available,
running context, residency, and CPU, GPU, or hybrid execution class.

### Pinned llama.cpp sidecar

The portable path launches one exact verified `llama-server` executable and one exact
GGUF artifact on IP-literal loopback in offline mode. Retonr owns arguments,
environment, health, context, tokenizer checks, output limits, process lifetime,
cancellation, and redacted logs.

No Hub shortcut, router, model directory, autoload, automatic fitting, remote media,
plugin, or mutable slot state is used in a qualified launch. CPU, Metal, CUDA, HIP,
Vulkan, and hybrid builds qualify independently.

The process boundary preserves the safe Rust core and is preferred to in-process FFI
unless measurements prove a product need that justifies a separate safety decision.

### Additional 0.x runtime candidates

LM Studio, vLLM, MLX LM, and generic compatible endpoints are candidates, not
qualified defaults. Each requires a named driver and exact platform, license, identity,
offline, setting, output-policy, resource, and cancellation evidence.

- LM Studio is an external proprietary application with native discovery but mutable
  runtime selection and no documented content digest sufficient by itself.
- vLLM is primarily a Linux workstation or controlled server path and supports
  dynamic behavior that must be disabled for qualification.
- MLX LM is primarily an Apple silicon experiment whose server documentation does
  not yet establish the required production identity and security contract.
- A generic compatible endpoint is transport-only and remains experimental without a
  runtime-specific identity driver.

The complete matrix is in
[Provider-neutral, user-controlled model runtimes](research/2026-08-12-provider-neutral-runtimes.md).

### Output policy

Retonr never enables a known statistical watermark, watermark generation setting,
output-signature processor, or opaque postprocessor in a qualified path. It
inventories every configured sampler, logits processor, adapter, template, system
prompt, renderer, parser, and postprocessor.

A runtime that requires an undisclosed watermark is ineligible for generation.
Review and controlled tests support only a bounded claim about the configured stack.
They cannot prove that weights contain no learnable statistical signal or predict
every future detector. Detector observations never rank live candidates or weaken
fidelity.

## Artifacts and offline operation

One file digest is insufficient for sharded or composed models. The canonical content
manifest records every required portable relative path, length, and byte digest. A
separate effective-package evidence record joins that content identity to immutable
origin revisions, member purposes, tokenizer and template roles, license decisions,
conversion inputs, tools, arguments, logs, exclusions, and outputs. Keeping content
identity separate avoids changing the artifact-set ID when review evidence grows while
still requiring the qualification workflow to prove that every output-affecting member
was included.

The implemented version 1 record is inert and provider-neutral. It binds the exact
artifact-set, runtime-build, and effective-state identities; requires one canonical
purpose set for every member path; and binds retained completeness, acquisition,
license-review, transformation, load-closure, and exclusion and isolation evidence by
digest. Managed evidence is accepted only with a managed runtime build. Attached
evidence requires a locally attested process or container build. Bounded JSON decoding
rechecks every referenced object and relationship. A separate inert qualification v2
record now consumes and rechecks the exact four-part subject for claim extraction. It
also binds the prompt, claim-output and claim-operation contracts, request and threshold
policies, language, hardware, suite, result evidence, license decision, and outcome. Its
distinct identifier cannot enter v1 activation. No production evidence producer,
active binding, live attestor, or runtime authority consumes either record yet.

The SQLite adapter now persists the artifact-set manifest, runtime-build identity,
effective runtime state, effective-package evidence, and qualification-v2 record as an
inert dependency chain. Schema v3 leaves all v1 authority records byte-for-byte
unchanged. Writes and reads recompute every content identity, require canonical JSON,
compare duplicated indexed references, reload the complete subject, and fail closed on
missing or cross-product state. Persistence is not attestation and creates no active
binding.

Current one-file offline import inspects without execution, rejects indirect and
special files, copies one regular file into private staging, hashes it, and registers
it without activation. A later artifact-set import must inspect and hash every member,
join reviewed license evidence, and require network-denied qualification before any
activation. Hard links, symbolic links, mutable tags, plugin code, pickle weights, and
unreviewed remote code are not active artifact identity.

Rewriting never downloads a missing runtime, model, tokenizer, template, adapter, or
plugin. Drift before or after a generation batch discards every candidate and
invalidates activation.

## Long documents and context

Artifact-declared context, observed runtime context, qualified context envelope, and
per-request source budget are separate values. Advertised capacity does not establish
position fidelity, memory safety, cancellation, or consistency.

The exact tokenizer calculates a conservative request budget after reserving space
for instructions, schemas, output, protected facts, document guidance, neighboring
context, and safety margin. Silent truncation, context shifting, summarization, or
automatic overflow recovery is prohibited.

Long files use model-free inventory, a source-linked document map, bounded unit
requests, unit validation, region consistency, adapter-owned reassembly, full
document verification, staging, and an exact report. The complete contract is in
[Non-destructive document and folder transactions](document-transactions.md).

## Persistence

SQLite is implemented for artifact manifests, installed state, v1 qualification and
invalidation records, activation decisions, active role pointers, and the five inert
evidence records required by qualification v2. The adapter
uses bundled SQLite behind a dedicated crate; domain and application types do not
expose SQL rows. Immediate transactions preserve the prior valid pointer when an
activation cannot commit. Activation and recovery require caller-reverified artifact
bytes before returning a binding. Qualification identifiers hash a versioned,
length-delimited encoding of the complete record. Stored record content, indexed
columns, and derived identities must agree before activation or recovery.

Profiles, evidence metadata, rules, versions, feedback, and redacted rewrite records
remain planned for the same bounded persistence layer. Migrations require forward
tests from every supported version with pre-migration backup, interruption,
corruption, and verified recovery fixtures before those stores freeze.
The implemented artifact repository exposes this authority only through the explicit
confirmed `model migrate` operation. It retains one SQLite write reservation across
source validation, a bounded logical backup into a rollback-mode snapshot,
serialization into the exact held repository file, same-handle verification,
synchronization, and the supported migration commit. Ordinary repository opens remain
exact-schema and non-migrating.

Content-derived embeddings live in a qualified embedding space identified by exact
artifact set, runtime, tokenizer, preprocessing, instruction, normalization,
dimensions, distance, truncation policy, and serialization. Any identity change
invalidates affected indexes.

## Markdown and DOCX

Markdown uses source offsets only as anchors. The adapter owns eligible UTF-8 ranges,
context escaping, structural fingerprints, protected constructs, reverse-order
splicing, output reparse, and non-target byte identity.

DOCX is handled directly as a bounded OPC package through ZIP and XML adapters. No
library is assumed to provide lossless preservation. The adapter owns eligible runs,
relationships, content types, XML bounds, untouched-part verification, reopen tests,
and qualified renderer evidence.

Spreadsheet support is post-1.0. It requires a separate SpreadsheetML adapter because
formulas, cell types, shared strings, names, references, validation, charts, macros,
and calculation state cannot be protected by generic XML replacement.

Every fixed preservation defect adds a minimized regression fixture.

## CLI and file transactions

The CLI is the reference product surface. It separates data on standard output from
diagnostics on standard error, never prompts in non-interactive mode, uses versioned
machine schemas and stable exit categories, neutralizes hostile terminal content,
and provides explicit dry-run, diff, report, cancellation, and recovery behavior.

File and directory discovery is model-free. A reviewed manifest freezes source
digests, paths, formats, capabilities, bounds, destinations, collisions, links,
atomicity, and exclusions. Outputs go to a separate destination by default. A staged
commit rechecks source identity and cannot delete a source tree.

## MCP, Agent Skills, and Agent Plugins

The agent order is:

1. Stable CLI machine contract
2. MCP 2026-07-28 over standard input
3. Thin filesystem Agent Skill
4. Agent Plugins 1.0.0 routine package
5. Named-client compatibility
6. Authenticated local API
7. MCP Streamable HTTP

The current MCP target uses self-contained requests with required per-request version
and capabilities metadata, deterministic `server/discover`, recognized result types,
and no protocol session. Connection identity never establishes a user, profile,
conversation, or operation.

Agent Plugins 1.0.0 is a Working Draft. Retonr pins its `plugin.json` and `mcp.json`
schemas and package semantics by exact digest. The routine package contains only a
routine skill and a standard-input MCP server entry. A separate privileged package
is considered only after profile scopes stabilize.

Agent Plugins specifies packaging, not signatures, distribution, updates,
permissions, or sandboxing. Retonr's release layer owns those controls. Package
validation resolves symlinks, junctions, reparse points, commands, working
directories, and component paths without executing code or accessing the network.

Skills over MCP remains an in-review optional extension and is not a 1.0 dependency.
The exact standards and gates are in
[Agent integration research](research/2026-08-12-agent-integrations.md).

## Portable knowledge exchange

Open Knowledge Format 0.2 is the current experimental target for agent- and
human-readable knowledge bundles. It can export research claims, support matrices,
style-policy explanations, document briefs, and redacted preference views as
Markdown with YAML frontmatter and links.

OKF does not replace SQLite, canonical JSON schemas, MCP, Agent Plugins, consent,
authorization, or transaction records. Its trust tiers are advisory and its
Attested Computation contract does not define packaging, sandboxing, permissions,
or a complete runtime protocol. Retonr pins an exact 0.2 revision for a compatibility
spike, preserves unknown fields, imports no authority, and executes no referenced
resource. See [Open Knowledge Format and Retonr](research/2026-08-12-open-knowledge-format.md).

## First-party loopback API

The API follows the standard-input agent path. It starts only through an explicit
command and binds to an actual loopback address. It uses versioned schemas, high
entropy local authentication, scopes, Host and Origin checks, no-store responses,
deadlines, cancellation, resource limits, redacted logs, and principal-scoped
operations.

The native desktop calls the application service in process and does not start a
local server for ordinary use.

MCP Streamable HTTP is added only after the local service and standard-input MCP pass.
It implements the exact modern transport and rejects legacy or session behavior not
supported by the pinned revision.

## Completed-response compatibility

The compatibility adapter accepts a bounded completed response payload and performs
no outbound request. It is not a transparent proxy or model backend.

Only pinned ordinary-text paths are eligible. Tool calls, structured outputs,
reasoning, refusals, citations, annotations, signatures, images, audio, and
unsupported events return the exact original as unsupported. Malformed or oversized
input returns no payload. Abstention and verification failure return the exact
original with distinct machine status.

Byte-range JSON string splicing preserves non-target bytes. A separate local record
states original and rewritten digests, target schema, eligible paths, adapter,
validator, and result. Retained upstream IDs, usage, and fingerprints remain labeled
as properties of the original response.

## Native desktop

The desktop is an installed native Rust application built after CLI and agent
contracts. It does not embed a browser, use an HTML or JavaScript frontend, render a
hosted application, or require loopback HTTP for ordinary operation.

The toolkit remains a decision gate. Comparable Slint, Iced, egui, or other maintained
native Rust spikes must prove the actual Retonr workflows across Windows, macOS, and
Linux. Selection requires:

- International multiline text and diff performance
- Complete keyboard and screen-reader semantics
- Input methods, bidirectional text, high contrast, scale, and reduced motion
- Native menus, dialogs, shortcuts, drag and drop, clipboard, and platform packaging
- Deterministic component, presentation-state, visual, and black-box testing
- Acceptable native dependencies, unsafe boundary, license, maintenance, startup,
  memory, graphics, and update behavior

Presentation sends typed commands to the application service and receives bounded
sequenced state. It has no independent product authority and renders imported or
generated content as untrusted text.

## Cross-platform release and updates

Support names exact Rust target triples, operating-system floors, architectures,
packages, graphics or accelerator backends, and hardware envelopes. Build and test
packages on the operating system they target.

Windows artifacts are signed. macOS artifacts are signed and notarized. Linux ships
only formats that pass clean install, upgrade, recovery, removal, and desktop
integration on named distributions. Bootstrap installers verify a signed manifest
and artifact digest before execution, install per user without elevation, stage side
by side, smoke-test, and switch atomically.

Update checks are explicit and separate from core operation. Runtime, model,
protocol, schema, dependency, and application upgrades invalidate only the evidence
they actually affect and never silently change a document or profile.

## Tooling and release evidence

| Concern | Tooling or policy |
| --- | --- |
| Format and lint | rustfmt and Clippy with warnings denied |
| Tests | cargo-nextest plus separate documentation and exact shipping-feature tests |
| Coverage | cargo-llvm-cov with an 80 percent repository floor |
| Properties and goldens | proptest and reviewed insta fixtures |
| CLI and HTTP | assert_cmd and wiremock |
| Fuzz and mutation | cargo-fuzz and cargo-mutants on applicable critical paths |
| Undefined behavior | Date-pinned Miri and target-qualified sanitizers |
| Dependency policy | cargo-deny, cargo-audit, and planned Cargo Vet |
| Rust API compatibility | cargo-semver-checks only for deliberately stable Rust APIs |
| Supply chain | Vendored frozen source, binary dependency metadata, per-artifact SBOM |
| Release | Independent unsigned rebuild comparison, checksums, signatures, and control-plane provenance |
| Native desktop | Toolkit-owned component tests plus qualified platform-native black-box and accessibility harnesses |

## Primary references

- [Rust releases](https://blog.rust-lang.org/releases/)
- [Cargo features](https://doc.rust-lang.org/cargo/reference/features.html)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Ollama API](https://docs.ollama.com/api/introduction)
- [llama.cpp server](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [pulldown-cmark](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/)
- [ECMA-376 Office Open XML](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- [MCP 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28)
- [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [Agent Skills](https://agentskills.io/specification)
- [Agent Plugins 1.0.0](https://agent-plugins.org/specification)
- [Open Knowledge Format 0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
- [Slint](https://slint.dev/)
- [Iced](https://iced.rs/)
- [egui](https://docs.rs/egui/latest/egui/)
- [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- [Cargo Vet](https://mozilla.github.io/cargo-vet/)
- [SLSA provenance](https://slsa.dev/spec/v1.2/provenance)
