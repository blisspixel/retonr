# Text watermark science and Retonr implications

## Review status

Reviewed: August 12, 2026.

Research cutoff: August 12, 2026. This is the available evidence through the
cutoff, not a prediction of results that may appear later in 2026.

Scope: watermarks embedded in generated text or in the generation process. Model
weight watermarks, training-data watermarks, generic AI-text classifiers, and file
metadata are included only where they clarify text provenance or an attack boundary.

This document is a research record. It does not select a detector, claim support for
one provider's mark, or authorize detector-guided rewriting.

## Executive judgment

Text watermarking is real, useful, and fundamentally conditional.

Generation-time schemes can put a statistically testable, key-dependent signal into
token choices. Google deployed SynthID-Text at large scale and published both its
Tournament sampling design and a reference implementation. The literature now
contains distribution-preserving schemes, sentence-level semantic schemes,
multi-bit payloads, public-verification constructions, mixed-document localization,
and stronger detectors for edited or low-entropy text. Results published in 2025
and 2026 materially improve particular quality, capacity, localization, and attack
trade-offs.

No result supports a universal conclusion that a faithful rewrite always removes a
watermark, that absence of a detectable mark proves human authorship, or that a
positive detector result proves who wrote the underlying ideas. Detection depends
on the exact scheme, key, tokenizer, detector, text length, entropy, language,
editing history, threshold, and deployment distribution. A provider mark can also
be stolen or spoofed. Thorough paraphrasing, translation, and semantic-preserving
code transformations remain important weaknesses, while stronger semantic marks
move some of the signal into representations that may survive lexical change.

Retonr should therefore treat watermark science as a provenance and product-claims
constraint, not as an optimization target:

- Retonr should reconstruct eligible prose for authorized editorial purposes and
  validate fidelity, not search detector scores or promise detector evasion.
- Product language should say that rewriting can change token-level and other
  source-wording signals, but cannot guarantee any detector outcome or erase
  provider-side evidence.
- A detected watermark is evidence that marked generation may have participated in
  a text's history. It is not proof of authorship, misconduct, or full provenance.
- A negative result means only that the tested detector did not find enough evidence
  under its configuration. It is not an authenticity certificate.
- Local-first generation reduces dependence on provider inference and logs after
  setup, but it does not certify that local output is unmarked or human-authored.
- Retonr should preserve source and rewrite records under user control so legitimate
  editorial history can be explained without relying on probabilistic detection.

## Definitions and threat models

The word *watermark* covers mechanisms with different claims. They should not be
collapsed into one feature.

### Mechanism classes

| Class | Signal | Typical embed point | Typical detector access |
| --- | --- | --- | --- |
| Token-bias or green-list | Excess key-selected tokens | Logits or sampling | Key and matching tokenizer |
| Distribution-preserving sampling | Correlation between key randomness and samples | Sampling | Key, tokenizer, sometimes detector training |
| Semantic or structural | Sentence embeddings, clusters, topics, syntax, or selected lexical structure | Rejection sampling, logits, or post-processing | Key plus semantic model or trained decoder |
| Multi-bit | Encoded payload such as model, tenant, or request class | Any of the above | Decoder, key, codebook, and error model |
| Signature or public verification | Cryptographic signature encoded in text | Rejection sampling or learned generation | Public verification material |
| In-model or learned | Generation behavior distilled or trained into weights | Training or model editing | Scheme-specific detector |
| Metadata or signed manifest | Provenance record outside the linguistic choices | File or transport layer | Signature chain and manifest parser |

Literal zero-width Unicode, font changes, hidden fields, and file metadata are not
the same as a statistical linguistic watermark. They can be useful provenance
layers, but copy-paste normalization, format conversion, or metadata stripping can
remove them without rewriting the visible words.

### Claim levels

A detector can support several different claims, and the strongest is often
unavailable:

1. **Presence:** this text contains enough evidence for a particular mark.
2. **Participation:** a marked generator or post-processor may have processed some
   of the text.
3. **Source attribution:** a particular provider, key, model family, or customer is
   associated with the mark.
4. **Authorship:** a person or model originated the ideas and expression.
5. **Conduct:** use of the system violated a rule or policy.

Most statistical watermarks directly address the first level. A sound operational
process needs other evidence to move from presence to participation or attribution.
Watermarks do not, by themselves, establish authorship or conduct.

### Adversaries

Evaluation should distinguish at least these actors:

- A normal editor who corrects, shortens, translates, quotes, or combines text
- A motivated scrubber who wants a marked passage to test negative
- A spoofer who wants unmarked or harmful text to test as a victim's marked output
- A watermark thief with repeated black-box queries to a marked service
- A detector oracle attacker who can adapt edits after observing scores or labels
- An open-model operator who can disable an inference-time watermark
- A model extractor who trains on marked outputs and may inherit their signal

Robustness to ordinary edits does not imply security against an adaptive attacker.
Likewise, an attack against one token partition does not establish universal removal
against undisclosed, semantic, learned, or layered marks.

## Token-bias and green-list watermarks

