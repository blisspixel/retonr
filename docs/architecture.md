# Architecture

## Status

This architecture defines boundaries for a benchmarked vertical prototype. It is not
an interface freeze. Public APIs, stored schemas, and adapter contracts freeze only
after they have survived evaluation, cross-platform tests, and at least one real
consumer at each boundary.

## Quality attributes

The design is ordered by these priorities:

1. Fidelity risk control
2. User ownership and privacy
3. Explicit failure and abstention
4. Structural and format preservation
5. Cross-platform correctness
6. Inspectability and stable automation
7. Personal style quality
8. Latency and resource efficiency

Style quality cannot compensate for a fidelity failure.

## System context

```text
                           +----------------------+
Authorized writing ------>| Profile compilation  |
Declared preferences ---->| and evidence store   |
                           +----------+-----------+
                                      |
                                      v
User or integration ---> Application service <--- Local model runtime
        |                             |
        |                             v
        |                    Rewrite transaction
        |                             |
        v                             v
 CLI / Desktop / API / MCP      Document adapters
                                      |
                                      v
                            Output and rewrite record
```

Every entry point calls the same application service. CLI, desktop, HTTP, MCP, and
agent skill packages do not reimplement profile, rewrite, validation, or persistence
logic.

## Dependency direction

Dependencies point inward toward domain types and policies.

```text
retonr-cli     rewrite-api       rewrite-mcp       rewrite-desktop
    \             |              |                /
     +------------+--------------+---------------+
                              |
                         rewrite-app
                              |
       +----------------------+----------------------+
       |                      |                      |
rewrite-engine       rewrite-grounded        rewrite-profile
       |                      |                      |
       |             rewrite-inference       rewrite-store
       |                      |                      |
       +----------------------+----------------------+
                              |
                        rewrite-types

Infrastructure implementations:
  rewrite-ollama
  rewrite-text-adapter
  rewrite-markdown-adapter
  rewrite-docx-adapter
  later: rewrite-inference-llamacpp

Inward-facing contract layers:
  rewrite-model -> rewrite-types
  rewrite-inference -> rewrite-model, rewrite-types

Infrastructure inference adapters depend on rewrite-inference. Contract layers never
depend on Ollama, HTTP, a model store, or a platform runtime.

Development-only consumers:
  rewrite-eval
  fuzz targets
  compatibility suites
```

The exact crate split may be consolidated during the first slice if a boundary has
no independent behavior. The dependency rules remain even if two modules initially
share a crate.

Generation strategy, runtime backend, installed model artifact, and qualified
artifact-runtime combination are separate identities. A strategy ID cannot stand in
for mutable backend, artifact, prompt, tokenizer, or parameter provenance.

## Rewrite transaction

```text
1. Probe and parse input
2. Identify eligible rewrite units and protected fragments
3. Load an immutable profile version
4. Derive risk features, typed invariants, and document obligations
5. Build an immutable transformation plan
6. Select a candidate generation strategy
7. Retrieve provenance-backed style evidence
8. Generate complete candidates
9. Run the common validation cascade
10. Select among eligible candidates lexicographically
11. Apply only accepted edits through the owning adapter
12. Reparse and verify the completed document
13. Commit the result atomically or return the original
14. Emit a versioned rewrite record
```

The selected generation strategy can increase validation requirements. It cannot
remove a shared validation step.

## Generation strategies

The first generative milestone implements only `Grounded`. The current CLI still
checks caller-supplied candidates, while the application layer has a provisional
grounded strategy exercised through a fake backend and bounded Ollama adapter. It is
not a qualified user-facing model path yet. Other strategies are introduced only
after the evaluation harness demonstrates a need.

| Strategy | Contract | Intended use |
| --- | --- | --- |
| `Literal` | Deterministic edits only, no generative model | Exact punctuation and declared mechanical rules |
| `Constrained` | Local sentence or paragraph rewrite with protected sentinels | Low-risk short prose |
| `Grounded` | Typed invariants, claims, context, candidates, and full validation | Default generative path |
| `Render` | Whole-unit reconstruction with the strictest validation | Experimental strong-mode work |

Routing inputs include entity and quantity density, negation, modality, conditions,
comparatives, citations, identifiers, coreference, tables, and cross-unit references.
Uncertain routing selects the stricter strategy.

A strategy emits a neutral bounded inference request. A backend returns an untrusted
response plus generation provenance. The strategy parses complete masked candidates,
and the engine alone restores protected values, validates candidates, and applies an
accepted edit. Production grounded requests do not expose protected raw surfaces to
the backend.

