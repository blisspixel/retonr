# Agent integration research and release gates

## Scope and status

This ledger records the integration baseline reviewed on 2026-08-12. It covers
Agent Plugins 1.0.0, Agent Skills, MCP 2026-07-28, the official Rust MCP SDK,
and the official MCP conformance suite. It is a planning input, not a claim that
Retonr currently implements these interfaces.

The ecosystem is moving quickly. Agent Plugins 1.0.0 is the current published
format, but its normative page labels it a Working Draft. MCP 2026-07-28 is the
current stable protocol revision. Every dependency, schema, client, and protocol
claim in this document must be revalidated at the entry gate for implementation
and again before a release candidate is signed.

The research snapshot used Agent Plugins repository commit
`bd383552095128f6effe895b9257cfd580a6d179`, `rmcp` tag 3.1.2 at commit
`02c62aef2e331e5cf79c06c744eb1eb052cc8ebd`, and MCP conformance repository
commit `c321dd32035556e6769d3724a8ee97d87c3faaac`. At those exact inputs, SHA-256
is `0a4aad95ce337878ad38802ebf0daa3fde76abe3f65400c86bcbb1ec0b3ab883` for
`plugin.schema.json`, `6539175bfcdf43085855183e86da40ea94b166547a72b47ae9a0a390516d3acb` for
`mcp.schema.json`, and
`ae2f4f6210fd729e2e318edd5bbfa31a43cee0bc608e48052fa26dbf1d939b57` for the
frozen MCP 2026-07-28 requirements file. These are research evidence, not
automatic future implementation pins.

The Agent Plugins normative page and repository were revalidated on August 13,
2026. Version 1.0.0 remains the published Working Draft, and repository main remains
at `bd383552095128f6effe895b9257cfd580a6d179`. No compatibility or sequencing
change is required.

## Decision summary

- Treat the Retonr CLI and application service as the product. Agent packaging is
  a thin adapter around that stable product, not an alternative implementation.
- Publish an Agent Plugins 1.0.0 directory containing an Agent Skill and an MCP
  stdio entry after the CLI contract and MCP server pass their own gates.
- Do not build an Agent Plugins client, marketplace, installer, or updater for
  1.0. Retonr is a plugin author and MCP server in this relationship.
- Make MCP stdio the first agent runtime. It preserves the local-first boundary,
  requires no listening socket, and is the transport most directly represented by
  an Agent Plugins package that launches an installed `retonr` executable.
- Add loopback Streamable HTTP only after stdio and the local service authority
  model pass. Do not add legacy HTTP+SSE to Retonr.
- Keep routine rewriting and privileged profile administration in separate plugin
  packages or separately enabled server authorities. Installing the routine plugin
  must not grant profile mutation, model management, filesystem, clipboard, or
  administration authority.
- Keep skills declarative and thin. Do not put rewrite rules, fidelity validation,
  profile compilation, credentials, or executable helper scripts in a skill.
- Do not depend on the in-review Skills over MCP extension for 1.0. A filesystem
  Agent Skill packaged by Agent Plugins can call stable MCP tools without that
  extension.
- Pin `rmcp` 3.1.2 and its selected Cargo features when implementation begins,
  subject to revalidation. Pin the official conformance prerelease that actually
  contains the frozen 2026-07-28 requirements rather than using an unqualified
  latest tag.
- Require named, clean-install compatibility tests on Windows, macOS, and Linux.
  Format conformance does not prove that any particular client exposes skills,
  resolves executables, names tools, handles cancellation, or presents results in
  the same way.

## Layer boundaries

| Layer | Owns | Must not own |
| --- | --- | --- |
| Retonr application service | Rewrite decisions, validation, profile authority, limits, cancellation, records | Transport framing or client-specific behavior |
| CLI | Human and process interface, configuration, files, terminal safety | Alternative rewrite or validation logic |
| MCP server | Protocol mapping, schemas, transport errors, tool authority | Hidden state tied to a connection or agent prompt |
| Agent Skill | When and how an agent should call Retonr | Security authority, credentials, validation, or rewriting logic |
| Agent Plugin | Portable discovery of the skill and MCP server entry | Installation source, signatures, updates, sandboxing, or permission policy |
| Agent client | Installation, trust, process launch, tool presentation, user consent | Retonr product decisions or fidelity claims |

Agent Plugins, Agent Skills, and MCP solve different problems. Agent Plugins
packages and locates components. Agent Skills supplies model-readable instructions.
MCP carries typed runtime calls. None of these layers replaces the application
service contract.

## Verified Agent Plugins 1.0.0 findings

### Status and package model

