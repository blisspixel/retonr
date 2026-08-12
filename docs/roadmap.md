# Versioned roadmap

## Roadmap rules

This roadmap is ordered by dependency and risk. It deliberately contains no calendar
or duration estimates.

The [phase execution plans](planning/README.md) expand milestones 0.2 through 1.0
into decisions, work packages, tests, qualification evidence, and handoff gates.

A milestone begins only when its entry criteria are met and closes only when every
exit gate passes. Features can move later if their evidence is weak. A version number
does not excuse an incomplete quality gate.

Private evaluation work may continue under a codename with namespace-neutral
internal identifiers. Public upload, package publication, and release remain blocked
until the naming, licensing, and applicable clearance gates are complete.

## Version policy

- `0.x` versions can change internal and external contracts while evidence is being
  gathered.
- Breaking changes require migrations and release notes once users have stored data
  or automated a surface.
- `1.0` freezes the supported profile, rewrite-record, CLI JSON, first-party API, and
  MCP compatibility contracts under semantic versioning.
- Experimental format features, models, and compatibility modes are labeled and do
  not silently become stable.
- Every release identifies exact supported platforms, models, formats, and protocol
  revisions.

## 0.0: Product and engineering foundation

### Entry criteria

- High-level concept documented
- Local workspace available
- Repository standards accepted

### Deliverables

- Candid product definition and competitive baseline
- Deferred-name policy and a contained future rename boundary
- Architecture, design, technology, evaluation, security, and quality documents
- Initial threat model
- Supported-platform policy
- Repository policy checks for prohibited content and oversized modules
- Rust toolchain pin and workspace conventions
- Windows, macOS, and Linux continuous-integration skeleton
- Decision-record template
- License policy and planned model manifest schema

### Exit gate

- Product thesis and exclusions are internally consistent.
- The semantic claim is fidelity-gated rather than universal.
- Internal prototype code uses namespace-neutral identifiers where practical.
- The public project identity is recorded in ADR 0006. Formal legal review and a
  fresh namespace check remain hard gates before package reservation or release.
- Documentation and repository policy checks pass.
- Public source publication uses the selected `Retonr` identity.

## 0.1: Evaluation and deterministic contracts

### Entry criteria

- 0.0 exit gate passes
- Authorized data collection and deletion policy approved
- Initial user-research protocol approved

### Deliverables

- Rust workspace with inward dependency rules
- Versioned core types for documents, rewrite units, plans, candidates, gate results,
  statuses, errors, and rewrite records
- Plain-text adapter and byte-preservation contract
- Mock generator and mock semantic evaluator
- Deterministic orchestration with cancellation
- Exact literal, protected-term, sentinel, and structure gates
- Lexicographic candidate selection
- Document-atomic abstention that returns the original byte-for-byte
- Evaluation harness with positive paraphrases and semantic hard negatives
- Baseline runners for no rewrite, direct prompt, style description, and retrieved
  examples
- Property tests, golden fixtures, initial fuzz targets, and coverage reporting

### Exit gate

- No model is required to test the full transaction.
- Every failure and abstention path has a stable status and reason.
- Deterministic gate regression suites contain zero known critical failures.
- Document-atomic abstention is byte-identical across Windows, macOS, and Linux.
- Overall line coverage is at least 80 percent.
- Validation and edit-application code meet the higher critical-path target.
- All four refinement passes complete.

## 0.2: Grounded plain-text engine and CLI vertical slice

### Entry criteria

- 0.1 exit gate passes
- Initial model artifacts and licenses reviewed
- Qualification hardware tiers declared

### Deliverables

- Versioned smoke, development, calibration, locked, and red-team benchmark
  governance
- Neutral strategy, backend, installed-artifact, and qualification contracts
- Pure inference fake and fake HTTP backend conformance suite
- Native Ollama version, inventory, details, capability, and artifact-drift probing
- Loopback-only model access with proxies and redirects disabled
- Pinned artifact identification and model manifest
- Headless model list, inspect, explicit download, offline import, verify, qualify,
  activate, deactivate, and remove lifecycle