## Document representation

The semantic document and adapter reconstruction state are separate. Core code can
refer to opaque source anchors but cannot inspect format-specific data.

```rust
struct DocumentIr {
    schema_version: u32,
    document_id: DocumentId,
    media_type: MediaType,
    source_digest: Digest,
    roots: Vec<NodeId>,
    nodes: BTreeMap<NodeId, Node>,
    rewrite_units: Vec<RewriteUnit>,
    capabilities: AdapterCapabilities,
}

struct Node {
    id: NodeId,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    role: StructuralRole,
    policy: RewritePolicy,
    source_anchor: SourceAnchorRef,
}

struct RewriteUnit {
    id: RewriteUnitId,
    node_id: NodeId,
    text: String,
    fragments: Vec<SourceFragment>,
    protected_spans: Vec<ProtectedSpan>,
    context: RewriteContext,
}

struct ParsedDocument {
    ir: DocumentIr,
    adapter_id: AdapterId,
    adapter_schema_version: u32,
    adapter_state: Vec<u8>,
    diagnostics: Vec<AdapterDiagnostic>,
}
```

Source ranges are half-open UTF-8 byte ranges after explicit decoding. Adapters also
retain enough state to preserve a byte order mark, original newline style, final
newline state, and format-specific fragments. Non-UTF-8 paths remain `OsString` or
`PathBuf`; they are never forced through a lossy string conversion.

IDs identify an ingest instance. A source anchor identifies the original location.
Neither is derived only from mutable prose.

Every adapter declares a capability matrix. Unsupported features are not silently
treated as supported.

## Adapter contract

```rust
trait DocumentAdapter {
    fn probe(&self, input: &[u8]) -> ProbeResult;

    fn parse(
        &self,
        input: &[u8],
        policy: &ParsePolicy,
    ) -> Result<ParsedDocument, AdapterError>;

    fn apply(
        &self,
        parsed: &ParsedDocument,
        edits: &[AcceptedEdit],
    ) -> Result<Vec<u8>, AdapterError>;

    fn verify(
        &self,
        before: &ParsedDocument,
        output: &[u8],
        edits: &[AcceptedEdit],
    ) -> VerificationReport;
}
```

The application never assembles Markdown or OOXML directly. Only the owning adapter
can apply and verify an edit.

## Claims and invariants

A full unrestricted Meaning IR is deferred. Early versions use typed evidence that
is honest about extraction confidence.

```rust
struct Invariant {
    kind: InvariantKind,
    canonical_value: String,
    original_surface: String,
    evidence: Vec<TextSpan>,
    enforcement: Enforcement,
}

struct Claim {
    subject: Argument,
    predicate: Predicate,
    object: Option<Argument>,
    polarity: Polarity,
    modality: Option<Modality>,
    temporal: Vec<TemporalQualifier>,
    quantities: Vec<QuantityRef>,
    conditions: Vec<Condition>,
    attribution: Option<Attribution>,
    evidence: Vec<TextSpan>,
    confidence: f32,
}
```

The same learned extractor cannot certify its own result. Exact gates, independent
extraction, clause alignment, bidirectional entailment, contradiction checks, and
document-level checks provide complementary evidence.

## Protected sentinel flow

URLs, paths, identifiers, quantities, dates, protected terms, and code-like spans are
replaced with typed sentinels before generation when the mapping is unambiguous.

```text
Payment is due on <DATE_1> for <MONEY_1>.
```

A candidate must contain each required sentinel exactly once in an allowed location.
The original surface value is restored, then the entire validation cascade runs.
Restoring a value is not semantic repair. If relationships changed or mapping is
ambiguous, the candidate fails.

## Validation cascade

1. Output schema and encoding validity
2. Sentinel count, type, and integrity
3. Protected terms and typed literals
4. Structural fingerprint
5. Novel entity and novel quantity rejection
6. Claim and relationship comparison
7. Sentence or clause alignment
8. Bidirectional entailment and contradiction assessment
9. Cross-sentence and cross-unit obligations
10. Declared constraints
11. Style, channel, and fluency scoring

Negation and modality are not described as purely deterministic checks. Surface
markers can contribute evidence, but scope and implied meaning require learned
assessment and conservative uncertainty handling.

