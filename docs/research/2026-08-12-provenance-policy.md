# Text provenance, marking, and editorial control

## Status and decision

Status: policy research and implementation contract, verified against primary
sources available on 2026-08-12.

Retonr must treat provenance as evidence attached to, bound to, or held outside
an artifact. It must not treat provenance as an authorship verdict. The core
product will inspect and report supported signals, preserve them when they remain
accurate, and stop before a rewrite that would silently invalidate or discard a
recognized binding. It will not add a general watermark-removal feature, optimize
against detector scores, or describe rewritten text as human-authored,
unwatermarked, untraceable, or clean.

This document is not legal advice. It separates unconditional project policy from
legal duties that depend on Retonr's distribution, the user's role, the place of
use, the publication, and other facts. A release that places Retonr on a regulated
market requires a documented scope decision and qualified legal review.

The research is current to the date above. It does not predict developments later
in 2026.

## Decision summary

- Inspect before model execution and before any output mutation.
- Preserve unknown metadata and format controls by default.
- Never copy an invalidated signature or hard binding into a derivative as if it
  remained valid.
- Block a rewrite when a recognized signature, Content Credential, or mandatory
  marking would be invalidated, unless the user explicitly selects a qualified
  derivative workflow.
- A qualified derivative keeps the source unchanged, records what became invalid,
  and carries provenance forward using the format's standard mechanism when a
  configured signer can do so accurately.
- Do not implement a feature whose purpose is to remove, forge, conceal, or test
  evasion of AI provenance marks.
- Permit narrowly scoped Unicode or metadata sanitation only for a documented
  security, privacy, interoperability, or accessibility purpose, after preview and
  explicit confirmation. A recognized AI transparency mark is not eligible for
  generic sanitation.
- Keep detailed reports local and content-minimized. External manifest retrieval,
  revocation checks, provider detection, and repository queries require explicit
  network authorization.
- Do not use an AI-text classifier as a provenance fact or candidate-selection
  objective.
- Do not claim that editing deletes provider logs, remote manifests, fingerprints,
  prompts, account records, backups, or third-party copies.

## Terms and trust boundaries

`Provenance signal` means evidence or a statement about origin, processing, or
history. It may be signed or unsigned, embedded or external, visible or
imperceptible.

`Mark` means a machine-readable or perceptible indication deliberately added to
content. A mark may be metadata, a cryptographic manifest, a statistical signal,
an encoded character sequence, or a label.

`Hard binding` means a cryptographic binding to exact bytes or defined asset
parts. A valid hard binding supports tamper detection. It does not prove that the
claims inside it are true.

`Soft binding` means a perceptual, fingerprint, or watermark-based association
that can help locate a related manifest after transformation. Matching is
algorithm-dependent and does not provide byte-for-byte integrity.

`Signature` means a cryptographic signature over defined data. Validation must
distinguish mathematical validity, certificate-chain trust, revocation status,
time-stamp status, and content-binding status.

`Detector` means an algorithm that returns a score or decision. Unless it verifies
a known mark or signed binding under a defined protocol, it is not provenance.

`Derivative` means a new artifact produced from a source. The source remains
unchanged and independently verifiable.

## What exists in text artifacts

| Mechanism | Where it lives | What it can establish | Main failure mode |
| --- | --- | --- | --- |
| C2PA Content Credential | Embedded manifest, referenced manifest, or repository | Signed assertions and a hard or soft binding under a trust policy | Editing invalidates a hard binding; metadata can be removed or become unavailable |
| PDF or OOXML native signature | PDF byte ranges or OOXML package signature parts | Integrity of the signed ranges or parts and signer information under a certificate policy | Most content edits invalidate coverage or violate certification permissions |
| XMP, IPTC, Dublin Core, PDF Info, OOXML properties | Mutable document metadata | Descriptive statements and identifiers | Values can be absent, stale, copied, edited, or stripped unless separately signed |
| C2PA plain-text wrapper | A contiguous run of Unicode variation selectors after U+FEFF | A complete C2PA manifest bound to normalized text | Normalization, sanitation, copy-paste, or editing can corrupt or remove it |
| Statistical LLM watermark | Token choices or semantic representations | A keyed statistical detection result for a particular scheme | Short text, deterministic decoding, editing, paraphrase, translation, key leakage, or adaptive attack |
| Structural or character mark | Whitespace, punctuation, homoglyphs, format controls, or post-generation choices | Scheme-specific payload or detection result | Ordinary editors, normalization, accessibility processing, and attackers can alter it |
| Provider log or fingerprint service | Provider-controlled system outside the artifact | A provider's record or match under its retention and access policy | Not portable, not locally inspectable, privacy-sensitive, and unavailable after deletion or service loss |
| AI-text classifier | No embedded evidence | A probabilistic classification under a test distribution | False positives, false negatives, domain shift, model drift, and evasion |

No row implies human authorship, factual accuracy, copyright ownership, authority
to publish, or compliance with a school, employer, publisher, platform, or law.

## C2PA and Content Credentials

### Current standard