Agent Plugins defines a portable directory format. Version 1.0.0 has two portable
component types: Agent Skills and MCP server entries. The required root manifest is
`plugin.json`. Skills live in immediate child directories of `skills/`, and MCP
configuration lives in root `mcp.json`. Client-specific data belongs under a
reverse-domain namespace in `plugin.json.extensions`, a matching top-level
directory, or both.

The package unit is a directory, not a ZIP, installer, registry object, or executable
bundle. The specification deliberately leaves distribution, installation sources,
marketplaces, enablement, update behavior, cache behavior, permissions, trust
prompts, sandboxing, and user experience to clients. A plugin that validates against
the portable format is not thereby trusted, signed, safely installed, or safely
executable.

The specification repository currently publishes normative text and two JSON
Schemas. It does not publish an automated Agent Plugins conformance runner. The
normative prose is authoritative when it conflicts with a schema. Retonr therefore
needs schema validation plus product-owned semantic fixtures for requirements that
JSON Schema cannot establish.

### Manifest contract

`plugin.json` is a closed JSON object. `$schema` and `name` are required. For 1.0.0,
the schema identifier is exactly:

```text
https://agent-plugins.org/schemas/1.0.0/plugin.schema.json
```

The name is 1 through 64 lowercase ASCII letters, digits, hyphens, or periods. It
must start and end with an alphanumeric character and cannot contain consecutive
hyphens or periods. `version`, `description`, `author`, `homepage`, `repository`,
`license`, `keywords`, and `extensions` are optional.

Semantic Versioning is recommended for the plugin `version`, but clients cannot
reject a manifest merely because the value is not valid Semantic Versioning. The
same permissive type-only rule applies to URL-like, email-like, and SPDX-like
metadata. Retonr must validate its own stronger release metadata policy separately.

Most manifest violations reject the entire plugin before component discovery. Two
cases are deliberately non-fatal: a client reports and ignores unknown top-level
fields, and it reports and ignores a non-object `extensions` value. Retonr packages
must still generate neither case.

Clients select locally implemented behavior from the canonical `$schema` value.
They must not download a schema while loading a plugin. Retonr release tests must
therefore validate against a reviewed, checked-in copy whose digest is recorded.

### Discovery and failure isolation

A skills-capable client inspects only immediate child directories of `skills/` and
recognizes a child only when its exact `SKILL.md` path resolves to a regular file.
It does not recursively discover nested skills. One invalid skill is skipped without
invalidating its valid siblings or MCP entries.

An MCP-capable client loads only root `mcp.json`. A top-level parse, schema, or
version failure disables MCP for the plugin. An invalid individual server entry
disables only that entry. A server launch, connection, authentication, or handshake
failure also leaves independent servers and skills available.

Every discovered, read, or executed package path must remain under the
filesystem-resolved plugin root after symlinks, junctions, reparse points, and other
platform equivalents are resolved. The specification defines narrow failure
boundaries, from rejecting the plugin for an escaping manifest to skipping only one
server for its escaping command or working directory. These containment rules do
not sandbox the launched process and do not constrain runtime paths passed as
ordinary arguments.

### MCP configuration

`mcp.json` contains exactly `$schema` and `mcpServers`. Its schema version must match
the version selected by `plugin.json`. The 1.0.0 schema identifier is exactly:

```text
https://agent-plugins.org/schemas/1.0.0/mcp.schema.json
```

Agent Plugins 1.0.0 represents `stdio`, `streamable-http`, and deprecated `sse`
entries. A client with MCP support must implement at least one of stdio or
Streamable HTTP and should implement both. It may omit legacy SSE. The declared
transport controls the initial connection attempt. The format defines no fallback
sequence.

For stdio, `command` is one executable token, not a shell command. It is either a
bare executable resolved through platform search rules or a plugin-relative path
beginning with `./`. `args` is a separate string array. A client may use the native
command interpreter when Windows requires it for a `.bat` or `.cmd` file, but it
must preserve the one-token command and separate arguments.

Clients provide `PLUGIN_ROOT` and a dedicated writable `PLUGIN_DATA` directory to
stdio subprocesses. Placeholder expansion is single-pass, non-recursive, and
limited to `args`, `env` values, and `cwd`. It does not apply to `command`,
environment keys, remote URLs, or HTTP headers. The plugin cannot override the two
reserved environment variables. A client may sanitize or omit ambient environment
variables, and a portable plugin cannot rely on unspecified ambient variables.

For Retonr, the portable package should use the separately installed bare `retonr`
command and pass MCP arguments separately. It should not ship per-platform wrapper
scripts. Retonr profiles, models, and application state remain in Retonr-owned data
locations, not `PLUGIN_DATA`; the latter can remain unused by the process.

