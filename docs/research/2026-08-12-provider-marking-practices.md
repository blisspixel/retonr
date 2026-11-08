# Provider marking practices and Retonr implications

## Review status

Reviewed: August 12, 2026.

Research cutoff: August 12, 2026. This is a current evidence snapshot, not a
prediction of provider behavior later in 2026.

Update: October 17, 2026, for OpenAI text marking only.

Update: November 7, 2026, for the xAI API text-marking statement, the Anthropic
model roster and detector-access statement, and Google's public SynthID Detector
expansion. August 12 findings stay in place except where a later update names the
point it supersedes.

Scope: provider-deployed text marking, file and media provenance, detector
availability, provider-side records, open-weight behavior, and regulatory
commitments. The companion [text watermark science review](./2026-08-12-text-watermark-science.md)
covers research methods and attack evidence in greater depth.

This document is not legal advice. It separates documented product behavior from
policy commitments and from absence of public information.

## Executive judgment

Provider behavior is not uniform enough to support a provider-wide `marked` or
`unmarked` label.

- Anthropic now documents an embedded text watermark for output from supported
  Claude models. It applies at model inference across Anthropic products and named
  cloud partners. Anthropic has not yet published the detector, algorithm,
  supported-model roster, or performance evidence.
- Google documents SynthID Text in the Gemini app and web experience. Google also
  publishes an optional generation-time implementation that any compatible model
  operator can configure. The public Gemini verification experience currently
  describes image, video, and audio checks, not a text check.
- OpenAI, Microsoft, Meta, Mistral, and Cohere have signed the provider section of
  the EU Code of Practice on Transparency of AI-generated Content. Their public
  materials reviewed here do not establish a comparable text watermark across
  their hosted text products. Several document media provenance, visible labels,
  or image watermarks instead.
- Downloadable weights do not, by themselves, determine output marking. The exact
  inference runtime, logits processors, sampling configuration, keys, and
  post-processors determine whether a generation-time mark is present.
- A provider-side prompt or response record is not a watermark. Rewriting a local
  copy cannot delete a record already held by a hosted service.
- A detected mark is evidence about a particular signal and processing path. It is
  not proof of authorship, misconduct, truth, or complete provenance. A negative
  result is not proof that a person wrote the text.

The bullets above are the August 12 judgment. Later primary-source reviews
supersede the named points:

- October 17, 2026: OpenAI documents textGrain for eligible EU ChatGPT and Codex
  text, and opt-in API text. See the OpenAI update.
- November 7, 2026: the xAI API documentation states that seven named Grok model
  IDs watermark text by default from that date. The Anthropic help article now
  lists a model roster and describes watermark detection as a private preview,
  not as a public detector. Google's October 7, 2026 SynthID Detector expansion
  covers image, video, and audio, not text.

Retonr should treat marking as provenance state to preserve and report, never as a
score to defeat. The defensible product claim is local, fidelity-gated
re-expression. The product must not claim watermark removal, detector evasion,
human authorship, or legal compliance.

## Evidence rules

Every material claim uses one of these statuses:

| Status | Meaning |
| --- | --- |
| Verified official behavior | A current official product document identifies a covered surface and the behavior is corroborated by inspectable code, a public standard verifier, a paper with artifacts, or another official technical source. This is not a claim that this review independently tested the hosted service. |
| Provider statement with undisclosed mechanism | A current first-party source asserts deployment, but the algorithm, keys, detector, model roster, thresholds, or evaluation material needed to reproduce the exact hosted behavior is unavailable. |
| Inference | The conclusion follows from disclosed architecture or runtime control, but the provider does not state it directly. The premises and uncertainty are identified. |
| Unknown | The reviewed primary sources do not resolve the question. This must not be rewritten as `absent`, `disabled`, or `unmarked`. |
| Changed or dated policy | A first-party statement is superseded, conflicts with a newer statement, or applies only at its publication date. It is retained to explain a change or unresolved contradiction, not as current product truth. |

The following source rules govern those classifications:

1. `Documented deployment` requires an official product document, system card,
   provider code repository, or first-party statement that names the mechanism and
   covered surface.
2. `Commitment` means a provider signed a code, made a policy pledge, or announced
   work. It does not establish current implementation coverage.
3. `Not publicly documented` means the reviewed official sources did not identify
   the behavior. It does not prove that no undisclosed mechanism exists.
4. A hosted product and a downloadable model are separate deployment paths. A
   provider's consumer application behavior cannot be imputed to its API,
   cloud-hosted model, or open weights without evidence.
5. Provider statements about imperceptibility, quality, robustness, or accuracy are
   reported as provider claims unless independent evidence is cited.

## Dated claim ledger and reproducibility

