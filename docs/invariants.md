# Product and engineering invariants

## Purpose

These invariants are the permanent boundaries of Retonr. Milestones may narrow a
feature, move it later, or reject an implementation. They may not waive an
invariant to preserve a release number.

Each architecture decision, work package, pull request, and release claim must name
the affected invariants. When two invariants appear to conflict, the safer behavior
is to preserve the original input, deny authority, avoid network access, or leave a
feature unsupported until the conflict is resolved.

## Product invariants

### INV-P01: The user controls the final expression

Retonr is an editorial tool. The user decides whether to rewrite, accept, edit,
export, or discard a result. A provider, model, detector, profile, or policy signal
cannot silently override that decision inside the product.

The product may inspect supported source-form signals, characteristic model phrasing,
invisible artifacts, and document metadata. Those inputs gain no editorial authority
over the derivative. Retonr does not claim to erase provider records, prove human
authorship, defeat every classifier, or satisfy an external disclosure obligation.

### INV-P02: Fidelity dominates style

A candidate that fails a literal, invariant, structure, format, or calibrated
semantic gate is ineligible. Style gain, detector movement, transformation size,
latency, or user-interface convenience cannot compensate for that failure.

Candidate selection is lexicographic. Fidelity gates run before style ranking. A
new strategy, model, runtime, format, or interface must use the same shared
validation cascade.

### INV-P03: Unsupported means unsupported

Capabilities are explicit and versioned. Unsupported or ambiguous content is
protected or causes a typed abstention. Retonr does not silently reinterpret a
structured document as plain text, weaken a preservation claim, or return a
best-effort mutation under a success label.

### INV-P04: Personalization is authorized and reversible

Profile evidence comes only from material the user owns or is authorized to use.
Evidence has provenance, consent state, eligibility, contribution bounds, and a
deletion path. Generated candidates and accepted output do not become evidence
without a separate explicit admission decision.

Every active profile is inspectable, versioned, exportable, restorable, and
deletable from application-controlled storage. The product does not ship presets
that imitate public figures or unrelated third parties.

### INV-P05: The core is viewpoint-neutral

Retonr does not decide which lawful ideas a user may express. It does not inspect
content for a regulator, provider, employer, platform, or remote policy service. It
does not add mandatory attribution, provenance claims, provider branding, hidden
source markers, or content telemetry.

The product is not a legal or compliance oracle. Users and deployers decide which
disclosures and rules apply to their work and remain responsible for those choices.
The project separately reviews legal duties that apply to its own development,
distribution, and operation. A user-responsibility statement cannot waive those
duties.

### INV-P06: Provenance changes are explicit

Retonr inspects supported credentials, signatures, metadata, and invisible controls
before normalization or model execution. It preserves the source and unknown format
state by default. It does not silently invalidate, strip, forge, or present a stale
binding as valid.

An edit that changes a recognized binding uses an explicit qualified derivative
workflow or abstains. Sanitation is a separate, narrowly authorized operation for a
documented security, privacy, interoperability, accessibility, or repair purpose.
Statistical watermark and source-classifier results never guide live generation,
retry, ranking, or acceptance.

A statistical source signal alone is not preservation-critical document state and
receives no special protection. This does not weaken the explicit handling required
for supported credentials, signatures, structural bindings, or unknown format state.

## Execution and data invariants

### INV-E01: Core work is local and offline after setup

Parsing, planning, profile access, validation, reassembly, and ordinary diagnostics
complete without network access. Generation through a selected local runtime may use
only its exact configured local transport. After explicit artifact installation or
offline import, every participating process operates with non-loopback outbound
connections denied. A local HTTP runtime may use only its exact configured
IP-literal loopback endpoint.

Downloads, updates, remote runtimes, and remote APIs are separate opt-in actions.
Opening a file or receiving a rewrite request never triggers a download or enables
network access.

### INV-E02: Runtime and provider choice belongs to the user

The core engine depends on small inference and embedding ports, not a vendor SDK or
mutable model name. Local sidecars, user-managed local services, and explicitly
configured API-compatible backends are adapters behind those ports.

No adapter receives exclusive product authority. No runtime may silently switch
provider, artifact, quantization, context, prompt template, execution backend,
language policy, privacy mode, or validation threshold.

Structured-output admission is an exact schema-digest capability, not a Boolean or a
transport-shape inference. Adapter-wide role and schema admission never establishes
that every inventoried artifact is qualified for their cross-product. The application
must join the exact artifact, role, runtime, qualification, and active binding before
use. A raw structured completion is untrusted input to a domain strategy and has no
semantic or rewrite authority by itself.

### INV-E03: Support binds to exact identity and evidence