Remote URLs must be absolute HTTP or HTTPS URLs without user information or a
fragment. Non-loopback endpoints require HTTPS. Literal configured headers are
visible package data and cannot contain secrets. Agent Plugins 1.0.0 has no portable
OAuth configuration or credential-reference field. Authentication discovery,
credential storage, and user interaction are client responsibilities.

### Installation, update, and supply-chain implications

Agent Plugins does not answer where a package came from, who signed it, whether an
update is authorized, which package version is installed, or whether its executable
matches its instructions. A `version` string can inform a client update check or
cache decision, but it is not a lockfile, dependency constraint, signature, digest,
or anti-rollback mechanism.

Retonr must supply those missing properties at its release layer:

- Publish the plugin directory from the same reviewed source revision as the CLI.
- Include the plugin in signed release artifacts and publish cryptographic digests.
- Record the compatible Retonr CLI range in the Agent Skill `compatibility` field
  and in release metadata, while recognizing that clients do not enforce it.
- Make installation and update explicit. Do not fetch or replace a plugin while a
  rewrite is running.
- Revalidate the plugin manifest, skill, MCP entry, package containment, and exact
  CLI compatibility after every update.
- Never copy credentials, profiles, model artifacts, or user content into the plugin
  directory.
- Treat a change in `command`, `args`, `env`, remote URL, headers, skill instructions,
  or client extension data as security-relevant in update review.

### Compatibility expectations

The Agent Plugins client list currently names VS Code, Cursor, GitHub Copilot,
ChatGPT and Codex, Kiro, Hermes Agent, OpenClaw, and Grok Bot. That page documents
incremental component and transport support. It is not a conformance test report and
does not freeze any client version.

A conformant client may support only skills or only MCP servers. It may ignore
client extensions. Tool qualification and display remain client policy. Retonr must
not assume that a valid package means both the skill and MCP entry will load, that a
bare executable will be found in a graphical application's environment, or that an
agent exposes the MCP tool under a particular qualified name.

The release matrix must name exact client versions and record, per operating
system, whether each client loads `plugin.json`, discovers the skill, launches the
stdio server, shows only the allowed tools, sends cancellation, accepts structured
results, and survives abstention and operational errors. Unsupported combinations
must be reported as unsupported rather than silently falling back to a client-owned
format.

## Agent Skills and Skills over MCP

Agent Plugins delegates skill validity to the Agent Skills specification. A valid
skill has a directory-matching `name`, a non-empty trigger-oriented `description`,
YAML frontmatter, and Markdown instructions. `license`, `compatibility`, and string
map `metadata` are optional. `allowed-tools` remains experimental and varies across
clients. It is never an authorization boundary.

Agent Skills permits arbitrary additional files and commonly uses `scripts/`,
`references/`, and `assets/`. Retonr's routine skill should contain only `SKILL.md`
unless a demonstrated compatibility need justifies a reference file. Avoiding
scripts removes an unnecessary cross-platform execution and supply-chain surface.

The skill should describe Retonr outcomes precisely: rewritten, unchanged,
abstained, unsupported, or operational failure. It should tell an agent to preserve
the user's disclosure choices and to present abstention honestly. It must not claim
human authorship, detector success, watermark removal, or formal semantic proof.

SEP-2640, the Skills over MCP extension, remains open and in review. Its current
direction exposes Agent Skills through MCP and delegates the content format to Agent
Skills. It is separate from Agent Plugins, whose purpose is installable directory
packaging. Retonr can ship a filesystem skill and MCP server in one Agent Plugin
without implementing SEP-2640. If the extension later becomes stable, it should be
added behind an explicit capability, with separate conformance and no change to the
filesystem plugin's authority.

## Verified MCP 2026-07-28 findings

### Modern lifecycle and versioning

MCP 2026-07-28 removes the protocol session and the
`initialize`/`notifications/initialized` exchange used by older revisions. Every
request is self-contained and includes these request metadata fields:

- `io.modelcontextprotocol/protocolVersion`
- `io.modelcontextprotocol/clientCapabilities`
- Optional `io.modelcontextprotocol/clientInfo`

Servers must implement `server/discover`, which reports supported versions,
capabilities, server identity, instructions, and cache metadata. A client may skip
discovery, make another call directly, and recover from
`UnsupportedProtocolVersionError`. Retonr should implement discovery fully and use
it in its product fixtures.

Unsupported versions return JSON-RPC `-32022` with supported and requested version
data. Every successful modern result contains `resultType`. Retonr emits
`"complete"` for ordinary final results. Treating a missing value as complete is a
client backward-compatibility rule, not permission for a modern Retonr server to
omit it.