- Local hardware and runtime probe plus deterministic `model recommend` and bounded
  `model eval --suite device` workflows
- One `Grounded` generation strategy
- Typed invariant and claim extraction
- Protected sentinel generation and restoration
- Independent semantic-evaluator port
- Full shared validation cascade
- Plain-text CLI with stdin, file input, text output, JSON output, diff, dry-run, trace,
  cancellation, and `--fail-on-abstain`
- Multiline standard input read to end of file without trimming, with exact newline
  and final-newline fixture coverage
- Safe interactive rendering for ANSI, OSC, C0, C1, carriage-return, hyperlink,
  clipboard, and bidi-control fixtures
- Exact raw output limited to non-terminal streams, files, or the double opt-in
  `--raw-terminal --yes`
- Stable diagnostic categories and provisional machine schema
- Local-only mode and automated no-network test
- Qualification report for 4B, 9B, and larger candidate tiers where available
- Predeclared quantization non-inferiority evidence against Q8 or a higher-precision
  reference for every lower-precision artifact called qualified
- Real CLI screenshots for rewrite, abstention, and trace inspection

### Exit gate

- The CLI works from clean installations on Windows, macOS, and Linux.
- Structured output and exit categories pass process-level compatibility tests.
- Model failures never bypass validation or corrupt the original.
- Selective risk, coverage, style, latency, and memory are reported together.
- Predeclared fidelity, coverage, and resource thresholds pass for every artifact
  called qualified.
- A qualified model tier fits at least one documented laptop class.
- CLI screenshots come from passing release builds.
- Overall and critical-path coverage gates pass.
- All four refinement passes complete.

## 0.3: Style profile and excellent CLI alpha

### Entry criteria

- 0.2 exit gate passes
- Product-validation participants and held-out data are available

### Deliverables

- SQLite migrations and immutable profile versions
- Authorized evidence ingestion and provenance
- Interpretable style features with confidence and sample counts
- Declared rules, enforcement levels, and conflict detection
- Channel overlays with sparse-data fallback
- FTS5 plus filtered brute-force embedding retrieval
- Exact embedding-space qualification with truncation disabled, artifact-drift
  checks, task-specific retrieval baselines, and reindex invalidation
- Per-source, per-session, and topic-diversity caps
- Cross-corpus leakage checks
- Typed interview and scenario acquisition
- Explicit preference feedback without automatic corpus contamination
- Profile inspect, edit, compare, export, import, restore, and delete commands
- Shell completion, manual pages, stable non-interactive behavior, and diagnostics
- Benchmark against the strongest simple profile baseline

### Exit gate

- Blind held-out evaluation shows meaningful owner preference over the strongest
  simple baseline without a material fidelity regression.
- Predeclared topic-confounding, cross-topic retrieval, topic-held-out preference,
  rare-phrase copying, canary, and cross-profile extraction thresholds pass with
  sufficient evidence.
- Profile export, import, migration, rollback, and deletion pass cross-platform tests.
- Generated candidates never become evidence.
- The CLI completes all non-voice profile workflows without desktop code.
- All four refinement passes complete.

## 0.4: Markdown beta

### Entry criteria

- 0.3 exit gate passes
- Markdown dialect and first supported feature set approved

### Deliverables

- Source-splice adapter using explicit UTF-8 byte ranges
- Initial support for plain inline prose in paragraphs and headings
- Preservation of code, raw HTML, links, destinations, autolinks, and unsupported
  constructs
- Reverse-order edit application
- Context-aware escaping
- Output reparse and structural fingerprint comparison
- Byte-identity check outside approved spans
- CommonMark, extension, malformed-input, LF, CRLF, Unicode, and final-newline fixtures
- Property, differential, and fuzz tests
- Capability reporting and actionable abstention for unsupported syntax
- Feature-by-feature expansion only after fixture gates pass

### Exit gate

- Supported fixtures preserve structure and non-target bytes exactly.
- No supported test introduces executable Markdown or changes a protected target.
- Unsupported syntax never receives a false preservation claim.
- Markdown behavior passes on all three operating systems.
- All four refinement passes complete.