A support claim identifies the model or embedding artifact, runtime build,
parameters, language, mode, format, operating system, architecture, execution
class, and hardware envelope that passed qualification. Mutable tags and family
names are discovery hints, not identities.

Artifact or runtime drift invalidates the active binding and discards the complete
candidate batch. Qualification records never imply formal semantic guarantees.
The claim-extraction role requires versioned runtime-build, artifact-set, effective-state,
and effective-package evidence identities whose truth and completeness are independently
established. Qualification schema v1 is observational and cannot authorize that role.
The domain can represent all four identities and recheck their structural relationships,
but their validity grants no authority. The separate qualification v2 record binds all
four identities for exactly claim extraction, has no authorization operation, and cannot
enter qualification v1 activation. The application must independently attest and recheck
the live tuple before and after use.

An effective-package record covers every artifact-set path exactly once with a bounded,
canonical purpose set. It binds completeness, acquisition, license review,
transformation, runtime load-closure, and exclusion evidence. Missing, extra, duplicated,
reordered, stale, cross-product, observed-only, or mode-incompatible evidence fails
closed. A digest is an equality binding to retained evidence, not proof that evidence is
truthful or complete.

Probabilistic claim extraction and deterministic comparison are separate evidence
stages. Retained comparison evidence binds to the exact extractor manifest, unit,
source and candidate text identities, and evidence-set identities. Incomplete, empty
for nontrivial source text, mismatched, or unresolved extraction cannot silently become
a clean semantic pass. Content digests are identity evidence, not anonymization.

### INV-E04: Inputs are immutable until verified commit

The source is never overwritten by default. Planning and generation cannot mutate
source bytes, stored evidence, or an active profile. Reassembly occurs only after a
complete candidate passes the common gates and adapter verification.

Cancellation, timeout, process failure, malformed output, resource exhaustion,
drift, or verification failure leaves the original intact. Recoverable explicit
in-place operations require a separately tested commit protocol.

### INV-E05: Format owners define preservation

Plain text owns newline and final-newline preservation. Markdown owns eligible byte
ranges, escaping, reparse, structural fingerprints, and untouched-byte identity.
DOCX owns package, relationship, XML, part, and formatting checks for its declared
subset. Clipboard, API strings, and agent tool strings carry no rich-format claim.

Every fixed fidelity or structure defect adds a minimized regression fixture before
the corresponding capability is restored.

### INV-E06: Bounds exist at every trust boundary

Files, archives, XML, JSON, schemas, prompts, model output, queues, retries,
concurrency, processing time, memory growth, logs, and retained records have explicit
bounds. Unknown fields, paths, redirects, symlinks, origins, hosts, credentials, and
executable content fail according to a deterministic policy.

### INV-E07: Large inputs use hierarchical bounded transactions

A long document or folder is never treated as one unconstrained prompt. Retonr first
builds a model-free inventory and format-owned unit map, then may produce a bounded
document plan and style context. Generation proposes edits only for explicit units
through bounded context packets. Unit, region, document, and batch verification run
before any output is committed.

High-level plans and summaries are untrusted guidance. They cannot add claims, expand
eligible spans, weaken protected content, or replace source text as the fidelity
reference. Context-window size advertised by a runtime does not establish long-input
quality.

Fidelity compares a candidate with the source and declared constraints. It is not a
general fact-checking service. A source assertion, name, date, or exact quantity is
preserved unless an explicit typed policy authorizes a bounded transformation. An
approximation policy may permit `50,143.65` to become `about 50,000`; without that
authority, changing it to `51,000` fails closed. A separate external-evidence service
would need its own sources, trust model, and qualification before making any claim
about truth.

A file or folder rewrite writes to a separate destination by default. It uses source
digests, a staged manifest, collision rules, atomic file commits where supported,
and a complete report. An interrupted batch leaves sources intact and identifies
which independent outputs, if any, were committed.

### INV-E08: Generation provenance is redacted evidence, not authority

When a generation call completes, its rewrite record identifies the strategy,
runtime, exact artifact, prompt template, serialized backend input, output schema,
candidate count, and bounded usage observations that were available. It never stores
raw source, output, candidates, protected values, profile samples, prompts, or model
reasoning by default.

Provenance cannot accept a candidate, weaken a fidelity gate, establish authorship
or ownership, prove semantic correctness, or create legal or policy authority.
Unknown or absent provenance remains visible as missing evidence, not silently
inferred state.

## Interface invariants

### INV-I01: The CLI is the reference product surface

Every core workflow is first completed and qualified through a documented,
scriptable CLI on Windows, macOS, and Linux. Human output and machine output are
separate contracts. Non-interactive commands do not prompt, progress does not
pollute data streams, and exit categories remain stable after the 1.0 freeze.