Connection or process identity is not a session, principal, task, conversation, or
profile. Cross-request state uses an explicit server-minted handle that the client
passes on every request. Retonr learning and long-operation handles therefore remain
principal-scoped, bounded, expiring, revocable, and independently authenticated.

The modern protocol also adds Multi Round-Trip Requests and a Tasks extension.
Neither is required for Retonr's baseline rewrite tools. Retonr can return one final
validated result and keep its own explicit application operation contract. Adding
either protocol feature later requires an independent use case and conformance gate.

### stdio transport

The client launches the server. The server reads one newline-delimited JSON-RPC
message per line from standard input and writes one per line to standard output.
Messages cannot contain embedded newlines. Standard output contains no banners,
progress decoration, prompts, or logs. UTF-8 diagnostics may go to standard error,
and clients must not assume standard-error text itself means failure.

Modern stdio servers do not send JSON-RPC requests to clients. Server needs for
additional input use an input-required result. Response and notification writes
share one channel and therefore require a single serialized writer.

The client cancels an in-flight stdio request with `notifications/cancelled` and the
request ID. After observing cancellation, a server should stop promptly and must not
send another message for that request. Closing standard input is the portable
graceful shutdown signal. Retonr must stop accepting work, cancel outstanding work,
and exit without a partial write or profile mutation.

### Streamable HTTP transport

Modern Streamable HTTP exposes one POST endpoint. Each JSON-RPC message is a new
POST with one request or notification body. Clients advertise both
`application/json` and `text/event-stream`; a request response is either one JSON
object or an SSE stream scoped to that request. Closing that SSE stream cancels the
request. A long-lived `subscriptions/listen` response stream carries opted-in change
notifications; it does not recreate a protocol session.

The modern transport has no standalone GET stream, session ID, sticky-session
requirement, `Last-Event-ID` resumption, or response redelivery. A broken response
stream loses the in-flight request. Any safe retry is a new request with a new
JSON-RPC ID and remains subject to Retonr's application-level idempotency contract.

Every request requires `MCP-Protocol-Version` and `Mcp-Method`. `Mcp-Name` is also
required for named calls such as `tools/call`. Header values must agree with the
body, including decoding the specified Base64 sentinel representation for values
that cannot be carried safely as plain ASCII. A mismatch returns HTTP 400 and
JSON-RPC `HeaderMismatch` code `-32020`. An unsupported version returns HTTP 400 and
`-32022`; an unknown RPC method returns HTTP 404 and `-32601`.

Servers must validate every present `Origin` and return HTTP 403 for an invalid
origin. Local servers should bind only to loopback and should authenticate every
connection. These protocol requirements do not make localhost trusted. Retonr's
stricter Host validation, bearer scope, no-store, resource-bound, redaction, and
disconnect-cancellation policies remain necessary.

The protocol permits `x-mcp-header` annotations that mirror selected tool arguments
into HTTP headers. Retonr's 1.0 tools have no routing need for this feature and should
not use it. Avoiding it prevents a second representation of user text and removes
header injection and intermediary disclosure risk.

### Tools and result behavior

Tool input and output use JSON Schema, with 2020-12 as the default dialect. Tools may
return structured content of any JSON type allowed by their declared output schema.
For backward compatibility, structured content should also be represented as a
serialized text content block. Retonr should use an object schema, one checked
structured outcome, and a matching bounded text representation.

`tools/list` and other cacheable list or read results require `ttlMs` and
`cacheScope`. Retonr tool discovery is deterministic. Profile-dependent or
authority-dependent results are private, and authenticated HTTP responses remain
`Cache-Control: no-store` under the stricter local API policy.

Tool annotations are untrusted unless they came from a trusted server. Retonr does
not use annotations as proof of user consent or authority. Malformed routing and
protocol violations use JSON-RPC errors. An actionable failure from a valid tool
invocation uses the tool-result error mechanism. Rewritten, unchanged, abstained,
and unsupported remain successful typed domain outcomes.

Roots, Sampling, Logging, Dynamic Client Registration, and HTTP+SSE are deprecated
in 2026-07-28. New Retonr code must not depend on them. Stdio diagnostics go to
standard error. Streamable HTTP authentication for Retonr 1.0 remains the explicitly
documented loopback bearer profile rather than an incomplete OAuth implementation.

## Official Rust SDK and conformance baseline

### Rust SDK

The official `rmcp` crate 3.1.2 is the current stable Rust SDK release reviewed here.
Its repository states support for stable MCP 2026-07-28 and compatibility with
2025-11-25 and earlier revisions. The release requires Rust 1.88 or later and uses
the 2024 edition, both within Retonr's current Rust 1.97.1 policy.

