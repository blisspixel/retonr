# Product definition

## Verdict

The idea makes sense as a local-first, fidelity-gated re-expression engine for
machine-generated and rough drafts. Its primary job is to reconstruct eligible prose
in the user's own voice while protecting meaning, structure, and declared constraints.
Upstream wording and supported artifacts are inputs to inspect, not permanent claims
on the finished draft. The strongest product is not a general style imitator or
detector bypass. It is a conservative control plane that combines personal style
evidence, explicit output hygiene, fact checks, structural checks, format-aware
reassembly, and abstention.

The product treats generated, delegated, and rough text as drafts. Its role is
comparable to applying ordinary editorial review, a house style, authorized notes,
or the user's own prior examples at software scale. The user remains the final
editor. Retonr reports what it changed but does not issue an authorship or legal
compliance certificate.

That is a meaningful product if it can prove three things:

1. Accepted output contains fewer named editorial defects than the source and the
   strongest transparent baseline.
2. Users prefer its accepted output over a simple prompt with a few writing samples.
3. It achieves those improvements without a material increase in semantic or
   structural failures.

The project should stop or simplify if it cannot pass all three tests.

## Initial users

The initial user is a privacy-sensitive professional who regularly edits rough or
generated drafts and cares about personal voice, accuracy, and local control.

Strong initial groups include:

- Developers and technical writers
- Founders and independent consultants
- Researchers and educators working outside high-stakes assessment workflows
- Support, documentation, and community leads
- Writers with sensitive unpublished material

Legal, medical, financial, compliance, and other high-stakes professional uses are
not initial target claims. They require separate domain evaluation and stronger
human-review controls.

## Jobs to be done

- Turn a machine-generated draft into a locally reconstructed expression of my own
  writing preferences.
- Take editorial control of eligible wording and supported mutable artifacts instead
  of treating an upstream tool's choices as untouchable.
- Rewrite a rough draft so it sounds recognizably like me.
- Keep names, claims, quantities, dates, links, paths, code, and formatting intact.
- Apply my explicit preferences consistently across chat, email, documentation, and
  long-form writing.
- Keep my writing corpus and drafts off third-party services.
- Show what changed and why a candidate passed or failed.
- Find and reduce explainable editorial anti-patterns without pretending they prove
  AI authorship.
- Let me provide the audience, main point, desired action, and non-negotiables without
  line-editing a long draft.
- Ask only the few document-specific questions that can materially improve the
  rewrite or prevent a wrong editorial assumption.
- Return my original when a safe rewrite cannot be established.
- Reuse one profile from the CLI, desktop app, scripts, editors, and local agents.
- Talk through my preferences locally when typing an interview is inconvenient.

## Positioning

Public description:

> A local-first re-expression engine that turns machine-generated and rough drafts
> into your voice while protecting facts, structure, and user-defined rules. It
> makes bounded, inspectable editorial changes and returns the original when it
> cannot validate a rewrite.

Developer description:

> A deterministic control plane around local text generation.

`Your voice, compiled` remains a useful technical metaphor only if profiles are
inspectable, versioned, reproducible from evidence, and more capable than an opaque
prompt.

## Competitive reality

Voice and brand-style features already exist in Grammarly, custom model styles,
Jasper Brand Voice, ToneClone, Apple Writing Tools, and other products. Noren is a
particularly close comparison because it combines an editable voice profile,
writing measurements, local-first positioning, Rust, and Tauri. Idiolect already
offers guided profile creation, scoring, an API, and MCP. Vale and Harper also
demonstrate demand for explicit or local writing tools.

The durable differentiation must come from the combination of:

- Operation offline after model installation
- Provider-neutral local reconstruction instead of forwarding upstream output
- Explicit inspection and handling of supported source-form signals and metadata
- User-owned, provenance-backed profile evidence
- Explicit rules and conflict detection
- Deterministic literal and structure gates
- Calibrated semantic risk evaluation
- Abstention instead of best-effort output
- Format-aware Markdown and bounded DOCX handling
- Auditable local interfaces and rewrite records
- High-quality evaluation data and failure fixtures

Local inference and a style prompt are not sufficient differentiation by themselves.

## Product principles

### Loss aversion before style gain

An elegant rewrite with a changed claim is a failed rewrite. Candidate selection may
never trade fidelity for style through a blended score.

### User agency

The user can inspect evidence, edit rules, exclude samples, undo learning, export a
profile, and remove profile data from application-controlled storage. Deletion
reports explain external backups, operating-system caches, crash dumps, swap, and
storage-device limits. A generated output does not become new evidence merely
because it was accepted.

### Editorial sovereignty and viewpoint neutrality

The user's work and final expression remain under the user's control. Core operation
does not add mandatory provider attribution, generated-by labels, hidden source
markers, content telemetry, or remote content-policy enforcement. The engine does
not decide which lawful ideas a user may express.

Retonr is not a compliance oracle. Users and deployers remain responsible for rules
that apply to their work, while the project remains responsible for duties that
apply to its own development and distribution. The complete boundary is defined in
[Editorial sovereignty and legal responsibility](governance/editorial-sovereignty.md).

### Honest uncertainty

The system distinguishes exact validation from learned assessment. It reports
abstention and uncertainty with stable reason codes.

### Explainable anti-slop quality

Retonr identifies concrete editorial problems such as canned transitions, repeated
conclusions, conversational residue, inflated sectioning, vague attribution, and
punctuation habits that conflict with the selected profile. These findings can guide
generation and rank candidates only after every fidelity gate passes.

This is editorial lint, not an AI-authorship detector. Provider classifiers and
published watermark tests remain isolated research diagnostics. The complete
boundary and reporting contract are defined in
[Editorial lint and the anti-slop quality loop](editorial-lint.md).