```rust
struct GateResult {
    gate_id: String,
    gate_version: String,
    status: GateStatus,
    severity: Severity,
    evidence: Vec<GateEvidence>,
    confidence: Option<f32>,
}

enum GateStatus {
    Pass,
    Fail,
    Uncertain,
    NotApplicable,
}
```

Strict policies treat `Uncertain` as ineligible. Semantic evaluators are versioned
independently from generators and calibrated on project-specific data.

## Candidate selection

Selection is lexicographic:

1. Reject a hard failure.
2. Reject disallowed uncertainty.
3. Require the calibrated semantic floor.
4. Require declared constraints.
5. Rank eligible candidates by personal style, channel fit, and fluency.
6. Use edit cost and a mode-specific divergence band as tie breakers.

No weighted score may allow a style improvement to offset lost meaning. Surface
divergence is not maximized and never enters the live system as a provenance-evasion
objective.

## Abstention and atomicity

```rust
enum RewriteStatus {
    Rewritten,
    UnchangedNoEligibleContent,
    Abstained,
    Failed,
}

enum Atomicity {
    Document,
    Unit,
    Region,
}
```

Document atomicity is the default for connected prose. If any required unit fails,
the original document is returned byte-for-byte. Unit and region modes are explicit
because partial rewriting can create inconsistent voice or references.

Unit atomicity is allowed only when the adapter and planner establish that a unit is
independent of failed neighbors. Region atomicity groups units connected by
coreference, list structure, table relationships, or discourse dependencies. Both
partial modes run cross-reference, reassembly, and completed-document validation
after edits are applied. Uncertain independence promotes the decision to document
abstention.

Abstention includes stable reason codes, failed gate IDs, affected source anchors,
and retry count. Retry applies only to retryable generation failures. Parser
ambiguity, unsupported format features, or invalid constraints do not trigger a
model retry.

## Profile architecture

Profiles are immutable versions compiled from explicit evidence.

```rust
enum EvidenceUseState {
    Active,
    RetrievalIneligible,
    InfluenceExcluded,
    ConsentRevoked,
    Deleted,
}

struct EvidenceRecord {
    id: EvidenceId,
    source_kind: EvidenceSource,
    source_digest: Digest,
    owner_confirmed: bool,
    channel: Option<Channel>,
    created_at: Option<Timestamp>,
    derived_from: Option<RewriteRecordId>,
    authorization: AuthorizationRecordId,
    use_state: EvidenceUseState,
    text_ref: Option<CorpusTextRef>,
    byte_len: u64,
}

struct DerivedStyleArtifact {
    id: DerivedArtifactId,
    source_evidence_ids: Vec<EvidenceId>,
    producer_identity: ProducerIdentity,
    embedding_space_id: Option<EmbeddingSpaceId>,
    payload_ref: DerivedPayloadRef,
    output_digest: Digest,
}
```

Evidence records contain intrinsic source facts and lifecycle state. Extracted style
features, token counts, vectors, topic assignments, and reliability estimates are
immutable derived artifacts with complete producer provenance. Influence exclusion,
consent revocation, and deletion invalidate the complete transitive derivation
closure. Retrieval ineligibility invalidates retrieval indexes and snapshots without
silently erasing separately approved aggregate observations.

Effective contribution combines ownership confidence, reliability, channel
relevance, recency, sample quality, diversity, and per-source caps. Fixed universal
weights are not frozen before empirical calibration.

Rules:

- Raw generated candidates never become evidence.
- Accepting output updates a preference signal, not the writing corpus.
- User-edited output is derivative and requires explicit confirmation before limited
  use.
- One file, session, or topic cannot dominate the profile.
- Sparse channel profiles shrink toward the general profile.
- Conflicting declared rules fail profile compilation with actionable diagnostics.
- Fidelity and document obligations outrank every style preference.

Retrieval prioritizes channel, communicative act, register, length, and style. Pure
topic similarity is avoided because it can copy names, facts, and phrases from the
corpus. Retrieved evidence is capped per source, traceable, and checked for leakage.

## Storage

SQLite is the initial local store for profiles, evidence metadata, migrations, model
manifests, installed-artifact state, append-only qualification and invalidation
records, activation decisions, active role pointers, and rewrite metadata. Artifact
activation is one transaction that verifies an installed digest and currently valid
qualification, appends the decision, and changes the active pointer. Startup and
recovery revalidate the complete binding before a runtime may use it. Corpus text and
sensitive trace content need an encryption design that works on desktop and headless
systems. That design must pass a dedicated cross-platform spike before its interface
freezes.