## 0.5: Calibration, strategies, and quality hardening

### Entry criteria

- 0.4 exit gate passes
- Locked evaluation set and adjudication policy approved

### Deliverables

- Calibrated semantic evaluator ensemble
- `Literal` deterministic strategy
- `Constrained` generative strategy
- Experimental `Render` strategy behind an explicit flag
- Risk-based router that can only increase strictness
- Candidate diversity and retry policy
- Mode-specific edit-cost and divergence bands
- Unit and region atomicity with consistency, cross-reference, reassembly, and
  partial-result fixtures
- Human adjudication workflow
- Confidence intervals by risk category
- Scheduled fuzz and mutation testing
- Performance and memory benchmarks on declared hardware tiers
- Pinned llama.cpp sidecar with independently qualified CPU, Metal, CUDA, HIP,
  Vulkan, and hybrid execution classes where advertised
- Controlled artifact conversion and quantization evidence plus Q4 non-inferiority
  comparison against Q8 or a higher-precision reference
- Cross-runtime and cross-backend differential suites
- Independent multilingual and mixed-language calibration strata for the proposed
  1.0 language matrix
- Cancellation and resource-limit stress tests
- Profile privacy and encryption decision completed

### Exit gate

- False acceptance and transformation coverage meet the thresholds set before the
  locked evaluation run.
- No strategy bypasses the shared cascade.
- Adding a strategy improves coverage or style without weakening the risk bound.
- At least one exact controlled artifact passes both Ollama and the pinned llama.cpp
  sidecar qualification, and every execution class advertised at this milestone has
  independent passing evidence.
- Cross-runtime and cross-backend differential suites pass or narrow the support
  matrix explicitly.
- Mutation testing demonstrates meaningful assertions in critical logic.
- Encryption claims match implemented and tested behavior.
- All four refinement passes complete.

## 0.6: Preview local API, MCP, and agent skills

### Entry criteria

- 0.5 exit gate passes
- Application service and stored schemas are stable enough for external consumers

### Deliverables

- First-party preview `/v0` loopback API
- Shared versioned wire DTOs and conformance fixtures across every interface
- Capability discovery, RFC 9457 transport errors, domain outcome envelopes,
  deadlines, cancellation, mutation operation IDs, and resource limits
- Principal-scoped long-operation creation, authenticated polling, and cancellation
  with no unvalidated candidate or output streaming
- Separate rewrite, profile-read, profile-write, and administration authority
- MCP over standard input
- MCP over Streamable HTTP
- MCP 2026-07-28 metadata and `server/discover` lifecycle without an initialize
  exchange or protocol session
- Applicable official MCP conformance with the documented custom-authorization
  exclusion, plus a named-client compatibility matrix
- Explicit, scoped, expiring learning handles
- Thin Agent Skills `SKILL.md` packages using MCP or `/v0`
- Skills over MCP experimentation isolated from the 1.0 compatibility gate
- Shared conformance fixtures for CLI, API, MCP, and skills
- Narrow non-streaming text-only compatibility adapter that accepts completed
  response payloads and makes no outbound request
- Exact compatibility outcome mapping: no payload for malformed or oversized input,
  original bytes for unsupported or abstained valid input, and verification failure
  mapped to a stable abstention reason
- Loopback-only service binding, authentication, Host and Origin checks, and redaction
- API and MCP documentation with complete local examples

### Exit gate

- All interfaces produce equivalent decisions and rewrite records for shared fixtures.
- Skills contain no duplicated engine logic.
- Supported MCP clients pass smoke and compatibility tests.
- Every compatibility outcome returns exactly the documented payload and status.
- No 1.0 service binds beyond loopback.
- Security and resource-limit tests pass.
- All four refinement passes complete.

## 0.7: Bounded DOCX support

### Entry criteria

- 0.6 exit gate passes
- DOCX subset and preservation definition approved
- ZIP and XML security limits approved

### Deliverables