| Provider and claim | Status | Dated primary evidence | Contradiction or limit | Independently reproducible now |
| --- | --- | --- | --- | --- |
| Supported Claude text carries a model-level embedded watermark | Provider statement with undisclosed mechanism | Claude Help Center, modified August 10, 2026 | Exact model IDs, algorithm, keys, detector, thresholds, languages, and performance are not published | No. A third party cannot reproduce or validate Anthropic's exact hosted mark from the public material |
| Anthropic was exploring watermarking rather than documenting deployed text marking | Changed or dated policy | Anthropic Transparency Hub, updated January 29, 2026 | Superseded by the August 10 Claude marking article. Reading the January page as current creates a false contradiction | The page state is archived, but it does not describe an implementation |
| Gemini app and web text uses SynthID Text | Verified official behavior | Google DeepMind product announcement, May 14, 2024; Nature article, version of record published October 23, 2024; current SynthID product page reviewed August 12, 2026 | These sources do not establish automatic coverage for Gemini API, AI Studio, Vertex AI, Workspace, or resellers | Partly. The method, reference code, evaluation data, and a compatible implementation are public. Google's production keys and exact hosted detector are not |
| OpenAI had no satisfactory deployed text provenance solution | Changed or dated policy | UK consultation response submitted February 25, 2025, with summary dated April 2, 2025 | This was OpenAI's stated position before signing the Article 50 Code. OpenAI's guidance updated August 11, 2026 still names image and audio SynthID, but does not expressly confirm or deny a new text mark | No exact OpenAI hosted text behavior can be reproduced because current implementation status is unknown. Superseded for hosted text by the October 5, 2026 textGrain announcement in the next row |
| OpenAI marks eligible hosted text with textGrain: on for eligible ChatGPT and Codex output in the EU, opt-in for API customers globally | Provider statement with undisclosed mechanism | OpenAI announcement, October 5, 2026; OpenAI Help Center provenance article and textGrain technical report, reviewed October 17, 2026 | The method is described in a technical report, but production keys, the detector, and the exact model roster are not public. The EU ChatGPT and Codex rollout was announced as occurring over the following weeks. A November 7, 2026 check of that announcement found no later first-party statement that the rollout had finished. API watermarking is off by default. Detector access is limited to approved researchers and expert organizations | No. Without the production key and detector, a third party cannot test hosted text for the mark |
| xAI API text from seven named Grok model IDs carries a default-on watermark from November 7, 2026 | Provider statement with undisclosed mechanism | SpaceXSI output-watermarking page, last updated October 31, 2026, reviewed November 7, 2026 | The page names the API surfaces, model IDs, and `x-watermark` response header. It does not publish the algorithm, key, or a public detector. Team-level per-model scope means the mark is not on every request. The header statement applies to that API response, not to copied prose that no longer has the header | No. Detector access is by application, and the production key is not public |
| Anthropic's help article lists which Claude models carry a text watermark | Provider statement with undisclosed mechanism | Claude Help Center marking article, reviewed November 7, 2026, showing "Updated this week" | This supersedes the August 12 statement that the article listed no model roster. Detection is described as a private preview for eligible organizations, which supersedes "forthcoming" as the access description. The algorithm, keys, thresholds, and performance evidence remain unpublished. Opus 4.6, Opus 4.5, Sonnet 4.5, and Haiku 4.5 have an empty text-watermark cell on this review | No. The private-preview detector and production key are not public |
| Microsoft 365 adds AI metadata to video and audio even when visible marks are off | Changed or dated policy | Microsoft Learn page updated August 7, 2026 | The same page says metadata is currently added only to images. The narrower image-only statement controls this review until Microsoft resolves the internal contradiction | Image C2PA can be inspected. Video and audio metadata coverage cannot be established from the contradictory page |
| Meta, Mistral, and Cohere hosted text output has a deployed machine-readable mark | Unknown | Section 1 Code signatures listed by the Commission, page updated August 12, 2026 | A commitment is not product implementation evidence. No current first-party text-marking specification was found | No provider-specific hosted text mark is available to reproduce from the reviewed material |
| Open-weight text output is necessarily unmarked | Inference rejected | Current Llama, Mistral, Command A+, and gpt-oss deployment materials expose operator-controlled inference | Weights do not force one universal runtime policy. An operator can add or omit a logits processor, post-processor, metadata layer, or log | Yes for one exact local stack. Inspect and hash the runtime, activate or omit a known processor, and test that configuration. The result does not generalize to every host |
| Azure OpenAI generated images carry C2PA Content Credentials in the documented classic path | Verified official behavior | Microsoft Learn, updated February 27, 2026 | The page expressly applies to images and Foundry classic, not generated text or every new-portal path | Yes. The signed manifest can be inspected with Content Credentials Verify or CAI open-source tools |
| OpenAI images on named first-party and API paths carry C2PA plus SynthID | Verified official behavior | OpenAI announcement, May 19, 2026; EU guidance updated August 11, 2026 | This is image evidence, not text evidence. Absence of either signal is inconclusive | Partly. C2PA is publicly verifiable and OpenAI offers verification. The SynthID embedder and production key are not under third-party control |
| Titan Image Generator output carries an invisible image watermark and C2PA metadata | Verified official behavior | Current Amazon Bedrock user guide reviewed August 12, 2026 | Image-specific and model-specific. It does not establish a mark for Titan Text or other Bedrock models | Partly. AWS exposes detection and C2PA can be inspected; the proprietary image embedder is not independently recreated |

## Mechanisms that must remain separate

| Mechanism | What it can show | What it cannot show by itself |
| --- | --- | --- |
| Statistical text watermark | A keyed generation process may have influenced token choices | Authorship, intent, misconduct, or every model that touched the text |
| Signed file metadata or C2PA manifest | A signer asserted an action on a particular asset and the manifest has not been invalidated | That copied text retains the claim, that the content is true, or that one person wrote it |
| Visible label or audible disclosure | A product or publisher disclosed AI generation to a viewer | Durable provenance after cropping, transcription, or removal |
| Provider detector | The tested input matched one supported signal under one detector and threshold | A universal AI-generated classification |
| Fingerprint or provider-side output log | A provider can compare content or a fingerprint with retained records | An embedded signal that travels with a copied document |
| Prompt, response, safety, or account log | A hosted service retained transaction evidence under its data policy | A property of the downloaded output or a record a local editor can erase |