### Narrow support is better than false preservation

Markdown and DOCX features graduate individually. Unsupported or ambiguous content
is protected or causes abstention.

### Local by default

Core operation uses local storage and local inference. Model installation, updates,
and explicitly selected remote backends are separate networked actions.

### Authorized personal style only

Profiles are built from the user's own writing or material the user is authorized
to use. The project does not ship third-party or public-figure imitation presets.

## Source form and provenance

Reducing retained upstream source form is a primary product motivation. It is also
a bounded technical property, not a promise of untraceability.

The product distinguishes four signal locations:

| Location | Product action | Claim boundary |
| --- | --- | --- |
| Token-selection and statistical watermark signals in the source wording | Generate new wording with a qualified local model and report controlled source-form diagnostics in research | No universal watermark-removal guarantee |
| Characteristic model phrasing and source classification signals | Move eligible prose toward an evidence-backed personal profile | A lower classifier score does not prove human authorship |
| Invisible Unicode, clipboard residue, and supported document metadata | Inspect, report, reject, preserve, or remove through an explicit format policy | Legitimate language and accessibility data must not be stripped blindly |
| Upstream prompts, outputs, account records, and service logs | Keep the reconstruction path local and avoid creating new remote copies by default | The product cannot inspect or delete data retained by another service |

A substantial rewrite naturally replaces upstream token choices. Source-form
diagnostics remain isolated from candidate ranking so the live engine never trades
meaning or style fidelity for a detector result.

Known provenance data should be detected where practical, represented in the rewrite
record, and handled explicitly. Rewriting can invalidate an existing content binding.
The system must not silently claim that a derived document retains the original
credential.

Legal requirements vary by role, mode, jurisdiction, and degree of transformation.
The product needs a mode-specific legal review before public release. It must not
assume that every rewrite qualifies as standard editing.

## Product validation gate

Before building the broad product surface:

1. Recruit users with authorized multi-channel writing corpora.
2. Create held-out prompts and owner-written reference responses.
3. Compare retrieved-example prompting, explicit style descriptions, compiled
   profiles, and compact adapters where hardware permits.
4. Measure blind owner preference, literal preservation, semantic false acceptance,
   structure preservation, transformation coverage, abstention, latency, and memory.
5. Observe profile onboarding, correction, and deletion workflows.
6. Continue with the compiled architecture only if it produces meaningful style
   gain over the best simple baseline without a material fidelity regression.

## 1.0 product boundary

Version 1.0 includes a polished CLI and native desktop application, TXT and declared
Markdown support, a bounded DOCX subset, non-destructive file and folder
transactions, local profiles, typed interviews, MCP, Agent Skills, a portable Agent
Plugin package, a stable local API, a documented text-only compatibility adapter,
cross-platform installers, published model and hardware recommendations, and
qualified rewriting for English, at least one additional Latin-script language, and
at least one non-Latin-script language.

Language and format support are independent claims. Mixed-language or unsupported
units are preserved or cause abstention according to the selected atomicity policy.
Formatted file preservation belongs to the owning document adapter. Plain-text
clipboard and inline API content do not carry a rich-format preservation claim.

Version 1.0 does not promise:

- Formal semantic equivalence for unrestricted language
- Perfect DOCX round-tripping for unsupported features
- PDF round-trip editing
- Spreadsheet rewriting, including XLSX prose cells
- Mobile applications
- Cloud synchronization or team profile management
- Unrestricted imitation of third parties
- Universal provenance or watermark removal
- Support for every upstream API event type
- Local voice acquisition, dictation, or speech output

## Primary product risks

| Risk | Why it matters | Early response |
| --- | --- | --- |
| Simple baselines perform just as well | The compiler adds complexity without value | Benchmark before interface freeze |
| Semantic validator false accepts | Incorrect text may look polished and trustworthy | Selective-risk metrics, hard negatives, abstention |
| Style score learns topic | Retrieval may copy facts or mismeasure voice | Topic-held-out evaluation and leakage gates |
| Local model requirements are too high | The product excludes typical laptops | Qualify model tiers and publish resource needs |
| Profile onboarding is burdensome | Users never reach a useful result | Progressive evidence collection and editable profiles |
| DOCX scope expands without bounds | Format work consumes the project | Capability matrix and unsupported-feature abstention |
| Impersonation and abuse | Personal style can be misused | Authorized-corpus policy, no presets, threat model, rate controls |
| Name conflict | Search and trademark confusion block packaged releases | Complete formal review and refresh namespace checks before distribution |

## Research references

- [Grammarly voice features](https://support.grammarly.com/hc/en-us/articles/23153676821773-Introducing-voice-features)
- [Jasper Brand Voice](https://help.jasper.ai/hc/en-us/articles/18618693085339-Brand-Voice)
- [Noren product overview](https://usenoren.ai/product)
- [Idiolect](https://idiolect.app/answers)
- [ToneClone](https://toneclone.ai/android/)
- [Apple Writing Tools](https://support.apple.com/en-us/121582)
- [Vale](https://docs.vale.sh/)
- [Harper](https://github.com/automattic/harper)
- [Personalized suggestions and authorship](https://arxiv.org/abs/2601.10236)
- [Personal style imitation study](https://arxiv.org/abs/2509.14543)
- [InMyStyle](https://arxiv.org/abs/2607.29238)
- [Panza](https://arxiv.org/abs/2407.10994)
- [EU AI Act](https://eur-lex.europa.eu/eli/reg/2024/1689/oj?locale=en)
- [European Commission Article 50 guidance](https://digital-strategy.ec.europa.eu/en/faqs/transparency-obligations-under-article-50-ai-act)
- [C2PA specification](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