- OPC package reader and bounded transform
- Initial support for unencrypted `.docx`, main-story paragraphs, table cells, and
  homogeneous run formatting
- Protection of hyperlink targets and non-text package content
- Explicit rejection of macros, encryption, signatures, fields, tracked changes,
  content controls, drawings, equations, and embedded objects in eligible units
- Untouched-part hash verification
- Relationship, content-type, and XML verification
- Office compatibility fixture suite
- ZIP bomb, XML depth, entity, external relationship, and resource-limit tests
- Incremental capability matrix for later WordprocessingML features

### Exit gate

- Every advertised feature passes structure, package, reopen, and fidelity fixtures.
- Untouched parts remain semantically unmodified and match their required hashes.
- Unsupported documents fail safely without partial corruption.
- The source is never overwritten by default.
- Cross-platform file and office-compatibility tests pass.
- All four refinement passes complete.

## 0.8: Desktop beta

### Entry criteria

- 0.7 exit gate passes
- First-party application service and profile contracts are stable
- Desktop UX and accessible component spike passes

### Deliverables

- Tauri desktop shell on Windows, macOS, and Linux
- Onboarding and explicit local or network state
- Model manager with license, digest, size, hardware, import, qualification, and removal
- Rewrite workbench with accessible side-by-side and linear diffs
- User-initiated multiline plain-text clipboard paste and copy with workbench-only
  least-privileged capabilities and no rich-format preservation claim
- Rewritten, unchanged, abstained, unsupported, and failed states
- Profile lab with evidence, rules, confidence, channels, conflicts, versions, and
  deletion
- Opt-in history and privacy-preserving diagnostics
- Secure Tauri capabilities and content security policy
- Explicit custom-command manifest, least-privileged per-window capabilities, and
  negative authorization tests
- Operation IDs and monotonic event sequences for cancellation and stale-event safety
- Full keyboard support and WCAG 2.2 AA implementation
- Native menus, dialogs, shortcuts, and platform packaging
- Signed test builds and update-path spike
- Real README screenshots for workbench and profile lab

### Exit gate

- Core workflows pass automated and manual accessibility review.
- Functional and visual tests pass on WebView2, WKWebView, and WebKitGTK.
- The desktop contains no independent rewrite or profile implementation.
- Generated or imported content is never rendered as untrusted HTML.
- Install, upgrade, migration, platform-specific rollback or recovery, and removal
  tests pass on supported systems.
- Screenshots come from passing release builds and have useful alt text.
- All four refinement passes complete.

## 0.9: Local voice, compatibility freeze, and release candidate

### Entry criteria

- 0.8 exit gate passes
- Speech runtime and model-license decisions approved
- Profile interview state machine stable in typed mode

### Deliverables

- Cross-platform microphone discovery and push-to-talk capture
- Local speech-to-text with visible model, language, size, license, and checksum
- Editable transcript before profile ingestion
- Local spoken prompts and responses with captions
- Immediate removal from application-controlled raw-audio storage by default
- Explicit audio-retention controls
- Complete typed fallback
- Voice permission, denial, cancellation, device-loss, and resource tests
- Complete CLI voice interview acceptance suite on Windows, macOS, and Linux
- Complete desktop voice interview acceptance suite on Windows, macOS, and Linux
- Shared typed and voice evidence-schema conformance fixtures
- No always-listening, wake-word, voice-cloning, or unconfirmed transcript admission
- Signed CLI and desktop release candidates
- Offline model import for generation, embeddings, speech recognition, and speech
  output
- Promotion of the preview `/v0` API to `/v1`
- Frozen 1.0 profile, rewrite-record, CLI JSON, `/v1` API, and MCP schemas
- Upgrade, downgrade, migration, recovery, and uninstall rehearsals
- Public security policy and disclosure path
- Complete user, administrator, API, MCP, and agent-skill documentation
- Final real screenshots, recordings where useful, and accessibility descriptions

### Exit gate

- Voice onboarding works locally on Windows, macOS, and Linux and produces the same
  approved evidence schema as typed input.