Small personal corpora use filtered brute-force vector scoring in Rust with vectors
stored as versioned blobs. SQLite FTS5 supports lexical retrieval. A vector extension
is not a required dependency until scale benchmarks justify it.

## Trace model

A rewrite record includes:

- Trace schema version
- Source and output content identifiers
- Adapter and schema versions
- Immutable profile version
- Model artifact digest, quantization, backend, and backend version
- Installed model identity and qualification-record identity
- Strategy, tokenizer, prompt template, parameters, output schema, and seed
- Planner and validator versions
- Retrieved evidence IDs
- Candidate lineage and gate results
- Final decision and abstention reason
- Timings and resource measurements
- Binary and configuration digests
- Locale and timezone when parsing depends on them

Default traces exclude raw input, output, candidates, profile samples, prompts, and
model reasoning. Short plain hashes are vulnerable to guessing, so local equality
tracking should use an installation-keyed identifier or remain opt-in.

Replay is best effort unless model artifact, backend, quantization, prompt, runtime,
and hardware behavior are all controlled.

## Markdown architecture

Markdown support declares an exact dialect. The first subset rewrites non-overlapping
plain text in paragraphs and headings.

The adapter:

1. Uses parser source offsets to identify eligible spans.
2. Protects code, raw HTML, destinations, autolinks, and unsupported constructs.
3. Applies edits from the highest byte offset to the lowest.
4. Escapes output for its exact inline context.
5. Requires byte identity outside approved spans.
6. Reparses the completed output.
7. Compares a structural fingerprint.
8. Abstains on overlap, parse changes, or unsupported syntax.

Nested event ranges are not treated as a lossless mutable syntax tree.

## DOCX architecture

DOCX support is package-preserving within a declared subset, not universally
lossless. The first subset is unencrypted `.docx`, main document paragraphs and
table cells, homogeneous run formatting, and no ambiguous active features.

The adapter:

- Copies untouched package parts without semantic modification.
- Changes only approved WordprocessingML text nodes.
- Preserves unknown parts, relationships, namespaces, and content types.
- Protects hyperlink targets.
- Rejects macros, encryption, signed documents, fields, tracked changes, content
  controls, drawings, equations, and embedded objects in an eligible unit.
- Limits entry count, expanded size, compression ratio, XML depth, and processing
  time.
- Disables DTD and external entity resolution.
- Never follows external relationships.
- Validates XML, hashes untouched parts, and opens compatibility fixtures in office
  software during qualification.

Run-level formatting alignment is ambiguous when word order changes. The first
subset therefore requires homogeneous formatting within a rewrite unit.

## Application ports

```rust
type PortFuture<'a, T> =
    Pin<Box<dyn Future<Output = T> + Send + 'a>>;

trait Generator: Send + Sync {
    fn capabilities(&self) -> GeneratorCapabilities;

    fn generate<'a>(
        &'a self,
        request: GenerationRequest,
        operation: &'a OperationContext,
    ) -> PortFuture<'a, Result<Vec<GeneratedCandidate>, GenerationError>>;
}

trait Embedder: Send + Sync {
    fn capabilities(&self) -> EmbeddingCapabilities;

    fn embed<'a>(
        &'a self,
        request: EmbeddingRequest,
        operation: &'a OperationContext,
    ) -> PortFuture<'a, Result<EmbeddingBatch, EmbeddingError>>;
}

trait SemanticEvaluator: Send + Sync {
    fn evaluate<'a>(
        &'a self,
        source: &'a str,
        candidate: &'a str,
        context: &'a EvaluationContext,
        operation: &'a OperationContext,
    ) -> PortFuture<'a, Result<SemanticReport, EvaluationError>>;
}
```

Generation, embeddings, retrieval, semantic evaluation, and validation remain
separate responsibilities. These ports are object-safe for runtime-selected local
backends, own request payloads across await points, require `Send` futures, and carry
cancellation and deadlines through `OperationContext`. A future static generic or
closed-enum implementation must preserve the same ownership and cancellation
contract.

## Services and integrations

The first-party local API is the automation boundary. During `0.x`, `/v0` is explicitly
preview and can change with release notes and migrations. The 0.9 compatibility gate
promotes the reviewed contract to `/v1` for 1.0. It has versioned wire schemas,
capability discovery, successful domain outcomes, RFC 9457 transport errors,
cancellation, conditional mutation semantics, explicit authority, and loopback-only
binding for 1.0.

