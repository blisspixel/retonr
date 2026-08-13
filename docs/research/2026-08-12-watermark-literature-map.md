# Text watermarking literature map

## Review status

Reviewed: August 12, 2026.

Scope: text produced by autoregressive and diffusion language models, with emphasis
on output provenance. Model ownership marks that require probing a suspect model,
training-data marks, document metadata, and literal Unicode markers appear only
where they clarify a boundary. Image, audio, and video watermark results are not
treated as evidence about text.

This is an evidence map, not a deployment claim. It records what each source tests,
which assumptions its result needs, and which comparisons are invalid. It does not
provide an operational recipe for defeating a watermark. Links to code use reviewed
commit snapshots where a public repository was identifiable.

## Evidence labels

Every substantive entry uses one or more of these labels:

- **Peer reviewed**: accepted conference or journal paper.
- **Preprint**: public manuscript without a verified peer-reviewed disposition at
  this review date.
- **Provider statement**: a claim made by a model or service provider, not an
  independent result.
- **Official implementation**: code identified by the authors or publisher as the
  implementation of the cited work.
- **Independent reproduction**: a separate group evaluates or reimplements a named
  method.
- **Inference**: a conclusion derived from cited evidence but not directly tested by
  that source.
- **Unknown**: evidence needed for the field is not public or was not located.

A paper can be peer reviewed while a particular claim in it remains author-reported.
Peer review is not independent reproduction. An open implementation is not proof
that a closed provider deployment uses the same configuration.

## Executive conclusions

1. There is no single text-watermark mechanism. Major families include token-bias
   marks, keyed sampling marks, distribution-preserving constructions, semantic and
   syntactic marks, post-hoc rewriting, weight-level marks, and multi-bit encodings.
   Their security definitions and required access differ enough that a single
   leaderboard is usually misleading.
2. A keyed verifier tests for one known signal. It does not determine whether text
   was written by AI in general. A style classifier tests correlations in prose. It
   is not a watermark detector. A metadata or Unicode inspector reports literal
   artifacts. These outputs must never be collapsed into one confidence score.
3. Watermark strength depends on entropy, length, tokenizer, decoding policy,
   language, content type, repeated text, and the detector's multiple-testing
   behavior. An operating point without all of those conditions is not portable.
4. Claims such as unbiased, distortion-free, undetectable, or quality preserving
   refer to different mathematical objects. Some average over random keys or
   messages, some hold only within a generation budget, and some are computational
   indistinguishability claims. None alone proves that every produced passage is the
   best text the unmodified model would have emitted.
5. The strongest empirical attack literature shows a two-sided attribution risk.
   Signals can be weakened, but some schemes can also be learned, collided, stolen,
   or spoofed. A positive result therefore cannot be treated as conclusive authorship
   or provider attribution without a calibrated threat model and corroborating
   evidence.
6. Semantic, structural, and multi-bit work extends the design space but does not
   remove the quality, latency, public-verification, key-management, and attack
   tradeoffs. Broad claims of paraphrase robustness usually cover a named set of
   paraphrasers and semantic metrics, not all meaning-preserving rewrites.
7. Strong watermarking is impossible under the explicit oracle and mixing
   assumptions of the ICML 2024 impossibility result. That result does not say weak
   watermarks are useless. It does rule out presenting practical text watermarking
   as unconditionally robust provenance.
8. The evidence supports an instrumented, humble research program: preserve raw
   observations, separate deterministic from probabilistic checks, publish
   calibration and failure cases, and say unknown when the provider key or detector
   is unavailable.

## Taxonomy and access model

### Objects that are often confused

| Object | Embedded signal | Verification authority | Bounded claim |
| --- | --- | --- | --- |
| Keyed generation watermark | Statistical pattern introduced during decoding | Holder of a detector key or configured detector | Compatibility with one known watermark family |
| Publicly verifiable watermark | Statistical or cryptographic pattern | Anyone with public parameters or a public key | Verification under the scheme's security model |
| Post-hoc text watermark | Lexical, semantic, or structural changes after generation | Scheme-specific detector | Compatibility with the post-hoc transformation |
| Weight-level watermark | Controlled structure in model parameters or behavior | Key holder, often with model or logit access | Evidence about a marked model or its outputs under stated access |
| Multi-bit watermark | Encoded payload rather than one presence bit | Keyed or public decoder | Recovery of a bounded message under stated noise |
| Metadata provenance | Signed or unsigned record outside the text stream | Manifest verifier or file inspector | Statement carried by the file or service, not linguistic evidence |
| Literal text artifact | Unicode controls, homoglyphs, spacing, or formatting | Any deterministic scanner | Presence of the exact artifact only |
| Style-pattern diagnostic | Measured lexical, syntactic, or discourse feature | Any implementation of the rule or classifier | Resemblance to a defined style pattern, not provenance |
| Post-hoc AI classifier | Learned correlation between corpora and labels | Classifier operator | Model-specific classification under its evaluation distribution |

### Mechanism families

- Token-bias methods alter next-token probabilities to favor a keyed subset and
  aggregate favored-token evidence.
- Keyed sampling methods couple pseudorandom values to the sampling operation and
  later align candidate tokens with the key sequence.
- Distribution-preserving methods seek equality, equality in expectation, or
  computational indistinguishability between marked and base distributions.
- Semantic, syntactic, and topic methods encode signals in sentence embeddings,
  abstract meaning structures, linguistic degrees of freedom, or topic-conditioned
  token groups.
- Post-hoc methods rewrite completed text without needing the original logits.
- In-model methods alter parameters or train behavior so that a signal is emitted
  without a separate logits processor.
- Multi-bit methods recover a payload such as a model, tenant, or transaction
  identifier. Their error model and privacy consequences exceed zero-bit detection.
- Localization methods scan for marked spans inside mixed-source documents. Their
  repeated search creates a different calibration problem from whole-document
  testing.

## Systematic review method

The map prioritizes proceedings pages, journal pages, arXiv version snapshots,
author repositories, provider documentation, and independent benchmark papers.
Inclusion required at least one of the following:

- a foundational mechanism or formal security definition;
- a peer-reviewed advance in robustness, quality, access, capacity, or scope;
- a public provider implementation or deployment;
- an independent benchmark, reproduction, or contradiction;
- an attack or impossibility result that changes the supported claim boundary; or
- a 2025 or 2026 result that materially expands localization, multilingual, code,
  public-verification, in-model, or multi-bit coverage.

For each work, the recorded operating point is the narrowest one available from the
primary source. If a paper reports only curves, averages, or an author-selected
threshold, this map says so. Missing fields are marked unknown rather than inferred.

