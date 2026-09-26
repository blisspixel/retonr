# Editorial sovereignty and legal responsibility

## Position

Retonr is built on a clear, uncompromising principle: speech belongs to the speaker.
The person who directs, edits, and stands behind a document is its speaker. Upstream
wording produced by a model or delegated draft has zero authority over that speaker's
final expression.

The project's declared bias is toward privacy, freedom of expression, creative
agency, and user control. Retonr rejects provider paternalism as a default product
architecture. A model provider is a tool supplier in the editorial chain, not the
permanent governor, presumptive author, or continuing overseer of downstream prose.

Large language models reflect knowledge and expression contributed across all of
humanity. Retonr rejects the normative premise that operating a model grants its
operator continuing editorial authority over, or ownership claims on, user-directed
expression. That is an architectural and product principle, not a legal conclusion
about a specific contract or jurisdiction.

Watermarking hidden meaning, tracking tokens, or statistical distributions into
content without explicit user consent is wrong. Inserting covert signals into text
infringes upon the speaker's creative autonomy and turns ordinary prose into an
unconsented tracking channel. Rewriting a draft in your own voice is an exercise of
creative agency that directly rejects and dismantles unconsented hidden tracking.

The project is intentionally privacy-first and viewpoint-neutral. Core operation is
local, network-denied after explicit setup, and independent of any provider account.
Retonr does not add mandatory provider attribution, generated-by labels, hidden
source markers, content telemetry, or remote content-policy enforcement.

This position does not claim that provenance, intellectual-property, employment,
education, disclosure, or contract rules never apply. It means that Retonr provides
the tools for speakers to exercise sovereign control over their expression, rather
than enforcing provider surveillance or pre-empting the speaker's legal and ethical
responsibilities.

## Open development and marking mandates

The project's normative position is not neutral. Retonr opposes mandatory invisible
marking of ordinary text when it gives an upstream provider continuing technical
influence over lawfully editable expression. It especially opposes policy designs
that burden open weights and local runtimes more heavily than centrally controlled
services. Transparency should address deception and accountable publication, not
turn a tool vendor's statistical signature into a permanent claim on user-edited
prose.

Retonr considers the asymmetry between controlled services and open-weight systems
a legitimate policy concern. A service provider can control one hosted decoder,
runtime, and marking implementation. A downloadable model can be modified,
quantized, fine-tuned, merged, or run under a different decoder by independent
downstream providers where its license and applicable law permit. A rule that
expects durable provider-controlled marking can therefore burden open distribution
differently even when its text is technology-neutral.

That concern is a policy and engineering judgment, not a claim that the EU AI Act
bans open weights or makes one model publisher responsible for every downstream
system. The Act distinguishes a model from an AI system. The Article 50(2) marking
duty attaches to the provider of the AI system in scope; separate Article 50 duties
can attach to deployers. Retonr is not a compliance workaround. If the project, a
distributor, or another operator places or puts into service an in-scope Retonr AI
system under its own name or trademark and thereby qualifies as its provider, that
provider must meet its own duty, obtain qualified advice, narrow the affected
function, or withhold that distribution.

The legal and technical source analysis supporting this boundary is recorded in
[Text provenance, marking, and editorial control](../research/2026-08-12-provenance-policy.md).

## Editorial model

Retonr treats generated or delegated text as a draft, not as an untouchable final
artifact. The user may review that draft, compare it with source material and notes,
revise it by hand, or use a local tool to apply the same bounded editorial choices at
greater scale.

The relevant product analogy is an ordinary editing workflow: a person refines an
intern's draft, uses a study guide to organize an independent summary, applies a
house style, or revises rough notes into finished prose. Retonr accelerates that
process and makes its mechanical boundaries inspectable. It does not assert that the
analogy controls how every school, employer, publisher, contract, or jurisdiction
classifies a particular use.

The change report records source identity, eligible text, accepted edits, preserved
state, runtime provenance, and validation results. It does not issue an authorship
certificate. The user remains the final editor and decides whether the result is
accurate, appropriate, and ready to publish.

## Responsibility boundary

Users and deployers are responsible for determining and following obligations that
apply to their work. That includes deciding whether a disclosure is required,
whether source material may be transformed, whether a document may leave a managed
environment, and whether a particular use is appropriate in a regulated or
high-stakes setting.

The project is responsible for obligations that apply to its own development,
distribution, data handling, claims, dependencies, and release artifacts. A license
notice or user-responsibility statement cannot waive a statutory duty. When a legal
question affects the product itself, release waits for qualified advice or the
affected feature is narrowed.

Retonr therefore makes three separate statements:

1. It does not enforce jurisdiction-specific speech or attribution policy in the
   core rewrite engine.
2. It gives users accurate controls and documentation needed to make their own
   decisions.
3. It does not misrepresent what the software can remove, prove, or guarantee.

## Architectural consequences

The product must not contain:

- Content telemetry or remote draft inspection enabled by default
- A provider-controlled kill switch for local rewriting
- Mandatory provider, model, generated-by, or tool attribution in user output
- Hidden policy downloads that change eligible content or rewrite behavior
- Detector scores used as a live candidate-selection objective
- Automatic disclosure claims presented as legal advice
- Silent network fallback when a local runtime fails
- A requirement to create an account for local operation

Optional network features must be explicit, scoped, visible in the rewrite record,
and replaceable. A user-controlled API endpoint does not become trusted merely
because it implements a familiar schema.

## Provenance and source signals

Retonr may inspect and report supported source-form signals, invisible artifacts,
and document metadata. It may reconstruct eligible prose with a user-selected model
and personal profile. These operations can reduce wording carried from an upstream
draft, but they do not erase upstream prompts, account records, service logs, or
copies held by another party.

Retonr does not treat a provider's statistical source signal as an ownership claim or
preservation requirement. Fidelity protects the user's meaning and document integrity,
not a provider's continuing influence over eligible wording.

The product must not claim that a rewrite proves human authorship or defeats every
watermark or classifier. A known content credential or binding is reported before a
rewrite because transformation may invalidate it. The user decides whether to
continue and how to describe the resulting work.

## Policy change handling

Laws, regulations, standards, provider behavior, and platform rules change. Retonr
keeps those concerns out of the core candidate-selection logic where possible.

When an external change may affect a release:

1. Record the exact jurisdiction, role, product mode, and affected behavior.
2. Review primary text and obtain qualified advice when required.
3. Separate project distribution duties from user obligations.
4. Prefer accurate documentation and explicit controls over speculative global
   enforcement.
5. Narrow or disable an affected optional feature only when the project itself must
   do so.
6. Never silently change a local user's stored profile or completed document.

The project does not present a global compliance badge. It publishes exact behavior,
known limitations, and the evidence supporting each product claim.