Desktop and agent integrations consume the same application service. They do not
contain independent rewrite, validation, profile, model-selection, or format logic.

### INV-I02: Agent integrations are thin, portable, and least privileged

The stable CLI machine contract comes first, MCP over standard input comes next,
and portable Agent Plugin packaging follows after both pass conformance. Streamable
HTTP and the first-party loopback API are added only for use cases that cannot use a
local subprocess.

Agent Skills, Agent Plugins, MCP servers, and client-specific extensions contain no
duplicate product logic or embedded credentials. Routine rewriting, profile reads,
profile mutation, learning, and administration use separate authority. Package and
protocol versions are pinned and revalidated before a support claim.

### INV-I03: The desktop application is native and downstream

The desktop application begins only after the CLI and agent contracts are proven.
It is a local installed application using a native Rust UI stack. It does not embed
a browser engine, render a hosted web application, require a local web server for
ordinary operation, or make a web application a product dependency.

The chosen UI stack must pass an architecture decision covering accessibility,
keyboard behavior, international text, visual testing, platform integration,
packaging, maintenance, licensing, and failure isolation on Windows, macOS, and
Linux.

### INV-I04: Loopback is an authority boundary

Local HTTP is disabled unless explicitly started. A loopback caller is not trusted
merely because it is local. Binding, authentication, scopes, Host and Origin checks,
resource limits, cancellation, and redaction are enforced before application work.
No 1.0 service binds beyond loopback.

## Engineering invariants

### INV-Q01: Cross-platform behavior is designed, not ported

Windows, macOS, and Linux are first-class implementation targets from the first
work package that affects them. Path, newline, terminal, process, signal,
permission, packaging, accessibility, and hardware behavior receive platform
fixtures before a support claim.

### INV-Q02: Quality claims require retained evidence

Formatting, strict linting, repository policy, tests, documentation, dependency and
license checks, and relevant property, fuzz, mutation, package, and platform checks
must pass for the revision being qualified. Warnings are errors.

Implemented Rust maintains at least 80 percent line coverage overall. Validation,
edit application, persistence migrations, authorization, deletion, artifact
activation, and format verification use higher targets and decision-table fixtures.
A configured continuous-integration job is not reported as passing until that exact
revision has a successful retained run.

### INV-Q03: The repository remains reviewable

Interfaces are small and explicit. Domain types encode states that must not be
confused. Unsafe and native integration code is isolated from the core. Oversized
catch-all modules, hidden generated artifacts, unexplained allow attributes, stale
comments, placeholder claims, and duplicated policy logic are release blockers.

Documentation distinguishes implemented, experimental, qualified, and planned
behavior. Generated-by or co-author attribution, emojis, and en or em dash
characters are prohibited in tracked repository content.

### INV-Q04: Main stays releasable

The repository converges continuously on one clean, passing main branch. Feature and
repair branches are focused, reviewed, and short-lived. Parallel branches do not
become competing product lines, long-running integration queues, or substitutes for
merging complete work.

Each completed milestone, including 0.2, 0.3, 0.4, and later phases, produces a
clean versioned release from a passing main revision. A milestone release includes
exact support claims, migrations, checksums, known limitations, and retained CI
evidence. Unfinished work remains disabled, experimental, or on a focused branch.

### INV-Q05: Stable current tools with a minimal dependency graph

Shipping code uses current stable or generally available language, protocol, library,
runtime, and packaging releases after review and qualification. Preview, beta,
release-candidate, nightly, alpha, and working-draft inputs are isolated to research
or an explicitly accepted compatibility boundary and never float silently into a
release.

Every dependency must own a necessary capability that is materially safer, more
portable, or more maintainable than a small local implementation. Transitive size,
unsafe code, build scripts, procedural macros, native libraries, licenses,
maintainer health, advisories, platform cost, and removal path are reviewed.
Unused, duplicate, superseded, and diagnostic-only dependencies do not remain in
shipping graphs.

`Latest` means the newest reviewed stable version that passes the complete relevant
matrix. It does not mean a floating version, automatic major upgrade, unreviewed
protocol revision, or automatic runtime or model update.

## Change control

An invariant can change only through an architecture decision that includes:

1. The concrete user problem that cannot be solved within the current boundary.
2. Primary-source evidence and tested alternatives.
3. Security, privacy, fidelity, compatibility, accessibility, and migration impact.
4. A rollback or data-recovery path.
5. Updated adversarial fixtures and release gates.
6. Explicit maintainer acceptance that the product contract itself is changing.

Ordinary feature work, dependency upgrades, schedule pressure, ecosystem trends,
and model-provider behavior are not sufficient reasons to bypass this process.