- No audio is persisted in application-controlled storage without explicit consent,
  and operating-system limitations are documented.
- The product remains fully usable without microphone or speech models.
- All frozen contracts pass backward-compatibility suites.
- Release candidates pass clean-install and offline-after-install tests.
- Overall coverage is at least 80 percent and critical-path thresholds pass.
- No open critical security, fidelity, data-loss, accessibility, license, or packaging
  defect remains.
- All four refinement passes complete.

## 1.0: Complete cross-platform product

Version 1.0 means the product works exceptionally well as a complete system, not that
the first prototype runs.

### Required capabilities

- Polished, documented, scriptable CLI
- Polished, accessible Tauri desktop application
- Local profiles with inspectable evidence, declared rules, channels, versioning,
  export, import, restore, and deletion
- Typed and local voice-assisted style interviews
- Qualified local generation and embedding models
- Hardware-aware model recommendation and user-runnable comparison without silent
  runtime, model, quantization, context, language, or execution-class downgrade
- Qualified rewriting for English, at least one additional Latin-script language,
  and at least one non-Latin-script language, plus declared mixed-language behavior
- Fidelity-gated TXT and supported Markdown rewriting
- Bounded, explicitly qualified DOCX rewriting
- Deterministic validation, calibrated semantic assessment, lexicographic ranking,
  and honest abstention
- Stable rewrite records and privacy-preserving diagnostics
- Stable first-party local API
- MCP standard input and Streamable HTTP
- MCP 2026-07-28 request metadata and discovery behavior plus explicitly qualified
  named-client compatibility
- Tested agent skill packages
- Text-only compatibility adapter conformant to its published pinned subset
- Signed Windows x86-64 and Arm64 CLI distributions, signed and notarized macOS
  distributions with qualified aarch64 and x86_64 slices, and verifiable signed
  Linux x86-64 and Arm64 artifacts with a declared glibc floor
- One-line PowerShell and POSIX shell bootstrap installers that fail closed while
  verifying every downloaded payload, plus an end-to-end verified inspect-first path,
  exact-version, no-admin, interrupted-update, rollback, and uninstall behavior
- Offline operation after explicit model installation or offline import
- Real CLI and desktop screenshots from passing builds

### Release gates

- Locked evaluation meets the published fidelity, coverage, style, and resource
  thresholds.
- All advertised models, runtimes, execution classes, languages, mixed-language
  patterns, formats, and hardware classes have exact qualification reports.
- Automatic language detection meets predeclared per-language misrouting and
  abstention bounds, and every advertised mixed-language set either passes locked
  qualification or returns the exact original without translation or boundary drift.
- CLI, desktop, API, MCP, and skills agree on shared conformance fixtures.
- Every supported platform passes clean install, update, migration, platform-specific
  rollback or recovery, cancellation, and removal testing.
- WCAG 2.2 AA target passes automated and manual review.
- Overall Rust line coverage is at least 80 percent, with higher critical-path
  thresholds.
- Formatting, strict linting, documentation, dependency, vulnerability, license,
  property, fuzz, mutation, and package tests pass.
- Threat model, legal review, provenance behavior, privacy documentation, security
  policy, model manifests, and release attestations are complete.
- Compatibility provenance includes original and rewritten digests, target schema,
  eligible paths, adapter and validator versions, rewrite status, and labeling that
  retained upstream IDs, usage, and fingerprints describe the original response.
- No god files or unreviewed size exceptions exist.
- All four refinement passes complete on the release candidate.
- Name and public namespaces have formal clearance.

## Explicitly after 1.0

- Broader WordprocessingML features
- PDF extraction and rewrite without perfect round trip
- Additional languages beyond the minimum qualified 1.0 set
- Browser extension
- Mobile applications
- Team profile collaboration and synchronization
- Optional remote backends
- Broader upstream API event compatibility
- Outbound provider gateway and semantic response streaming
- Custom per-user model training if baseline evaluation proves it worthwhile
- Voice dictation outside the style-interview workflow
- Voice cloning