Literal zero-width characters and hidden file fields are also not equivalent to a
generation-time statistical watermark. Anthropic describes its text mark as woven
into the generated text, and Google describes SynthID Text as changing token
sampling probabilities. Neither provider describes its text scheme as a hidden
Unicode tag.

## EU regulatory and code commitments

[Article 50 of the EU AI Act](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-50)
requires providers of systems that generate synthetic audio, image, video, or text
to make outputs machine-readably marked and detectable as artificially generated or
manipulated, subject to technical feasibility and specified exceptions. The
Commission's current [Article 50 guidelines](https://digital-strategy.ec.europa.eu/en/policies/guidelines-ai-transparency-obligations)
explain provider and deployer roles, standard-editing exceptions, and the distinct
disclosure duty for certain public-interest text.

Changed legal source warning: the AI Act Service Desk says its displayed Article 50
text has not yet been updated for the Digital Omnibus amendment. The Commission's
guidelines were last updated August 6, 2026. Product claims should therefore rely on
current counsel and operative text, not copy the explorer page as a complete legal
opinion.

The voluntary [Code of Practice on Transparency of AI-generated Content](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content)
provides a recognized compliance route. Section 1 covers provider marking and
detection. Section 2 covers deployer labels. This is separate from the Code of
Practice for General-Purpose AI Models, which addresses different AI Act duties.