The implementation pin should use an exact crate version and Cargo lockfile, review
the crate checksum and advisories, disable unneeded default features, and enable only
the server and transport features required by the current work package. Stdio should
not pull in HTTP client, OAuth, task, elicitation, or legacy support without a
reviewed need. Streamable HTTP features are added only in its later work package.

An SDK is implementation help, not a protocol oracle. Retonr must retain wire-level
fixtures and run the official conformance suite. SDK upgrades require a wire-schema
diff, feature diff, advisory and license review, and the complete product and
protocol gates.

### Conformance suite

The stable npm `latest` tag for `@modelcontextprotocol/conformance` is 0.1.16 and
predates MCP 2026-07-28. It cannot be the release referee for this protocol revision.
The reviewed prerelease line is 0.2.0-alpha; 0.2.0-alpha.11 is current on 2026-08-12.
The frozen `requirements/2026-07-28.yaml` file states that the specification release
was anchored to 0.2.0-alpha.10.

Pin the exact available prerelease and lock its integrity. Run the frozen
requirements set rather than a floating `all`, `draft`, or version-filtered suite:

```console
npx @modelcontextprotocol/conformance@0.2.0-alpha.11 server \
  --url http://127.0.0.1:PORT/mcp \
  --requirements 2026-07-28
```

The official server command tests a Streamable HTTP URL. It does not replace
Retonr's process-level stdio conformance runner. The official requirements also mark
extension, added-after-release, and pending scenarios as not scored. Retonr should
run those for visibility where applicable, but it must not describe an optional or
pending case as required protocol conformance.

Expected-failure files make known failures visible; they do not turn a failure into
conformance. A Retonr release gate accepts no expected failure for a scored required
scenario. The harness performs JSON wire-schema checks, but Retonr still needs its
own malicious, boundedness, authority, cancellation, redaction, and cross-interface
fixtures.

## Corrections and clarifications for current planning

| Current planning statement | Correction or clarification |
| --- | --- |
| First-party integrations are packaged as stable `SKILL.md` skills. | A standalone skill remains useful, but the current portable bundle target is Agent Plugins 1.0.0: root `plugin.json`, `skills/`, and optional root `mcp.json`. |
| Agent Skills packaging and installation trust are one decision. | Separate format validity from distribution, signatures, installation, enablement, permissions, sandboxing, and updates. Agent Plugins defines none of those client policies. |
| Run the official MCP conformance suite. | Pin the 0.2.0 prerelease containing `requirements/2026-07-28.yaml`. The stable 0.1.16 npm line predates the target protocol. |
| Format conformance establishes broad agent support. | Client adoption is incremental. Prove exact client, version, operating system, component, and transport combinations from clean installs. |
| Package containment protects plugin execution. | Containment prevents package-path escape only. It does not sandbox the stdio subprocess or constrain runtime arguments. |
| A plugin version controls compatibility and updates. | `version` is optional metadata. Agent Plugins defines no resolver, lockfile, update source, signature, digest, or anti-rollback policy. |
| MCP 2026-07-28 has no initialization session and adds `server/discover`. | Confirmed. Keep this target, and also require `resultType`, per-request metadata, cache metadata, modern headers, and modern cancellation behavior. |
| Skills over MCP is in review and not a 1.0 dependency. | Confirmed. Agent Plugins and filesystem Agent Skills already cover the 1.0 packaging need. |

## Explicit integration invariants

### Agent Plugin package invariants

- `AP-01`: A released plugin is one inspected directory with exactly one root
  `plugin.json`; no alternate manifest can override it.
- `AP-02`: `plugin.json` and `mcp.json` select the same supported Agent Plugins
  schema version. Checked-in schema bytes and digests are release inputs.
- `AP-03`: Retonr emits no unknown manifest or MCP fields, even where a client would
  treat a violation as non-fatal.
- `AP-04`: Every package path remains under the resolved root on Windows, macOS, and
  Linux. Release packages contain no symlinks, junctions, reparse points, device
  files, sockets, or alternate data stream dependencies.
- `AP-05`: Each skill is an immediate child of `skills/`, has a regular `SKILL.md`,
  and has a frontmatter name identical to its directory.
- `AP-06`: The routine plugin contains only the routine rewrite skill and a local
  stdio MCP entry. It contains no privileged profile or administration authority.
- `AP-07`: The MCP command is one bare `retonr` executable token and arguments are a
  separate array. No shell, wrapper, command string, or user-controlled command is
  involved.
- `AP-08`: Plugin configuration contains no credential, bearer token, profile data,
  user content, absolute user path, remote endpoint, or secret-like header.
- `AP-09`: Plugin loading, validation, installation, and inspection execute nothing,
  start no server, download no model, and make no network request.