MCP implements standard input first, then POST-only Streamable HTTP. MCP 2026-07-28
has no initialize exchange or protocol session. Requests carry required protocol
version and client capabilities plus optional `clientInfo` in request metadata, and
the server implements `server/discover`. Multi-step learning uses explicit opaque
handles that are scoped, expiring, revocable, principal-bound, and tamper-resistant.
MCP handlers reconstruct state from explicit identifiers rather than hidden protocol
sessions. Older protocol support is limited to named clients with conformance
fixtures.

Streamable HTTP uses a documented custom loopback bearer profile rather than standard
MCP OAuth authorization. Its server-side scopes, challenge, rotation, and revocation
behavior are explicit, and standard input remains preferred when a named client
cannot inject the token.

Agent skill packages use the stable `SKILL.md` format and contain instructions,
schemas, examples, and explanatory authority requirements only. They call MCP or the
current first-party API and never contain a second rewrite implementation. Server
authorization remains authoritative, and experimental `allowed-tools` metadata is
not trusted for security. Skills over MCP remains an isolated experiment until its
working-group proposal is stable and qualified.

The 1.0 compatibility adapter is an offline local post-processor. A caller submits a
completed, supported response payload and receives the same supported shape after
rewriting. The adapter makes no upstream request and stores no upstream credentials.
It does not rewrite tool calls, structured JSON, reasoning, refusals, citations, or
other event types. It uses byte-range JSON string splicing, verifies non-target bytes,
and returns local status and provenance through an explicit sidecar or envelope. A
true outbound reverse proxy or remote generation backend is a separate post-1.0
feature with its own network and credential design.

Desktop presentation has no independent product authority. Static Tauri capabilities
expose only the minimum command surface for a labeled window. Sensitive commands also
require an opaque, expiring application grant bound to the exact window session,
resource, operation, and action. Route state is never an authorization input.

Long-running desktop work uses a two-step delivery contract. Rust creates a suspended
window-owned operation and returns its initial snapshot. The frontend subscribes with
a per-invocation Tauri channel; Rust validates ownership, acknowledges the installed
channel, and only then starts work. Targeted events have contiguous monotonic sequence
numbers. A gap requires an authoritative snapshot query. Window close or reload
revokes ownership and cancels desktop-owned work, while durable staging is resumed
only through a new explicit command. Global broadcasts never carry privileged state.

## Cross-platform constraints

- Paths remain `PathBuf` and `OsString` through core and process boundaries.
- Application directories come from platform APIs rather than hard-coded paths.
- Source line endings and final newline state are preserved.
- Output is written to a temporary file in the destination directory, flushed, and
  replaced through a platform-tested atomic operation.
- Source overwrite is opt-in and includes a recoverable backup policy.
- Processes use argv arrays, never constructed shell commands.
- Cancellation reaches HTTP, database, model, and child-process work.
- Windows locking, long paths, reserved names, and case folding have dedicated tests.
- macOS universal packaging and signing are tested on macOS.
- Linux system dependencies and package formats are tested on supported runners.

## Architecture decisions still open

- Final product and crate name
- Encryption and key-recovery design for desktop and headless use
- Exact semantic evaluator ensemble and calibration policy
- Model manifest, artifact drift, and qualification invalidation policy
- Initial supported Markdown extension set
- DOCX validation and office-compatibility tooling
- Desktop frontend framework and accessible component system
- Tauri command, capability, updater, and operation-state contracts
- Local speech runtime and distributable model licenses
- Exact API compatibility subset
- Local API authentication, MCP custom bearer profile, and compatibility-adapter
  status side channel

Each decision receives a checked-in decision record before implementation depends
on it.

## References

- [CommonMark 0.31.2](https://spec.commonmark.org/0.31.2/)
- [pulldown-cmark source offsets](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.Parser.html)
- [ECMA-376 OOXML](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- [WordprocessingML document structure](https://learn.microsoft.com/en-us/office/open-xml/word/structure-of-a-wordprocessingml-document)
- [MCP 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28)
- [Agent Skills specification](https://agentskills.io/specification)
- [Style transfer evaluation](https://aclanthology.org/N19-1049/)
- [Negation and sentence representations](https://aclanthology.org/2022.blackboxnlp-1.20/)
- [Factual consistency methods](https://aclanthology.org/2022.naacl-main.287/)
- [AlignScore](https://aclanthology.org/2023.acl-long.634/)
- [StyleDistance](https://aclanthology.org/2025.naacl-long.436/)