The influential KGW construction samples a pseudorandom green subset of the
vocabulary from the recent token context and a secret key, adds a positive logit
bias to green tokens, and then samples normally. Detection reconstructs the same
partitions and tests whether the observed green-token count is unexpectedly high.
The original [ICML 2023 paper](https://proceedings.mlr.press/v202/kirchenbauer23a.html)
and [official implementation](https://github.com/jwkirchenbauer/lm-watermarking)
remain the clearest baseline.

Its core controls expose the general trade-off:

- The green fraction changes the null rate and the number of eligible promoted
  tokens.
- A larger logit bias generally improves detectability and edit tolerance, but can
  distort token choice and downstream quality.
- More context in the seed makes partitions less repetitive and harder to learn,
  but an edit corrupts more subsequent contexts.
- A context-free or fixed partition survives local edits better, but repeated
  service queries reveal more about the partition.
- Detection gains evidence with length and with generation entropy. Deterministic
  or nearly deterministic spans leave little capacity for a mark.

The official implementation recommends ignoring repeated n-grams during detection.
Repeated contexts reuse pseudorandom choices and violate the independent-trial
assumption behind a naive z-test. This is not a cosmetic detail: incorrect counting
can make p-values anti-conservative.

[On the Reliability of Watermarks for Large Language Models](https://proceedings.iclr.cc/paper_files/paper/2024/hash/d78e9e4316e1714fbb0f20be66f8044c-Abstract-Conference.html)
expanded the design with edit-robust seeding and windowed detection. It reported
that strong human paraphrases could remain detectable when enough evidence was
aggregated, with about 800 observed tokens on average at a stated false-positive
rate of `1e-5` in that experiment. This is evidence about the studied scheme,
attacks, lengths, and distributions. It is not a general lower bound on how much
rewriting removes every watermark.

Later token methods adapt where or how strongly they embed:

- [SWEET](https://aclanthology.org/2024.acl-long.268/) applies a watermark only in
  higher-entropy code positions, improving the quality and signal trade-off for a
  constrained domain.
- [Unbiased Watermark for Large Language Models](https://proceedings.iclr.cc/paper_files/paper/2024/hash/c5b00c5bdcc6fe35907dbcca03d27652-Abstract-Conference.html)
  changes the sampling construction to preserve the original distribution in
  expectation rather than adding a persistent one-directional logit bias.
- [STA-1](https://aclanthology.org/2025.acl-long.391/) samples a token and then
  accepts according to the mark. It is designed to be unbiased in expectation and
  to lower the risk of poor choices in low-entropy spans compared with earlier
  unbiased methods.
- [Topic-Based Watermarks](https://aclanthology.org/2026.findings-acl.1220/)
  choose topic-aligned token subsets, reporting improved lexical and paraphrase
  resilience with a lightweight standard-generation integration.
- [A Linguistics-Aware LLM Watermarking via Syntactic Predictability](https://aclanthology.org/2026.acl-long.2115/)
  is part of the 2026 movement toward placing signal where linguistic alternatives
  are more predictable or less harmful.

These refinements reduce particular costs. They do not abolish the amount-of-choice
constraint: a generator cannot encode a strong linguistic signal in a span with one
acceptable continuation without changing the span or using a separate channel.

## Distribution-preserving sampling and SynthID-Text

### Earlier distortion-free constructions

[Robust Distortion-free Watermarks for Language Models](https://arxiv.org/abs/2307.15593)
maps a keyed random-number sequence to language-model samples using inverse-transform
or exponential-minimum sampling. Detection aligns candidate text with the key
sequence. Random insertion, deletion, and substitution can be handled through
alignment, but detector cost and key management are greater than a simple count.
The paper reported reliable detection after substantial random edits for higher
entropy base-model text, while only about one quarter of roughly 100-token Alpaca
instruction responses met its `p <= 0.01` criterion. That result directly shows why
instruction following, factuality, and low entropy matter.

[Undetectable Watermarks for Language Models](https://proceedings.mlr.press/v247/christ24a.html)
formalizes a cryptographic property: without the secret key, a computationally
bounded observer should not distinguish marked output from the original model
distribution. "Undetectable" in that work means hidden from an observer without
the key, not impossible for the keyed detector to detect and not robust to arbitrary
semantic rewriting.

[Watermarking Language Models with Error Correcting Codes](https://arxiv.org/abs/2406.10281)
uses error-correcting structure to improve statistical power and resilience to
edits, deletions, and translation while retaining a distribution-preserving design.
Error correction allocates redundancy; it does not create unlimited capacity.

### Tournament sampling

Google DeepMind's [SynthID-Text paper](https://www.nature.com/articles/s41586-024-08025-4)
describes a keyed random seed for each generation step and multiple pseudorandom
`g` functions that score candidate tokens. Tournament sampling draws `2^m`
candidates from the language-model distribution for `m` tournament layers. At each
layer, paired candidates compete under one `g` function until a final token wins.
The detector measures correlation between observed tokens and the same keyed
`g` values.

Important properties follow from the construction:

- With two candidates per match, Tournament sampling is single-token
  non-distortionary when averaged over random seeds. Repeated context masking can
  extend the non-distortion statement to specified sequences.
- More tournament layers put more evidence into a token up to a saturation point.
- Detection improves with text length and next-token entropy.
- Factual, deterministic, and otherwise low-entropy output is harder to mark.
- A context window links an edit to several downstream scores. The paper used the
  previous four tokens in its experiments.
- The mark can be combined with speculative sampling. The paper reported negligible
  serving overhead in its production-oriented configuration.

The paper evaluated non-distortionary SynthID-Text in a live experiment covering
nearly 20 million Gemini responses. The reported thumbs-up and thumbs-down rates did
not show a meaningful user preference penalty. That is unusually strong deployment
evidence for aggregate perceived quality. It does not prove identical correctness
for every constrained answer, language, code task, or creative choice.

The [Google reference repository](https://github.com/google-deepmind/synthid-text),
[Google safeguards documentation](https://ai.google.dev/responsible/docs/safeguards/synthid),
and [Hugging Face implementation](https://huggingface.co/docs/transformers/internal/generation_utils)
are the relevant public artifacts. The production-ready Transformers integration
uses `SynthIDTextWatermarkingConfig` and a logits processor. Its Bayesian detector
must be trained for a particular key on independent data representative of expected
production text. Google documents three outcomes: watermarked, not watermarked, and
uncertain. Google also states that thorough rewriting or translation can greatly
reduce detector confidence and that the method is less effective on factual
responses.

This leads to three operational conclusions:

1. The open algorithm does not reveal a provider's deployed secret key or detector.
2. A locally reproduced SynthID configuration is not evidence that it matches a
   provider deployment.
3. A detector trained on one content distribution should not be assumed calibrated
   for another model, language, domain, or editing process.

## Semantic, sentence-level, and post-hoc watermarks

Token-context hashes are fragile when most tokens are regenerated. Semantic methods
try to anchor the mark to meaning, sentence representations, topics, or linguistic
structure.

[SemStamp](https://aclanthology.org/2024.naacl-long.226/) hashes sentence embeddings
into locality-sensitive semantic regions and uses sentence-level rejection sampling
until a candidate occupies a permitted region. A margin improves tolerance to small
embedding changes. The [official repository](https://github.com/abehou/SemStamp)
also contains k-SemStamp. Sentence-level sampling can survive paraphrases that erase
token overlap, but it needs multiple candidate sentences and a stable embedding
geometry. This adds latency, can reduce diversity, and creates dependence on the
embedder's domain and language behavior.

[A Semantic Invariant Robust Watermark](https://proceedings.iclr.cc/paper_files/paper/2024/hash/1a2131ebe25bd55e4fc734126ea583ed-Abstract-Conference.html)
maps semantic context to watermark logits through a trained model, targeting synonym
substitution and paraphrase invariance. Later work showed the security qualification:
[Revisiting the Robustness of Watermarking to Paraphrasing Attacks](https://aclanthology.org/2024.emnlp-main.1005/)
used limited generation access to reverse-engineer enough behavior to improve
paraphrase attacks against semantic-invariant schemes.

[PostMark](https://aclanthology.org/2024.emnlp-main.506/) is a black-box post-hoc
method. It chooses an input-dependent set of semantically related words and inserts
them after generation. Post-hoc methods can operate on text from an API that does
not expose logits, but rewriting an already complete answer creates its own fidelity
and computational risks.

[SimMark](https://aclanthology.org/2025.emnlp-main.1567/) uses sentence-embedding
similarity, rejection sampling, and soft counting to improve paraphrase robustness
without model-internal access. [Robust Multi-bit Text Watermark with LLM-based
Paraphrasers](https://proceedings.mlr.press/v267/xu25k.html) trains two 1.1B-parameter
paraphrasers whose semantic differences encode sentence-level bits. Its reported
clean detection AUC exceeded 99.99 percent and it was tested under substitution and
sentence paraphrase, but it introduces learned models and classifier calibration
into both embedding and decoding.

The 2026 frontier combines semantics with explicit keys and localization:

- [TextSeal](https://arxiv.org/abs/2605.12456) adds dual-key Gumbel-max generation,
  entropy-weighted scoring, and multi-region localization. It reports no inference
  overhead, strong diluted-span detection, downstream reasoning preservation, and a
  6,000-comparison human evaluation across five languages. It also reports
  watermark transfer into distilled models. The [Meta Seal repository](https://github.com/facebookresearch/meta-seal)
  is the official open-source entry point.
- [SAFESEAL](https://arxiv.org/abs/2605.23175) preserves named entities while using
  key-conditioned context-aware synonym selection through Tournament sampling, then
  uses a key-conditioned contrastive detector. It reports high BERTScore and entity
  similarity under its benchmark and addresses cross-provider keys explicitly.
- [Topic-Based Watermarks](https://aclanthology.org/2026.findings-acl.1220/) and
  syntactic methods make the partition itself content-aware instead of treating all
  vocabulary alternatives as equally suitable.

Semantic robustness is always relative to a representation and attack set. A
semantic encoder can miss negation, quantities, entities, modality, or domain-specific
equivalence. A mark that intentionally survives broad paraphrase also raises a
spoofing concern: content can be changed in a harmful way while enough semantic or
lexical signal remains. Semantic preservation metrics are useful evaluation
features, not formal guarantees that meaning is unchanged.

## Multi-bit, identity, and signature watermarks

A zero-bit mark answers a presence question. Multi-bit schemes try to carry a model
identifier, tenant code, request class, or other payload. Payload capacity makes
traceability more expressive, but each bit competes for limited linguistic entropy
and requires redundancy against edits.

Representative developments include:

- [Advancing Beyond Identification](https://aclanthology.org/2024.naacl-long.224/)
  allocates token positions across message parts and demonstrated messages of at
  least 32 bits alongside zero-bit detection.
- [Towards Codable Watermarking](https://proceedings.iclr.cc/paper_files/paper/2024/hash/abdc8c031aa6c6917c3b593166e5e340-Abstract-Conference.html)
  uses a proxy model to construct probability-balanced vocabulary partitions. Its
  [official code](https://github.com/lancopku/codable-watermarking-for-llm) exposes
  the implementation.
- [Multi-Bit Distortion-Free Watermarking](https://arxiv.org/abs/2402.16578)
  extends key-based distribution-preserving sampling with an efficient message
  decoder.
- [Provably Robust Multi-bit Watermarking](https://www.usenix.org/conference/usenixsecurity25/presentation/qu-watermarking)
  uses pseudorandom segment assignment. For a 20-bit message in 200 tokens it
  reported a 97.6 percent match rate and a bounded-edit analysis averaging 17 edits
  per paragraph under its stated setting.
- [StealthInk](https://openreview.net/forum?id=dktpDfUTtj) targets distribution
  preservation while carrying provenance fields.
- [XMark](https://aclanthology.org/2026.acl-long.672/) reduces logit distortion and
  improves payload recovery in shorter texts. Its
  [official code](https://github.com/JiiahaoXU/XMark) is available.
- [MirrorMark](https://arxiv.org/abs/2601.22246) uses measure-preserving mirrored
  randomness and a context scheduler. It reports a 54-bit payload in 300 tokens,
  with higher bit accuracy and identification at 1 percent FPR than its baselines.
- [A Distortion-minimization Watermarking Framework](https://www.usenix.org/conference/usenixsecurity26/presentation/zhai)
  models quality, robustness, and capacity as explicit distortion costs and uses
  periodically optimized syndrome-trellis codes.

Recent work also exposes a measurement trap. [BREW](https://arxiv.org/abs/2605.00348)
argues that decoding a plausible payload is not the same as verifying a watermark
and reports catastrophic false positives for some earlier error-correcting-code
extractors. Its two-stage design first estimates a message and then verifies it over
shifted windows. Payload recovery metrics must therefore be reported alongside
clean-text false positives and rejection behavior.

### Public verification and cryptographic meaning

Making a symmetric detector public can leak the material needed to forge or remove
its mark. Publicly verifiable constructions try to separate generation authority
from verification:

- [Publicly-Detectable Watermarking for Language Models](https://eprint.iacr.org/2023/1661)
  encodes a public-key signature through rejection sampling and error correction.
  Its security and distribution claims rely on stated cryptographic and entropy
  assumptions. The [official code](https://github.com/jfairoze/publicly-detectable-watermark)
  is available.
- [An Unforgeable Publicly Verifiable Watermark](https://proceedings.iclr.cc/paper_files/paper/2024/hash/214d2cffc381938be6f7254d5382904f-Abstract-Conference.html)
  uses separate learned generation and detection networks. Its
  [official code](https://github.com/THU-BPM/unforgeable_watermark) accompanies the
  paper.
- [PVMark](https://arxiv.org/abs/2510.26274) addresses the tension between opaque
  private detection and key exposure under public detection.

A cryptographic signature can strongly authenticate a payload if the intact signed
representation verifies. Encoding that signature into mutable natural language is
still a communication problem. Short output, low entropy, editing, and payload loss
remain relevant. Public verification also needs key rotation, revocation, provider
identity, canonicalization, versioning, and an interpretation policy.

## Detector calibration and false positives

### A detector is a hypothesis test, not an oracle

For a keyed count-based detector, the null hypothesis is usually that the candidate
was not produced with that key and scheme. A p-value is the probability, under the
specified null model, of a score at least as extreme as the observation. It is not
the probability that the candidate is human-written. Converting it to a posterior
requires prior prevalence and a validated likelihood model.

Production reporting should include at least:

- The exact scheme, detector, key version, tokenizer, normalization, and threshold
- Eligible and scored token counts, not just source character count
- The null distribution and whether calibration is analytic or empirical
- TPR at operational FPRs, with confidence intervals and sample counts
- An uncertain or insufficient-evidence outcome
- Performance by length, entropy, language, domain, model, and edit class
- Corrections for every searched key, model, window, span, or payload
- Calibration drift and out-of-distribution behavior

Overall accuracy and clean AUC can hide the failure mode that matters. A detector
with a 1 percent FPR will falsely flag many documents when used at platform scale or
when marked text is rare. High-stakes attribution normally needs a far lower
operational FPR plus corroborating evidence.

### Exact and empirical calibration

[Three Bricks to Consolidate Watermarks](https://arxiv.org/abs/2308.00113)
demonstrated that simple normal-approximation z-tests and repeated n-grams can
underestimate very small false-positive rates. It introduced non-asymptotic tests
and unique-event scoring whose empirical FPR better matched the claimed level. A
[statistical framework of pivots and optimal rules](https://arxiv.org/abs/2404.01245)
further formalizes detector design around pivotal statistics whose null distribution
does not depend on unknown text-generation parameters.

Analytic control remains conditional on implementation and assumptions. Natural
language tokens are dependent; repeated contexts, tokenizer normalization,
templates, quotations, code, and fixed phrases can break a naive independent
Bernoulli model. Empirical calibration must therefore use large, untouched negative
sets drawn from the real deployment distribution, including human, templated,
translated, edited, quoted, and mixed-source material.

The SynthID Bayesian detector illustrates a different approach. It learns the
watermarked and unwatermarked score distributions and can expose upper and lower
thresholds for positive, negative, and uncertain results. Its official guidance
requires independent, representative training data for each key. That requirement
is part of detector validity, not optional model tuning.

### Scanning and multiplicity

Whole-document aggregation can miss a short marked passage diluted by human text.
Scanning every possible span can recover it, but selecting the maximum score raises
the false-positive rate unless the threshold accounts for the search.

The ICLR reliability study introduced windowed approaches. [Efficiently Identifying
Watermarked Segments](https://aclanthology.org/2025.acl-long.316/) develops
geometry-cover detection and adaptive localization for long mixed-source text.
[TextSeal](https://arxiv.org/abs/2605.12456) uses multi-region localization. These
are promising for long documents, but a result must identify the tested window
family and its family-wise or equivalent error control.

### Safe interpretation

A professional detector result should read like one of these:

- "Evidence for watermark configuration X was detected in this span at the stated
  threshold. This does not establish authorship or policy violation."
- "The test was inconclusive because the eligible span was too short or outside the
  calibrated distribution."
- "No sufficient evidence for configuration X was detected. This does not show that
  the text is human-authored or that no other mark is present."

It should not read "AI-written," "human-written," or "watermark removed" unless a
separate, much stronger evidence chain supports that claim.

## Robustness under editing and attacks

### Local edits, cropping, and dilution

Token substitution, deletion, insertion, whitespace normalization, Unicode
homoglyphs, and cropping change the scored events. Context-sensitive schemes can
lose several scores after one edit, while fixed partitions and semantic schemes may
retain more signal. Diluting a marked span inside unmarked material lowers a global
mean. Windowed and localized detectors partially address dilution.

The 2025 [WaterPark evaluation](https://aclanthology.org/2025.findings-emnlp.1148/)
integrates ten watermarkers and twelve attacks. Its central lesson is methodological:
robustness rankings depend on watermark and attack design choices, and a unified
testbed reveals weaknesses hidden by each method's native benchmark. A product
claim should be based on a versioned attack matrix, not one paraphraser and one
quality proxy.

[Watermark Smoothing Attacks](https://aclanthology.org/2025.findings-emnlp.264/)
selectively rewrite areas where model confidence predicts watermark evidence. It
tested ten marks and open models from 1.3B to 30B parameters. [SIRA](https://openreview.net/forum?id=fE3kgW7kMp)
targets high-self-information positions, exploiting the common choice to embed in
high-entropy tokens. Both show that adaptive attacks can focus edits rather than
rewrite uniformly.

### Paraphrase and full regeneration

Paraphrasing is not one attack. Results vary with paraphraser capacity, number of
passes, temperature, prompt, target length, detector feedback, and how semantic
quality is judged.

[Can AI-Generated Text be Reliably Detected?](https://arxiv.org/abs/2303.11156)
showed that recursive paraphrasing reduced several watermark and classifier
detectors while often retaining measured quality. The ICLR reliability paper found
that paraphrases could leak enough source n-grams for later detection when long
enough text was available. These results are compatible: paraphrase weakens a
token mark, but the remaining evidence depends on how fully tokens are regenerated
and how much text the detector sees.

Semantic and sentence-level methods are designed to survive more lexical change,
but representation-guided attacks and watermark stealing can improve paraphrases
against them. No current benchmark establishes robustness to every high-quality
paraphraser or future model.

### Translation

Translation can regenerate nearly every token and change token count, order, and
segmentation. Back-translation sometimes leaks enough structure to preserve a mark;
an asymmetric workflow that keeps the translated text can be much more destructive.

[Evaluating Text Watermarking Under Real-World Cross-Lingual Manipulations](https://aclanthology.org/2025.findings-emnlp.390/)
tested KGW, Unigram, EXP, and cross-lingual XSIR across English, Arabic, Chinese,
and Indonesian. Clean AUCs were at least 0.99 in the reported setting, but asymmetric
translation and translation-plus-paraphrase drove several AUCs close to chance.
Reported examples include KGW at 0.55 for Arabic-to-English and Unigram at 0.57 in
its worst English-Arabic directions. XSIR also varied by language because semantic
clusters and tokenizer representations did not transfer uniformly.

This is strong evidence against claims that translation universally preserves or
universally removes a watermark. Direction, language, tokenizer, and scheme matter.

### Watermark collision

If a second model applies a different logit watermark while translating or
paraphrasing, signals may collide. [Lost in Overlap](https://aclanthology.org/2025.findings-naacl.37/)
shows that such collisions can amplify removal attacks against logit-based marks and
affect downstream attribution. Multi-provider deployment therefore needs explicit
tests for nested, competing, and sequential marks.

### Watermark stealing and spoofing

Secret keys do not prevent attackers from learning observable biases through enough
queries.

[Watermark Stealing in Large Language Models](https://proceedings.mlr.press/v235/jovanovic24a.html)
introduced an automated black-box attack that approximates watermark rules from
benign service queries. In its experiments, under USD 50 of API queries enabled
both spoofing and scrubbing with average success over 80 percent across studied
schemes. The [research code and examples](https://watermark-stealing.org/) are
public.

[On the Learnability of Watermarks](https://proceedings.iclr.cc/paper_files/paper/2024/hash/a86d17b6cd70366d56ab48d2a05a4df1-Abstract-Conference.html)
shows that a student model can distill watermark behavior from a marked teacher.
That enables open-model marking, but also spoofing. Low-distortion marks required
more samples, and later fine-tuning on normal text weakened learned marking.

The risk continued in 2025 and 2026:

- [DITTO](https://arxiv.org/abs/2510.10987) turns watermark inheritance during
  distillation into provider-impersonation spoofing.
- [Defending Against Spoofing with Contrastive Representation Learning](https://openreview.net/forum?id=n5hmtkdl7k)
  targets piggyback attacks that preserve a mark while changing content harmfully.
- [SEEK](https://openreview.net/forum?id=RbdLnwEEjk) jointly targets scrubbing and
  spoofing instead of treating them as separate objectives.
- [DualGuard](https://arxiv.org/abs/2512.16182) uses two complementary signals to
  detect paraphrase and trace piggyback spoofing.

These defenses improve empirical frontiers, but they do not turn a positive
statistical mark into a cryptographic proof that the marked provider generated the
final meaning. A detection service itself is a security boundary. Rate limits,
coarse verdicts, query auditing, key rotation, detector diversity, and human review
may reduce oracle leakage, while public-verification schemes need separate
unforgeability analysis.

## Language, code, length, and content regime

### Multilingual behavior

"Language agnostic" at the algorithm level does not mean equally reliable across
languages. Tokenizers have different fertility and Unicode representations.
Vocabulary partitions may contain unequal numbers of plausible continuations.
Semantic encoders and synonym resources can be language-skewed. A fixed number of
tokens represents different amounts of text in different scripts.

The 2025 cross-lingual study found clean detection but language-dependent quality,
diversity, and attack robustness. TextSeal's 2026 human study across five languages
is stronger breadth evidence for that system, but still does not qualify every
language or mixed-language document. Each supported language needs separate
positive and negative calibration, edit attacks, native review, and tokenizer-aware
length reporting.

### Code and other low-entropy text

Code, mathematics, exact quotations, facts, tables, schemas, legal clauses, and API
syntax offer fewer acceptable alternatives than open-ended prose. Biasing token
choice can damage correctness, or the generator can skip so many positions that the
signal becomes weak.

[Who Wrote this Code?](https://aclanthology.org/2024.acl-long.268/) introduced
SWEET to watermark only sufficiently high-entropy code tokens. [Is the Watermarking
of LLM-Generated Code Robust?](https://arxiv.org/abs/2403.17983) then applied
semantics-preserving AST transformations. Variable renaming and dead-code insertion
often drove reported TPR below 50 percent without changing program behavior. Its
[official code](https://github.com/uiuc-arc/llm-code-watermark) provides the attack
implementation.

Newer code-specific and low-entropy detectors report gains, including [Low-Entropy
Watermark Detection](https://aclanthology.org/2025.findings-acl.739/) and 2026
[signature filtering](https://arxiv.org/abs/2606.18430). These should be read as
scheme-specific improvements. Compilation, unit tests, functional equivalence,
refactoring, identifier normalization, and generated comments all require separate
measurement.

For Retonr, code and structured syntax should remain outside ordinary prose
rewriting. Protecting them is both a fidelity requirement and the right response to
their low watermark capacity.

### Short text

Short spans provide fewer trials and less payload capacity. A detector should return
insufficient evidence instead of lowering a threshold until a label appears.
Multi-bit decoding is especially difficult because the same limited evidence must
distinguish both presence and payload. XMark and related 2026 work improve this
regime, but do not eliminate the statistical limit.

### Long and mixed-source text

Long clean output accumulates evidence. Long documents also create repeated
contexts, many tested windows, and realistic mixtures of human, quoted, templated,
and model-generated spans. Global averaging can dilute a short mark. Local scans can
find it but need multiplicity-corrected calibration. A document-level positive does
not imply that every sentence was generated or even changed by the marked system.

Retonr's unit-level rewrite and document-atomic fidelity decision are compatible
with this evidence. Provenance records should identify source units and accepted
rewrite units without inferring authorship from any detector.

## Quality, capacity, and security trade-offs

The principal trade-offs are structural:

| Increase | Likely benefit | Likely cost or risk |
| --- | --- | --- |
| Logit bias or tournament depth | More evidence per eligible token | More distribution pressure or compute |
| Green-set stability | Edit and paraphrase survival | Easier black-box learning and spoofing |
| Context dependence | Key complexity and lower repeated bias | Edit propagation and reduced robustness |
| Semantic invariance | Survival under lexical paraphrase | Learned-model cost, representation error, piggyback risk |
| Payload bits | Finer attribution | More tokens, distortion, decoding error, privacy risk |
| Redundancy or error correction | Edit tolerance | Lower effective capacity and longer text |
| Rejection sampling candidates | Better fit to a target region | Latency, diversity loss, candidate quality variance |
| Detector span search | Dilution resistance and localization | Multiple-testing burden and compute |
| Public detector detail | Auditability and access | Oracle feedback, attack development, key leakage if symmetric |
| In-model learning | Persistence without a custom sampler | Fine-tuning removal, model utility risk, spoofing by distillation |

"Distortion-free" is also scope-sensitive. It may mean equality in distribution
over a random key, equality of each token marginal, preservation up to a generation
budget, or computational indistinguishability to an observer without the key. It
does not mean that every marked sample equals the unmarked sample that would have
been drawn, or that every user-preferred answer is unchanged.

Quality evaluation should include factuality, task success, entity and quantity
preservation, diversity, latency, and native human preference. Perplexity,
BERTScore, embedding similarity, and LLM judges are insufficient alone. This is
especially important where a mark can choose a fluent but wrong low-probability
token.

## Formal limitations

Two kinds of impossibility result set the professional boundary.

[Can AI-Generated Text be Reliably Detected?](https://arxiv.org/abs/2303.11156)
relates the best possible detector's ROC behavior to the total-variation distance
between human and model text distributions. If a high-quality paraphraser makes its
output distribution close to a relevant human distribution, a detector must accept
more false negatives, more false positives for that human group, or both. This
result concerns distributional detection and does not say that a known keyed mark
has no value on unedited output.

[Watermarks in the Sand](https://proceedings.mlr.press/v235/zhang24o.html) proves
that strong watermarking is impossible under two explicit assumptions: an attacker
has a quality oracle and a perturbation oracle whose random walk mixes over
high-quality outputs. The generic attacker does not need the private key or scheme.
The theorem does not prove that every practical attack is cheap today. It rules out
an unconditional promise that no efficient attacker can remove a mark without
quality loss when those assumptions hold.

The correct conclusion is bounded utility:

- Watermarks can increase the cost of casual removal and enable high-confidence
  source evidence in calibrated, cooperative ecosystems.
- They can help platform moderation, training-data hygiene, incident investigation,
  and provider accountability when combined with logs and provenance.
- They cannot force every generator to mark output, prevent use of unmarked local
  models, or certify negative results.
- They cannot establish sole human or model authorship after collaborative editing.
- Stronger persistence generally increases quality, capacity, privacy, or spoofing
  pressure somewhere else in the system.

## State of the art through August 12, 2026

The field has moved beyond a single green-list baseline, but results remain hard to
compare because threat models and operating points differ.

### Evidence with relatively high maturity

- KGW-style token partitioning has peer-reviewed theory, attacks, and an official
  implementation. Its strengths and weaknesses are well characterized.
- SynthID-Text has a peer-reviewed design, public reference code, a maintained
  Transformers integration, and unusually large production quality evidence.
- Exact or non-asymptotic calibration and repeated-context handling are established
  requirements, not optional refinements.
- Paraphrase, translation, dilution, watermark stealing, and spoofing are validated
  attack classes across multiple research groups.
- Low entropy, short length, multilingual tokenization, and mixed-source documents
  are distinct qualification regimes.

### Active 2025 and 2026 frontier

- WaterPark provides broader cross-scheme adversarial comparison.
- STA-1 improves unbiased low-entropy sampling risk.
- SimMark and learned paraphraser marks push robustness to sentence semantics.
- USENIX 2025 multi-bit work improves edit-bounded payload recovery.
- SEEK, DualGuard, and contrastive defenses treat spoofing as a first-class goal.
- XMark, MirrorMark, BREW, and DMW improve short-text payload accuracy,
  distribution preservation, verification structure, or explicit
  capacity-quality-robustness optimization.
- TextSeal combines distribution-preserving generation, multilingual human
  evaluation, mixed-document localization, and watermark radioactivity.
- SAFESEAL combines entity protection, key-conditioned semantic substitution, and
  key-conditioned detection.
- 2026 multilingual and code-oriented work broadens evaluation, but neither domain
  has a universal cross-model solution.

Most 2026 claims above are one-paper empirical frontiers. Independent reproduction,
adaptive red teaming, low-FPR calibration at scale, cross-language review, and
longitudinal deployment evidence are still limited. "State of the art" should mean
best reported result under a named benchmark and threat model, not solved problem.

### Current policy and provider context

The [European Commission's Code of Practice on Transparency of AI-generated
Content](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content)
addresses machine-readable marking and detection under Article 50. It explicitly
frames technical solutions as effective, interoperable, robust, and reliable as far
as technically feasible, taking content-specific limitations and state of the art
into account. The Commission's [2026 technical study](https://op.europa.eu/en/publication-detail/-/publication/6c981119-4829-11f1-8095-01aa75ed71a1/language-en)
evaluates watermarking, structural marking, metadata, logging, and generic
detection as complementary approaches.

Google publicly documents SynthID-Text limitations and production use. Anthropic's
[August 2026 help article](https://support.claude.com/en/articles/16266773-how-claude-marks-ai-generated-content)
states that supported Claude models embed text marks and that a detected mark is a
signal of processing, not full provenance. It also states that short text, heavy
editing, paraphrase, translation, mixing, older models, and unsupported surfaces can
produce no detectable mark. The public statement does not disclose enough of the
deployed algorithm, keys, detector calibration, or independent evaluation to infer
its behavior from KGW or SynthID-Text results.

Policy compliance and scientific certainty are separate. A layered system of
watermark, signed metadata, logs, labels, and editorial records can cover different
failure modes. No layer should be described as infallible.

## Implications for Retonr

### Product purpose and claims

Retonr should remain an editorial re-expression system, not a watermark removal
service. Its legitimate value is to help a user express authorized facts and ideas
in their own established style while preserving protected content and structure.

Public claims should stay within this language:

- Retonr reconstructs eligible prose rather than carrying all upstream wording
  forward.
- Reconstruction can reduce supported source-wording signals and document artifacts
  that remain in a copied or generated draft.
- The result may differ under a particular text detector because its tokens and
  sentences changed.
- Retonr does not identify, target, measure, or optimize for provider watermark
  detectors.
- Retonr cannot guarantee watermark removal, detector evasion, human classification,
  anonymity, or erasure of provider logs and signed provenance.

Avoid "clean," "undetectable," "de-watermarked," "humanized," "AI-proof," or
"guaranteed human" as product outcomes. Do not report a negative third-party
detector result as product proof.

### Architecture boundaries

The core rewrite path should not accept a watermark detector score as a candidate
objective or retry signal. Doing so would turn a bounded editorial system into an
adaptive detector oracle and would couple product behavior to proprietary,
uncalibrated, mutable classifiers.

Keep these interfaces separate:

1. **Source intake:** record the source type and any provenance information the user
   elects to retain.
2. **Editorial generation:** use the authorized style profile, explicit brief, and
   local model to propose complete rewrites of eligible units.
3. **Deterministic fidelity gates:** preserve source claims, entities, quantities,
   structure, formatting, and excluded spans within each declared check.
4. **Semantic assessment:** measure bounded fidelity properties without claiming a
   proof of meaning preservation.
5. **Rewrite record:** identify changed, unchanged, and abstained units plus model
   and profile identities, under local user control.

Do not add provider-key discovery, repeated detector queries, green-list estimation,
watermark-specific synonym tables, targeted translation loops, or detector-driven
candidate selection. Those mechanisms are unnecessary for the editorial product and
would materially change its purpose.

### User interaction

The guided editorial brief proposed for Retonr is aligned with the science. A few
high-information questions about purpose, audience, main point, stance, and protected
facts give the local generator positive editorial direction. They reduce dependence
on blind paraphrasing and make fidelity easier to evaluate.

Useful questions include:

- What is the one point the reader should retain?
- Is there a position, concern, or decision that must sound like yours?
- Who is the reader and what should they do next?
- Which facts, quotations, names, and technical terms must remain exact?
- Should the system preserve the current structure or propose a new one?

The user should not need to read every source draft before supplying this brief, but
they remain the final editor. A voice interface can collect the same small brief
locally. A versioned preference graph can retain explicit, revocable style feedback
over time without treating watermark or detector feedback as style evidence.

### Provenance and records

A legitimate editing history is more useful than a probabilistic authorship label.
Retonr should retain, at the user's option:

- Source hash and local source identity
- Source provenance metadata if present, without claiming it is complete
- Exact local model artifact and runtime identity
- Style-profile and editorial-brief versions
- Protected-span inventory
- Unit-level acceptance, abstention, and validation results
- Output hash and timestamp

Records should be local, redacted by default, exportable, and deletable under the
product's governance rules. They should not expose original text through hashes of
short or guessable spans. A Retonr record explains the transformation process; it is
not a certificate of human authorship.

Retonr should not strip signed provenance or file metadata silently. Format adapters
should preserve supported metadata when fidelity policy requires it, expose any
metadata loss in the change report, and write to a new destination. Linguistic
rewriting and metadata preservation are independent decisions.

### Evaluation

Watermark detectors may be useful in a research-only observability suite to learn
whether ordinary editorial transformations incidentally change known public marks.
They should never be release objectives or acceptance gates. If such evaluation is
performed, it should:

- Use only public schemes, public keys created for the experiment, or provider tools
  used under their terms
- Keep a locked, non-adaptive evaluation set separate from generation
- Report the full source and output score distributions, not only pass rates
- Include positive, negative, short, long, mixed, repeated, multilingual, code, and
  translated controls
- Calibrate per detector and correct for scanned spans and multiple keys
- Measure fidelity and task quality independently
- Treat provider API verdicts as mutable external evidence
- Avoid publishing operational key-recovery or evasion recipes

The release decision should continue to depend on fidelity, format, security,
privacy, and resource evidence. A detector result must not compensate for a fidelity
failure or cause a candidate to be preferred.

### Security and abuse controls

Retonr does not need to inspect a user's motives to maintain a clear product
boundary. It can make misuse less attractive by design:

- No detector-score endpoint in the product
- No watermark removal or evasion mode
- No iterative optimization against external classifiers
- No claim that translation, paraphrase, or a local model defeats a provider mark
- No silent provenance stripping
- No bulk account-attribution or identity-payload extraction
- Clear logs of transformations without first-party content telemetry

These boundaries still allow legitimate editing of rough, delegated, generated, and
owner-authored drafts.

## Research and release gates

Before any watermark-related feature or claim enters a Retonr release, require:

- A named user need that is editorial or provenance-oriented, not evasion-oriented
- A threat model covering ordinary editing, adaptive scrubbing, spoofing, and
  detector-oracle access
- Exact scheme, implementation revision, key policy, tokenizer, and detector
  identity
- Independent negative calibration at the intended operational FPR
- Separate results for each supported language, content class, and length band
- Quality and fidelity evaluation independent of watermark metrics
- Mixed-source and multiple-testing calibration
- A public interpretation contract with positive, negative, uncertain, and
  insufficient-evidence meanings
- Security review of key exposure, query leakage, payload privacy, rotation, and
  revocation
- Legal and policy review for any provider-specific detection integration

No current product requirement justifies shipping a watermark detector in Retonr.
The immediate use of this research is to constrain claims, protect provenance, and
keep the editorial architecture independent from detector optimization.

## Primary source index

### Foundations, sampling, and detection

- [A Watermark for Large Language Models](https://proceedings.mlr.press/v202/kirchenbauer23a.html)
- [Official KGW implementation](https://github.com/jwkirchenbauer/lm-watermarking)
- [On the Reliability of Watermarks for Large Language Models](https://proceedings.iclr.cc/paper_files/paper/2024/hash/d78e9e4316e1714fbb0f20be66f8044c-Abstract-Conference.html)
- [Robust Distortion-free Watermarks for Language Models](https://arxiv.org/abs/2307.15593)
- [Undetectable Watermarks for Language Models](https://proceedings.mlr.press/v247/christ24a.html)
- [Unbiased Watermark for Large Language Models](https://proceedings.iclr.cc/paper_files/paper/2024/hash/c5b00c5bdcc6fe35907dbcca03d27652-Abstract-Conference.html)
- [Three Bricks to Consolidate Watermarks](https://arxiv.org/abs/2308.00113)
- [A Statistical Framework of Watermarks](https://arxiv.org/abs/2404.01245)
- [Watermarking Language Models with Error Correcting Codes](https://arxiv.org/abs/2406.10281)
- [STA-1](https://aclanthology.org/2025.acl-long.391/)

### SynthID-Text

- [Scalable Watermarking for Identifying Large Language Model Outputs](https://www.nature.com/articles/s41586-024-08025-4)
- [Google DeepMind reference implementation](https://github.com/google-deepmind/synthid-text)
- [Google Responsible Generative AI Toolkit guidance](https://ai.google.dev/responsible/docs/safeguards/synthid)
- [Hugging Face Transformers implementation](https://huggingface.co/docs/transformers/internal/generation_utils)

### Semantic and post-hoc methods

- [SemStamp](https://aclanthology.org/2024.naacl-long.226/)
- [SemStamp official implementation](https://github.com/abehou/SemStamp)
- [A Semantic Invariant Robust Watermark](https://proceedings.iclr.cc/paper_files/paper/2024/hash/1a2131ebe25bd55e4fc734126ea583ed-Abstract-Conference.html)
- [PostMark](https://aclanthology.org/2024.emnlp-main.506/)
- [SimMark](https://aclanthology.org/2025.emnlp-main.1567/)
- [Robust Multi-bit Text Watermark with LLM-based Paraphrasers](https://proceedings.mlr.press/v267/xu25k.html)
- [TextSeal](https://arxiv.org/abs/2605.12456)
- [Meta Seal official repository](https://github.com/facebookresearch/meta-seal)
- [SAFESEAL](https://arxiv.org/abs/2605.23175)

### Multi-bit and public verification

- [Advancing Beyond Identification](https://aclanthology.org/2024.naacl-long.224/)
- [Towards Codable Watermarking](https://proceedings.iclr.cc/paper_files/paper/2024/hash/abdc8c031aa6c6917c3b593166e5e340-Abstract-Conference.html)
- [Publicly-Detectable Watermarking for Language Models](https://eprint.iacr.org/2023/1661)
- [An Unforgeable Publicly Verifiable Watermark](https://proceedings.iclr.cc/paper_files/paper/2024/hash/214d2cffc381938be6f7254d5382904f-Abstract-Conference.html)
- [Provably Robust Multi-bit Watermarking](https://www.usenix.org/conference/usenixsecurity25/presentation/qu-watermarking)
- [XMark](https://aclanthology.org/2026.acl-long.672/)
- [MirrorMark](https://arxiv.org/abs/2601.22246)
- [BREW](https://arxiv.org/abs/2605.00348)
- [Distortion-minimization Watermarking](https://www.usenix.org/conference/usenixsecurity26/presentation/zhai)

### Attacks, limits, and evaluation

- [Can AI-Generated Text be Reliably Detected?](https://arxiv.org/abs/2303.11156)
- [Watermarks in the Sand](https://proceedings.mlr.press/v235/zhang24o.html)
- [Watermark Stealing in Large Language Models](https://proceedings.mlr.press/v235/jovanovic24a.html)
- [On the Learnability of Watermarks](https://proceedings.iclr.cc/paper_files/paper/2024/hash/a86d17b6cd70366d56ab48d2a05a4df1-Abstract-Conference.html)
- [Revisiting Robustness to Paraphrasing](https://aclanthology.org/2024.emnlp-main.1005/)
- [Is the Watermarking of LLM-Generated Code Robust?](https://arxiv.org/abs/2403.17983)
- [Lost in Overlap](https://aclanthology.org/2025.findings-naacl.37/)
- [WaterPark](https://aclanthology.org/2025.findings-emnlp.1148/)
- [Cross-lingual Manipulations](https://aclanthology.org/2025.findings-emnlp.390/)
- [Watermark Smoothing Attacks](https://aclanthology.org/2025.findings-emnlp.264/)
- [DITTO](https://arxiv.org/abs/2510.10987)
- [Efficiently Identifying Watermarked Segments](https://aclanthology.org/2025.acl-long.316/)

### Official policy and deployment context

- [EU Code of Practice on Transparency of AI-generated Content](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content)
- [EU technical study on marking and detecting AI-generated text](https://op.europa.eu/en/publication-detail/-/publication/6c981119-4829-11f1-8095-01aa75ed71a1/language-en)
- [How Claude marks AI-generated content](https://support.claude.com/en/articles/16266773-how-claude-marks-ai-generated-content)