- `AP-10`: Skill instructions are descriptive only. A server-enforced scope decides
  authority regardless of prompt text, `allowed-tools`, metadata, or client UI.
- `AP-11`: A plugin update is installed only after signature, digest, schema,
  containment, semantic, exact CLI compatibility, and named-client gates pass.
- `AP-12`: Independent component failure is reported at the narrowest specified
  boundary and never silently activates an alternate transport or executable.

### MCP protocol invariants

- `MCP-01`: Every modern request contains the exact protocol version and client
  capabilities in `_meta`; `clientInfo` remains optional.
- `MCP-02`: `server/discover` is always available for a supported version and
  reports deterministic capabilities and supported versions.
- `MCP-03`: Every successful modern result includes a recognized `resultType`.
- `MCP-04`: No request relies on connection, process, or stream identity for
  principal, profile, conversation, learning, operation, or idempotency state.
- `MCP-05`: Stdio uses one newline-delimited JSON-RPC message per line. Standard
  output is protocol-only and all writes pass through one serialized writer.
- `MCP-06`: After stdio cancellation or HTTP response-stream closure, no further
  message for the cancelled request is emitted and no mutation can commit late.
- `MCP-07`: Modern HTTP accepts only the supported POST contract. It emits no modern
  session ID, standalone GET stream, DELETE lifecycle, or resumable event ID.
- `MCP-08`: HTTP protocol, method, name, Origin, Host, content type, authentication,
  and body limits are checked before application work.
- `MCP-09`: Retonr does not advertise a capability, extension, notification, list
  change, task, prompt, resource, or transport that lacks complete conformance
  fixtures.
- `MCP-10`: Tool schemas are closed and bounded. Schema validation runs before model
  work and output validation runs before any result is released.
- `MCP-11`: Domain outcomes and protocol errors remain distinct. Abstention is not a
  transport error, and malformed protocol input is not a successful tool outcome.
- `MCP-12`: Deprecated Roots, Sampling, Logging, Dynamic Client Registration, and
  HTTP+SSE are absent from the new implementation.

### Retonr product invariants across agent surfaces

- `INT-01`: CLI, MCP, skills, Agent Plugins, local API, and future desktop commands
  call the same application service and validation cascade.
- `INT-02`: The baseline agent surface accepts complete bounded plain text and
  supported Markdown values, never arbitrary local paths, clipboard authority, raw
  audio, or unrestricted document bytes.
- `INT-03`: An agent receives only a complete validated output. Candidate tokens,
  unvalidated fragments, prompts, traces, and protected profile evidence are never
  streamed.
- `INT-04`: Default agent authority is rewrite and check only. Profile reads,
  profile writes, learning, model lifecycle, and administration require separate,
  explicit server-enforced authority.
- `INT-05`: Starting or calling the routine plugin makes no external network request
  and cannot install, update, pull, remove, or activate a model.
- `INT-06`: Source and output content do not enter logs, standard error, crash
  reports, telemetry, skill files, plugin data, or client-visible traces by default.
- `INT-07`: For an equivalent owned request and authority, CLI and MCP agree on
  status, stable reason, output bytes, protected values, profile version, model and
  validator identities, digests, and redacted rewrite record.
- `INT-08`: An agent prompt cannot relax resource limits, validation, atomicity,
  privacy, offline mode, or authorization. Restrictive policy is deny-wins.
- `INT-09`: Unknown provider provenance and absent watermark information never make
  input unsupported and never alter candidate selection. Retonr optimizes verified
  rewriting under user rules, not detector results.

## CLI-first logical order of operations

1. Freeze and test the complete non-interactive CLI contract, including stdin,
   stdout, JSON, exit categories, cancellation, configuration precedence, offline
   enforcement, and cross-platform executable discovery.
2. Freeze one application request and outcome contract. Run the same deterministic
   fake-backed fixtures in process and through the CLI before adding another
   interface.
3. Define the minimal agent tool surface as `rewrite` and `check`. Use closed,
   bounded input and output schemas and no path, clipboard, model, or mutation
   authority.
4. Add MCP 2026-07-28 stdio with exact `rmcp` and feature pins. Pass wire fixtures,
   process lifecycle, cancellation, malformed input, backpressure, and shutdown
   tests on Windows, macOS, and Linux.
5. Write one routine Agent Skill that teaches an agent when to use `rewrite`, how to
   interpret all domain outcomes, and when to preserve the original. Validate it
   with the Agent Skills reference validator and adversarial activation fixtures.
6. Assemble the Agent Plugins 1.0.0 routine package. Validate both canonical schemas
   locally, then run semantic path, command, environment, failure-isolation, and
   no-execution-on-load fixtures.
