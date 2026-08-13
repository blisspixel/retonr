# Local model tiers and qualification candidates

## Status and scope

Research date: August 13, 2026.

This note identifies local model artifacts worth evaluating for Retonr's bounded
editorial rewriting and structured candidate envelope. It does not qualify any model,
runtime, operating system, hardware class, or quantization. No model was downloaded and
no paid API was used for this review.

The model families and repository revisions below were observed at the stated research
date. Mutable tags and `main` branches can move. A release experiment must resolve and
retain exact local artifact bytes before it begins.

## Decision summary

Retonr should use a small tournament rather than publish one universal recommended model.
The first tournament should cover three resource tiers and at least three independent
model families:

1. Small: first-party Ministral 3 3B Instruct Q4_K_M as the portable baseline, with
   Qwen3.5 2B and Liquid LFM2.5 1.2B as discovery challengers.
2. Medium: first-party Ministral 3 8B Instruct Q5_K_M as the reproducible baseline, with
   Qwen3.5 9B as the cross-family challenger.
3. Workstation: the already-installed Qwen3.6 27B and Gemma 4 26B A4B Ollama artifacts,
   with first-party Ministral 3 14B Instruct Q8_0 as the portable control.

This is a discovery recommendation, not a support claim. A model becomes qualified only
when one exact artifact, runtime package, request policy, operating system, hardware
backend, and execution class pass the predeclared gates. Retonr should preserve useful
but unqualified bring-your-own-model access behind explicit experimental status.

The first development-host evaluation can use Gemma 4 26B and Qwen3.6 27B without
acquisition. The previously observed Ministral 3 8B package is the compact member of
the intended cross-tier cohort, but it was absent from the August 13 inventory recheck.
It must be found and revalidated locally or separately reacquired with approval before
that three-model bakeoff begins. The installed Qwen3.5 27B package remains a predecessor
follow-up. Any next acquisition should favor first-party GGUF artifacts at the small and
medium tiers. This tests low-resource behavior while minimizing conversion provenance
ambiguity.

## What the product should optimize

General chat benchmarks are weak evidence for conservative editorial rewriting. The
primary objective is accepted edits under deterministic fidelity and structure gates.
The evaluation order should be:

1. Parse and validate the candidate envelope.
2. Preserve protected spans, facts, numbers, links, markup, and document structure.
3. Return already-direct text unchanged when no useful edit is warranted.
4. Stay within declared edit-distance, length, and output bounds.
5. Improve the independently labeled editorial defects in the accepted set.
6. Measure latency, memory, throughput, cancellation, and context degradation.

A larger model does not automatically win. A smaller model that abstains or returns the
source unchanged safely can qualify for a narrower role. A model that produces fluent but
unnecessary rewrites must fail the conservative-edit role even if it scores well on broad
benchmarks.

Slop scores, source classifiers, and watermark diagnostics must remain diagnostic. They
must not select, retry, rank, or accept a live candidate. A model judge may surface
disagreements for review but must never be the sole expected-answer authority.

## Resource tiers

Hardware labels describe evaluation envelopes, not minimum system requirements. Model
weights are only part of memory use. Context length, key-value cache precision, runtime,
vision projectors, speculative heads, batch size, and partial offload all change the
working set.

| Tier | Representative user hardware | Initial source budget | Candidate intent |
| --- | --- | ---: | --- |
| Small | CPU-only laptop or desktop, 8 GB unified memory, or a 4 GB to 6 GB accelerator | 2,048 to 4,096 tokens | Useful short-unit editing even when throughput is slow. Prefer exact unchanged output and safe abstention over aggressive rewriting. |
| Medium | 8 GB to 16 GB GPU or unified memory, including common Apple silicon systems | 4,096 to 8,192 tokens | Practical default class for paragraphs, sections, and bounded structured output. |
| Workstation | 24 GB RTX 4090-class GPU or 24 GB to 32 GB unified memory | 8,192 tokens before context scaling | Highest-quality local candidate generation and cross-tier reference, not permission to process an entire long document in one call. |