The [final Code dated June 10, 2026](https://ec.europa.eu/newsroom/dae/redirection/document/129555)
recognizes that no single technique generally satisfies every effectiveness,
interoperability, robustness, and reliability objective. Its provider measures use
a layered approach that includes signed metadata, imperceptible watermarks, and
detector access. For free-form text, its watermark submeasure addresses outputs
longer than 200 tokens, acknowledges lower reliability than some other modalities,
and permits text detector access to be limited to verified expert users.

The Code also treats privacy-preserving output fingerprints or direct output logs as
optional supplementary techniques. It says those techniques are insufficient by
themselves and does not create a general commitment to retain prompts or user
interactions. Where output logging is used, the Code calls for user control,
limited retention, deletion rules, and compliance with data protection law.

The Commission's current [Section 1 signatory list](https://digital-strategy.ec.europa.eu/en/news/strong-backing-code-practice-transparency-ai-generated-content)
includes Anthropic, Cohere, Google, Meta, Microsoft, Mistral, and OpenAI. Signature
shows a commitment to the Code's provider measures. It does not prove that every
model version, region, reseller, API, or legacy product already emits a detectable
text mark.

## Provider comparison

| Provider | Hosted text claim status | File or media provenance documented | Public text detector | Downloadable or local behavior |
| --- | --- | --- | --- | --- |
| Anthropic | Provider statement with undisclosed mechanism: the November 7, 2026 help table names the Claude models with a text-watermark cell. Opus 4.6, Opus 4.5, Sonnet 4.5, and Haiku 4.5 are not in that set | Signed C2PA metadata for supported generated files | Private preview for eligible organizations; not a public detector | No downloadable Claude weights; the mark is asserted across named hosted surfaces and cloud partners |
| xAI API | Provider statement with undisclosed mechanism: default-on text for grok-4.7, grok-4.6, grok-4.5, grok-4.3, grok-4.20-reasoning, grok-4.20-non-reasoning, and grok-4.20-multi-agent from November 7, 2026, on every documented API endpoint, subject to team-level per-model scope | Not established by the watermarking page | By application; not public. The page says `x-watermark: true` means that API response was generated with the watermark, and that any other value or a missing header means that request was not | No downloadable-weight claim is made on the watermarking page. Consumer Grok surfaces are outside that page |
| Google | Verified official behavior: SynthID Text for Gemini app and web | SynthID across image, audio, and video products | No public Gemini text check documented; open-source detector components exist for operator-configured marks | Inference: SynthID Text is optional runtime instrumentation, not an inherent property of Gemma or other weights |
| OpenAI | Provider statement (October 5, 2026): textGrain for eligible EU ChatGPT and Codex text and opt-in API text; unknown in the August 12 review | C2PA and SynthID for specified image, audio, and video paths; public verification tooling | No public text detector; access limited to approved researchers and expert organizations | Inference: `gpt-oss` runs in operator-controlled runtimes and the reviewed reference path identifies no mandatory text mark |
| Microsoft | Unknown: no current Copilot or Azure text watermark identified in public product documentation | Microsoft 365 media labels and image metadata; Azure OpenAI image C2PA | No public text detector documented | Inference: open-weight or third-party local models remain subject to the operator's runtime and output policy |
| Meta | Unknown: no current Meta AI or Llama API text watermark identified in public product documentation | Platform labels, image provenance work, and a Meta AI image detection research demo | No public text detector documented | Inference: downloaded Llama weights run under operator-controlled sampling and post-processing |
| Mistral | Unknown: no current hosted text watermark identified in public product documentation | No current text-specific provenance implementation identified | No public text detector documented | Inference: Mistral supports local open-weight deployment and the selected runtime controls marking |
| Cohere | Unknown: no current hosted text watermark identified in public product documentation | No current text-specific provenance implementation identified | No public text detector documented | Inference: Command A+ can run on private or air-gapped infrastructure and the selected runtime controls marking |
| AWS | Unknown for Bedrock text models | Verified official behavior for Titan Image Generator invisible watermarks and C2PA metadata | Image watermark detection is documented | Inference: behavior depends on the selected model and serving stack; Titan's documented mark is image-specific |

## Provider findings

### Anthropic Claude

Claim status: provider statement with undisclosed mechanism.

Anthropic's current [Claude marking documentation](https://support.claude.com/en/articles/16266773-how-claude-marks-ai-generated-content)
was modified August 10, 2026 and was the clearest provider statement of a deployed
text watermark in the August 12 review. The November 7 roster update and the xAI
API section below are later primary sources.

- Claude models launched in the EU on or after August 2, 2026 support
  machine-readable marking at launch. Generated text carries an embedded watermark,
  and supported generated files carry digitally signed provenance metadata.
- For supported models, embedded text marks apply across Claude Platform API,
  Claude, Claude Code, Claude Cowork, and Claude Tag, worldwide. Anthropic also names
  AWS, Google Cloud, and Microsoft Foundry. Signed file metadata may vary with each
  platform's file features.
- Anthropic describes the text mark as an imperceptible model-level watermark that
  travels through copy and paste and may survive some editing. Its no-quality-change
  statement is a provider claim. No public technical specification or independent
  evaluation accompanies the help article.
- Supported SVG, PNG, and JPG outputs can carry signed C2PA provenance metadata.
  This is a separate layer from the statistical text watermark.
- Anthropic says it is adding support to earlier models. The article does not list
  exact supported model identifiers, tokenizer and language coverage, minimum
  reliable lengths, false-positive thresholds, key governance, or test results.
- Anthropic says user and third-party detection support is forthcoming. A public
  detector or detection API should not be represented as available until the
  promised technical documentation is published.

Anthropic explicitly limits interpretation. A detected mark means content may have
been processed by Claude. Proofreading, translation, summarization, and file
conversion can mark output whose ideas or source text came from a person or another
system. A mark may become undetectable after heavy editing, paraphrasing,
translation, mixing, short extraction, metadata stripping, or use of an unsupported
model or surface.

This means `Claude-marked` is a processing-history claim, not an authorship claim.
It also means the categorical statement `all Claude text is watermarked` is false on
Anthropic's own documented limitations.

Changed policy: Anthropic's [Transparency Hub](https://www.anthropic.com/transparency/voluntary-commitments)
was updated January 29, 2026 and described watermarking as an area Anthropic was
exploring. That dated statement is superseded for supported models by the August 10
help article. The change is material and explains why older summaries saying Claude
does not watermark text are no longer reliable.

Independent reproducibility: none for the exact Claude mark as of the August 12
cutoff. A third party cannot reproduce a controlled test without the algorithm,
key material, detector, and threshold. The November 7, 2026 update below records
the later model roster. The file-level C2PA layer will be independently
inspectable where a supported signed file is available.

#### Update: November 7, 2026 (model roster and detector access)

The August 12 section remains the historical review. On November 7, 2026 the same
[Claude marking article](https://support.claude.com/en/articles/16266773-how-claude-marks-ai-generated-content)
showed "Updated this week" and a model table. That table supersedes the earlier
statement that the article listed no model identifiers.

Text-watermark cells are marked for first-party Claude output, and for the named
cloud partners unless a footnote says otherwise, on these rows: Fable 5.1, Fable 5,
Mythos 5.1, Mythos 5, Opus 5.5, Opus 5, Opus 4.8, Opus 4.7, Sonnet 5.5, Sonnet 5,
Sonnet 4.6, and Haiku 5.5.

The text-watermark cells are empty, while the file Content Credentials cell is
marked, for Opus 4.6, Opus 4.5, Sonnet 4.5, and Haiku 4.5. This review does not
treat secondary reports of a later date for those empty rows as a provider
statement.

Footnotes on the reviewed page: Amazon Bedrock rollout for Mythos 5, Opus 4.7, and
Sonnet 4.6 is stated to complete by November 10, 2026. For Sonnet 4.6, outputs may
not reliably carry the text watermark when temperature, top_p, or top_k is set to
a non-default value. That sampling condition is a coverage fact, not a Retonr
control.

Watermark detection is described as a private preview for eligible organizations,
including enterprises checking their own Claude outputs. It is not documented as a
public detector. The algorithm, keys, thresholds, languages, and performance
evidence remain unpublished. A detected mark remains a processing-history signal.
The categorical statement that all Claude text is watermarked is still false on
Anthropic's own table.

Watch item `W-2026-11-07-anthropic-roster`. Domain: provider text marking.
Discovery and research cutoff: November 7, 2026. Primary page reviewed that day;
the page says "Updated this week" and is mutable. Evidence state:
`provider_statement`. Triage: `narrows` the August 12 "no roster" and "detector
forthcoming" descriptions, and leaves the undisclosed mechanism in place. No
qualification record changes, because no runtime is admitted. Product effect: the
ledger and the public edit-level notice. No detector is added to the live product.

### xAI API

Claim status: provider statement with undisclosed mechanism for the named API
surfaces. Unknown for consumer Grok products and for model IDs the page does not
list.

The [output watermarking page](https://docs.x.ai/developers/advanced-api-usage/watermarking),
last updated October 31, 2026 and reviewed November 7, 2026, is published under
the name SpaceXSI. It is the primary source for this row. Secondary news reports
are not.

The page states that, starting November 7, 2026, responses from these model IDs
include a watermark and that it is on by default: `grok-4.7`, `grok-4.6`,
`grok-4.5`, `grok-4.3`, `grok-4.20-reasoning`, `grok-4.20-non-reasoning`, and
`grok-4.20-multi-agent`. It says watermarking is enabled on every SpaceXSI API
endpoint, including `https://api.x.ai` and regional endpoints, for the Responses
API and the Chat Completions API. Covered output is ordinary text, structured
output, and tool-call arguments.

The page says the mark is applied while the model generates text by shaping token
choice, and that it adds no characters, markup, or hidden text. It also says
watermarking does not change response quality or speed. That quality and speed
statement is a provider claim. This review did not test it.

The page says `x-watermark: true` on the API response means that request's output
was generated with the watermark, and that any other value, or no header, means
it was not. That statement is about the HTTP response for that request. Copied
prose does not carry the header. A local document with no header is not a case of
the provider saying that document is unmarked.

Detection of an arbitrary passage is by application. The page says the detector
reports watermarked, not watermarked, or uncertain, and that longer text gives a
stronger signal. The detector is not public. The algorithm and key are not
published.

Scope condition, recorded as a fact about when the provider says the mark applies:
watermarking is controlled per model for the whole team, so it is not a property
of every API key in isolation. A team admin can turn it off for a model after
confirming that an exception under applicable law applies, and some models may be
locked on. Changes apply to new requests. This review does not describe how to
change that setting. Retonr does not use the condition as an objective.

The page does not establish marking for the Grok consumer applications, for model
IDs it does not name, or for downloadable weights. A Code of Practice signature
was not rechecked for this update, so signature status stays unknown.

Independent reproducibility: none for the hosted mark. There is no public
detector or production key.

Watch item `W-2026-11-07-xai-api-text`. Domain: provider text marking. Discovery
date of the page update: October 31, 2026. Effective date stated by the provider:
November 7, 2026. Research cutoff: November 7, 2026. Evidence state:
`provider_statement`. Triage: `new`. No qualification record exists to invalidate.
Product effect: ledger only. The live engine does not call this API, does not read
`x-watermark`, and does not rank candidates by any detector result.

### Google Gemini and SynthID Text

Claim status: verified official behavior for Gemini app and web; unknown for other
Gemini surfaces; operator-controlled inference for open weights.

Google's [SynthID overview](https://deepmind.google/models/synthid/) says SynthID
Text is used for text generated in the Gemini app and web experience. It adjusts
token probability scores during generation. That source does not establish that
every Gemini API, AI Studio, Vertex AI, Workspace, or third-party hosted response is
watermarked. Product coverage must be proven per surface.

Google also publishes [SynthID Text developer guidance](https://ai.google.dev/responsible/docs/safeguards/synthid)
and an [official reference repository](https://github.com/google-deepmind/synthid-text).
The guidance was last updated April 9, 2025. The production implementation is
available in Hugging Face Transformers 4.46 and later. It is a logits processor
applied after top-k and top-p sampling, requires no additional model training, and
requires an operator-chosen secret configuration.

The included Bayesian detector can return `watermarked`, `not watermarked`, or
`uncertain`. Operators choose thresholds and may keep the detector private, expose
an API, or publish it. Google warns that factual and low-choice text is harder to
mark, thorough rewriting or translation can greatly reduce confidence, and the
scheme is not designed to stop a motivated adversary.

Google's public Gemini check currently describes verification of images, video, and
audio. The [SynthID Detector portal](https://deepmind.google/models/synthid/) also
describes those media types, not text. The open-source text detector is not a
universal Google text detector because it needs the matching secret configuration
and detector training.

For local use, downloaded Gemma or other compatible weights do not automatically
emit a SynthID mark merely because Google published the model. The local operator
must activate a matching watermark configuration in the generation runtime. A
provider-neutral product therefore needs to inventory effective logits processors,
not infer marking from the model family name.

Corroboration and reproducibility: Google announced the Gemini app and web
deployment on May 14, 2024. The peer-reviewed [SynthID Text article](https://www.nature.com/articles/s41586-024-08025-4),
published October 23, 2024, reports a production experiment covering about 20
million Gemini responses and provides data for a smaller controlled human
evaluation. The algorithm and operator-controlled implementation can be reproduced
locally with a new private key. Google's exact hosted key, detector, and current
surface-by-surface rollout cannot be independently reproduced.

Google's Article 50 [Code signature statement](https://blog.google/company-news/outreach-and-initiatives/public-policy/eu-ai-act-transparency-code-of-practice/)
is dated July 24, 2026. It adds a regulatory commitment but does not broaden the
documented text coverage beyond the named Gemini app and web sources.

#### Update: November 7, 2026 (SynthID Detector expansion)

Google's [SynthID Detector announcement](https://blog.google/innovation-and-ai/models-and-research/google-deepmind/synth-id-ai-content/),
dated October 7, 2026 and updated October 29, 2026, opens a public detector for
image, video, and audio watermarks from Google and named partners. The reviewed
announcement does not add a public text check and does not change the Gemini app
and web text claim above. An independent October 21, 2026 probing note about
Gemini API text is not a Google product document, so it does not move API text
coverage out of `unknown`.

### OpenAI

Claim status: unknown for current hosted text marking; changed or dated policy for
the earlier no-solution statement; verified official behavior for specified media.

OpenAI's current [EU AI Act guidance](https://help.openai.com/en/articles/12141645-eu-ai-act-openai-resources-and-customer-guidance)
was updated August 11, 2026. It describes C2PA, Content Credentials, SynthID
watermarking for images and audio, a verification portal, and a Content Provenance
API. It does not identify a deployed text watermark or text detector. OpenAI's
[content provenance announcement](https://openai.com/index/advancing-content-provenance/),
dated May 19, 2026, likewise documents media provenance and warns that absence of a
signal supports no conclusion.

In a [UK copyright consultation response](https://cdn.openai.com/global-affairs/b89a7434-7cb9-47a7-b4a7-b50b1a1a0afc/openai-uk-ai-and-copyright-consultation.pdf)
submitted February 25, 2025, with a summary dated April 2, 2025,
OpenAI said it had not found a text provenance solution meeting its accuracy,
quality, and tamper-resistance goals. The later Code signature establishes a
commitment, but no current official product document reviewed here identifies which
hosted text systems, if any, now implement a machine-readable text mark.

This is an unresolved implementation gap, not proof of a contradiction. The
consultation answer is a dated provider position. The August 11 guidance neither
repeats it nor announces a text replacement. Signing the Code does not answer the
product question.

OpenAI's [gpt-oss documentation](https://help.openai.com/en/articles/11870455-openai-open-weight-models-gpt-oss)
says the weights run on infrastructure controlled by the operator through common
inference stacks and are not ChatGPT or OpenAI API models. The reviewed model and
setup materials do not identify a mandatory text watermark. This is not proof that
every third-party gpt-oss service is unmarked. A host can add SynthID Text or another
logits processor, post-processor, metadata layer, or log.

Independent reproducibility: OpenAI's C2PA image manifests can be inspected and its
public verification path can be exercised on supported media. The reviewed sources
provide no equivalent controlled test for hosted text.

#### Update: October 17, 2026 (textGrain)

The paragraphs above record the August 12 review and are kept as dated history.

On October 5, 2026, OpenAI published [Our approach to EU text provenance rules](https://openai.com/index/eu-text-provenance/).
It states that:

- API customers globally can opt in to text watermarking for select models, and
  text watermarking remains off by default in the API.
- OpenAI would add an invisible watermark to eligible ChatGPT and Codex text output
  in the European Union over the following weeks, and is not making it a global
  default at launch.
- Text detector access is open by application and initially limited to approved
  researchers and expert organizations. The detector is not publicly available.

OpenAI's [provenance signals help article](https://help.openai.com/en/articles/8912793-provenance-signals-in-openai-generated-content), reviewed October 17, 2026, lists
text as "textGrain watermarks (EU only)", describes API activation through project
and organization settings, and says the mark changes word-choice randomness rather
than adding hidden characters. The [textGrain technical report](https://cdn.openai.com/pdf/e9508624-d767-41b6-a26d-e34ca798ada6/textgrain-entropy-calibrated-watermarking-for-language-model-text.pdf) describes the
generation and detection procedures; the production key and detector are not public.

Provider-reported limits, not independently reproduced here: detection is lower for
short and low-flexibility text such as mathematics and code, varies by language,
and weakens with editing and translation. OpenAI states that a detected mark does not
establish authorship, ownership, or responsibility, and that the absence of a
detected mark does not prove human authorship.

Watch triage: `provider_statement`, `contradicts` the August 12 status of "unknown"
for OpenAI hosted text. Resolution of affected claims and records follows the
[external change watch](../external-change-watch.md) process.

A November 7, 2026 reread of the October 5 announcement found no later first-party
statement that the EU ChatGPT and Codex rollout had finished. The "following weeks"
limit stays in force. API text marking remains off by default on that announcement.

### Microsoft

Claim status: unknown for text; verified official behavior for the documented Azure
OpenAI image path; internally contradictory provider statements for some Microsoft
365 media metadata.

Microsoft's [Microsoft 365 watermark documentation](https://learn.microsoft.com/en-us/microsoft-365/copilot/watermarks)
was updated August 7, 2026. It describes optional visible video and audible audio
watermarks controlled by policy and optional visible image watermarks controlled by
users. It does not document an embedded text watermark for Copilot prose.

The page contradicts itself about metadata. Two passages say additional metadata is
added to generated video and audio even when visible marks are disabled. A later
note says the information is currently added only to images. This review accepts
only image metadata as established and classifies video and audio metadata as
unknown until Microsoft publishes a correction or a format-level test establishes
the behavior.

[Azure OpenAI Content Credentials](https://learn.microsoft.com/en-nz/azure/ai-foundry/openai/concepts/content-credentials?view=foundry-classic)
was updated February 27, 2026 and documents C2PA manifests for DALL-E and
GPT-image-1 series output in the specified Foundry classic image path. It is not
evidence of text marking. Microsoft's Section 1 Code signature is the current
provider commitment; public implementation details for generated text remain
unknown.

Microsoft also resells models from other providers. The model provider's mark may
apply even when the endpoint is on Microsoft infrastructure. Anthropic, for
example, says supported Claude text watermarks apply through Microsoft Foundry.
The cloud vendor name alone is therefore not enough to determine output policy.

Independent reproducibility: C2PA manifests from the documented Azure OpenAI image
path can be inspected with the named public or open-source tools. The reviewed
materials provide no reproducible Microsoft text watermark test.

### Meta

Claim status: unknown for hosted text marking; inference for local Llama output.

Meta's [statement on signing the transparency Code](https://about.fb.com/news/2026/07/meta-is-signing-the-eu-ai-act-code-of-practice-on-transparency-of-ai-generated-content/)
was published July 28, 2026. It points to platform labeling, C2PA work, and an image
detection research demo. Meta has separately documented
[invisible watermarking for images generated by Meta AI](https://about.fb.com/news/2023/12/meta-ai-updates/)
and
[platform labels for detected or disclosed AI media](https://about.fb.com/news/2024/04/our-approach-to-labeling-ai-generated-content-and-manipulated-media/).
These are not evidence of
an embedded text watermark in Meta AI responses or Llama API output.

Meta's official [Llama model repository](https://github.com/meta-llama/llama-models)
provides downloadable weights and local inference examples. The reviewed model
cards and reference inference path do not identify a mandatory text watermark.
Once weights are downloaded, sampling, post-processing, logging, and optional
watermark processors are controlled by the operator. A hosted Llama service may
therefore behave differently from the same model family run locally.

Meta has also published text-watermark research. Research publication does not
establish deployment in Meta AI or Llama products.

Independent reproducibility: a local operator can inspect and control an exact
Llama reference inference path. That can establish whether that path contains a
known watermark processor, but cannot establish undisclosed behavior in Meta AI or
the Llama API.

### Mistral

Claim status: unknown for hosted text marking; inference for local open-weight
output.

Mistral is a Section 1 Code signatory. Its public product and model documentation
reviewed for this report does not identify a deployed statistical text watermark,
signed text provenance layer, or provider text detector. That status should be
represented as `not publicly documented`, not `unmarked`.

Mistral's [deployment documentation](https://docs.mistral.ai/models/deployment)
distinguishes managed and cloud deployments from local open-weight deployment
through vLLM, TensorRT-LLM, TGI, and other runtimes. The local runtime determines
sampling and output processing. The same named Mistral model can therefore have a
different marking policy across Mistral's API, a cloud reseller, and a local
installation.

Independent reproducibility: an exact local Mistral stack can be inspected and run
with controlled sampling. The provider's hosted marking state cannot be reproduced
from the published material.

### Cohere

Claim status: unknown for hosted text marking; inference for private or open-weight
output.

Cohere is a Section 1 Code signatory. Its public product and API documentation
reviewed for this report does not identify a deployed statistical text watermark,
signed text provenance layer, or provider text detector. That is an implementation
unknown, not evidence of absence.

Cohere's [Command A+ model documentation](https://docs.cohere.com/docs/command-a-plus)
describes Cohere-hosted access, a managed Model Vault, and Apache-2.0 open-weight
deployment. Cohere also documents private and fully air-gapped use. Output marking
in those controlled deployments is a property of the actual serving runtime and
configuration, not an unavoidable property of the weights.

Command A+ model `command-a-plus-05-2026` was released May 20, 2026. A local operator
can inspect and test its exact serving stack. That does not reproduce Cohere SaaS
behavior or prove that every third-party Command A+ host is unmarked.

### AWS and Amazon Bedrock

Claim status: verified official behavior for named image models; unknown for Bedrock
text models and third-party model surfaces except where that provider documents a
mark.

AWS documents invisible watermarks and C2PA metadata for every image from
[Amazon Titan Image Generator](https://docs.aws.amazon.com/bedrock/latest/userguide/titan-image-models.html),
plus a Titan and Nova image watermark detector. The documentation does not extend
that claim to Titan Text, Nova text output, or every model available in Bedrock.

Bedrock is also a marketplace and serving layer for third-party models. As with
Microsoft Foundry, the selected model provider and exact deployment can add its own
mark. Anthropic says supported Claude marks apply through AWS. `Generated on
Bedrock` is therefore not a sufficient marking classification.

Independent reproducibility: C2PA metadata can be inspected and AWS exposes an
image watermark detector for Titan Image Generator and Nova Canvas. The proprietary
embedder cannot be recreated from public code, and the result says nothing about
Bedrock text output.

## Provider-side records are a separate boundary

Hosted retention varies by product, contract, safety status, and feature. The
following examples are operationally important because they refute the claim that a
later rewrite can erase upstream evidence:

- Anthropic's [commercial data retention documentation](https://privacy.anthropic.com/en/articles/7996866-how-long-do-you-store-my-organization-s-data)
  describes default API input and output deletion within 30 days, subject to
  product, safety, legal, and contractual exceptions. Its separate
  [covered-model retention notice](https://privacy.claude.com/en/articles/15425996-data-retention-practices-for-covered-models)
  describes 30-day safety retention across covered platforms, including contexts
  that formerly qualified for zero data retention.
- OpenAI's [API data controls](https://platform.openai.com/docs/models/default-usage-policies-by-endpoint)
  describe abuse-monitoring logs that may contain prompts and responses for up to
  30 days by default, with endpoint-specific application state and eligibility for
  modified or zero data retention.
- Google Cloud's [Vertex AI zero-data-retention guide](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/vertex-ai-zero-data-retention)
  distinguishes possible prompt logging for abuse monitoring from feature storage.
  Search and Maps grounding can retain prompts, context, and generated output for
  30 days under the documented terms.
- Microsoft documents content classification, pattern detection, and possible
  review in [Foundry abuse monitoring](https://learn.microsoft.com/en-in/azure/foundry/openai/concepts/abuse-monitoring).
  Eligible customers can apply for modified abuse monitoring. Exact current terms
  must be checked for the deployment and contract.
- Mistral's [zero-data-retention documentation](https://help.mistral.ai/en/articles/347612-can-i-activate-zero-data-retention-zdr)
  limits ZDR to approved Scale-plan stateless API calls. Stateful conversations,
  agents, files, libraries, and consumer chat require storage for their features.
- Cohere's [enterprise data commitments](https://cohere.com/enterprise-data-commitments)
  say its SaaS platform normally deletes logged prompts and generations after 30
  days, subject to misuse, legal, contractual, and training exceptions. Approved
  ZDR and private or partner deployments have different access and retention.
- AWS says in its [Titan Image service card](https://docs.aws.amazon.com/ai/responsible-ai/titan-image-generator/overview.html)
  that Bedrock does not store or review customer prompts or generations. Customer
  application logs, cloud infrastructure logs, and third-party provider terms still
  need separate review.

These records can support abuse response, service operation, or account history.
They do not travel with copied text. Retonr cannot inspect, change, or delete them
unless a provider separately exposes an authorized deletion interface.

## Professional implications for Retonr

### Treat marking as exact deployment state

Model qualification should record at least:

- provider, product surface, exact model identifier, and model release
- hosted, reseller-hosted, or local execution class
- tokenizer, sampling implementation, and all effective logits processors
- post-processors, export path, and metadata-preservation policy
- marking policy as `documented-enabled`, `documented-disabled`, `unknown`, or
  `not-applicable`
- source links and review date for every non-unknown policy

Do not store provider watermark secret keys in ordinary qualification records. A
local runtime with an undisclosed or uninspectable processor is not fully qualified
for deterministic output-policy claims.

### Preserve provenance without making false assertions

On import, Retonr should inventory file metadata and verify supported signed
manifests without treating them as truth certificates. On export, it must not copy a
source C2PA assertion onto transformed content as though the original signature
covered the new asset.

Until Retonr has a first-party signing design, the conservative behavior is:

1. Preserve the original source and its manifest in a user-controlled transaction
   record.
2. Report whether the transformation or format conversion invalidated, omitted, or
   could not preserve source metadata.
3. Export the transformed document without a misleading inherited assertion.
4. Never label the new file `human-authored`, `unmarked`, or `provider-free`.

A later first-party provenance feature could sign a narrow assertion such as
`Retonr transformed this source under this local transaction`. It still could not
certify the source's authorship or erase upstream processing history.

### Keep detection out of the rewrite objective

Provider detectors are scheme-specific, may require secret configuration, and may
be unavailable or restricted to verified experts. Sending private text to a hosted
detector can also violate the local-first privacy boundary.

If detector support is ever added, it should be an explicit diagnostic with:

- user consent before any network request
- named provider, detector version, key or configuration identity where permitted,
  threshold, language, and supported length
- raw result states including `uncertain`, `unsupported`, and `unavailable`
- no retry loop that edits text to reduce a score
- no acceptance gate based on appearing human

The core rewrite and fidelity decision must not consume a watermark or generic AI
detector score.

### Keep logs and marks separate in the user interface

Retonr can accurately say that a qualified local rewrite makes no new request to the
upstream provider. It cannot say that the upstream provider has no record of the
source interaction. A local transaction log under user control can document what
Retonr did, but it is not equivalent to provider logs, a provider watermark, or an
authorship certificate.

### Do not infer legal status from editorial intent

Article 50 contains exceptions for standard editing and for systems that do not
substantially alter input or semantics. The deployer disclosure rule for certain
public-interest text also has a human-review and editorial-responsibility exception.
Those are fact-specific legal tests, not product toggles. Retonr should expose an
accurate transformation record and let the user obtain legal guidance for the
applicable role, jurisdiction, publication, and workflow.

## Exact product claims

### Defensible when the implementation evidence supports them

- `Retonr reconstructs eligible prose with a qualified local model and validates
  fidelity against the source.`
- `This rewrite ran locally and did not send the document to the upstream model
  provider.`
- `Re-expression can change token-level statistical patterns and can omit or
  invalidate file metadata.`
- `Retonr does not test or optimize text for detector evasion.`
- `Retonr records the local transformation it performed under user control.`
- `A detected provider mark indicates possible processing by that marked system; it
  does not establish authorship or misconduct.`
- `A missing mark or negative detector result does not establish human authorship.`
- `A light-touch edit is not a provider-mark removal operation, and Retonr does not test for a provider mark.`
- `Retonr cannot erase records already retained by an upstream provider.`

### Claims that must not be made

- `Retonr removes Claude, SynthID, xAI, or other AI watermarks.`
- `A light edit, or any other edit level, removes a provider mark.`
- `A missing response header, or a header that is absent from copied prose, proves the text is unmarked.`
- `Retonr makes AI text undetectable.`
- `Retonr guarantees a human detector result.`
- `No watermark means a person wrote the text.`
- `A watermark proves that Claude, Gemini, or another model authored the ideas.`
- `All output from this provider is watermarked.`
- `Open-weight model output is unwatermarked.`
- `Metadata-free means provenance-free.`
- `A local rewrite deletes provider logs.`
- `Retonr output is EU AI Act compliant.`

## Product decision

Do not add a first-party statistical text watermark or a multi-provider detector
dependency to the near-term core. Both add key management, model-runtime coupling,
language and length qualification, privacy questions, and a continuing adversarial
test burden. They also do not advance Retonr's central promise of faithful,
user-controlled local re-expression.

The near-term professional response is narrower and stronger:

- qualify exact local runtimes and effective output processors
- preserve the original and transformation record under user control
- inventory and report supported provenance metadata
- disclose unknown marking state instead of guessing
- avoid detector-guided rewriting
- keep all claims at the processing-history level supported by evidence