7. Run the package from clean installations of a small named client matrix. Start
   with clients that exercise distinct host families and all three operating
   systems. Expand claims only after an exact version passes.
8. Implement the authenticated loopback local API and then MCP Streamable HTTP.
   Pass the official frozen 2026-07-28 server requirements plus Retonr's stricter
   local authority and cancellation suites.
9. Add a separately installable privileged profile plugin only after profile CLI
   operations and scopes are stable. Its installation and launch must be an explicit
   authority decision and must not broaden the routine plugin.
10. Evaluate Skills over MCP only after SEP-2640 reaches a stable published state,
    the official Rust SDK supports the extension, and a real Retonr use case is not
    already served by the filesystem Agent Plugin.

The native desktop application follows the excellent CLI and agent surfaces. It
does not become a prerequisite for agent integration, and no web application is
introduced by this sequence.

## Conformance and release gates

### Gate A: source and schema control

- Pin Agent Plugins normative text and both schemas by release identifier and
  digest.
- Pin the Agent Skills validator and record its supported specification revision.
- Pin `rmcp` exactly, select a minimal feature set, retain Cargo.lock, and record the
  crate checksum, license, minimum Rust version, and advisory result.
- Pin the official conformance package and npm integrity. Do not use a floating
  distribution tag in CI.

### Gate B: Agent Plugin package

- Validate positive and negative `plugin.json` and `mcp.json` fixtures against the
  official schemas.
- Add semantic fixtures for unsupported schema identifiers, mismatched versions,
  unknown fields, invalid names, immediate-child discovery, wrong filesystem kinds,
  escaping symlinks, Windows junctions and reparse points, command tokenization,
  working-directory containment, reserved environment variables, placeholder
  expansion, URL policy, duplicate-case headers, and isolated entry failure.
- Inspect the release artifact and prove it contains no secret, profile, model,
  user content, executable wrapper, or unexpected file.
- Prove package validation and inspection execute no code and perform no network
  access.

### Gate C: Agent Skill

- Pass the Agent Skills reference validator.
- Test explicit invocation, correct model-selected activation, non-activation on
  unrelated tasks, abstention handling, unsupported input, operational failure, and
  hostile document instructions.
- Prove the skill cannot expand server scopes or bypass a confirmation.
- Keep the skill usable when clients qualify the MCP tool name differently.

### Gate D: MCP stdio

- Pass product-owned JSON-RPC wire and process fixtures on all supported operating
  systems.
- Cover clean startup, discovery, list, call, structured results, line framing,
  Unicode, oversized frames, nesting, duplicate in-flight IDs, one-writer ordering,
  cancellation, broken pipes, standard-input closure, forced shutdown, crash and
  restart, queue saturation, and no late result after cancellation.
- Prove standard output contains no non-protocol byte and standard error follows the
  content-redaction policy.

### Gate E: MCP Streamable HTTP

- Run every scored scenario in the frozen official 2026-07-28 requirement set with
  no expected failures.
- Run current non-scored applicable scenarios for visibility and retain their
  results without calling them required conformance.
- Cover protocol and method headers, encoded names, Origin and Host attacks,
  authentication and scopes, loopback-only binding, content types, JSON and SSE
  responses, slow consumers, disconnect cancellation, cache policy, redirects,
  and accidental session or legacy endpoint behavior.
- Run the MCP Inspector against the exact release build, but do not substitute an
  interactive Inspector check for automated conformance.

### Gate F: cross-interface equality

- Run the same deterministic corpus through in-process service, CLI, MCP stdio,
  Streamable HTTP, skill-directed tool calls, and Agent Plugin launches.
- Compare every product field defined by `INT-07` and normalize only documented
  transport metadata.
- Require byte-identical original return on document-atomic abstention and every
  supported preservation failure.

### Gate G: named client compatibility

- Start each test from a clean client installation with no inherited Retonr plugin
  or MCP configuration.
- Record exact client version, operating system, architecture, plugin component
  support, transport, install method, executable lookup, tool naming, consent UI,
  cancellation behavior, and result presentation.
- Run successful rewrite, unchanged, abstained, unsupported, cancellation,
  malformed input, server-not-found, model-unavailable, and upgrade-skew cases.
- Make the published compatibility matrix no broader than retained evidence.

### Gate H: release and revalidation

- Formatting, linting, tests, coverage, policy checks, supply-chain checks, and all
  platform CI jobs pass with warnings treated as errors.
- Signed CLI and plugin artifacts derive from the same reviewed revision and publish
  independent digests.