The taxonomy was cross-checked against **Peer reviewed** systematization in
[SoK: Watermarking for AI-Generated Content](https://doi.org/10.1109/SP61157.2025.00178),
IEEE Symposium on Security and Privacy 2025, and the **Peer reviewed** survey
[A Survey of Text Watermarking in the Era of Large Language
Models](https://doi.org/10.1145/3691626), ACM Computing Surveys 2024. Those surveys
are routing sources. Mechanism and operating-point claims below cite the original
work or a named independent evaluation.

## Foundational token-bias and calibration work

### KGW: A Watermark for Large Language Models

**Evidence:** **Peer reviewed**, ICML 2023. [Proceedings and
paper](https://proceedings.mlr.press/v202/kirchenbauer23a.html).
[Official implementation snapshot](https://github.com/jwkirchenbauer/lm-watermarking/tree/82922516930c02f8aa322765defdb5863d07a00e),
**Official implementation**.

- **Mechanism and access:** a keyed pseudorandom green list is derived from recent
  tokens and its logits receive a positive bias. Embedding needs next-token logits.
- **Detector and key:** a key-only detector reconstructs green lists and uses a
  favored-token count, commonly summarized with a one-sided z statistic. The base
  model API and prompt are not required for the basic detector.
- **Claimed operating point:** the paper demonstrates short-span detection on OPT
  models and reports examples below 25 tokens at nominal false-positive probability
  below `1e-5`. That is not a universal minimum length.
- **Scope and quality:** English open-ended generation on OPT-family models;
  perplexity and human inspection are prominent quality measures. Code, translation,
  constrained factual responses, and current instruction models were not the core
  validation set.
- **Attack and reproduction:** later peer-reviewed work tests paraphrase, mixing,
  stealing, spoofing, collision, and generic removal. Multiple independent toolkits
  implement variants, but differences in hashing, context width, repeated-token
  handling, and thresholds make the variants non-identical.
- **Bounded conclusion and gap:** KGW established the modern token-bias baseline and
  an interpretable null test. It did not establish universal quality neutrality,
  secure source attribution, or calibrated scanning across arbitrary domains.

### Three Bricks to Consolidate Watermarks

**Evidence:** **Peer reviewed**, IEEE WIFS 2023, DOI
[10.1109/WIFS58808.2023.10374576](https://doi.org/10.1109/WIFS58808.2023.10374576).
[Paper snapshot](https://arxiv.org/abs/2308.00113v1).
[Official implementation snapshot](https://github.com/facebookresearch/three_bricks/tree/d16ad0666e1a2349c6cd7d229eb6e02d86f31155),
**Official implementation**.

- **Mechanism and access:** strengthens statistical testing for token-level marks,
  evaluates task utility, and extends zero-bit scoring toward multi-bit messages.
- **Detector and key:** keyed detector; proposes exact or more reliable tests and
  repeated-token handling instead of assuming a normal approximation is calibrated.
- **Claimed operating point:** studies nominal false-positive rates below `1e-6` and
  shows that naive z-test p-values can understate empirical false positives on large
  natural-text trials.
- **Scope and quality:** Wikipedia null text and classical NLP tasks supplement
  perplexity. Language and modern instruction-following breadth remain limited.
- **Attack and reproduction:** public code exists. The calibration criticism is
  consistent with the broader statistical literature, but exact behavior depends on
  token dependence and preprocessing.
- **Bounded conclusion and gap:** nominal p-values are not enough. Null calibration
  must be validated on the deployed domain, tokenizer, preprocessing, and scan
  policy.

### On the Reliability of Watermarks for Large Language Models

**Evidence:** **Peer reviewed**, ICLR 2024. [Proceedings and
paper](https://proceedings.iclr.cc/paper_files/paper/2024/hash/d78e9e4316e1714fbb0f20be66f8044c-Abstract-Conference.html).
The KGW code snapshot above covers this work, **Official implementation**.

- **Mechanism and access:** studies improved KGW-style detection after human or
  machine rewriting and when marked text is embedded in a longer document.
- **Detector and key:** keyed whole-text and windowed detectors. Some window
  procedures search many candidate spans and therefore need search-aware thresholds.
- **Claimed operating point:** after strong human paraphrasing, the paper reports an
  average of about 800 observed tokens for detection at nominal false-positive rate
  `1e-5` in its experiment.
- **Scope and quality:** English passages, human rewrites, model paraphrases, and
  mixed documents. Meaning retention and leakage of original n-grams are central to
  the observed robustness.
- **Attack and reproduction:** later DIPPER, generic-removal, stealing, and collision
  work demonstrates stronger or different attack models. Results are not direct
  contradictions because edit budgets, paraphrasers, detector knowledge, and text
  lengths differ.
- **Bounded conclusion and gap:** some marks survive realistic edits when enough
  original signal remains. This does not support a fixed detection length or
  robustness claim for arbitrary rewriting.

### Unigram-Watermark

**Evidence:** **Peer reviewed**, ICLR 2024. [Paper and
metadata](https://mlanthology.org/iclr/2024/zhao2024iclr-provable/).
[Official implementation snapshot](https://github.com/XuandongZhao/Unigram-Watermark/tree/b96cdb4d52771e3cbd543a9d9aeeaec8d0790ca2),
**Official implementation**.

- **Mechanism and access:** uses one fixed keyed vocabulary partition rather than a
  context-dependent green list. Embedding requires logits.
- **Detector and key:** key-only favored-token test; prompt and base-model API are
  not required for the basic detector.
- **Claimed operating point:** the paper reports detection and edit-robustness curves
  across three models and two datasets, not one portable deployment threshold.
- **Scope and quality:** generation quality is primarily bounded and measured through
  perplexity and task data. The fixed partition changes the security and collision
  surface relative to context-dependent KGW.
- **Attack and reproduction:** evaluated by later mixed-source, generic-removal,
  WaterBench, WaterPark, and collision studies.
- **Bounded conclusion and gap:** a fixed partition can improve robustness under the
  paper's edit model. Public evidence does not establish resistance to key learning,
  provider-to-provider collision, or all language and task distributions.

### Statistical framework for pivots and optimal rules

**Evidence:** **Peer reviewed**, The Annals of Statistics 2025. [Journal
article](https://doi.org/10.1214/24-AOS2468).

- **Mechanism and access:** provides a hypothesis-testing framework based on pivotal
  statistics and optimizes detector power for representative watermark families.
- **Detector and key:** assumes a provider-supplied secret key and a null statistic
  whose distribution can be controlled.
- **Claimed operating point:** derives asymptotic false-negative exponents and
  optimized rules rather than a universal production threshold.
- **Scope and quality:** statistical efficiency, not broad linguistic quality, is
  the principal object. Numerical studies support the theory under the modeled null.
- **Attack and reproduction:** theory clarifies calibration but does not cover every
  dependence, domain shift, key compromise, or adaptive transformation.
- **Bounded conclusion and gap:** a valid pivot can control false positives under its
  assumptions. A detector must still show that deployed text and preprocessing meet
  those assumptions.

## Distribution-preserving, cryptographic, and in-model work

### Robust Distortion-Free Watermarks

**Evidence:** **Peer reviewed**, TMLR 2024. [Paper and
metadata](https://openreview.net/forum?id=FpaCL1MO2C).
[Official implementation snapshot](https://github.com/jthickstun/watermark/tree/80d4ec8f4280da2a2cada03adfc8940593d1964c),
**Official implementation**.

- **Mechanism and access:** inverse-transform or exponential-minimum sampling maps a
  randomized key sequence to model samples. Generation needs the token distribution.
- **Detector and key:** the verifier aligns candidate tokens to a secret random
  sequence. Model access can improve some instantiations but the basic result centers
  on the key sequence.
- **Claimed operating point:** for OPT-1.3B and LLaMA-7B, the paper reports
  `p <= 0.01` from 35 tokens after 40 to 50 percent random token edits. For Alpaca-7B
  responses near 100 median tokens, only about 25 percent were detected at that
  threshold.
- **Scope and quality:** English C4 and instruction case studies. Distribution
  preservation holds under the paper's randomized-key construction and bounded
  generation budget, not as a per-key claim for unlimited adaptive queries.
- **Attack and reproduction:** later generic-removal and reverse-engineering work
  attacks this family. Public code allows reproduction but pins old model and
  dependency assumptions.
- **Bounded conclusion and gap:** exact sampling can avoid the direct logit bias of
  KGW while retaining statistical power. Low-entropy instruction responses and
  repeated-query security remain material limitations.

### Undetectable Watermarks for Language Models

**Evidence:** **Peer reviewed**, COLT 2024. [Proceedings and
paper](https://proceedings.mlr.press/v247/christ24a.html).

- **Mechanism and access:** a cryptographic construction based on one-way functions
  makes marked and base outputs computationally indistinguishable to parties without
  the secret key, including adaptive query users.
- **Detector and key:** private-key detection; the formal construction assumes the
  cryptographic and language-model conditions stated in the paper.
- **Claimed operating point:** this is primarily a theoretical existence and
  security result, not a production-scale benchmark with a single text-length point.
- **Scope and quality:** quality is addressed through distributional
  indistinguishability, not human evaluation across deployed tasks or languages.
- **Attack and reproduction:** no broad independent production reproduction was
  located. Practical efficiency and integration evidence are limited.
- **Bounded conclusion and gap:** a watermark need not be statistically visible to a
  keyless observer under cryptographic assumptions. This does not imply robustness
  to arbitrary meaning-preserving edits or practical deployability.

### DiPMark

**Evidence:** **Peer reviewed**, ICML 2024. [Paper and
metadata](https://openreview.net/forum?id=c8qWiNiqRY).
[Official implementation snapshot](https://github.com/yihwu/DiPmark/tree/34abbeb527243c79bda8043313bb797a731f4ae7),
**Official implementation**.

- **Mechanism and access:** samples a random token subset and applies a
  distribution-preserving reweighting function during decoding. Embedding requires
  token probabilities.
- **Detector and key:** accessible key-only detection without the prompt or model
  API, under the published configuration.
- **Claimed operating point:** the paper reports detectability and token-edit curves
  across models and tasks, not a stable provider-independent threshold.
- **Scope and quality:** distribution preservation is a mathematical property of the
  randomized procedure. Empirical quality uses automatic task and text measures.
- **Attack and reproduction:** public code exists. Later work on breaking
  distribution-preserving marks and UWbench challenges repeated-generation and
  robustness interpretations.
- **Bounded conclusion and gap:** marginal distribution preservation is stronger
  quality evidence than perplexity alone, but does not settle multi-query drift,
  implementation deviations, or adversarial attribution.

### SynthID-Text

**Evidence:** **Peer reviewed**, Nature 2024. [Journal
article](https://www.nature.com/articles/s41586-024-08025-4).
[Official implementation snapshot](https://github.com/google-deepmind/synthid-text/tree/addb4a158143c7c6851a1308f78b89fceed59683),
**Official implementation**. [Developer limitations and detector
choices](https://ai.google.dev/responsible/docs/safeguards/synthid), **Provider
statement**.

- **Mechanism and access:** combines multiple keyed tournament sampling layers with
  a learned Bayesian detector. Embedding needs logits and can integrate with
  speculative sampling.
- **Detector and key:** the reference detector is trained on marked and unmarked
  examples for a chosen watermark configuration. Google documents private,
  API-mediated, and public detector deployment choices.
- **Claimed operating point:** the paper reports true-positive rates at fixed
  empirical false-positive rates across temperatures and tasks. It also reports
  about 0.57 percent sampling latency overhead in one Gemma 7B-IT setup and a blind
  live quality experiment covering nearly 20 million Gemini responses.
- **Scope and quality:** multiple models, tasks, temperatures, automatic capability
  tests, human side-by-side ratings, and production feedback. The provider explicitly
  says factual low-entropy answers offer less embedding opportunity and thorough
  rewriting or translation can greatly reduce confidence.
- **Attack and reproduction:** the code release supports independent study. A 2026
  preprint below independently analyzes the scheme, but no public evidence proves
  that every Gemini or other provider configuration equals the reference release.
- **Bounded conclusion and gap:** SynthID is the strongest public evidence of
  production-scale text watermarking without detected aggregate quality loss in the
  tested setup. Failure to detect aggregate loss is not proof of identical quality
  for every passage, language, or task.

### STA-1

**Evidence:** **Peer reviewed**, ACL 2025. [Proceedings and
paper](https://aclanthology.org/2025.acl-long.391/).

- **Mechanism and access:** an accept-or-resample procedure designed to preserve the
  original token distribution in expectation and reduce low-entropy failure risk.
- **Detector and key:** no prompt or white-box detector access is required after
  generation under the paper's scheme.
- **Claimed operating point:** the paper compares detectability, latency, robustness,
  and low-entropy risk across datasets, but does not define one portable threshold.
- **Scope and quality:** low- and high-entropy English tasks, with distributional
  analysis and standard text-quality measures.
- **Attack and reproduction:** authors report code, but an immutable author snapshot
  was not identified in this review. Independent UWbench work studies the broader
  unbiased family.
- **Bounded conclusion and gap:** STA-1 directly addresses an important low-entropy
  weakness. It does not establish unlimited-query distribution equality or
  multilingual and code fidelity.

### GaussMark

**Evidence:** **Peer reviewed**, ICML 2025. [Proceedings and
paper](https://proceedings.mlr.press/v267/block25a.html).

- **Mechanism and access:** adds controlled Gaussian perturbations to model weights
  and uses Gaussian independence testing, moving the mark from each decoding step to
  model structure.
- **Detector and key:** provider-held secret structure; verification uses the
  scheme's statistical test and may require model-specific probability access.
- **Claimed operating point:** formal validity and power bounds plus empirical curves
  are reported. No single cross-model text-length threshold is portable.
- **Scope and quality:** open models and task benchmarks; reports essentially no
  quality loss within its tested measurements and no generation-time overhead.
- **Attack and reproduction:** no independent end-to-end reproduction was located in
  this review. Fine-tuning, quantization, model merging, extraction, and broad
  multilingual behavior remain model-transformation questions.
- **Bounded conclusion and gap:** weight-level marking may avoid per-request sampling
  cost. It requires control over model weights and does not by itself make output
  attribution public or unforgeable.

### Distribution-adaptive watermarking

**Evidence:** **Peer reviewed**, NeurIPS 2025. [Paper and
metadata](https://openreview.net/forum?id=CMmKcHFDKL).

- **Mechanism and access:** jointly reasons about the watermark and detector, using a
  surrogate distribution to produce a distortion-free, distribution-adaptive scheme.
- **Detector and key:** scheme-specific detector; the surrogate and key assumptions
  matter to both efficiency and model agnosticism.
- **Claimed operating point:** author-reported comparisons span detectability,
  robustness, and distortion rather than one deployment threshold.
- **Scope and quality:** open-model experiments and distributional analysis; public
  evidence for closed providers, many languages, and code is incomplete.
- **Attack and reproduction:** no independent reproduction located at review time.
- **Bounded conclusion and gap:** joint detector design can improve the evaluated
  frontier. The result cannot be compared directly with a keyed sampler unless the
  same surrogate, key exposure, threshold, and attack budget are used.

## Semantic, syntactic, topic, and post-hoc work

### SIR semantic-invariant watermark

**Evidence:** **Peer reviewed**, ICLR 2024. [Proceedings and
paper](https://proceedings.iclr.cc/paper_files/paper/2024/hash/1a2131ebe25bd55e4fc734126ea583ed-Abstract-Conference.html).
[Official implementation snapshot](https://github.com/THU-BPM/Robust_Watermark/tree/6d43991530cb670bc3129792d3839a2482932002),
**Official implementation**.

- **Mechanism and access:** derives watermark logits from semantic embeddings of the
  preceding context through a trained transform, rather than only a short token
  prefix. Embedding requires logits plus the semantic model.
- **Detector and key:** a keyed semantic-context detector with trained embedding and
  transform components.
- **Claimed operating point:** reports detection, synonym-substitution, paraphrase,
  and security-robustness curves rather than one portable threshold.
- **Scope and quality:** selected open models and English C4-style text with
  automatic semantic and generation-quality measures.
- **Attack and reproduction:** public code and later integration in MarkLLM provide
  reproducible baselines. WaterPark and collision work broaden the attack model.
- **Bounded conclusion and gap:** semantic context can reduce the short-context
  robustness-security tradeoff in the tested setting. The auxiliary model adds
  domain, language, latency, and version dependencies, and semantic similarity is
  not a formal fidelity guarantee.

### SemStamp and k-SemStamp

**Evidence:** **Peer reviewed**, NAACL 2024 and Findings of ACL 2024.
[SemStamp](https://aclanthology.org/2024.naacl-long.226/) and
[k-SemStamp](https://aclanthology.org/2024.findings-acl.98/).
[Official implementation snapshot](https://github.com/abehou/SemStamp/tree/97db73d11fd80f376a02b0a604d500627622f7e6),
**Official implementation**.

- **Mechanism and access:** rejection-samples complete sentences into keyed regions
  of an embedding space. SemStamp uses locality-sensitive hashing; k-SemStamp uses
  learned clusters.
- **Detector and key:** sentence encoder plus keyed transition rule and a statistical
  test. Generation needs repeated candidate sampling but not direct logit editing.
- **Claimed operating point:** papers report AUC and robustness across paraphrasers
  and domains, not a universal minimum sentence count.
- **Scope and quality:** English C4 and BookSum-style text, sentence-level semantic
  encoders, automated and human quality checks. The official code notes hardware and
  random-seed sensitivity.
- **Attack and reproduction:** no independent broad-language reproduction was
  located. Robustness is relative to named embedding models and paraphrasers.
- **Bounded conclusion and gap:** sentence-level marks can preserve signal through
  attacks that destroy token overlap. They introduce rejection cost and dependence
  on the semantic encoder's domain, language, and collision behavior.

### PostMark

**Evidence:** **Peer reviewed**, EMNLP 2024. [Proceedings and
paper](https://aclanthology.org/2024.emnlp-main.506/).

- **Mechanism and access:** after generation, an input-dependent set of words derived
  from semantic embeddings is inserted through controlled rewriting.
- **Detector and key:** black-box with respect to the source LLM; the detector relies
  on the post-hoc scheme and its embedding model.
- **Claimed operating point:** the paper reports robustness and quality curves,
  including human assessment, rather than a provider-independent threshold.
- **Scope and quality:** English long-form text. Quality and paraphrase robustness
  exhibit an explicit tradeoff.
- **Attack and reproduction:** no independent reproduction covering later frontier
  paraphrasers was located.
- **Bounded conclusion and gap:** post-hoc marking permits third-party use without
  logits. It necessarily edits completed text and therefore needs stronger semantic,
  factual, quotation, and formatting-fidelity evaluation than a generation sampler.

### STELA

**Evidence:** **Peer reviewed**, ACL 2026. [Proceedings and
paper](https://aclanthology.org/2026.acl-long.2115/).
[Preprint snapshot and code link](https://arxiv.org/abs/2510.13829v1).

- **Mechanism and access:** modulates token-bias strength using part-of-speech
  n-gram estimates of syntactic predictability, weakening the signal where language
  is constrained.
- **Detector and key:** detector avoids source-model logits, using public linguistic
  statistics and scheme parameters. Embedding still needs generation-time control.
- **Claimed operating point:** cross-method detection and robustness averages are
  reported, not one portable threshold.
- **Scope and quality:** English, Chinese, and Korean, selected to span analytic,
  isolating, and agglutinative structures.
- **Attack and reproduction:** author code is linked from the paper; an immutable
  snapshot was not captured in this review. Independent replication is unknown.
- **Bounded conclusion and gap:** linguistic freedom is a relevant alternative to
  model entropy for public detection. Three languages do not establish general
  multilingual calibration or dialect fairness.

### SWAN

**Evidence:** **Peer reviewed**, ACL 2026. [Proceedings and
paper](https://aclanthology.org/2026.acl-long.1681/).

- **Mechanism and access:** prompts generation toward selected Abstract Meaning
  Representation structures and detects the resulting semantic-structure pattern.
- **Detector and key:** uses an off-the-shelf AMR parser and a one-proportion test;
  generation does not require model training.
- **Claimed operating point:** on RealNews, the paper reports up to 13.9 percentage
  points AUC improvement over prior methods under tested paraphrasing.
- **Scope and quality:** English news-style text and AMR coverage. Meaning retention
  and parser correctness are central dependencies.
- **Attack and reproduction:** independent reproduction and non-English AMR evidence
  are unknown.
- **Bounded conclusion and gap:** semantic structure can outlast lexical changes in
  the tested setting. The claim is bounded by parser errors, representable meanings,
  generation cost, and the possibility that valid paraphrases change AMR structure.

### Topic-based watermarking

**Evidence:** **Peer reviewed**, Findings of ACL 2026. [Proceedings and
paper](https://aclanthology.org/2026.findings-acl.1220/).

- **Mechanism and access:** selects a prompt-relevant topic vocabulary and biases
  generation toward semantically aligned members.
- **Detector and key:** scheme-specific topic and favored-token detector; embedding
  needs generation control.
- **Claimed operating point:** author-reported robustness improves under selected
  paraphrase and lexical perturbation tests; no universal threshold is given.
- **Scope and quality:** multiple LLMs and text benchmarks, with fluency and coherence
  measures. Topic classification and domain shift are added dependencies.
- **Attack and reproduction:** independent reproduction is unknown.
- **Bounded conclusion and gap:** topic alignment may spend watermark strength on
  contextually plausible words. It can also alter topical emphasis, and the evidence
  does not establish neutrality for precise, legal, scientific, or quoted text.

### TextSeal and post-hoc rephrasing study

**Evidence:** **Preprint**, December 2025. [Provider research
page](https://ai.meta.com/research/publications/how-good-is-post-hoc-watermarking-with-language-model-rephrasing/),
**Provider statement**. [Meta Seal repository
snapshot](https://github.com/facebookresearch/meta-seal/tree/e7e2c9176fcd316d7aaf36c3006eb417ab4b043e),
**Official implementation**.

- **Mechanism and access:** evaluates compute-scaled post-hoc rephrasing, candidate
  search, and entropy-aware detection as a watermarking pipeline.
- **Detector and key:** depends on the chosen post-hoc transformation and detector;
  no source-model logits are required.
- **Claimed operating point:** reports quality-detectability frontiers, not one
  threshold. This source is not the same evidentiary object as the May 2026 preprint
  also named TextSeal that studies localized generation-time marking.
- **Scope and quality:** model-based quality comparisons and compute scaling. Exact
  closed-model behavior can change independently of the paper.
- **Attack and reproduction:** code is public; independent peer-reviewed
  reproduction was not located.
- **Bounded conclusion and gap:** more post-hoc compute can improve a measured
  frontier. It still changes text and must be tested against task-specific fidelity,
  not only generic similarity.

## Multi-bit and public-verification work

### MPAC

**Evidence:** **Peer reviewed**, NAACL 2024. [Proceedings and
paper](https://aclanthology.org/2024.naacl-long.224/).

- **Mechanism and access:** allocates token positions to message subunits and extends
  a zero-bit token-bias base to encode messages of at least 32 bits.
- **Detector and key:** key-only extraction without source-model access; zero-bit
  detection remains available.
- **Claimed operating point:** robustness, extraction accuracy, and latency are
  reported over selected corruption rates and lengths, not one deployment threshold.
- **Scope and quality:** open-model English generation with automatic quality
  measures.
- **Attack and reproduction:** later USENIX 2025, StealthInk, and XMark papers use it
  as a comparator and report weaknesses at short lengths or larger messages.
- **Bounded conclusion and gap:** position allocation made practical payloads more
  plausible. It does not make an embedded user identifier private, unforgeable, or
  reliably recoverable from every short response.

### UPV neural public verification

**Evidence:** **Peer reviewed**, ICLR 2024. [Paper and
metadata](https://openreview.net/forum?id=gMLQwKDY3N).
[Official implementation snapshot](https://github.com/THU-BPM/unforgeable_watermark/tree/1fb9526bd8d932816927cfb67e44ee20741b5d9c),
**Official implementation**.

- **Mechanism and access:** uses separate neural networks for watermark generation
  and detection, with shared token embeddings, so the public detector need not expose
  the generation secret.
- **Detector and key:** public neural detector; embedding requires generation-time
  control and the private generation network.
- **Claimed operating point:** reports high detection accuracy and efficient neural
  verification on selected open-model experiments, not a universal low-false-positive
  threshold.
- **Scope and quality:** English open-model generation and automatic quality and
  security analyses.
- **Attack and reproduction:** official code exists. Later stealing and spoofing
  studies show why empirical unforgeability must be evaluated under broader
  black-box access.
- **Bounded conclusion and gap:** separating generation and detection authority is a
  meaningful architecture. A complexity argument and selected attacks do not prove
  cryptographic unforgeability or cross-domain calibration.

### Publicly-Detectable Watermarking

**Evidence:** **Peer reviewed**, IACR Communications in Cryptology 2024. [Journal
article](https://doi.org/10.62056/AHMPDKP10).
[Official implementation snapshot](https://github.com/jfairoze/publicly-detectable-watermark/tree/08269490aab51334afcc42235e5eee0ba6201de1),
**Official implementation**. A separate TMLR submission was rejected, but the later
journal publication is the governing disposition.

- **Mechanism and access:** embeds a publicly verifiable cryptographic signature
  through rejection sampling and error correction during high-entropy opportunities.
- **Detector and key:** private signing key for generation and public verification
  key for detection.
- **Claimed operating point:** implementations on open models from 2.7B to 70B are
  reported; formal correctness, soundness, and distortion-freeness are the principal
  claims rather than a universal length threshold.
- **Scope and quality:** model and entropy conditions in the construction; broad
  linguistic and production evaluation is limited.
- **Attack and reproduction:** no independent production reproduction located. The
  public detector changes spoofing analysis but creates public scanning and key
  lifecycle requirements.
- **Bounded conclusion and gap:** public verification is cryptographically possible
  under stated assumptions. Practical costs, key lifecycle, robustness outside the
  stated edit model, and independent production evidence still require disclosure.

### Watermarking many adaptive users

**Evidence:** **Peer reviewed**, IEEE Symposium on Security and Privacy 2025. [DOI
and proceedings record](https://doi.org/10.1109/SP61157.2025.00084).

- **Mechanism and access:** constructs multi-user marks from adaptively robust,
  undetectable zero-bit schemes and introduces an approximate-enough-blocks
  robustness abstraction.
- **Detector and key:** zero-bit detection plus tracing material for individual or
  colluding users; the construction inherits the base scheme's key and security
  assumptions.
- **Claimed operating point:** primarily a generic reduction and formal security
  result. It preserves the base scheme's short-snippet detection while requiring
  longer excerpts for user tracing, without one universal length.
- **Scope and quality:** theoretical language-model abstraction with construction
  analysis rather than broad multilingual human evaluation.
- **Attack and reproduction:** no independent production reproduction was located.
  The work directly identifies repeated adaptive prompting as a gap in earlier
  guarantees.
- **Bounded conclusion and gap:** adaptive, multi-user security needs a formal model
  distinct from single-output detectability. Deployment also creates privacy,
  collusion, retention, revocation, and due-process obligations.

### Provably Robust Multi-bit Watermarking

**Evidence:** **Peer reviewed**, USENIX Security 2025. [Proceedings and
paper](https://www.usenix.org/conference/usenixsecurity25/presentation/qu-watermarking).

- **Mechanism and access:** pseudo-randomly assigns token positions to message
  segments and applies error-correcting codes for payload recovery.
- **Detector and key:** scheme-specific keyed extraction; embedding needs controlled
  generation.
- **Claimed operating point:** a 20-bit message in 200 tokens reached 97.6 percent
  match rate in the reported setup versus 49.2 percent for the cited baseline. The
  authors report tolerance of average paragraph edit distance 17 under that setting.
- **Scope and quality:** open models, English generation, automatic quality and edit
  experiments. An edit-distance proof does not cover every semantic rewrite.
- **Attack and reproduction:** peer reviewed, but independent replication and
  privacy analysis of embedded identifiers are unknown.
- **Bounded conclusion and gap:** coding theory improves payload recovery under a
  defined channel. Attribution still requires secure key issuance, collision policy,
  revocation, and due process around false attribution.

### StealthInk

**Evidence:** **Peer reviewed**, ICML 2025. [Paper and
metadata](https://openreview.net/forum?id=dktpDfUTtj).

- **Mechanism and access:** multi-bit sampling designed to preserve the base text
  distribution when averaged over messages or keys while carrying provenance data.
- **Detector and key:** extraction does not require the prompt or source-model API,
  but uses scheme parameters and secret material.
- **Claimed operating point:** derives a token lower bound at a fixed equal-error
  rate and reports task-level curves. Equal-error rate is not interchangeable with a
  fixed low false-positive operating point.
- **Scope and quality:** diverse English tasks and automatic quality, detectability,
  and resilience measures.
- **Attack and reproduction:** a 2026 paper reports an independent reimplementation
  for comparison, but a stable official code snapshot was not located here.
- **Bounded conclusion and gap:** multi-bit stealth in expectation is a useful formal
  property. Message-conditioned worst cases, short outputs, key privacy, and public
  auditability remain open deployment concerns.

### XMark

**Evidence:** **Peer reviewed**, ACL 2026. [Proceedings and
paper](https://aclanthology.org/2026.acl-long.672/).

- **Mechanism and access:** a logit-level multi-bit encoder and tailored decoder are
  designed to reduce distortion and improve short-text message recovery.
- **Detector and key:** scheme-specific decoder; generation requires controlled
  logits.
- **Claimed operating point:** paper reports decoding accuracy across tasks, text
  lengths, and payload sizes. It does not establish one provider-independent point.
- **Scope and quality:** open-model downstream tasks and standard quality measures.
- **Attack and reproduction:** the proceedings page says code will be released; no
  immutable public snapshot was verified at this review date.
- **Bounded conclusion and gap:** XMark advances the short-text quality-capacity
  frontier against its selected baselines. Payload privacy, spoofing, localization,
  and independent reproduction remain open.

## Multilingual, code, and mixed-source work

### Cross-lingual consistency and X-SIR

**Evidence:** **Peer reviewed**, ACL 2024. [Proceedings and
paper](https://aclanthology.org/2024.acl-long.226/).

- **Mechanism and access:** evaluates three existing marks through translation and
  proposes X-SIR, which aligns watermark partitions across languages.
- **Detector and key:** keyed detector for each tested scheme; X-SIR adds cross-lingual
  semantic structure.
- **Claimed operating point:** the tested baseline AUC values fell toward random
  guessing after cross-language transformation, while X-SIR improved consistency in
  the paper's selected languages and models.
- **Scope and quality:** two LLMs, three watermark methods, and multiple translation
  directions. Translation quality is part of the attack validity.
- **Attack and reproduction:** this is peer-reviewed independent evidence against
  the compared schemes, though exact provider deployments were not tested.
- **Bounded conclusion and gap:** monolingual detection results cannot be generalized
  across translation. Cross-lingual defense needs native-speaker quality, dialect,
  script, tokenizer, and low-resource-language evaluation.

### SAEMark

**Evidence:** **Peer reviewed**, NeurIPS 2025. [Paper and
metadata](https://openreview.net/forum?id=tXnyVPNOfa).

- **Mechanism and access:** uses sparse-autoencoder features to steer personalized
  multi-bit marks with inference-time sampling, including black-box LLM settings.
- **Detector and key:** feature and message-aware decoder; the exact black-box access
  contract and candidate-generation budget affect reproducibility.
- **Claimed operating point:** reports multilingual extraction and quality curves,
  not a universal threshold.
- **Scope and quality:** multiple languages and models with automatic and human
  evaluation as reported by the authors.
- **Attack and reproduction:** independent reproduction was not located.
- **Bounded conclusion and gap:** learned feature spaces may support personalized
  multilingual marks without provider logits. Dependence on the feature model,
  candidate budget, closed API drift, and payload privacy remains.

### SWEET for code

**Evidence:** **Peer reviewed**, ACL 2024. [Proceedings and
paper](https://aclanthology.org/2024.acl-long.268/).
[Official implementation snapshot](https://github.com/hongcheki/sweet-watermark/tree/853b47eb064c180beebd383302d09491fc98a565),
**Official implementation**.

- **Mechanism and access:** applies KGW-style marking only where next-token entropy
  exceeds a threshold, avoiding many low-entropy code tokens.
- **Detector and key:** keyed token detector with the same entropy selection;
  generation and full detection may need source-model probabilities.
- **Claimed operating point:** reports improved code execution quality and detection
  over baselines on selected code-generation benchmarks, not one language-independent
  threshold.
- **Scope and quality:** code tasks and functional tests expose why prose perplexity
  is inadequate for code. Evaluated programming languages and models remain bounded.
- **Attack and reproduction:** public code and later STONE comparison exist.
- **Bounded conclusion and gap:** entropy gating reduces damage in code, but entropy
  is not a syntax or semantic guarantee. A high-entropy token can still be critical.

### STONE for code

**Evidence:** **Peer reviewed**, Findings of EACL 2026. [Proceedings and
paper](https://aclanthology.org/2026.findings-eacl.207/).
[Official implementation snapshot](https://github.com/inistory/STONE-watermarking/tree/bb5d809c0c494a219411e861f2313cca2b9fd7b4),
**Official implementation**.

- **Mechanism and access:** excludes syntax-critical tokens rather than using entropy
  alone and proposes a combined correctness, detectability, and imperceptibility
  metric.
- **Detector and key:** keyed detection over eligible non-syntactic tokens;
  generation requires parser-aware token control.
- **Claimed operating point:** paper reports comparative benchmark curves rather than
  one cross-language threshold.
- **Scope and quality:** code correctness, syntax, and detectability on selected
  models, tasks, and programming languages.
- **Attack and reproduction:** official code includes baselines. External replication
  and behavior on generated build files, formulas, markup, and mixed prose-code
  documents are unknown.
- **Bounded conclusion and gap:** syntax-aware eligibility is stronger than entropy
  as a preservation heuristic. Only execution, type, security, and project tests can
  establish code utility for a concrete artifact.

### Mixed-source localization

**Evidence:** **Peer reviewed**, ACL 2025. [Proceedings and
paper](https://aclanthology.org/2025.acl-long.316/).
[Official implementation snapshot](https://github.com/XuandongZhao/llm-watermark-location/tree/87cab921dc5fcdef62ce3b6410a791d387780d2e),
**Official implementation**.

- **Mechanism and access:** geometric-cover detection asks whether any marked segment
  exists, while adaptive online learning estimates its boundaries.
- **Detector and key:** wraps keyed detectors for KGW, Unigram, and Gumbel families.
  It does not create evidence without the underlying key and compatible mark.
- **Claimed operating point:** evaluates mixed documents from 3,000 to 18,000 tokens
  with 300-token marked spans in reported settings. Results vary after paraphrase.
- **Scope and quality:** synthetic mixtures of known marked and unmarked sources;
  boundary accuracy and detection are primary.
- **Attack and reproduction:** code is public. Calibration under real editorial
  revisions, many overlapping windows, multiple provider marks, and selected-result
  reporting remains incomplete.
- **Bounded conclusion and gap:** long-document detection must be treated as a search
  problem, not a whole-document score. Family-wise false-positive control and honest
  uncertainty at boundaries are mandatory.

### TextSeal localized generation-time mark

**Evidence:** **Preprint**, May 2026. [Versioned paper
snapshot](https://arxiv.org/abs/2605.12456v1). The public Meta Seal snapshot is
linked above, **Official implementation** at the suite level.

- **Mechanism and access:** a Gumbel-max-derived dual-key sampler with entropy-weighted
  scoring, multi-region localization, and support claims for speculative decoding and
  multi-token prediction.
- **Detector and key:** private keyed detector with regional scores; generation needs
  controlled sampling.
- **Claimed operating point:** authors report no inference overhead, preservation on
  reasoning benchmarks, and 6,000 blind comparisons across five languages. Exact
  deployment thresholds are configuration-specific.
- **Scope and quality:** reasoning tasks, five-language human evaluation, mixed text,
  and distillation transfer.
- **Attack and reproduction:** provider-authored preprint and code suite; independent
  peer-reviewed reproduction is not yet available.
- **Bounded conclusion and gap:** the work is a serious 2026 localization candidate,
  not settled evidence that it dominates SynthID in independent deployment. The name
  collision with the post-hoc TextSeal work must be resolved in citations.

## Attacks, contradictions, and impossibility results

### DIPPER paraphrasing evaluation

**Evidence:** **Peer reviewed**, NeurIPS 2023. [Proceedings and
paper](https://proceedings.neurips.cc/paper_files/paper/2023/hash/575c450013d0e99e4b0ecf82bd1afaa4-Abstract-Conference.html).

- **Mechanism and access:** a controlled paragraph paraphraser varies lexical and
  reorder changes to stress watermark and non-watermark detectors.
- **Detector and key:** evaluation includes a KGW-style detector and several learned
  detectors at fixed false-positive rate.
- **Claimed operating point:** the paper reports large detection drops after
  paraphrasing and 80 to 97 percent retrieval defense detection at 1 percent false
  positive in a 15 million generation database.
- **Scope and quality:** English paragraph generation; semantic and lexical measures
  support but do not prove perfect meaning preservation.
- **Attack and reproduction:** open models, code, and data are reported. Later
  semantic marks explicitly respond to this attack family.
- **Bounded conclusion and gap:** paraphrase robustness must name the paraphraser and
  fidelity test. Retrieval is a different privacy and storage architecture, not a
  stronger watermark detector.

### Can AI-Generated Text Be Reliably Detected?

**Evidence:** **Preprint**, March 2023. [Versioned paper
snapshot](https://arxiv.org/abs/2303.11156v2).

- **Mechanism and access:** evaluates recursive paraphrasing against watermark,
  learned, zero-shot, and retrieval detectors and derives a total-variation-based
  limit as model text approaches the human distribution.
- **Detector and key:** broad detector study; its impossibility statement is not
  restricted to one keyed watermark construction.
- **Claimed operating point:** reports empirical degradation across named detectors,
  but the central theoretical conclusion is asymptotic and assumption-dependent.
- **Scope and quality:** English datasets and then-current models and paraphrasers.
- **Attack and reproduction:** later peer-reviewed work independently confirms that
  paraphrase is a central failure mode, while cryptographic watermark work studies a
  different cooperative-generator setting.
- **Bounded conclusion and gap:** passive AI-text classification becomes impossible
  as distributions converge. This does not prove that a cooperating generator cannot
  embed a keyed signal.

### Watermarks in the Sand

**Evidence:** **Peer reviewed**, ICML 2024. [Proceedings and
paper](https://proceedings.mlr.press/v235/zhang24o.html).

- **Mechanism and access:** proves strong watermarking impossible when an efficient
  attacker has a quality oracle and a perturbation oracle that mixes among
  high-quality outputs.
- **Detector and key:** result includes private-key detectors and does not require the
  attacker to know the scheme or key.
- **Claimed operating point:** a formal impossibility theorem plus empirical attacks
  on KGW, Kuditipudi, and Unigram. It is not a claim that every inexpensive rewrite
  defeats every weak watermark.
- **Scope and quality:** text and preliminary multimodal evidence; the theorem's
  oracle and mixing assumptions are explicit.
- **Attack and reproduction:** peer-reviewed independent contradiction to claims of
  unconditional strong robustness.
- **Bounded conclusion and gap:** practical systems may still raise attacker cost or
  detect low-effort reuse. They cannot honestly claim robust attribution without
  addressing the theorem's assumptions.

### Learnability of watermarks

**Evidence:** **Peer reviewed**, ICLR 2024. [Proceedings and
paper](https://proceedings.iclr.cc/paper_files/paper/2024/hash/a86d17b6cd70366d56ab48d2a05a4df1-Abstract-Conference.html).

- **Mechanism and access:** studies whether a model can learn a watermark pattern
  from marked data and emit it without the original insertion algorithm.
- **Detector and key:** evaluates existing keyed detectors against learned emission;
  the learner observes watermarked examples.
- **Claimed operating point:** reports learnability and spoofing behavior across
  selected marks and training settings, not a universal query count.
- **Scope and quality:** open models and watermark families available in 2023.
- **Attack and reproduction:** watermark stealing and radioactivity later confirm
  that signals can transfer or be approximated under other settings.
- **Bounded conclusion and gap:** watermark emission is not exclusive evidence of
  access to the original provider. Attribution requires an unforgeability model, not
  only detectability.

### Watermark stealing

**Evidence:** **Peer reviewed**, ICML 2024. [Proceedings and
paper](https://proceedings.mlr.press/v235/jovanovic24a.html).
[Official implementation snapshot](https://github.com/eth-sri/watermark-stealing/tree/b8d207deb2b4ba758638a41d3a77891993acd351),
**Official implementation**.

- **Mechanism and access:** estimates watermark preferences from black-box marked
  outputs, then evaluates both spoofing and scrubbing consequences.
- **Detector and key:** attacks several state-of-the-art keyed schemes without direct
  key access.
- **Claimed operating point:** authors report average success above 80 percent for
  both attack classes with under USD 50 in API queries in their setup.
- **Scope and quality:** named open and API-like models, tasks, watermark parameters,
  and quality checks.
- **Attack and reproduction:** official code is public. Results do not automatically
  transfer to undisclosed 2026 provider schemes, but the threat class does.
- **Bounded conclusion and gap:** repeated public outputs can leak enough structure
  to undermine source attribution. Providers need key rotation, rate analysis, and
  spoofing evaluation, none of which makes a positive detector result conclusive.

### Translation and watermark collision

**Evidence:** **Peer reviewed**, Findings of NAACL 2025. [Proceedings and
paper](https://aclanthology.org/2025.findings-naacl.37/).

- **Mechanism and access:** studies how a later watermarking system can overwrite,
  collide with, or coexist with an earlier logit-based mark during paraphrase,
  translation, or masked regeneration.
- **Detector and key:** separate detectors for original and downstream marks.
- **Claimed operating point:** reports multi-round true-positive and z-score curves,
  not one cross-system collision probability.
- **Scope and quality:** four logit-based marks and selected transformation systems.
- **Attack and reproduction:** peer-reviewed evidence directly relevant to
  sequential use of multiple marked providers.
- **Bounded conclusion and gap:** the last model in an editorial chain can alter the
  detectability and interpretation of earlier signals. A provider-positive result
  cannot be equated with sole authorship or final editing responsibility.

### WaterPark robustness evaluation

**Evidence:** **Peer reviewed**, Findings of EMNLP 2025. [Proceedings and
paper](https://aclanthology.org/2025.findings-emnlp.1148/).

- **Mechanism and access:** integrates 10 watermarkers and 12 attack classes in a
  common framework and maps design choices to robustness outcomes.
- **Detector and key:** preserves each scheme's access assumptions; it does not make
  private detectors public.
- **Claimed operating point:** reports a matrix of method, attack, strength, and
  quality results. Any single average hides substantial interaction effects.
- **Scope and quality:** multiple open models, datasets, transformations, and quality
  measures. Closed provider systems remain outside reproducible coverage.
- **Attack and reproduction:** an independent benchmark by a separate group and the
  broadest peer-reviewed robustness comparison located in this review.
- **Bounded conclusion and gap:** no family is uniformly best across attacks and
  quality constraints. Retonr should store full condition records, not import a
  leaderboard rank.

### UWbench and repeated-query unbiasedness

**Evidence:** **Preprint**, September 2025. [Versioned paper
snapshot](https://arxiv.org/abs/2509.24048v1).

- **Mechanism and access:** benchmarks unbiased watermarks along unbiasedness,
  detectability, and robustness axes and studies distribution drift across batches.
- **Detector and key:** evaluates several keyed unbiased families under a common
  protocol.
- **Claimed operating point:** derives an impossibility of perfect distribution
  preservation over infinitely many queries and recommends token-modification curves
  as more stable than paraphraser-specific robustness summaries.
- **Scope and quality:** open models and public watermark implementations.
- **Attack and reproduction:** this is independent critical evaluation, but it was
  not peer reviewed at the review date.
- **Bounded conclusion and gap:** one-sample unbiasedness must not be promoted as
  unlimited adaptive indistinguishability. The field still needs peer-reviewed,
  standardized repeated-query tests.

### Independent SynthID analysis

**Evidence:** **Preprint**, March 2026. [Versioned paper
snapshot](https://arxiv.org/abs/2603.03410v1), **Independent reproduction**.

- **Mechanism and access:** theoretically analyzes tournament sampling and
  independently reimplements the public SynthID reference method.
- **Detector and key:** uses the public algorithm with known experimental keys, not
  Google's private production detector.
- **Claimed operating point:** derives detection and robustness behavior and reports
  empirical validation across selected models and transformations.
- **Scope and quality:** public models and reference code; production Gemini traffic
  is not independently accessible.
- **Attack and reproduction:** valuable independent evidence, but not peer reviewed
  at the review date.
- **Bounded conclusion and gap:** public reference behavior can be scrutinized. It
  cannot authenticate or characterize an undisclosed provider configuration.

### Unforgeable robust signatures

**Evidence:** **Preprint**, February 2026. [Versioned paper
snapshot](https://arxiv.org/abs/2602.15323v1).

- **Mechanism and access:** introduces robust or recoverable signatures intended to
  combine undetectability, robustness, unforgeability, and recovery under bounded
  substitutions.
- **Detector and key:** cryptographic signing and verification keys; formal security
  depends on the new primitive and stated edit metric.
- **Claimed operating point:** primarily theoretical. The robustness model is Hamming
  substitutions, not arbitrary semantic rewriting, insertion, deletion, or
  translation.
- **Scope and quality:** formal language-model abstraction; broad empirical
  deployment evidence is unknown.
- **Attack and reproduction:** no peer-reviewed or independent implementation
  evidence located.
- **Bounded conclusion and gap:** the work sharpens the target security definition.
  It is not yet evidence that practical prose can carry such signatures without
  material fidelity or efficiency cost.

## Benchmarks and implementations

### WaterBench

**Evidence:** **Peer reviewed**, ACL 2024. [Proceedings and
paper](https://aclanthology.org/2024.acl-long.83/).

- Tunes methods toward a common watermark strength before comparing generation and
  detection, reducing one common source of unfair comparison.
- Adds instruction-following evaluation, including an LLM judge, beyond perplexity.
- Its strength matching, model judge, prompts, and then-current methods define the
  benchmark. Later methods and independent human ratings are not implied.
- **Bounded conclusion:** WaterBench is a useful common baseline, not a final
  production ranking.

### MarkLLM

**Evidence:** **Peer reviewed**, EMNLP 2024 system demonstration. [Paper
snapshot](https://arxiv.org/abs/2405.10051v1).
[Official implementation snapshot](https://github.com/THU-BPM/MarkLLM/tree/c45ddc40f7b761beabe55a1b8dc4690e531d1c6d),
**Official implementation**.

- Implements a broad set of watermark methods, quality measures, and attack
  evaluation pipelines behind one toolkit.
- Integration does not make algorithms semantically identical to their author code.
  Hashing, tokenizer, normalizer, model, revision, and default drift must be audited.
- **Bounded conclusion:** use MarkLLM to establish a reproducible harness, then
  validate material results against pinned author implementations.

### WaterPark

The peer-reviewed WaterPark paper above is the preferred attack-robustness harness
because it exposes interactions across 10 watermarkers and 12 attack classes. Its
value is the condition matrix, not a scalar winner.

### Provider reference implementations

- SynthID-Text is the clearest public provider reference implementation and includes
  documented three-way detector outcomes: watermarked, not watermarked, and
  uncertain.
- Meta Seal is a public research suite, but similarly named TextSeal works must be
  disambiguated by paper title, date, mechanism, and commit.
- A provider statement that a private model emits a mark is not sufficient to run an
  independent detector or to infer its mechanism. Until technical documentation,
  detector access, and calibration are public, record the mechanism and independent
  reproducibility as **Unknown**.

## Benchmark incompatibilities

The following results must not be compared as if they measured the same quantity:

1. **Nominal p-value versus empirical false-positive rate.** A theoretical tail
   probability under an independent-token null is not the observed rate on repeated,
   domain-specific natural text.
2. **Fixed false-positive rate versus equal-error rate.** Equal-error thresholds can
   imply an unacceptable false-positive rate for high-consequence attribution.
3. **AUC versus a deployment point.** AUC averages over thresholds. It does not show
   performance at `1e-5` false positives or at the minimum supported length.
4. **Per-document versus per-window error.** Scanning thousands of overlapping spans
   changes the probability that at least one span crosses a threshold.
5. **Zero-bit versus multi-bit.** Presence detection, exact-message recovery,
   bitwise accuracy, and top-k identity recovery are different tasks.
6. **Marginal distribution preservation versus per-key behavior.** Equality after
   averaging over keys or messages does not prove that a fixed deployed key is
   invisible across many queries.
7. **Information-theoretic versus computational indistinguishability.** The latter
   depends on adversary resources and cryptographic assumptions.
8. **Bounded generation budget versus unlimited service traffic.** Some
   distortion-free proofs consume or repeat a key sequence and explicitly cap the
   supported budget.
9. **Token edits versus semantic rewrites.** A percentage of insertions, deletions,
   or substitutions is not equivalent to a paraphrase with the same meaning.
10. **Named paraphraser versus paraphrase robustness.** Results depend on model,
    prompt, decoding, strength, iterations, and semantic-fidelity filter.
11. **Translation versus same-language paraphrase.** Tokenizer, syntax, script, and
    information preservation all change.
12. **Whole-document versus mixed-source detection.** Dilution and search change
    both power and calibration.
13. **English prose versus code.** Perplexity or preference ratings do not establish
    compilation, tests, semantics, security, or formatting fidelity.
14. **Open base model versus closed provider.** A reference algorithm on Gemma,
    Llama, or OPT does not reveal a private production configuration.
15. **Perplexity versus task utility.** Similar likelihood can coexist with changed
    facts, instructions, quotations, formulas, code behavior, or authorial stance.
16. **LLM judge versus blinded human or deterministic tests.** Judges have model,
    prompt, order, and style biases and cannot replace executable or factual checks.
17. **Aggregate no-difference versus equivalence.** A non-significant average change
    does not prove a bounded per-output loss or statistical equivalence without a
    prespecified margin and power analysis.
18. **Detection versus attribution.** A learnable or stolen signal can be present in
    text not emitted by the claimed provider.
19. **Watermark versus style classifier.** A proprietary AI detector may use style,
    retrieval, metadata, a watermark, or an ensemble. Its score cannot be interpreted
    as watermark evidence without documentation.
20. **Literal artifact versus statistical mark.** Removing metadata, hidden Unicode,
    or formatting proves only that those literal carriers are absent.

## Minimum reference benchmark suite

This suite is the minimum evidence needed before Retonr publishes a paper-style claim
about text watermark effects. It is a research protocol, not an evasion recipe.

### Reproducibility record

For every cell, retain:

- paper and implementation identity, including commit and local patch digest;
- model weights, tokenizer, runtime, precision, and license identity;
- prompt dataset revision and exact split;
- random seeds, key lifecycle, decoding settings, and generation length;
- watermark parameters and detector threshold selection procedure;
- preprocessing, normalization, repeated-token handling, and language detector;
- attack or transformation implementation and its independent fidelity checks;
- raw outputs, scores, confidence intervals, exclusions, and failed runs; and
- hardware, software, elapsed compute, and declared nondeterminism.

### Watermark families

At minimum include:

- KGW token bias;
- Unigram fixed partition;
- Kuditipudi keyed sampling;
- one distribution-preserving reweighting method such as DiPMark or STA-1;
- SynthID-Text reference implementation;
- one semantic method such as SemStamp or SWAN;
- one post-hoc method such as PostMark;
- one multi-bit method with an explicit payload length; and
- an unwatermarked control with identical decoding settings.

If a private provider detector is evaluated, it is a separate black-box condition.
Do not substitute its score for a known reference implementation.

### Models and content strata

- At least three model families and two useful size classes, with one instruction
  model and one base model.
- Open-ended narrative or news-like prose, summarization, rewriting, factual short
  answers, reasoning, low-entropy constrained generation, and quoted-source tasks.
- Code with deterministic compilation and test execution across at least two
  syntactically different programming languages.
- English plus a minimum of Chinese, Korean, Spanish, and Arabic, with native-speaker
  or professionally reviewed quality subsets.
- Native non-English prompts, not only translations from English.
- Human text stratified by domain, proficiency, dialect, editing assistance, and
  translated status. Consent and licensing must permit the evaluation.
- Short passages, ordinary responses, long passages, and long mixed-source documents
  with known segment boundaries.

### Calibration and power

- Report ROC and precision-recall curves, but also prespecified operating points at
  false-positive rates `1e-2`, `1e-3`, and, where sample size supports estimation,
  `1e-5`.
- Never claim empirical validation of `1e-5` from a null sample too small to estimate
  that rate with useful confidence.
- Report bootstrap or exact confidence intervals and the number of independent null
  documents, keys, prompts, and seeds.
- Calibrate separately by length, language, domain, repetition rate, tokenizer, and
  entropy stratum.
- For localization, report document-level family-wise false-positive rate, boundary
  error, marked-span recall, and uncertainty intervals.
- For multi-bit marks, report zero-bit detection, exact-message recovery, bit error,
  identity collision, and unknown-message rejection separately.
- Include repeated-query tests with one fixed key and cross-key tests. Do not average
  those regimes together.

### Quality and fidelity

- Blind human pairwise preference with a prespecified equivalence or non-inferiority
  margin, adequate power, and language-qualified raters.
- Task accuracy, instruction compliance, factuality, quotation identity, named-entity
  preservation, and numerical consistency.
- Perplexity or likelihood, diversity, repetition, lexical distribution, and style
  feature shifts, reported as diagnostics rather than a complete quality score.
- Executable code tests, compilation, static analysis, and security regression tests.
- For post-hoc methods, exact edit maps and preservation checks for markup,
  references, tables, formulas, code spans, and document structure.
- Per-output harm tails, not only aggregate means. Record the worst validated
  regressions and their content strata.

### Robustness and attribution safety

Evaluate transformation classes without publishing optimized bypass instructions:

- bounded token insertion, deletion, substitution, and cropping curves;
- multiple independently implemented paraphrase families with semantic, factual,
  and human fidelity review;
- translation and return translation across typologically different languages;
- dilution and localization in mixed human and model text;
- sequential processing by differently keyed watermark systems;
- copying, repeated-query learning, collision, stealing, and spoofing threat models;
- formatting and tokenization changes that occur in ordinary document workflows;
- key mismatch, key rotation, expired key, unknown scheme, and unavailable verifier;
  and
- base-rate simulations showing the positive predictive value in realistic use.

### Required baselines

- No mark with identical generation settings.
- No mark with ordinary editorial transformations.
- Style-pattern diagnostics evaluated only as style diagnostics.
- A passive AI-text classifier reported separately when useful.
- Deterministic Unicode, metadata, and formatting inspection reported separately.
- A scheme-aware detector with the wrong key to expose cross-key collision.
- Human text that intentionally contains common so-called AI style patterns.
- Model text that avoids those style patterns without any known watermark change.

## Citation-audit checklist

Before a Retonr paper, technical report, README, or release note cites a watermark
result, verify all of the following:

- [ ] The title, author order, year, venue, DOI, and publication status match the
  proceedings or journal record.
- [ ] A preprint is not described as peer reviewed because it was submitted to a
  venue or discussed by a provider.
- [ ] The cited URL identifies a stable proceedings page, DOI, versioned preprint,
  or immutable code commit.
- [ ] Provider statements, official implementations, independent reproductions, and
  Retonr inferences are labeled separately.
- [ ] The mechanism family and generation-time access are stated correctly.
- [ ] Detector access is explicit: public, API-mediated, secret-key, model-assisted,
  or unavailable.
- [ ] The claimed operating point includes false-positive definition, length,
  language, model, tokenizer, decoding, and dataset.
- [ ] AUC, equal-error rate, p-value, empirical false-positive rate, true-positive
  rate, message accuracy, and bit accuracy are not interchanged.
- [ ] The quality claim names its measures, sample size, uncertainty, and whether it
  tested equivalence or merely failed to find a difference.
- [ ] The attack claim names the threat model and fidelity criteria without turning
  the report into operational evasion guidance.
- [ ] The cited proof's assumptions, budget, key distribution, and adversary access
  are recorded.
- [ ] Distribution-preserving language specifies whether it is per token, per text,
  marginal over keys, averaged over messages, bounded-budget, computational, or
  asymptotic.
- [ ] The implementation matches the paper configuration or deviations are listed.
- [ ] An author implementation is not called an independent reproduction.
- [ ] Later peer-reviewed contradictions and material preprints are cited beside the
  original claim.
- [ ] Closed-provider behavior is not inferred from an open reference algorithm.
- [ ] Mixed-source scanning reports family-wise rather than only per-window error.
- [ ] A positive mark is described as a signal compatible with a scheme, not proof
  of sole authorship, misconduct, or provider responsibility.
- [ ] A negative result is not described as proof that no watermark or AI assistance
  exists.
- [ ] Literal artifact, metadata, style, passive-classifier, retrieval, and keyed
  watermark evidence remain separate fields.
- [ ] Every table row contains enough configuration metadata to reproduce or reject
  the comparison.
- [ ] Unknown evidence is written as unknown, not filled with a plausible mechanism.

## Retonr research and product boundaries

### What a style or slop diagnostic can honestly do

Retonr can define transparent style diagnostics for repetition, stock transitions,
excessive headings, canned conclusions, overused punctuation, vague intensifiers,
unnecessary restatement, or other explicitly documented patterns. Such a tool can be
valuable in an editorial quality loop when it:

- reports the exact passages and rule that triggered;
- lets the user enable, disable, or tune each rule;
- evaluates preservation and authorial preference separately;
- compares before and after without claiming an ideal human style; and
- never labels the score as AI probability, authorship, or watermark evidence.

The informal product name can be direct, but the machine-readable contract should
use a term such as `style_diagnostics`. The result measures chosen editorial
patterns. Human writing can trigger them, model writing can avoid them, and a
watermark can exist without any visible style pattern.

### What Retonr cannot currently claim

- It cannot determine whether arbitrary text was generated or assisted by AI.
- It cannot verify or remove a private statistical watermark without the compatible
  detector, key, and published operating conditions.
- It cannot prove that a rewrite removed every existing or future watermark.
- It cannot infer that stylistic improvement changed a provider's private detector
  outcome.
- It cannot treat absence of metadata or hidden Unicode as absence of a statistical
  mark.
- It cannot treat a provider-positive detector result as proof that the provider
  authored the final meaning, that no human edited it, or that the user violated a
  rule.
- It cannot claim that a probabilistic semantic check formally guarantees preserved
  meaning, facts, formulas, formatting, or code behavior.

### Defensible QA architecture

Store independent observations in an append-only report:

1. deterministic file, metadata, Unicode, and formatting inspection;
2. explicit style diagnostics with locations and rule versions;
3. structural and semantic preservation evidence for the requested edit;
4. optional scheme-specific verifier results with provider, key identifier,
   threshold, calibration record, text length, and verifier version;
5. optional passive-classifier results labeled as classifier evidence; and
6. exact edits, model identity, runtime identity, user approvals, and unresolved
   uncertainty.

No aggregation function should convert these fields into a universal AI score. A QA
loop can optimize clarity, fidelity, and user-selected style without optimizing
against a hidden detector. If a verifier is available, measure it as a research
outcome and report the result without making successful evasion a product invariant.

## Open research gaps

- Independent, preregistered reproduction of closed-provider text watermark
  deployments, including exact false-positive and uncertainty policies.
- Per-output and subgroup quality effects, not only aggregate preference rates.
- Calibration on translated, non-native, dialectal, accessibility-assisted, and
  professionally edited human text.
- Common tests for fixed-key repeated queries and adaptive users.
- Public-verification systems with practical latency, key rotation, revocation,
  privacy, and anti-spoofing evidence.
- Multi-provider sequential editing where several marks, tokenizers, and detectors
  interact.
- Long mixed documents with real revision histories rather than synthetic span
  insertion alone.
- Structured documents where prose changes coexist with immutable formulas, code,
  references, fields, comments, and layout.
- Code watermark benchmarks that include repository context, compilation, tests,
  security properties, formatting, and maintenance edits.
- Meaningful equivalence margins and powered human studies for the phrase quality
  preserving.
- Base-rate-aware decision policies and due process for consequential positive
  attribution.
- Standard disclosure of detector access, key scope, minimum supported length,
  abstention behavior, and invalid-input behavior.
- A neutral interoperation vocabulary for detector unavailable, incompatible scheme,
  uncertain, negative under stated conditions, and positive under stated conditions.

## Recurring surveillance protocol

The field and provider behavior change quickly. Re-run this evidence review when a
provider changes output marking, publishes a detector, rotates a scheme, or exposes
new technical documentation, and when a major watermark, benchmark, attack, or
impossibility result appears.

Each review should:

1. search ACL Anthology, PMLR, OpenReview venue decisions, USENIX, IACR, arXiv, and
   provider technical documentation;
2. record publication disposition separately from manuscript availability;
3. pin code and model artifacts before running comparisons;
4. add new methods only after mapping their access and security definition;
5. rerun the minimum suite against the previous pinned baseline;
6. publish regressions and null results, not only new best scores;
7. revise product claim language before changing implementation behavior; and
8. retain the previous evidence snapshot so historical release claims remain
   interpretable.

The objective is not to chase every provider claim. It is to keep Retonr's language,
tests, and boundaries synchronized with reproducible evidence while remaining clear
about uncertainty.