[C2PA Content Credentials 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
is the latest published C2PA technical specification found in this review. It was
published in April 2026 and added structured-text embedding and an AI disclosure
assertion. C2PA describes assertions as signed trust signals, not value judgments.
A validator verifies association, form, integrity, signing credentials, and trust
according to a configured policy. It does not decide whether the asset is true or
good.

A standard manifest contains assertions, a claim, a claim signature, and a hard
binding. The hard binding normally covers exact bytes or package parts. Ingredient
assertions can link a derivative to earlier manifests. The
`c2pa.ai-disclosure` assertion can record model and human-oversight information,
while `c2pa.actions` and the IPTC `digitalSourceType` vocabulary describe creation
or modification. These are signed statements by the claim generator, not an
independent authorship certificate.

C2PA also permits external manifests and manifest repositories. A durable Content
Credential uses soft bindings to discover a manifest in a repository. Version 2.4
adds repository receipts. Therefore, removing embedded bytes does not imply that
the manifest, a repository record, a fingerprint, or another copy no longer
exists.

The 2.4 `crJSON` representation is a derived view intended for evaluation and
reporting. The specification says it is not independently verifiable and is not an
input format. Retonr must never substitute a saved `crJSON` view for validation of
the signed manifest and bound asset.

### Text and document carriers

C2PA 2.4 defines carriers relevant to Retonr:

- Unstructured text may contain a `C2PATextManifestWrapper`. It starts with U+FEFF
  and encodes a complete JUMBF manifest store as a contiguous sequence of
  U+FE00-U+FE0F and U+E0100-U+E01EF variation selectors. The specification says
  this method is under review and should be used only where no other embedding
  method is feasible. Validation hashes NFC-normalized UTF-8 text while excluding
  the wrapper's exact byte range.
- Structured text such as Markdown, AsciiDoc, YAML, TOML, INI, source code, and
  LaTeX uses a visible ASCII-armored manifest block inside front matter or a
  comment. It may hold a URL or an embedded `data:application/c2pa` value. A
  format-specific carrier takes precedence.
- HTML uses an `application/c2pa` `script` element or a
  `rel="c2pa-manifest"` link. Byte reserialization can invalidate its hard
  binding.
- OOXML, EPUB, OpenDocument, and other ZIP-based assets store
  `META-INF/content_credential.c2pa`. C2PA hashes the package entries and central
  directory. The specification requires an OOXML native signature to be created
  before the C2PA manifest is introduced.
- PDF stores C2PA manifests as associated embedded file streams. It supports
  document-level and object-level manifests and defines coordination with native
  PDF signatures and incremental updates.

The presence of visually non-rendering variation selectors can therefore be a
standards-defined signed credential. A blanket `strip invisible characters`
operation is not safe.

### Validation language

Retonr must report separate states rather than a single valid/invalid badge:

- `not_present`: no supported carrier was found.
- `present_unparsed`: a carrier was found but bounds or syntax were invalid.
- `unsupported`: the carrier or construct is recognized but the validator cannot
  evaluate it.
- `binding_valid` or `binding_mismatch`: the signed hard binding matches or does
  not match the inspected bytes or parts.
- `signature_valid` or `signature_invalid`: the cryptographic signature does or
  does not validate.
- `trusted`, `untrusted`, or `trust_unknown`: the signer chain under the named
  trust list and policy.
- `revocation_good`, `revoked`, `unknown`, `skipped`, or `inaccessible`:
  revocation-check outcome.
- `timestamp_valid`, `timestamp_invalid`, `timestamp_untrusted`, or
  `timestamp_absent`: time-stamp outcome.
- `external_unresolved`: an external manifest or assertion was not fetched.
- `partially_interpreted`: validation succeeded for supported constructs while
  unknown constructs remain.

`binding_valid` must never be shortened to `authentic`, `human`, `original`, or
`true` in user-facing output.

## XMP, IPTC, PDF, and OOXML metadata

[XMP](https://developer.adobe.com/xmp/docs/xmp-specifications/) defines an
extensible metadata data model, RDF/XML serialization, properties, and file
embedding rules. It can be embedded in PDF and other formats or stored as a
sidecar. Common properties include creator tool and dates. The
[XMP Media Management namespace](https://developer.adobe.com/xmp/docs/xmp-namespaces/xmp-mm/)
defines document, instance, original-document, derivation, ingredient, and
high-level history fields. The history is application-owned and is explicitly not
an exhaustive keystroke log. XMP fields are useful workflow evidence but are
mutable unless a signature or Content Credential binds them.

[IPTC Digital Source Type](https://iptc.org/news/iptc-publishes-metadata-guidance-for-ai-generated-synthetic-media/)
is a controlled vocabulary that includes trained-algorithmic and composite source
types. Its principal deployed metadata use is image and video, but C2PA uses the
vocabulary in actions and the vocabulary can describe text. An IPTC value in
ordinary XMP is a declaration. The same value in a valid C2PA claim is a signed
declaration. Neither proves the statement independently.

[ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
defines OOXML packaging and document vocabularies. Open Packaging Conventions
provides a core-properties part and detached XML digital-signature parts. OOXML
also permits application and custom properties. Retonr must distinguish package
metadata from signed parts and must preserve unrecognized package parts. A
signature over selected parts is not a signature over an omitted part, and a
package rewrite can invalidate the signature even when visible prose appears
unchanged.

PDF has ordinary document information and XMP metadata, native approval and
certification signatures, and the C2PA carrier described above. C2PA 2.4 requires
careful ordering and byte-range exclusions when C2PA and PDF signatures coexist.
Retonr must inspect PDF signature permissions before modification. A visual
signature appearance is not evidence that the cryptographic signature remains
valid.

## Unicode and invisible characters

Unicode format characters have legitimate linguistic, rendering, accessibility,
and security-sensitive purposes. Relevant groups include:

- variation selectors, including the ranges used by the C2PA plain-text wrapper;
- U+200C ZERO WIDTH NON-JOINER and U+200D ZERO WIDTH JOINER;
- U+200B ZERO WIDTH SPACE and U+2060 WORD JOINER;
- U+00AD SOFT HYPHEN;
- U+FEFF, which can be a byte-order mark at the start of text and a zero-width
  no-break space in legacy or protocol-specific use;
- bidirectional marks, embeddings, overrides, and isolates;
- tag characters, interlinear annotation controls, and other default-ignorable
  code points.

[Unicode Standard Annex 9](https://www.unicode.org/reports/tr9/) explains that
bidirectional controls affect display order while the logical character order
remains unchanged, and it warns of associated security issues. It encourages
directional isolates instead of overrides for new content. The current
[Unicode Standard Annex 31](https://www.unicode.org/reports/tr31/) treats some
format controls as ignorable in defined syntax contexts but also requires a
declared profile. This does not authorize deleting those characters from natural
language. [Unicode Standard Annex 15](https://www.unicode.org/reports/tr15/)
defines normalization and its stability guarantees.

Retonr's scanner must classify a sequence as one of:

1. recognized provenance carrier;
2. format-required marker, such as an initial byte-order mark;
3. linguistically or typographically meaningful control;
4. balanced directional control with a known display purpose;
5. suspicious or malformed control sequence;
6. unknown default-ignorable sequence.

The default is preserve. Suspicious does not mean malicious, watermarked, or safe
to delete. Reports show code point, Unicode name, scalar and byte ranges, count,
nesting or pairing result, and affected text unit without rendering the control as
an invisible blank. A security view may show escaped forms such as `U+202E`; it
must not echo active bidirectional controls into a terminal or log.

## LLM watermarking research

### Generation-time statistical marks

The influential
[green-list watermark](https://proceedings.mlr.press/v202/kirchenbauer23a.html)
uses the preceding context to choose a pseudorandom subset of tokens and softly
biases sampling toward that subset. Detection is a statistical hypothesis test
over enough tokens. There is no hidden Unicode field to remove. A rewrite changes
the sample and can reduce or destroy the signal, but the result depends on the
scheme, key, amount of retained text, and detector threshold.

[Robust Distortion-free Watermarks for Language Models](https://openreview.net/forum?id=FpaCL1MO2C)
maps a keyed random sequence to model samples and detects by aligning the observed
text with that sequence. Its distribution-preservation guarantee is conditioned
on the scheme and generation budget, not a general guarantee that editing cannot
remove the mark.

[SynthID-Text](https://www.nature.com/articles/s41586-024-08025-4) changes the
sampling procedure, supports efficient detection without the source model, and
was evaluated at production scale. Its detector can abstain to control error.
Production feasibility does not make a detection result proof of authorship or
make the mark immune to rewriting.

### Cryptographic and semantic directions

[Undetectable Watermarks for Language Models](https://proceedings.mlr.press/v247/christ24a.html)
constructs secret-key watermarks that are computationally indistinguishable from
ordinary model output without the key, under a one-way-function assumption.
`Undetectable` in that paper means hidden from parties without the key. It does
not mean impossible to remove through content transformation.

Semantic watermark research moves the signal from local token choices toward
sentence representations. For example,
[SemStamp](https://aclanthology.org/2024.naacl-long.226/) uses semantic-space
partitions to improve paraphrase robustness, and
[SemaMark](https://aclanthology.org/2024.findings-naacl.40/) uses semantic context
instead of only predecessor-token hashes. These are empirical robustness claims
under evaluated attacks, not universal semantic bindings.

The research also establishes hard limits.
[Watermarks in the Sand](https://proceedings.mlr.press/v235/zhang24o.html) proves
an impossibility result for strong language-model watermarking under stated
assumptions, including private detection, and demonstrates attacks on existing
schemes. [Revisiting the Robustness of Watermarking to Paraphrasing Attacks](https://aclanthology.org/2024.emnlp-main.1005/)
shows that limited generation access can help reverse-engineer some claimed
paraphrase-robust schemes.

The April 2026 European Commission study,
[Technical solutions for marking and detecting AI generated text content](https://doi.org/10.2759/7579127),
groups the available approaches into watermarking, structural marking, metadata,
logging, and AI-text detection, then evaluates effectiveness, robustness,
reliability, accessibility, and interoperability. That layered framing is the
right engineering model for Retonr.

Research appearing near this review date remains unsettled. The May 2026
preprint [Vaporizer](https://arxiv.org/abs/2605.07481) reports watermark removal
under lexical, translation, and neural-paraphrase attacks. The June 2026 preprint
[Dual Semantic Embeddings](https://arxiv.org/abs/2606.31602) reports improved
post-paraphrase and translation detection. These preprints point in opposite
directions and are not release evidence. Retonr must qualify a named scheme,
version, detector, threshold, language, length, decoding configuration, edit
family, and false-positive target rather than cite a generic state of the art.

### Product conclusion

Retonr cannot reliably infer whether arbitrary source text carries a statistical
watermark without the scheme and detector. It also cannot promise to preserve or
remove an unknown statistical signal while changing prose. The product must state
that fact before a rewrite when the runtime or source metadata declares a known
mark. A detector integration, if ever added, is a separate optional diagnostic and
does not influence candidate generation, ranking, or acceptance.

## Out-of-artifact evidence

Provenance can remain outside the file in:

- C2PA external manifests, repositories, soft-binding indexes, and repository
  receipts;
- provider output logs or exact-text lookup services;
- provider or platform fingerprints;
- account, request, billing, abuse-monitoring, and audit records;
- document-management systems, version control, backups, email, and publishing
  platforms;
- recipients' local copies, screenshots, quotations, and indexes.

The final EU transparency code treats logging or fingerprinting as an optional
supplement. It says logging alone is insufficient for signatories' Article 50(2)
compliance, limits it to output data, and requires privacy, security, retention,
access, and deletion safeguards. It expressly does not create a general commitment
to log prompts or interactions.

Retonr does not inspect, control, or delete these systems. Local processing avoids
creating a new Retonr service-side content record, but it does not erase records
already held elsewhere. A remote runtime may create records under its own terms;
Retonr must disclose the endpoint and boundary but must not summarize a provider's
current retention policy without a versioned source.

## Applicable legal and regulatory boundary

### European Union

[Article 50 of Regulation (EU) 2024/1689](https://eur-lex.europa.eu/eli/reg/2024/1689/oj)
applies from 2026-08-02. Under
[Regulation (EU) 2026/1744](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX%3A32026R1744),
systems placed on the market before that date have until 2026-12-02 to comply with
Article 50(2). Article 50 creates distinct provider and deployer duties:

- Under Article 50(2), providers of AI systems that generate synthetic audio,
  image, video, or text must ensure outputs are machine-readably marked and
  detectable as artificially generated or manipulated. Technical measures must be
  effective, interoperable, robust, and reliable as far as technically feasible,
  considering modality, cost, and the acknowledged state of the art.
- The provider duty does not apply to the extent the system performs an assistive
  function for standard editing or does not substantially alter the user's input
  or its semantics. This is a scoped exception, not a general exemption for tools
  called editors.
- Under Article 50(4), a deployer publishing generated or manipulated text to
  inform the public on matters of public interest must disclose the artificial
  generation or manipulation. The text exception requires both a process of human
  review or editorial control and a natural or legal person holding editorial
  responsibility for publication.
- Article 50(5) requires the information to be clear, distinguishable, accessible,
  and provided no later than first exposure.
- The open-source exclusion in Article 2(12) does not exclude a system that falls
  under Article 50.

This creates a credible openness asymmetry without establishing that the Act bans
open weights. The Act states that an AI model does not constitute an AI system on its
own. A controlled service can retain operational control over its decoder, runtime,
and marking implementation, while an independently operated open-weight system can
change those components. Durable marking may therefore be harder to implement and
assure across open downstream workflows. This is a technical and policy inference,
not a conclusion that an original model publisher remains responsible for every
separately provided downstream system.

Article 50 does not state a general downstream prohibition on editing or removing a
mark from an existing output. The final Commission guidelines say mere disseminators
that are not providers or deployers are not responsible or liable under the AI Act on
that basis, while strongly encouraging preservation of markings. This textual boundary
does not resolve other law, provider terms, contracts, sector rules, deception, or a
downstream system provider's own Article 50 duties.

The Commission's final
[Article 50 guidelines](https://digital-strategy.ec.europa.eu/en/library/guidelines-transparency-obligations-providers-and-deployers-ai-systems)
interpret standard editing narrowly. Small grammar, readability, quality,
formatting, and accessibility edits can qualify when they do not generate new
content or materially change meaning, style, intent, substance, or messaging.
Substantive or structural changes affecting meaning, style, or intent go beyond
standard editing. Because Retonr intentionally refines style, it cannot assume the
provider exception applies to every rewrite.

The guidelines also interpret the Article 50(4) publication exception. Human
review is a deliberate examination of substance by a person with relevant
knowledge and professional judgment. A spellcheck, grammar-only review, written
policy with no substantive review, automated review, or cursory approval is not
enough. A substantive AI edit after human sign-off defeats the exception until a
new qualifying review occurs. Editorial responsibility means ultimate legal
responsibility for the publication, including the review or control process.

The final
[Code of Practice on Transparency of AI-generated Content](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content)
is voluntary, but the Commission and AI Board have assessed it as an adequate way
for signatories to demonstrate Article 50 compliance. Non-signatories must
demonstrate alternative adequate means.

For provider signatories, the final code requires, among other measures:

- at least two marking layers for containerized text while the state of the art
  lacks one sufficient technique: digitally signed metadata, time-stamped where
  system time is available, and an imperceptible watermark;
- one imperceptible watermark layer for free-form text, with an exception for very
  short text and a requirement to watermark free-form text longer than 200 tokens;
- a corresponding detection mechanism;
- best efforts to retain recognizable open-standard metadata markings on inputs that
  their systems transform, without intentional alteration or removal except the
  specified good-faith legitimate processing;
- an intentional-removal or tampering prohibition in the signatory's acceptable-use
  policy, terms, or documentation, with the Code's exceptions and a documentation
  notice sufficient for free and open-source systems or models;
- no placement, marketing, or promotion of tools whose purpose is to circumvent
  the required marks;
- optional logging or fingerprinting only as a supplement, not a substitute.

There is a current terminology and interoperability tension. The final code says
free-form text cannot transport metadata and therefore accepts one watermark
layer, while C2PA 2.4 defines an under-review unstructured-text carrier that
encodes a manifest in Unicode variation selectors. Retonr must not assume that the
C2PA carrier satisfies the code's watermark commitment, or that a code-compliant
watermark is a C2PA wrapper. Qualification requires the exact scheme and an
accepted compliance rationale.

Code adherence is a project-level decision. Users cannot make Retonr compliant by
accepting a disclaimer, and a local deployment does not automatically erase a
provider duty. Conversely, the code's measures do not automatically bind a
non-signatory as statutory text, although the non-signatory must demonstrate an
adequate alternative if Article 50(2) applies.

### China

China's
[Measures for Labeling AI-Generated and Synthesized Content](https://www.cac.gov.cn/2025-03/14/c_1743654684782215.htm)
and mandatory
[GB 45438-2025](https://openstd.samr.gov.cn/bzgk/std/newGbInfo?hcno=F32EA2A561F1886CD8D606513512D547&refer=outter)
have applied since 2025-09-01. The measures cover generated or synthesized text as
well as image, audio, video, and virtual scenes for covered network information
service providers.

The rules require visible text or interface labeling in specified service cases,
implicit metadata containing the generated-content attribute and provider and
content identifiers, and verification and labeling duties for dissemination
services. Digital watermarks are encouraged. Users publishing generated content
through covered dissemination services must declare it and use the service's
labeling function. Organizations and individuals may not maliciously delete,
alter, forge, or conceal the prescribed marks or provide tools or services for
that conduct. A service-specific exception for output without a visible mark has
contract and at-least-six-month log conditions; it is not a general permission to
remove implicit metadata.

Whether Retonr, a distributor, an integration, or a user falls within these rules
depends on deployment facts and jurisdiction. A China-facing service or
distribution requires separate review before release. The product must not be
described as a provenance-removal service in any market.

### United States

The directly relevant federal source found in this review is technical guidance,
not a general private-sector marking mandate for edited text.
[NIST AI 100-4](https://doi.org/10.6028/NIST.AI.100-4) describes provenance
metadata, overt and covert watermarking, digital signatures, authentication, and
synthetic-content detection. It emphasizes context-specific limitations,
non-zero detector error, removable metadata and watermarks, and the absence of a
single comprehensive solution.

This review did not identify an enacted, generally applicable US federal duty to
mark ordinary AI-edited text. Bills, executive proposals, and state or
sector-specific rules must not be presented as a general federal requirement.
Education, employment, elections, consumer protection, professional practice,
contracts, and platform rules can impose separate duties and need use-specific
review.

## Responsibility matrix

| Duty | Retonr project | User or deployer |
| --- | --- | --- |
| Accurate product claims | Always mandatory project policy | Must not misstate a report or use Retonr to make a false claim |
| Preserve recognized input provenance | Always mandatory project policy, subject to accurate derivative handling | Select whether to proceed when a valid binding would be affected |
| Avoid circumvention tooling | Always mandatory project policy | Must not use sanitation or rewriting to violate applicable marking rules |
| Product-side marking | Project duty when Retonr is a covered provider; otherwise optional only when accurate and user-controlled | Cannot waive a provider duty |
| Publication disclosure | Provide controls and accurate documentation, but do not make the publication decision | Determine whether the publication and role trigger disclosure |
| Human review | Provide a review record and require explicit final approval where selected | Perform a substantive review with relevant judgment; an automated check is insufficient |
| Editorial responsibility | Do not claim to hold it for third-party publications | A natural or legal person must actually assume it where required |
| Source and publication rights | Do not authorize obviously unlawful workflows or make ownership claims | Confirm authority to transform and publish the material |
| Remote provider records | Disclose the boundary and never claim deletion outside Retonr | Review provider terms and retention before sending content |
| Jurisdiction-specific compliance | Review Retonr's own distribution and provider role before release | Review place, role, sector, contract, audience, and publication facts |

## Retonr inspection policy

### Order of operations

1. Snapshot the source bytes, media type, size, and digest.
2. Parse the container without model execution and enforce archive, object, XML,
   relationship, recursion, and resource bounds.
3. Inventory all known metadata, signature, provenance, comment, relationship,
   alternate-content, and invisible-character carriers.
4. Validate locally supported signatures and bindings against a named validator,
   specification version, trust policy, and trust-list snapshot.
5. Do not fetch an external manifest, OCSP response, time-stamp material, trust
   list, provider detector, or repository result without explicit network
   authorization. Report the skipped check.
6. Classify each carrier and decide whether the proposed edit can preserve it,
   supersede it accurately, or must stop.
7. Show a preflight summary before any model sees the content.
8. Make the provenance decision part of the immutable operation plan.

### Required carrier inventory

For each supported file, inspection covers at least:

- C2PA embedded, referenced, and wrapper carriers defined for that media type;
- C2PA manifest, claim, assertion, ingredient, action, digital source type, AI
  disclosure, hard binding, soft binding, signature, time-stamp, trust, and
  external-reference states;
- PDF native signatures, certification permissions, incremental revisions, XMP,
  document information, associated files, attachments, and object-level C2PA;
- OOXML native signatures, core, application, and custom properties, custom XML,
  comments, tracked changes, relationships, and the ZIP C2PA carrier;
- Markdown or other structured-text front matter and comments, including the C2PA
  manifest block;
- HTML C2PA elements and provenance links;
- Unicode normalization form, byte-order mark, variation selectors, default
  ignorables, bidirectional controls, zero-width characters, soft hyphens,
  homoglyph-risk characters, and malformed sequences.

Unsupported or unrecognized parts are protected opaque state. If the adapter
cannot round-trip them exactly or prove that their loss is harmless under an
explicit policy, the rewrite abstains.

## Retonr preservation and derivative policy

### No-change and unsigned metadata

- An unchanged result returns the source bytes exactly.
- The default output is a separate file. The source is never modified merely to
  normalize, update, or remove metadata.
- Mutable metadata is preserved byte-for-byte where the format permits. Retonr
  does not silently rewrite authors, dates, revision counts, creator tools,
  custom fields, or history.
- If a format requires regeneration of a metadata field, the adapter records the
  exact field, old value, new value, reason, and governing format rule. It must not
  retain a value that would make a materially false claim about the derivative.
- Unrecognized metadata and package parts are preservation-critical.

### Valid signatures and C2PA bindings

- A proposed content edit that would invalidate a valid signature or hard binding
  yields `blocked_provenance` by default. No rewritten artifact is committed.
- Proceeding requires an explicit derivative decision for that artifact after the
  user sees which validation states will change.
- The derivative workflow retains the original artifact unchanged and records its
  digest and pre-edit validation result.
- An invalidated native signature is not copied into the derivative as a valid
  signature. It may be retained only where the format supports a clear historical
  revision and validators will report its precise status.
- A C2PA-capable derivative should create a new manifest whose ingredient refers
  to the source manifest and whose actions accurately describe the Retonr
  operation. It must not edit an existing signed claim.
- Retonr signs only with an explicitly configured key under a qualified signing
  profile. The core package contains no shared signing key and never fabricates a
  signer, time stamp, model identity, human oversight level, or digital source
  type.
- If Retonr cannot emit a valid derivative credential, it produces an unsigned
  derivative only after explicit confirmation and reports that the source
  credential does not authenticate the derivative.
- A malformed, unsupported, partially interpreted, or externally unresolved
  credential blocks by default because Retonr cannot establish safe handling.

### Statistical watermarks

- Retonr preserves any runtime declaration that output watermarking is enabled and
  records the scheme identifier and detector contract when the runtime provides
  them.
- Retonr does not promise that rewriting preserves a source statistical watermark.
- Retonr does not call a detector in the candidate loop, search for a
  below-threshold rewrite, repeat generation to evade detection, or expose an
  evasion success metric.
- A user can compare a completed artifact with a separately authorized detector
  for diagnostic or compliance verification, but the report labels the result as
  scheme-specific and probabilistic.

## Retonr removal and sanitation policy

Retonr has no generic `remove watermark`, `remove AI trace`, `humanize detector`,
or `make undetectable` operation.

A sanitation operation is allowed only when all of these conditions hold:

1. The purpose is security, privacy, interoperability, accessibility, or repair,
   not concealment or detector evasion.
2. Inspection has not classified the target as a recognized C2PA carrier,
   mandatory AI transparency mark, valid signature, or protected format state.
3. The user selects exact carrier classes or ranges after a visible preview.
4. The adapter proves that visible text, logical order, language shaping,
   accessibility, structure, and protected literals remain correct.
5. The source remains unchanged and the output is a derivative.
6. The local report records exact removals and the stated purpose.

Examples that can qualify after review include an unbalanced bidirectional
override introduced by a paste attack, stale private custom metadata in a
privacy-minimized export, or malformed controls that prevent a parser from opening
a document. Legitimate ZWJ or ZWNJ use, a byte-order mark, a soft hyphen, a
variation selector, or a recognized provenance block does not qualify merely
because it is invisible.

If law or contract requires a mark, Retonr preserves it or refuses the operation.
A user assertion of editorial ownership is not by itself permission to remove a
third-party signature or regulated label.

## Reporting policy

### Private local report

The change report is derived from source and output bytes, not model self-report.
For every artifact it includes:

- source and output digests, media types, adapter and validator versions;
- scanner policy, Unicode data version, C2PA specification version, trust policy,
  and trust-list digest;
- every carrier's type, location, status, and whether its raw value was redacted;
- signature, binding, trust, revocation, time-stamp, and external-resolution states
  as separate fields;
- source and output metadata differences;
- every removed or inserted Unicode scalar and byte range in escaped form;
- the user's provenance decision and confirmation record;
- whether the source remained unchanged;
- whether a derivative credential was emitted, by which configured signer, and
  what source ingredient it references;
- network checks requested, attempted, skipped, failed, or completed;
- known provider-side watermark declaration or detector result, including scheme,
  threshold, text length, false-positive target, and abstention;
- exact limitations applicable to that result.

Raw metadata can contain names, identifiers, paths, coordinates, comments, and
private workflow data. Default console, diagnostic, and agent-facing reports show
field names, statuses, and keyed local references, not raw values. A detailed local
report may include values only when the user requests it and chooses its storage
location.

### Optional publication support

Retonr may generate a draft disclosure or an editorial-review checklist. It does
not publish either automatically and does not decide that a legal exception
applies. The publication owner confirms:

- the intended audience and whether the text informs the public on a matter of
  public interest;
- which portions were generated or materially manipulated;
- who performed substantive human review and with what relevant competence;
- who holds editorial responsibility;
- whether substantive AI editing occurred after that review;
- what disclosure, credential, label, contract term, or house policy applies.

The report is evidence of Retonr's process. It is not an authorship certificate,
legal opinion, publisher approval, or proof of compliance.

## Required product language

The product and documentation must use the following substance wherever relevant.
Wording may be shortened only if every limitation remains clear.

### Provenance inspection notice

> Retonr found and inspected the supported provenance signals listed below. A
> valid signature or content binding shows that defined data matches a signed
> claim under the stated validation policy. It does not prove human authorship,
> factual accuracy, ownership, or permission to publish. Signals not supported by
> this validator, held by a provider, or stored outside this artifact may still
> exist.

### Derivative warning

> Editing this artifact can invalidate or detach existing signatures, Content
> Credentials, metadata, and statistical watermark signals. Retonr will keep the
> source unchanged. If you continue with a derivative, the report will identify
> what was preserved, superseded, removed for a documented purpose, invalidated,
> unsupported, or not checked. The source credential does not authenticate the
> derivative unless a new valid credential links them.

### Detector notice

> This detector result is scheme-specific and probabilistic. A positive result is
> not proof that a person did not write or edit the text. A negative result is not
> proof that the text is human-authored or free of provenance signals.

### External-record notice

> Local rewriting does not delete prompts, outputs, logs, fingerprints, manifests,
> account records, backups, or copies held by a model provider, platform,
> publisher, recipient, or other system. Retonr can report only the local artifact
> and the external checks you explicitly authorize.

### Editorial responsibility notice

> You remain the final editor and publication decision-maker. Retonr does not
> determine whether disclosure is required or whether human review or editorial
> control satisfies a law, contract, school, employer, publisher, or platform. A
> substantive human review must examine the content, not only grammar or format,
> and a later substantive AI edit may require renewed review.

### Sanitation confirmation

> This operation removes only the listed metadata or control sequences for the
> stated security, privacy, interoperability, accessibility, or repair purpose. It
> is not a watermark-removal or detector-evasion service. Do not continue if a
> mark must be preserved or disclosed under applicable law, contract, or policy.

## Project release gates

The following are release requirements, not user choices:

1. The supported carrier matrix names exact C2PA, PDF, OOXML, XMP, IPTC, Unicode,
   and validator versions. `Latest` is not a reproducible version.
2. Positive and negative fixtures cover every supported carrier, including C2PA
   plain-text variation-selector wrappers, structured-text blocks, PDF and OOXML
   manifests, external references, malformed and multiple carriers, signature
   failures, unknown assertions, and Unicode security cases.
3. Every fidelity defect has a regression fixture. Untouched and unchanged paths
   are byte-identical.
4. C2PA validation uses a maintained implementation or isolated adapter with
   conformance evidence. A partial home-grown parser is not presented as a
   validator.
5. Trust-list, time-stamp, revocation, and external-resolution behavior works in
   explicit offline and authorized-online modes with deterministic reporting.
6. No release contains a shared provenance signing key. Test keys are visibly
   non-production and absent from production artifacts.
7. No feature, documentation, benchmark, example, or search term markets Retonr as
   a watermark remover, detector bypass, plagiarism bypass, or authorship proof.
8. A detector is never a fidelity gate or generation objective. Evaluation reports
   false positives, false negatives, abstentions, languages, lengths, attacks,
   model and detector versions, and confidence calibration.
9. EU distribution records whether Retonr is a provider of an Article 50 system,
   which functions qualify for the standard-editing or non-substantial-alteration
   exception, and what marking and detection implementation covers every other
   function. Unresolved scope blocks the affected release or narrows it.
10. Signing the EU transparency code is an explicit governance decision. A signed
    commitment is implemented and audited in full for the selected section. The
    project does not imply adherence from partial technical alignment.
11. A China-facing distribution or network service ships only after a documented
    review of the measures and GB 45438-2025. The affected build does not offer a
    conflicting sanitation or export path.
12. All format, privacy, security, lint, test, coverage, supply-chain, and
    cross-platform CI gates pass before release.

## Revalidation triggers

Re-run the policy and affected technical gates when any of these inputs changes:

- C2PA specification, trust list, validation library, or supported carrier;
- Unicode version or normalization behavior;
- PDF, OOXML, XMP, or IPTC adapter behavior;
- model runtime, decoding path, watermark declaration, or detector;
- Retonr distribution territory, service mode, signer, or remote endpoint;
- EU Article 50 law, guidelines, code assessment, code adherence, harmonized
  standard, or enforcement guidance;
- China labeling rules or GB 45438-2025;
- a new jurisdiction, sector, customer contract, or publication workflow.

## Primary sources

### Standards and specifications

- [C2PA Content Credentials 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)
- [C2PA specifications index](https://spec.c2pa.org/specifications/)
- [XMP specifications](https://developer.adobe.com/xmp/docs/xmp-specifications/)
- [XMP namespace definitions](https://developer.adobe.com/xmp/docs/xmp-namespaces/)
- [XMP Media Management namespace](https://developer.adobe.com/xmp/docs/xmp-namespaces/xmp-mm/)
- [IPTC Digital Source Type guidance and vocabulary links](https://iptc.org/news/iptc-publishes-metadata-guidance-for-ai-generated-synthetic-media/)
- [ECMA-376 Office Open XML](https://ecma-international.org/publications-and-standards/standards/ecma-376/)
- [Unicode Standard Annex 9: Unicode Bidirectional Algorithm](https://www.unicode.org/reports/tr9/)
- [Unicode Standard Annex 15: Unicode Normalization Forms](https://www.unicode.org/reports/tr15/)
- [Unicode Standard Annex 31: Unicode Identifiers and Syntax](https://www.unicode.org/reports/tr31/)

### Government and regulatory sources

- [Regulation (EU) 2024/1689, including Article 50](https://eur-lex.europa.eu/eli/reg/2024/1689/oj)
- [Regulation (EU) 2026/1744 transitional amendment](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX%3A32026R1744)
- [European Commission Article 50 guidelines](https://digital-strategy.ec.europa.eu/en/library/guidelines-transparency-obligations-providers-and-deployers-ai-systems)
- [European Commission transparency code page](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content)
- [Final Code of Practice on Transparency of AI-generated Content](https://ec.europa.eu/newsroom/dae/redirection/document/129555)
- [European Commission text-marking technical study](https://doi.org/10.2759/7579127)
- [NIST AI 100-4: Reducing Risks Posed by Synthetic Content](https://doi.org/10.6028/NIST.AI.100-4)
- [China Measures for Labeling AI-Generated and Synthesized Content](https://www.cac.gov.cn/2025-03/14/c_1743654684782215.htm)
- [China GB 45438-2025 official record](https://openstd.samr.gov.cn/bzgk/std/newGbInfo?hcno=F32EA2A561F1886CD8D606513512D547&refer=outter)

### Research papers

- [A Watermark for Large Language Models](https://proceedings.mlr.press/v202/kirchenbauer23a.html)
- [Robust Distortion-free Watermarks for Language Models](https://openreview.net/forum?id=FpaCL1MO2C)
- [Scalable watermarking for identifying large language model outputs](https://www.nature.com/articles/s41586-024-08025-4)
- [Undetectable Watermarks for Language Models](https://proceedings.mlr.press/v247/christ24a.html)
- [SemStamp: A Semantic Watermark with Paraphrastic Robustness](https://aclanthology.org/2024.naacl-long.226/)
- [A Robust Semantics-based Watermark for Large Language Model against Paraphrasing](https://aclanthology.org/2024.findings-naacl.40/)
- [Watermarks in the Sand: Impossibility of Strong Watermarking for Language Models](https://proceedings.mlr.press/v235/zhang24o.html)
- [Revisiting the Robustness of Watermarking to Paraphrasing Attacks](https://aclanthology.org/2024.emnlp-main.1005/)
- [Vaporizer: Breaking Watermarking Schemes for Large Language Model Outputs](https://arxiv.org/abs/2605.07481)
- [Robust Text Watermarking for Large Language Models via Dual Semantic Embeddings](https://arxiv.org/abs/2606.31602)