- Re-run affected gates when an Agent Plugins schema, Agent Skills specification,
  MCP revision, SDK patch, conformance package, named client, tool schema, command,
  authority scope, or distribution mechanism changes.

## Principal risks and responses

### Working Draft churn

Agent Plugins 1.0.0 is published but still labeled Working Draft. A schema or
normative behavior can change before implementation starts. Pin reviewed inputs,
isolate the package adapter, and revalidate before freezing a Retonr contract.

### False confidence from schema validation

The schemas do not establish path containment, URL origin policy, environment
semantics, signatures, trust, safe execution, or runtime compatibility. Pair schema
validation with semantic fixtures and named-client execution evidence.

### Executable and plugin version skew

A client can load a new skill while finding an older `retonr` on its process search
path. Require a machine-readable CLI capability query, fail closed on an unsupported
wire or tool schema, and report the resolved executable and version without leaking
its full path by default.

### Ambient environment leakage

Agent clients choose the stdio base environment and may inherit secrets. Retonr
should ignore unrelated environment variables, prevent credentials from reaching
logs or records, and never require ambient cloud credentials for the routine local
plugin.

### Permission ambiguity

Agent Plugins does not standardize permission prompts. A skill description or
client approval cannot authorize profile mutation. Keep default tools non-mutating
outside their returned value, split privileged packages, and enforce scopes inside
Retonr.

### Cross-client presentation drift

Clients can expose tool names, skill activation, output, errors, and confirmation
differently. Keep the skill free of client-specific syntax, publish only tested
combinations, and retain client-specific adapters outside the portable core.

### Local HTTP exposure

Loopback does not prevent hostile local processes or browser DNS rebinding. Delay
HTTP until the authenticated service contract is ready, validate Origin and Host,
use high-entropy bearer scopes, bind only to an actual loopback address, and cancel
on disconnect.

### Premature extension adoption

Tasks and Skills over MCP are optional extensions. Adding them early increases wire,
state, and compatibility surface without improving the baseline local rewrite. Keep
them out of 1.0 until a stable specification and measured product need exist.

## Primary sources

### Agent Plugins

- [Agent Plugins overview](https://agent-plugins.org/)
- [Agent Plugins 1.0.0 specification](https://agent-plugins.org/specification)
- [Plugin author guide](https://agent-plugins.org/plugin-authors)
- [Plugin manifest reference](https://agent-plugins.org/plugin-authors/manifest)
- [Skills packaging reference](https://agent-plugins.org/plugin-authors/skills)
- [MCP server packaging reference](https://agent-plugins.org/plugin-authors/mcp-servers)
- [Client loading and discovery](https://agent-plugins.org/client-implementers/loading-and-discovery)
- [Client MCP runtime](https://agent-plugins.org/client-implementers/mcp-runtime)
- [Client conformance checklist](https://agent-plugins.org/client-implementers/conformance)
- [Canonical Agent Plugins schemas](https://agent-plugins.org/schemas)
- [Agent Plugins specification repository](https://github.com/agentplugins/agent-plugins-spec)
- [Agent Plugins compatible clients](https://agent-plugins.org/compatible-clients)

### Agent Skills and skills distribution

- [Agent Skills specification](https://agentskills.io/specification)
- [Agent Skills reference validator](https://github.com/agentskills/agentskills/tree/main/skills-ref)
- [Skills over MCP working group](https://modelcontextprotocol.io/community/working-groups/skills-over-mcp)
- [SEP-2640 review](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2640)
- [Experimental Skills over MCP repository](https://github.com/modelcontextprotocol/experimental-ext-skills)

### MCP

- [MCP 2026-07-28 specification](https://modelcontextprotocol.io/specification/2026-07-28)
- [MCP 2026-07-28 key changes](https://modelcontextprotocol.io/specification/2026-07-28/changelog)
- [Versioning and compatibility](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
- [Server discovery](https://modelcontextprotocol.io/specification/2026-07-28/server/discover)
- [Stdio transport](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio)
- [Streamable HTTP transport](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
- [MCP tools](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)
- [MCP authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)
- [MCP deprecated features](https://modelcontextprotocol.io/specification/2026-07-28/deprecated)
- [MCP security best practices](https://modelcontextprotocol.io/docs/2026-07-28/tutorials/security/security_best_practices)

### Rust SDK and conformance

- [Official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
- [`rmcp` 3.1.2 release](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.1.2)
- [Official MCP conformance suite](https://github.com/modelcontextprotocol/conformance)
- [Frozen MCP 2026-07-28 requirements](https://github.com/modelcontextprotocol/conformance/blob/main/requirements/2026-07-28.yaml)
- [MCP conformance npm package](https://www.npmjs.com/package/@modelcontextprotocol/conformance)