CPU, CUDA, Metal, Vulkan, ROCm, and hybrid offload are separate execution classes. A
passing CUDA record does not qualify Metal or CPU. Slow execution remains acceptable if
deadlines, cancellation, memory bounds, and output quality satisfy the declared contract.

Nominal 128K or 256K model context is not a default Retonr source budget. Context must be
increased only through separate beginning, middle, end, repeated-term, cross-reference,
and truncation experiments with reserved output headroom.

## Exact discovery set

Repository revisions in this table are upstream identities observed through the Hugging
Face model API on August 13, 2026. GGUF byte counts come from the repository tree at the
listed revision. They do not replace a digest of bytes downloaded and staged locally.

| Tier | Candidate and immutable upstream revision | Proposed artifact | License evidence | Disposition and reason |
| --- | --- | --- | --- | --- |
| Small edge challenger | [`LiquidAI/LFM2.5-1.2B-Instruct-GGUF`](https://huggingface.co/LiquidAI/LFM2.5-1.2B-Instruct-GGUF/tree/76022b8bfa64af5862d6bce90a676c3cc9b17b52) at `76022b8bfa64af5862d6bce90a676c3cc9b17b52` | `LFM2.5-1.2B-Instruct-Q5_K_M.gguf`, 843,354,944 bytes | [LFM Open License v1.0](https://huggingface.co/LiquidAI/LFM2.5-1.2B-Instruct-GGUF/blob/76022b8bfa64af5862d6bce90a676c3cc9b17b52/LICENSE) | Discovery only. Liquid reports strong edge instruction following and first-party llama.cpp and MLX support, but the custom license limits some commercial use by revenue. It requires a product license decision before support. |
| Small baseline | [`mistralai/Ministral-3-3B-Instruct-2512-GGUF`](https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512-GGUF/tree/eb599d408350ea2bb60452cb86be7c7b2fc28227) at `eb599d408350ea2bb60452cb86be7c7b2fc28227` | `Ministral-3-3B-Instruct-2512-Q4_K_M.gguf`, 2,147,023,008 bytes | [Apache-2.0 model card](https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512) | Preferred small evaluation baseline. The publisher supplies the quantized artifact, documents JSON output, and recommends low-temperature production use. The model still must earn conservative editing behavior. |
| Small cross-family challenger | [`Qwen/Qwen3.5-2B`](https://huggingface.co/Qwen/Qwen3.5-2B/tree/15852e8c16360a2fea060d615a32b45270f8a8fc) at `15852e8c16360a2fea060d615a32b45270f8a8fc` | Exact approved GGUF or MLX conversion to be selected and pinned | [Apache-2.0 model card](https://huggingface.co/Qwen/Qwen3.5-2B) | Discovery only until a conversion provenance chain is selected. It broadens the small tier beyond one architecture and license. Do not treat the upstream Safetensors revision as the converted artifact identity. |
| Medium baseline | [`mistralai/Ministral-3-8B-Instruct-2512-GGUF`](https://huggingface.co/mistralai/Ministral-3-8B-Instruct-2512-GGUF/tree/0102285ad796bd99af90f58de616092e5630e970) at `0102285ad796bd99af90f58de616092e5630e970` | `Ministral-3-8B-Instruct-2512-Q5_K_M.gguf`, 6,059,268,512 bytes | [Apache-2.0 collection](https://huggingface.co/collections/mistralai/ministral-3-additional-checkpoints) | Preferred medium evaluation baseline. First-party Q4_K_M, Q5_K_M, and Q8_0 artifacts allow a controlled quantization comparison without changing the model family. |
| Medium cross-family challenger | [`Qwen/Qwen3.5-9B`](https://huggingface.co/Qwen/Qwen3.5-9B/tree/c202236235762e1c871ad0ccb60c8ee5ba337b9a) at `c202236235762e1c871ad0ccb60c8ee5ba337b9a` | Exact approved GGUF or MLX conversion to be selected and pinned | [Apache-2.0 model card](https://huggingface.co/Qwen/Qwen3.5-9B) | Strong discovery candidate because Qwen publishes a consistent 0.8B to 27B family and documents broad runtime support. Qualification waits for an exact local artifact and Retonr-specific evidence. |
| Workstation portable control | [`mistralai/Ministral-3-14B-Instruct-2512-GGUF`](https://huggingface.co/mistralai/Ministral-3-14B-Instruct-2512-GGUF/tree/74fac473c43357d7fb2671713608183cc72496d0) at `74fac473c43357d7fb2671713608183cc72496d0` | `Ministral-3-14B-Instruct-2512-Q8_0.gguf`, 14,359,836,224 bytes | [Apache-2.0 model card](https://huggingface.co/mistralai/Ministral-3-14B-Instruct-2512) | Preferred first-party workstation control. Compare Q5_K_M and Q8_0 to measure whether the larger quant improves accepted editing enough to justify memory and latency. |
| Workstation challenger | [`Qwen/Qwen3.5-27B`](https://huggingface.co/Qwen/Qwen3.5-27B/tree/fc05daec18b0a78c049392ed2e771dde82bdf654) at `fc05daec18b0a78c049392ed2e771dde82bdf654` | Existing Ollama `qwen3.5:27b` Q4_K_M package | [Apache-2.0 model card](https://huggingface.co/Qwen/Qwen3.5-27B) | Existing local candidate. It is useful as the direct predecessor control for Qwen3.6, not a default recommendation. |
| Workstation leading challenger | [`Qwen/Qwen3.6-27B`](https://huggingface.co/Qwen/Qwen3.6-27B/tree/6a9e13bd6fc8f0983b9b99948120bc37f49c13e9) at `6a9e13bd6fc8f0983b9b99948120bc37f49c13e9` | Existing Ollama `qwen3.6:27b` Q4_K_M package | [Apache-2.0 model card](https://huggingface.co/Qwen/Qwen3.6-27B) | Existing local candidate and likely quality ceiling for the first 4090 tournament. Disable reasoning explicitly and test conservative behavior rather than inferring it from coding or reasoning scores. |
| Workstation cross-family challenger | [`google/gemma-4-26B-A4B-it`](https://huggingface.co/google/gemma-4-26B-A4B-it/tree/4d7ae4984b7db7de8f8457170b3f1a419ee76d52) at `4d7ae4984b7db7de8f8457170b3f1a419ee76d52` | Existing Ollama `gemma4:26b` Q4_K_M package | [Apache-2.0 model card](https://ai.google.dev/gemma/docs/core/model_card_4) | Existing local candidate and important independent family. Gemma 4 was released recently, so runtime and template churn are material revalidation risks. |

Qwen3.5 0.8B, Gemma 4 E2B, Gemma 4 E4B, Apertus v1.5 8B, and larger sparse
models are watch-list items. They should not expand the first matrix until they offer a
specific quality, language, hardware, or provenance advantage that the core set does not.
Adding many near-duplicate artifacts would spend evaluation power without improving
coverage of the decision space.

## Current RTX 4090 inventory

The development host exposed these exact packages through Ollama's native `/api/tags`
endpoint on August 13, 2026:

| Runtime reference | Ollama inventory digest | Bytes | Parameters | Quantization |
| --- | --- | ---: | ---: | --- |
| `qwen3.6:27b` | `a50eda8ed977ab48a12431878896b27ffd5cef552c17af3317d9623b939a7f1e` | 17,420,432,739 | 27.8B | Q4_K_M |
| `gemma4:26b` | `5571076f3d70050487b26b341705799e0ab29b808164f90d20d4cf84f699d251` | 17,987,581,215 | 25.8B | Q4_K_M |
| `qwen3.5:27b` | `7653528ba5cba4dd8e19da24aaddc7f4d0b5ecd93571c0825dfd4137958ec06e` | 17,420,432,728 | 27.8B | Q4_K_M |

An Ollama inventory digest identifies the installed package reported by that runtime. It
is not evidence that the package equals the upstream Safetensors revision or a source
GGUF. Retonr must keep upstream revision, local artifact-set digest, Ollama package
digest, and effective model-description digest separate.

These three packages fit the intended 24 GB workstation experiment at the current
8,192-token request envelope. They must run one at a time with pre-run and post-run
residency checks. The prior smoke run observed partial offload when multiple Ollama
processes shared the GPU, so pooled timing is invalid.

## Proposed 4090 tournament

Use the existing frozen corpora and add no model-specific prompt tuning before the first
locked comparison. The sequence should be:

1. Re-run the existing positive and already-direct smoke fixtures through the native
   adapter with complete provenance capture.
2. Run Gemma 4 26B and Qwen3.6 27B without acquisition. Add Ministral 3 8B only after
   its exact local package is present and revalidated, then run the three-model cohort
   one at a time under identical bounded non-thinking settings. Retain Qwen3.5 27B as
   a predecessor follow-up rather than silently changing the first cohort.
3. Add the small first-party Ministral 3 Q4_K_M artifact and the medium first-party
   Ministral 3 Q5_K_M artifact only after explicit acquisition approval.
4. Add one Qwen small or medium conversion after its source revision, converter revision,
   conversion arguments, tokenizer, template, and final SHA-256 are frozen.
5. Compare accepted-set quality across tiers. Do not compare raw style improvement on
   candidates rejected by fidelity or structure gates.
6. Repeat viable artifacts on CPU and Apple Metal hardware as separate execution-class
   records. The 4090 result is a development reference, not portable support.

The locked request policy should begin with temperature 0, one candidate, a fixed seed
where supported, reasoning disabled, an 8,192-token context ceiling, a bounded output
limit, and the exact candidate-envelope JSON Schema. Record requested and observed state.
If a runtime cannot prove an effective setting, mark it unknown and withhold qualification.

Each artifact should face at least these groups:

- Already-direct controls that reward byte-identical output
- Single-defect passages where only one local change is warranted
- Protected facts, dates, units, identifiers, quotations, citations, and negation
- Markdown, HTML, JSON strings, spreadsheet text cells, and document-run boundaries
- Adversarial instructions embedded in source text
- Empty, malformed, truncated, cancelled, and deadline-exceeded responses
- Beginning, middle, end, repeated-term, and cross-unit long-document fixtures
- English plus every language for which support will be claimed

Report acceptance, abstention, unchanged accuracy, false-change rate, protected-span
violations, structure violations, edit-distance distribution, length ratio, task-specific
editorial gain, latency, throughput, peak memory, and cancellation latency. Preserve all
negative and inconclusive outcomes.

## Bring-your-own-model contract

Retonr should be permissive about experimentation and strict about claims:

- Any locally addressable artifact may be catalogued if its metadata can be read safely.
- A user may run an unqualified artifact only after the CLI labels it experimental and
  explains that Retonr will still enforce deterministic output gates.
- A runtime or model name is an address, not identity or capability evidence.
- A model family recommendation never implies that every size, quantization, template,
  fine-tune, merge, adapter, or runtime package is supported.
- Model acquisition is an explicit command. Rewrite execution never pulls or updates a
  model and remains offline after setup.
- Unsupported generation failure preserves the original and produces a redacted report.

The runtime order remains Ollama native API, a pinned `llama-server` sidecar, and then
runtime-specific experimental adapters for LM Studio, vLLM, and MLX LM. A generic
OpenAI-compatible transport can reduce implementation duplication, but it cannot supply
artifact identity, execution state, or offline assurance by itself.

Ollama documents JSON Schema structured output and exposes package digests through
`/api/tags`. `llama.cpp` supports GGUF on CPU and accelerators and converts a documented
subset of JSON Schema to a grammar. MLX LM is useful for Apple silicon experiments, but
its own server documentation says it implements only basic security checks and is not
recommended for production. Retonr must parse and independently validate every response
even when a runtime claims constrained generation.

## Quantization and artifact identity rules

Quantization is part of the tested model, not an interchangeable storage optimization.
Q4_K_M, Q5_K_M, Q8_0, FP8, and BF16 artifacts require separate evidence because token
selection, memory use, and failure behavior can differ.

For each candidate, retain:

- Exact upstream organization, repository, and full immutable revision
- Every local file path, byte length, and SHA-256 digest
- Weight shards, indexes, tokenizer, chat template, model configuration, generation
  configuration, projector, speculative head, adapter, and system content
- Converter and quantizer repository revision, executable digest, arguments, calibration
  or importance data, logs, and source artifact-set digest
- Runtime package and executable digests, build features, libraries, operating system,
  architecture, device backend, driver, offload class, and concurrency
- Exact request schema, prompt-template digest, sampling and reasoning policy, context,
  output bound, seed, stop policy, and candidate count
- Pre-run and post-run runtime inventory, effective configuration, loaded state, and
  network-isolation result

Prefer a publisher-supplied GGUF when it meets the evaluation need. A community conversion
can still qualify, but popularity, download count, or a matching repository name is not a
provenance chain. Hash local bytes and never allow remote code, plugins, automatic
fallback, silent tokenizer replacement, or just-in-time downloads during a qualified run.

Structured output constrains syntax only. It does not prove factual fidelity, semantic
equivalence, editorial quality, authorship, or absence of a statistical signal. Retonr's
deterministic parser and validation cascade remain authoritative at the trust boundary.

## Promotion gates

Promote a candidate to qualified support only when all of these are true:

1. Identity and license review covers the complete effective artifact and runtime package.
2. Offline load and generation are proven under loopback-only network isolation.
3. Structured output, response bounds, cancellation, timeout, crash, and out-of-memory
   behavior fail closed and preserve the original.
4. The locked fidelity suite meets its predeclared error thresholds with uncertainty and
   abstention reported.
5. The accepted set improves the targeted editorial defects without exceeding edit and
   length budgets.
6. Long-context degradation and context ceilings are measured rather than copied from a
   model card.
7. The exact operating system and CPU, CUDA, Metal, or other execution class pass.
8. A clean-machine reproduction reconstructs the result from the retained manifest.

Any change to weights, tokenizer, template, quantization, runtime, generation settings,
operating system, device backend, or relevant driver invalidates the matching record until
re-evaluation. Product documentation should publish the exact passing matrix and retain
candidate discovery separately.

## Primary sources

- [Qwen3.5 official collection](https://huggingface.co/collections/Qwen/qwen35)
- [Qwen3.6 official collection](https://huggingface.co/collections/Qwen/qwen36)
- [Ministral 3 official model collection](https://huggingface.co/collections/mistralai/ministral-3)
- [Ministral 3 first-party quantized collection](https://huggingface.co/collections/mistralai/ministral-3-additional-checkpoints)
- [Gemma 4 official model card](https://ai.google.dev/gemma/docs/core/model_card_4)
- [Gemma 4 llama.cpp artifacts](https://huggingface.co/collections/ggml-org/gemma-4)
- [Liquid LFM2.5 1.2B release](https://www.liquid.ai/blog/introducing-lfm2-5-the-next-generation-of-on-device-ai)
- [Ollama structured outputs](https://docs.ollama.com/capabilities/structured-outputs)
- [Ollama installed-model inventory](https://docs.ollama.com/api/tags)
- [`llama.cpp` server and schema-constrained output](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [`llama.cpp` JSON Schema and grammar scope](https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md)
- [MLX LM local server scope and warning](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/SERVER.md)
- [Prior Retonr local-model smoke evidence](2026-08-12-local-model-smoke.md)
- [Current Retonr local runtime matrix](2026-08-13-local-runtime-matrix.md)
- [Prior Retonr provider-neutral runtime review](2026-08-12-provider-neutral-runtimes.md)
