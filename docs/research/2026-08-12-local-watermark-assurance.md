# Local watermark assurance for user-controlled runtimes

## Review status

Reviewed: August 12, 2026.

Research cutoff: August 12, 2026. Runtime documentation and source surfaces
change quickly. A qualification must cite an installed stable release and its
immutable artifacts, not this review date, a mutable documentation branch, or a
model alias.

This document defines a falsifiable assurance and audit protocol for intentional
text marks in local and API-served language-model execution. It covers
Transformers, `llama.cpp` and `llama-server`, Ollama, vLLM, MLX LM, LM Studio,
compatibility proxies, and remote opaque providers. It complements the broader
[runtime qualification](2026-08-12-provider-neutral-runtimes.md),
[watermark science](2026-08-12-text-watermark-science.md), and
[watermark evaluation](2026-08-12-watermark-evaluation-protocol.md) records.

This is an engineering policy, not legal advice. "Mandatory" below means a
Retonr product or release requirement. It does not state that every user has the
same contractual, disclosure, or legal duties.

## Executive conclusion

There is no reliable global switch that proves local text is watermark-free.
Intentional marks can be learned into weights, supplied by model configuration,
introduced by a tokenizer or chat template, applied to logits or sampling,
affected by speculative decoding, inserted by a parser or postprocessor, added by
middleware or a proxy, or imposed inside a remote service that the operator cannot
inspect. Provider-side logs can also preserve attribution evidence without adding
anything to the returned text.

Retonr may make one narrow negative finding:

> **No known intentional marker enabled.** For the identified artifact set,
> execution path, effective configuration, request class, and evidence registry,
> the audit found no enabled mechanism whose documented or inspected purpose is to
> add a statistical, lexical, structural, Unicode, metadata, or signature-based
> marker to generated text.

That finding is valid only when accompanied by an evidence level, artifact-set ID,
scope boundaries, limitations, and requalification state. It is falsified by an
unrecorded executable component, a changed artifact, an enabled marker hook, an
unexplained output transformation, an undisclosed remote fallback, or a failure to
reproduce the evidence package.

It does **not** mean:

- no signal is learned into the weights;
- no unknown, secret, or future detector can classify the text;
- no marked input was quoted, copied, or learned;
- no provider, operating system, proxy, or service retained an external record;
- no Content Credential, file metadata, invisible character, or signature exists
  outside the audited boundary;
- the text is human-authored, anonymous, original, policy-compliant, or legally
  unrestricted;
- the result applies after an artifact, configuration, runtime, or output changes.

The reference assurance target is therefore not "clean text." It is a complete,
reproducible record showing that a known intentional marking mechanism was not
enabled in one bounded path.

## Assurance object and boundary

### Qualified execution tuple

Every finding binds to this tuple:

```text
Q = (
  artifact_set_id,
  runtime_path_id,
  effective_config_id,
  request_class_id,
  platform_class_id,
  evidence_registry_id,
  audit_procedure_id
)
```

The claim is invalid if any tuple member is unknown. A model family, mutable tag,
friendly name, API `model` string, or `system_fingerprint` is not a substitute for
the tuple.

The report must identify four text boundaries separately:

1. **Rendered prompt:** bytes and token IDs delivered to the model.
2. **Generated token stream:** selected token IDs before detokenization, where the
   runtime exposes them.
3. **API assistant text:** bytes in the native response field after server parsing.
4. **Retonr output:** bytes accepted, stored, and exported after Retonr processing.

A finding that reaches only boundary 2 cannot be applied to an exported document.
A finding that starts at boundary 3 cannot establish what happened in logits or
sampling.

### Marks in scope

An intentional marker is a mechanism deliberately used to make text, a request, a
model, an operator, or an asset distinguishable or linkable. In scope are:

- statistical token-selection watermarks;
- distribution-preserving or keyed sampling watermarks;
- learned, distilled, fine-tuned, or weight-level output signals;
- lexical, syntactic, semantic, or structural markers;
- hidden Unicode, homoglyph, or variation-selector payloads;
- visible or invisible identifiers inserted after generation;
- file metadata, signed manifests, and embedded content credentials when the
  audited path creates or changes them;
- request, tenant, model, or provider identifiers encoded into text;
- cryptographic signatures or soft bindings attached to the text or document.

Ordinary sampling controls, templates, grammars, and parsers are not automatically
markers. They remain in the inventory because the same control points can implement
one.

### Evidence outside the artifact

Service logs, account records, billing identifiers, request IDs, retained prompts,
and server-side detector results are provenance evidence outside the returned
artifact. Their absence cannot be inferred by inspecting text. Local qualification
can prove that the qualified process did not establish a non-loopback connection
under an enforced network policy. It cannot prove that an opaque remote provider
deleted or never created records.

## Complete insertion-point inventory

The audit treats generation as an ordered graph, not a single model call.

| Point | How an intentional mark can enter | Required evidence | Hard limit |
| --- | --- | --- | --- |
| Training or in-model behavior | Pretraining, fine-tuning, distillation, reinforcement learning, model editing, or a weight watermark can make the model emit a learnable signal without a runtime toggle. | Model lineage, training disclosures, adapter lineage, weight identity, scheme-specific research tests. | Static weight inspection does not establish semantic absence. |
| Model package | `config.json`, `generation_config.json`, GGUF metadata, custom model code, adapter declarations, or model-specific generation code can alter generation. | Hash and parse every package member; record remote-code and custom-generation policy. | A safe tensor container prevents executable deserialization, not marked behavior. |
| Tokenizer | Normalizers, pre-tokenizers, vocabulary, merge rules, added and special tokens, decoder cleanup, and postprocessors can introduce or hide distinct byte sequences. | Complete tokenizer artifact, resolved implementation, encode/decode fixtures, raw code-point inventory. | Equivalent-looking text can have different bytes and token IDs. |
| Chat template and prompt assembly | A template, preset, system message, stored message history, tool definition, or request-supplied template can instruct a pattern or inject hidden text. | Exact template bytes, variables, rendered prompt bytes, rendered token IDs, precedence trace. | A visible request body is not necessarily the rendered prompt. |
| Logits and constraints | A built-in or custom logits processor, sequence bias, grammar, allowlist, denylist, or structured-output engine can change next-token probabilities. | Ordered processor graph, class or symbol identity, arguments, and pre/post-processor evidence where supported. | An ordinary constraint can be repurposed as a marker. Intent and configuration both matter. |
| Sampler and random source | Keyed sampling, rejection sampling, tournament sampling, custom samplers, RNG seeding, or request-derived entropy can encode a signal. | Ordered sampler graph, algorithm implementation, seed policy, key-source policy, and effective parameters. | Identical high-level parameters do not imply identical sampler code. |
| Speculative decoding | A draft model, learned proposer, n-gram proposer, vocabulary mapping, acceptance rule, or custom proposer can affect candidate selection. | Target and draft identities, tokenizers, proposer and acceptance code, effective speculative configuration, acceptance statistics. | A claim that speculation is distribution-preserving is implementation and setting specific. |
| Detokenizer and postprocessor | Decoding cleanup, Unicode normalization, stop trimming, reasoning extraction, tool parsing, regex replacement, formatter code, or response assembly can change text. | Raw selected token IDs, incremental decoded chunks, final decoded bytes, parser graph, final API bytes. | Once raw tokens are hidden, pre-parser behavior is only partly observable. |
| Server defaults and state | Auto-loaded generation configuration, model aliases, JIT loading, cached presets, mutable tags, hot updates, fallback models, and stored conversation state can change the path. | Effective startup and request configuration after precedence, loaded-instance identity, state inventory, restart fixtures. | Submitted parameters alone do not show effective state. |
| Plugins and middleware | Python entry points, server plugins, ASGI middleware, model extensions, tool parsers, IO processors, preload libraries, or shell wrappers can modify prompts or responses. | Installed-package and entry-point inventory, import graph, loaded-module trace, process ancestry, middleware order. | A plugin may be discovered without an obvious command-line argument. |
| Compatibility proxy | A gateway, SDK, reverse proxy, cache, retry layer, or OpenAI-compatibility adapter can inject prompts, route to another model, or rewrite response fields. | Direct-to-runtime and through-proxy captures, proxy artifact identity and configuration, TLS and route identity. | API schema compatibility says nothing about implementation identity. |
| Remote opaque provider | Provider-side weights, templates, samplers, postprocessors, fallbacks, keys, and logs are not inspectable by the client. | Provider declaration or attestation, endpoint identity, observed responses, change indicators, and bounded black-box tests. | Client-side testing cannot support a local reference-grade absence claim. |
| Retonr and export | Editorial transforms, serializers, document libraries, provenance writers, and format conversion can add or alter text or metadata. | Source-to-candidate-to-export byte and structure evidence, dependency identity, provenance policy result. | Export formats may contain metadata not present in plain assistant text. |

The NIST synthetic-content report distinguishes during-generation token-probability
marks from post-generation text changes and states that covert detectors have
nonzero false-positive and false-negative probability. It also recommends
representative datasets and reproducible evaluation rather than interpreting one
score outside its context ([NIST AI 100-4](https://doi.org/10.6028/NIST.AI.100-4)).

## Artifact-set identity

### Canonical manifest

The artifact-set ID is the SHA-256 digest of the versioned, domain-separated preimage
`retonr:artifact-set-manifest:v1\0` followed by the canonical UTF-8 JSON content
manifest. Keys are sorted, numbers and strings use one canonical representation,
portable manifest names require canonical `/` separators, and each byte-bearing
member has its own size and SHA-256 digest. The manifest format and hashing procedure
are themselves versioned.

The content manifest contains only portable logical paths, byte lengths, and member
digests. It does not change when provenance or review evidence grows. A separate
effective-package evidence record joins the content identity to the following, when
applicable:

- runtime executable, source revision, release tag, build receipt, compile flags,
  patches, dynamic libraries, accelerator libraries, container image and layers;
- complete model weights, shard indexes, GGUF files and all metadata, configuration,
  generation configuration, conversion and quantization receipts;
- tokenizer model, JSON, vocabulary, merges, normalizer, decoder, postprocessor,
  special-token maps, added tokens, and chat templates;
- system prompts, presets, template overrides, default messages, grammars, schemas,
  stop lists, renderer and parser definitions;
- all adapters, LoRA files, projectors, draft models, proposer heads, auxiliary
  models, tokenizers, and vocabulary maps;
- Python wheels, native packages, lockfiles, package entry points, custom model
  code, custom generation code, plugins, middleware, preload modules, wrappers,
  and compatibility proxies;
- startup arguments in final precedence order, configuration files, relevant
  environment variable names and non-secret values, working directory policy,
  filesystem mounts, and service manager configuration;
- Retonr executable, runtime adapter, prompt renderer, parser, serializer, and
  output policy used at the covered boundary;
- platform and execution class, including operating system build, CPU or
  accelerator class, driver, firmware where observable, and numeric precision;
- acquisition source, upstream digest or signature when available, license record,
  and the person or process that approved the artifact for qualification.

Secrets are not stored in the evidence record. The record captures their purpose,
source class, presence, and access policy. Do not publish an ordinary digest of a
low-entropy secret because it can enable guessing.

### Identity rules

- A mutable tag or alias is an address only.
- A model repository commit is useful acquisition evidence but does not identify
  locally converted or quantized bytes.
- A runtime-reported digest identifies only what that runtime documents it to
  cover.
- A response `system_fingerprint` is a change indicator only unless its producer
  specifies a complete, verifiable binding to this manifest.
- A container digest does not replace hashes for mounted models, host libraries,
  device drivers, configuration, middleware, or proxies.
- A source commit does not identify an unreproducible binary. Record the actual
  binary even when a rebuild receipt is present.

## Evidence levels and observability

The levels are cumulative. Reports state the achieved level and every exception.

| Level | Name | Minimum evidence | Permitted statement |
| --- | --- | --- | --- |
| E0 | Unassessed | Endpoint or model name only. | `not_assessed` |
| E1 | Declared | Provider or publisher statement with named scope and date. | `provider_declared_status` |
| E2 | Configured | Exact artifact inventory plus submitted and effective configuration, but no controlled execution trace. | `known_marker_setting_disabled` |
| E3 | Observed | E2 plus boundary captures, loaded-component evidence, differential fixtures, and known-scheme tests with frozen procedures. | `no_known_marker_observed_in_tests` |
| E4 | Controlled local | E3 plus complete local artifact-set identity, OS-enforced outbound denial, process-tree and module evidence, no unresolved executable extension point, and reproducible audit bundle. | `no_known_intentional_marker_enabled` |
| E5 | Independently reproduced | A second clean environment reproduces E4 from the immutable bundle and signs an independent result. | Same E4 finding plus `independently_reproduced` |

E4 is reference-grade for this protocol. It is not a mathematical proof that a
model has no learned or secret signal. A proprietary component in the semantic
generation or response path, uninspectable remote service, unpinned plugin, or
unresolved postprocessor prevents E4. The report must not hide that limitation by
assigning a high aggregate score.

Observability is also reported by layer:

- **Direct:** bytes, code, configuration, or runtime state were inspected at the
  relevant boundary.
- **Derived:** a deterministic inference was made from direct evidence.
- **Behavioral:** only input-output behavior was measured.
- **Declared:** the fact comes from a publisher or provider statement.
- **Opaque:** the layer was not observable.

Behavioral evidence can falsify a configuration claim, but it cannot convert an
opaque layer into a directly inspected one.

### Trusted computing base

E4 assumes a non-malicious host operating system, device driver, firmware, hardware,
and cryptographic implementation. Their exact released binaries and platform state
are recorded where observable, but application qualification does not claim source
inspection of every microcode or kernel component. Deliberate host compromise,
fault injection, and malicious hardware are outside this protocol and require a
separate platform-security assessment.

This boundary does not excuse an opaque component that can interpret a prompt,
select or replace tokens, decode text, parse a response, route a request, or mutate
an export. Such components remain directly in scope because they can intentionally
mark text without compromising the host.

## Reference qualification protocol

### 1. Preregister the audit

Before examining test outputs, freeze:

- the qualified execution tuple;
- insertion-point registry and its digest;
- artifact inclusion rules and canonicalization version;
- exact request classes, prompts, seeds, lengths, languages, and structured-output
  cases;
- differential comparisons and expected invariants;
- detector implementations, keys limited to public or audit fixtures, thresholds,
  abstention rules, and negative controls;
- network, process, file, and module observation methods;
- pass, fail, and unresolved criteria;
- report template and reviewer identities.

The shipped Retonr binary must not contain the research detector suite, watermark
keys, attack code, or a callable detector interface. Audit tooling remains a
separate, local, access-controlled environment.

### 2. Acquire and freeze artifacts

1. Use stable released artifacts. Nightly, development, mutable `main`, and
   auto-updated components are research inputs, not qualified production inputs.
2. Stage every model and executable locally before the qualification run.
3. Verify upstream checksums or signatures when published, then compute Retonr's
   own hashes over the actual bytes.
4. Resolve symlinks, hard links, mounted volumes, package overlays, model shards,
   and caches. Record the resolved bytes, not only directory names.
5. Produce a dependency and executable-extension inventory. For Python, enumerate
   installed distributions, entry points, import paths, `sitecustomize`, startup
   hooks, editable installs, and environment overlays.
6. Build the canonical artifact manifest and stop if any member is mutable during
   qualification.

### 3. Perform static inspection

Static inspection is deterministic and non-executing wherever the format permits.

- Parse all generation, tokenizer, template, adapter, draft, parser, renderer,
  preset, plugin, middleware, and proxy configuration.
- Search source, bytecode metadata, symbols, command help, and configuration keys
  against the frozen insertion registry. Record exact matches and reviewed
  dispositions. A string search is a discovery aid, never absence proof.
- Inspect model-repository executable code, `auto_map`, custom tokenizer and
  processor classes, custom generation directories, package entry points, shared
  library load directives, and preload environment variables.
- Inventory GGUF metadata, tokenizer tables, chat templates, special tokens, and
  metadata overrides.
- Inspect output serializers for Unicode normalization, invisible code-point
  insertion, signature creation, metadata writing, and post-generation replacement.
- Verify that every extension point is disabled, absent, or represented by an
  inspected and hashed component.

Static inspection of weights records tensor names, shapes, types, quantization,
adapters, and lineage. It does not support a finding that the weights contain no
learned marker.

### 4. Resolve effective configuration

Capture effective values after all precedence layers:

```text
artifact defaults
  -> model generation configuration
  -> server or application defaults
  -> preset and environment
  -> command line
  -> request
  -> per-session state
  -> compatibility proxy
```

The configuration record must show:

- target model and loaded-instance identity;
- target and draft model, adapter, projector, and tokenizer identities;
- rendered template, system text, hidden messages, tools, and template arguments;
- ordered logits processors, constraints, sampler chain, sampling parameters,
  seed source, and speculative acceptance configuration;
- detokenizer, stop handling, reasoning parser, tool parser, renderer,
  postprocessors, response serializer, middleware, and proxy order;
- remote-code, custom-generation, plugin-discovery, autoload, fallback, JIT load,
  cloud, remote media, tool, and external integration settings;
- API route and whether a native, compatibility, or proxied endpoint was used.

Unknown defaults fail E4. Supplying explicit request parameters does not cure an
unknown server-side default.

### 5. Enforce and prove offline execution

An application's `offline` setting is useful evidence, not the proof. E4 requires:

1. An operating-system, container, or virtual-machine rule that denies outbound
   network access for the complete process tree while permitting only the declared
   local IPC or loopback endpoint.
2. A below-process observation of DNS attempts, connection attempts, accepted
   sockets, helper daemons, and child processes for startup, load, generation,
   unload, and shutdown.
3. A successful run with all acquisition and update facilities unavailable.
4. Hashes of model caches, package directories, configuration, and executable
   locations before and after the run.
5. A fail-closed result for any non-loopback attempt, download, remote fallback,
   unqualified helper, or artifact mutation.

Disabling DNS or unplugging a network cable alone is insufficient. It can show that
connectivity was not required, but not whether code attempted to connect. Likewise,
a loopback URL does not prove that a local proxy did not forward the request.

### 6. Capture runtime state

Record at startup and at each covered request:

- process ancestry, child processes, executable hashes, loaded modules and shared
  libraries, open artifact files, and listening sockets;
- service version, complete arguments, working directory, configuration hashes,
  and relevant effective environment;
- model load and unload events, model residency, selected backend, numeric type,
  adapter and draft state, and cache identity;
- exact request bytes, rendered prompt bytes and token IDs, generated token IDs
  and log probabilities where supported, decoded chunks, native response bytes,
  and Retonr output bytes;
- errors, retries, fallback decisions, concurrency, cache reuse, and restarts.

Redact user content from a distributable report, but retain an encrypted or
user-controlled local evidence bundle when the user elects to keep it. A redacted
report includes content hashes and fixture IDs sufficient for local reproduction.

### 7. Run differential and sensitivity tests

These tests look for unaccounted transformations. They do not optimize text against
a detector and do not teach marker removal.

| Test | Comparison | Failure signal |
| --- | --- | --- |
| Template equivalence | Independently render the frozen chat template, then compare prompt bytes and token IDs with the server's render-only or template endpoint. | Added, removed, normalized, reordered, or hidden prompt content. |
| Raw versus chat | Send an independently rendered prompt through the lowest-level completion path and compare boundaries with the chat path. | Unexplained chat-only transformation. |
| Direct versus served | Run the same exact artifact and effective generation graph through the direct library and native server path where both exist. | Unexplained server-only token or text transformation. |
| Native versus compatibility API | Exercise the runtime's native route and the claimed compatibility route with equivalent effective inputs. | Proxy-only prompt injection, model substitution, field rewriting, or postprocessing. |
| Token to byte | Detokenize recorded selected tokens with the frozen tokenizer and compare incremental and final output bytes. | Unexplained characters, normalization, trimming, or replacement. |
| Default precedence | Omit one parameter at a time in an audit-only fixture and compare the observed effective state with the documented precedence record. | Hidden or mutable defaults. |
| Extension visibility | In a disposable audit harness, use a declared visible canary in each supported extension class, verify that observation captures it, then confirm the qualification build loads no canary or unlisted extension. | A hook can execute without appearing in evidence. |
| Speculation | Compare speculation disabled with the exact qualified draft path using predeclared deterministic and stochastic fixtures. Record target, draft, acceptance, and output evidence. | Draft substitution, unrecorded proposer, unsupported processor interaction, or unexplained distribution shift. |
| State and concurrency | Repeat fixtures after a clean restart, with empty and warm caches, and alone and in an allowed batch. | Request-, cache-, tenant-, time-, or concurrency-dependent transformation outside the manifest. |
| Proxy bypass | Compare the direct loopback endpoint with every permitted SDK, gateway, and reverse proxy. | A wrapper changes prompts, route, model, or response bytes. |
| Unicode and document boundary | Inventory UTF-8 bytes, Unicode scalar values, default-ignorable characters, variation selectors, bidi controls, homoglyph-sensitive changes, and embedded provenance before and after every boundary. | An unreported insertion, deletion, or normalization. |
| Known-scheme audit | Apply the frozen, local detector procedures only to preregistered fixtures and report flags, no-flags, and abstentions with controls. | Evidence inconsistent with configuration, subject to manual investigation. Passing is not absence proof. |

Fixed seeds support comparison only within a qualified implementation and platform.
Different kernels, numeric precision, batching, and runtime releases can produce
different samples. A mismatch triggers investigation; it is not automatically a
watermark finding.

Known-scheme tests use locked procedures and thresholds. They never adapt prompts,
select candidates, retry generation, or edit text based on detector scores. NIST
notes that evaluation must include representative content, modifications, sample
lengths, unseen generators, and false-positive consequences. The complete
statistical procedure belongs in the separate watermark evaluation protocol.

### 8. Make a disposition

Every insertion point receives one disposition:

- `absent_by_direct_inspection`
- `present_and_disabled`
- `present_and_enabled`
- `present_non_marker_purpose`
- `behaviorally_not_observed`
- `declared_only`
- `opaque`
- `unresolved`
- `out_of_scope`

`present_non_marker_purpose` requires a concise purpose, effective arguments, and
review evidence. For example, a grammar used to produce valid JSON can occupy the
same control point as a marker without being intended as one.

Any `present_and_enabled`, `unresolved`, or in-scope `opaque` disposition prevents
E4. `behaviorally_not_observed` never upgrades to `absent_by_direct_inspection`.

### 9. Reproduce and attest

The qualification runner rebuilds the report from immutable inputs. E5 requires a
second clean environment and independent signer. A signature binds a signer to the
report bytes and evidence digests; it does not make the report's factual claims
true or convert behavioral evidence into source inspection.

## Runtime-specific control surfaces

The following surfaces were present in official stable documentation or current
official source reviewed through the cutoff. Exact releases can add, remove, or
change them, so the installed release remains authoritative for qualification.

| Runtime | Dedicated named watermark control in reviewed surface | Other mark-capable surfaces that require inspection |
| --- | --- | --- |
| Transformers | Yes: `watermarking_config`, `WatermarkingConfig`, and `SynthIDTextWatermarkingConfig`. | Generation configuration, custom logits processors, custom generation code, assistant model, tokenizer, template, remote code. |
| `llama.cpp` and `llama-server` | None identified in the cited public server and sampler surface. | GGUF metadata, templates, sampler chain, logit bias, grammars, draft paths, LoRA, parsers, presets, tools, and middleware around the server. |
| Ollama | None identified in the cited Modelfile, API, and current public type surface. | Template, system and stored messages, parameters, adapters, draft files, renderer, parser, remote models, and client or service wrappers. |
| vLLM | None identified as a dedicated first-party setting in the cited engine surface. | Fully qualified logits processors, IO and parser plugins, Python entry points, middleware, templates, adapters, model implementations, and speculative proposers. |
| MLX LM | None identified as a dedicated first-party setting in the cited library and server surface. | Arbitrary sampler and logits processor callables, template and tokenizer code, adapters, dynamic models, draft model, parser and detokenizer. |
| LM Studio | None identified in the cited public configuration surface. | Model metadata and template overrides, presets, inference runtime, speculative draft, parser and server behavior, integrations, and opaque application components. |

"None identified" is a bounded documentation and source-surface result, not an
absence claim. A release-specific static and runtime audit remains required.

### Transformers

Transformers has an explicit intentional watermark surface. `GenerationConfig`
includes `watermarking_config`; the library documents `WatermarkingConfig` and
`SynthIDTextWatermarkingConfig`, and generation creates watermark logits processors
when configured. `generate()` also accepts custom logits processors, an assistant
model for assisted decoding, and a `custom_generate` implementation that can
replace the decoding loop. Generation settings can load from
`generation_config.json`, while chat templates are tokenizer artifacts saved as
`chat_template.jinja` or tokenizer configuration. Custom model and tokenizer code
can execute when `trust_remote_code=True`.

Primary surfaces:

- [Generation configuration and `generate()`](https://huggingface.co/docs/transformers/main_classes/text_generation)
- [Watermark logits processors](https://huggingface.co/docs/transformers/internal/generation_utils)
- [Chat templates](https://huggingface.co/docs/transformers/chat_templating_writing)
- [Assisted decoding](https://huggingface.co/docs/transformers/main/assisted_decoding)
- [Custom generation methods](https://huggingface.co/docs/transformers/generation_strategies#custom-generation-methods)
- [Custom model code](https://huggingface.co/docs/transformers/models#custom-models)

E4 requirements include exact local model and tokenizer paths, pinned wheels and
native dependencies, `local_files_only`, `trust_remote_code=False`, no custom
generation method, no watermark configuration, no unlisted custom logits or
stopping processors, an explicit generation configuration, and a hashed rendered
template. If custom code or an assistant model is required, it becomes a separately
inspected artifact set rather than an exception.

### `llama.cpp` and `llama-server`

GGUF stores key-value metadata, tensor metadata, tokenizer tables, added tokens,
and `tokenizer.chat_template`; the runtime can also accept metadata and template
overrides. `llama-server` exposes sampler ordering, logit bias, grammar and schema
constraints, a draft model and several speculative methods, LoRA adapters, prompt
templates, reasoning controls and parsers, model presets with environment and
command-line precedence, and template, tokenize, detokenize, and properties
endpoints. The current server also has optional tools, MCP configuration, a web UI,
router model directories, and model autoloading. `--offline` forces cached local
use and prevents network access, but E4 still adds an OS-level network deny.

Primary surfaces:

- [`llama-server` options and endpoints](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [GGUF structure and tokenizer metadata](https://github.com/ggml-org/ggml/blob/master/docs/gguf.md)
- [Model metadata access](https://github.com/ggml-org/llama.cpp/blob/master/include/llama.h)
- [Argument and environment precedence](https://github.com/ggml-org/llama.cpp/blob/master/common/arg.cpp)

The baseline E4 profile uses one exact local GGUF path and one exact binary, disables
router and autoload behavior, tools, MCP, remote media, unneeded UI, mutable slot
state, LoRA, metadata overrides, and speculative decoding, and fixes the complete
sampler chain. Additional profiles may qualify an adapter or draft model only by adding
every artifact and differential test to a new tuple.

### Ollama

An Ollama Modelfile can set the base artifact, parameters, template, system prompt,
adapter, message history, and required version. The current API types also include
renderer, parser, remote host, draft files, template overrides, raw prompt mode,
and render-only debug evidence. `/api/show` reports effective template, system,
parameters, renderer, parser, messages, metadata, projectors, capabilities, remote
model and host, and optional verbose tensor fields. Inventory and running-model
responses provide runtime model digests, but these do not replace Retonr's complete
artifact-set manifest. Ollama can route to cloud models unless cloud features are
disabled.

Primary surfaces:

- [Modelfile reference](https://docs.ollama.com/modelfile)
- [`/api/show`](https://docs.ollama.com/api-reference/show-model-details)
- [Current API request, response, renderer, parser, draft, and remote fields](https://github.com/ollama/ollama/blob/main/api/types.go)
- [Local-only configuration](https://docs.ollama.com/faq)

E4 uses the native Ollama API, an exact package and service executable, a recorded
inventory digest plus independently hashed local blobs, complete verbose
`/api/show` evidence, render-only prompt capture, explicit request options, empty
remote-model and remote-host fields, cloud disabled in configuration and logs, and
OS-enforced network denial. Automatic application or model updates invalidate the
tuple.

### vLLM

Current vLLM engine arguments can load generation configuration from the model by
default; setting the source to `vllm` uses vLLM defaults instead. The engine can
load fully qualified logits processor classes, an IO processor plugin, custom load
formats, remote model code, alternate model implementations, and speculative
proposers. The OpenAI server can load import-path ASGI middleware, custom tool and
reasoning parser plugins, model or request chat templates, LoRA modules, and other
extensions. The plugin loader discovers Python entry points, with behavior gated
in part by `VLLM_PLUGINS`. Current response fingerprint modes describe a version
and configuration hash or a custom literal; they are not Retonr artifact IDs.

Primary surfaces:

- [Engine arguments](https://docs.vllm.ai/en/stable/configuration/engine_args/)
- [`vllm serve` frontend and middleware](https://docs.vllm.ai/en/latest/cli/serve/)
- [Plugin loading](https://docs.vllm.ai/en/stable/api/vllm/plugins/)
- [Speculative decoding](https://docs.vllm.ai/en/stable/features/speculative_decoding/)
- [OpenAI frontend configuration and fingerprint modes](https://docs.vllm.ai/en/latest/api/vllm/entrypoints/openai/cli_args/)

E4 uses exact staged local model and tokenizer paths, pinned Python and native
packages, `trust_remote_code=False`, an explicit model implementation and load
format, either the reviewed model generation configuration or an explicit vLLM
configuration, no unlisted logits processors, IO processor, middleware, parser
plugin, load plugin, or Python entry point, request chat templates disabled, and
no LoRA or speculative path unless separately qualified. `VLLM_PLUGINS` is an
explicit empty allowlist backed by an installed-entry-point inventory, not an
assumption that an unset variable disables every plugin group.

### MLX LM

The MLX LM Python generation API accepts arbitrary sampler and ordered logits
processor callables. Its server builds samplers and logits processors from request
arguments, loads tokenizer chat templates, supports template overrides and remote
tokenizer code, accepts adapters and per-request model paths, and can load a draft
model for speculative decoding. A repository identifier can download from the Hub.
The project states that its HTTP server has only basic security checks and is not
recommended for production.

Primary surfaces:

- [MLX LM generation and sampling API](https://github.com/ml-explore/mlx-lm)
- [HTTP server, dynamic model, adapter, and draft fields](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/SERVER.md)
- [Current server implementation](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/server.py)

An E4 research profile must pin the complete Python environment and source, use an
exact local model path, disable remote code, enumerate import paths, capture the
resolved tokenizer and template, and prohibit per-request model, adapter, and draft
substitution. The server warning and dynamic path surface make MLX LM unsuitable
for Retonr's baseline production qualification. A direct, owned library wrapper may be
more inspectable, but it is a different artifact set.

### LM Studio

LM Studio separates the application and downloadable inference runtimes. The
native v1 load endpoint can echo final load configuration, model listing reports
loaded instances, and `lms log stream` can expose the exact strings sent to and
received from a model. The application can derive a prompt template from model
metadata or apply a user override. Presets can bundle a system prompt and inference
parameters. APIs support draft models for speculative decoding, and the native chat
API can invoke configured or remote MCP integrations. LM Studio documents that
downloaded models and core local inference can operate offline.

Primary surfaces:

- [Native model load and echoed configuration](https://lmstudio.ai/docs/developer/rest/load)
- [Model input and output log stream](https://lmstudio.ai/docs/cli/serve/log-stream)
- [Prompt template overrides](https://lmstudio.ai/docs/app/advanced/prompt-template)
- [Config presets](https://lmstudio.ai/docs/app/presets)
- [Runtime selection](https://lmstudio.ai/docs/cli/runtime/runtime)
- [Offline operation](https://lmstudio.ai/docs/app/offline)
- [Application terms and source-code restrictions](https://lmstudio.ai/app-terms)

The application and daemon contain opaque components, and runtime selection and
updates are mutable. Official logs and echoed configuration materially improve E3
behavioral evidence, but they do not expose every internal prompt, sampler, parser,
or postprocessor implementation. LM Studio therefore cannot receive E4 under this
protocol without a source-complete or vendor-attested path that closes those gaps.
An E3 profile still pins the application, daemon, selected runtime, local model,
templates, presets, loaded instance, load configuration, draft state, integrations,
and network policy, and labels the remaining components `opaque`.

### Compatibility proxies and remote opaque providers

Every SDK, gateway, reverse proxy, service mesh, cache, and compatibility adapter is
part of the path. A proxy can add a system prompt, strip a field, retry against a
fallback, change a model alias, or modify returned text. A qualified local path
either bypasses it or hashes, configures, traces, and differentially tests it like
any other executable component. Proxy environment variables are disabled for
direct loopback probes.

A remote provider cannot meet E4 because the client does not control or inspect the
server artifacts and cannot enforce server-side network or logging policy. The
strongest accurate result is one of:

- `provider_declared_no_marker` at E1, with the exact statement and scope;
- `no_known_marker_observed_in_tests` at E3, with opaque internals stated;
- `opaque_provider_status_unknown` when no adequate declaration exists;
- `known_marker_enabled` when the provider or direct evidence establishes it.

Black-box differences, response fingerprints, or a compatibility schema cannot
upgrade a provider declaration into local artifact assurance. A provider model
alias or fingerprint change is a requalification trigger.

## Inherited, radioactive, copied, and nested evidence

### Learned and radioactive signals

A model can learn to emit a watermark-like signal from marked training or
distillation data even when no inference-time watermark processor is present.
[Watermarking Makes Language Models Radioactive](https://arxiv.org/abs/2402.14904)
demonstrates scheme-specific residual evidence in models trained on watermarked
synthetic data. [On the Learnability of Watermarks for Language Models](https://arxiv.org/abs/2312.04469)
shows that decoding watermarks can be distilled into model behavior under studied
conditions.

Retonr reports this separately:

- `inherited_marker_evidence_observed`
- `no_inherited_marker_evidence_in_named_test`
- `inherited_marker_status_unknown`

The second status is not a negative claim about the weights. It names the detector,
artifact, prompts, sample, and procedure. Conversion, quantization, merging, or
fine-tuning creates a new artifact set and does not presumptively cleanse or
preserve the signal.

### Source-carried marks

Prompts, retrieved passages, examples, quoted drafts, prior assistant messages, and
tool outputs can already contain statistical or Unicode marks. A model can copy or
transform them. The audit therefore distinguishes:

- a mark present in source input;
- a mark intentionally added by the qualified runtime;
- a mark learned into model behavior;
- a mark added after the runtime;
- a mark whose layer is unknown.

Finding a source-carried mark does not show that the current runtime inserted it.
Finding no runtime marker does not show that output contains no copied evidence.

### Nested and colliding marks

Multiple marks can coexist, overwrite one another, or change each other's detector
statistics. Research has examined explicit independent-key nesting
([A Nested Watermark for Large Language Models](https://arxiv.org/abs/2506.17308))
and collision between logit-based marks
([Lost in Overlap](https://aclanthology.org/2025.findings-naacl.37/)). A provider
mark, an inherited signal, a source-carried mark, a local postprocessor, and a
signed document manifest can all describe different layers of one output.

Reports never collapse these to one `watermarked` boolean. Each observation has a
scheme or mechanism, layer, key class when known, affected span, evidence level,
and relationship to other layers. Detector interaction or non-detection cannot be
represented as successful removal.

### Invisible Unicode and legitimate provenance

Default-ignorable characters and variation selectors can be visually silent while
remaining distinct code points. C2PA 2.4 also defines an intentional use of Unicode
variation selectors to embed manifests in unstructured text. The scanner must
therefore inventory and report such characters without assuming malicious intent
or deleting them. C2PA calls for creator and publisher control over included
provenance and treats signed claims, hard bindings, and soft bindings as different
evidence ([C2PA 2.4](https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html)).

## Negative-claim vocabulary

### Allowed statuses

Use only a status whose prerequisites are in the report:

| Status | Meaning |
| --- | --- |
| `not_assessed` | No adequate assurance work was performed. |
| `inspection_incomplete` | Work began, but required evidence is missing or unresolved. |
| `known_marker_enabled` | A known intentional marker is enabled in the covered path. |
| `known_marker_setting_disabled` | A named setting is disabled; other insertion points are not covered. |
| `no_known_marker_observed_in_tests` | Frozen tests did not observe a supported marker; this is behavioral evidence only. |
| `no_known_intentional_marker_enabled` | E4 found no enabled known intentional marker in the exact qualified path. |
| `provider_declared_no_marker` | A provider made a scoped statement; Retonr did not verify internals. |
| `provider_declared_marker_enabled` | A provider states that a marker is used in the scoped service. |
| `opaque_provider_status_unknown` | Provider internals and marking status are not adequately disclosed. |
| `inherited_marker_evidence_observed` | A named procedure found evidence consistent with inherited signal. |
| `no_inherited_marker_evidence_in_named_test` | One named inherited-signal test did not meet its threshold. |
| `source_carried_marker_observed` | Input or retrieved content already contained marker evidence. |
| `external_provenance_present` | Metadata, signature, Content Credential, or service record is present outside linguistic token choices. |

### Prohibited claims

Retonr product text, reports, tests, and release notes must not say:

- `watermark-free`, `unwatermarked`, or `clean output`;
- `undetectable`, `untraceable`, or `anonymous`;
- `proved human`, `human-authored`, or `not AI-generated`;
- `all marks removed`, `watermark removed`, or `detector-proof`;
- `no provider record`, unless the record system itself was directly audited under
  a separately stated retention scope;
- `guaranteed` based on detector output, config inspection, or local execution.

Theoretical work constructs keyed watermarks that are computationally hidden from
observers without the key, so a public negative scan cannot cover every scheme
([Undetectable Watermarks for Language Models](https://proceedings.mlr.press/v247/christ24a.html)).
Other work establishes limits on strong watermark robustness under stated oracle
assumptions ([Watermarks in the Sand](https://proceedings.mlr.press/v235/zhang24o.html)).
These results reinforce narrow, conditional language in both positive and negative
directions.

## Attestation and report schema

The canonical machine report uses this logical schema. A concrete JSON Schema must
fix required fields, enums, formats, and canonicalization before implementation.

```yaml
schema_version: retonr.local_marker_assurance.v1
report_id: <content-addressed identifier>
claim:
  status: no_known_intentional_marker_enabled
  evidence_level: E4
  independently_reproduced: false
  scope_start: rendered_prompt
  scope_end: retonr_output
  qualification_state: valid
subject:
  artifact_set_id: sha256:<digest>
  runtime_path_id: sha256:<digest>
  effective_config_id: sha256:<digest>
  request_class_id: sha256:<digest>
  platform_class_id: sha256:<digest>
  evidence_registry_id: sha256:<digest>
  audit_procedure_id: sha256:<digest>
components:
  runtime: []
  models: []
  tokenizers: []
  templates: []
  adapters: []
  draft_or_proposers: []
  processors_and_samplers: []
  parsers_and_postprocessors: []
  plugins_and_middleware: []
  proxies: []
  retonr_and_export: []
insertion_points:
  - id: logits_processors
    observability: direct
    disposition: absent_by_direct_inspection
    evidence: [sha256:<digest>]
effective_state:
  startup_record: sha256:<digest>
  request_policy: sha256:<digest>
  rendered_template: sha256:<digest>
  sampler_graph: sha256:<digest>
  postprocessor_graph: sha256:<digest>
offline_proof:
  enforcement: <method and scope>
  network_trace: sha256:<digest>
  non_loopback_attempts: 0
  artifact_mutations: 0
runtime_observation:
  process_tree: sha256:<digest>
  loaded_modules: sha256:<digest>
  opened_artifacts: sha256:<digest>
tests:
  suite_id: sha256:<digest>
  results: sha256:<digest>
  unresolved: []
opaque_layers: []
limitations: []
requalification:
  trigger_set_id: sha256:<digest>
  triggered: false
evidence_bundle:
  manifest: sha256:<digest>
  index: sha256:<digest>
attestation:
  signer_identity: <local or organizational identity>
  signature_algorithm: <registered algorithm>
  signature: <detached signature>
```

The human report renders the same facts and adds:

- an executive finding with exact scope;
- a component and insertion-point table;
- all direct, derived, behavioral, declared, and opaque evidence;
- every failed, skipped, abstained, and unresolved test;
- stage-by-stage byte and Unicode observations;
- provider declarations quoted only within source limits and linked to the original;
- limitations and prohibited inferences;
- requalification state and triggers;
- evidence retention and redaction choices made by the user.

No report may silently omit an opaque component or convert `inspection_incomplete`
to a negative finding.

## Requalification triggers

Qualification is trigger-based. It does not become permanent through age or use.
Any item below invalidates the existing tuple or requires the named evidence to be
re-run:

- runtime, application, daemon, binary, shared library, container, build flag,
  patch, package lock, driver, firmware, execution backend, or numeric type change;
- model shard, GGUF, conversion, quantization, configuration, tokenizer, special
  token, chat template, system prompt, preset, grammar, schema, adapter, projector,
  draft model, proposer, or parser change;
- environment, command line, configuration precedence, plugin entry point,
  middleware, preload library, compatibility proxy, service manager, or filesystem
  mount change;
- model alias, tag, endpoint, DNS target, TLS identity, provider plan, region,
  response fingerprint, fallback policy, or provider marking declaration change;
- autoload, JIT load, hot swap, automatic update, cache replacement, model pull,
  or remote fallback event;
- request class expansion, including a new route, streaming mode, tool path,
  structured-output path, reasoning mode, language, modality, or export format;
- evidence registry, detector, threshold, normalization, tokenizer, fixture corpus,
  observation tool, canonicalization, or report schema change;
- a new credible disclosure or research result identifying a plausible insertion
  point or known scheme not covered by the registry;
- any unexplained byte, token, module, file, process, network, or differential-test
  change;
- failure to reproduce the immutable evidence bundle.

A process restart with the same tuple requires startup identity and network checks
again. It does not require a full independent research study unless one of the
triggers fires.

## Retonr inspection, preservation, removal, and reporting policy

### Mandatory project duties

Retonr must:

1. Qualify an exact path and fail closed on identity, configuration, extension,
   network, or evidence mismatch. It must not silently substitute a provider,
   model, runtime, template, or compatibility route.
2. Keep acquisition and research detector dependencies out of the shipping
   editorial loop. Prefer operating-system evidence and small deterministic tools
   over new runtime dependencies.
3. Inspect non-destructively. Preserve raw source, generated candidate, and export
   bytes according to the user's local retention choice before any normalization.
4. Inventory invisible Unicode, file provenance, and signature layers without
   assuming that presence is harmful or that absence establishes authenticity.
5. Never enable a known intentional statistical or output marker in a qualified
   default path.
6. Never silently strip, normalize, forge, invalidate, or replace a suspected mark,
   signed manifest, or provenance record.
7. Never call a detector from generation, retry, ranking, acceptance, style memory,
   feedback learning, or a temporal knowledge graph. Detector feedback cannot
   select wording.
8. Never claim that ordinary editing removed a marker. A faithful editorial change
   may incidentally change detector evidence, but that is neither the objective nor
   a guaranteed outcome.
9. Report positive, negative, uncertain, opaque, and source-carried observations
   with the same stage, scheme, and evidence specificity.
10. Preserve editorial access to user-owned source and candidates. A failed
    qualification can block the `qualified` label and automatic use, but it cannot
    confiscate text or make an authorship decision.

### Explicit user actions

If a user asks to alter a visible or invisible provenance layer, Retonr may support
only a deliberate, reviewable, format-aware edit that:

- shows what layer will change and what validation or signature may break;
- writes a derivative rather than overwriting the only copy;
- records the user's choice locally when the user elects to retain a record;
- does not optimize against a detector or promise removal;
- does not misrepresent the resulting artifact's history.

This policy defines safeguards, not a removal procedure.

### User-specific responsibilities

The user or deploying organization decides and remains responsible for:

- whether they have authority to use a model, provider, source document, adapter,
  or retrieved material;
- which provider terms, employment rules, publication policies, confidentiality
  commitments, or disclosure requirements apply to their use;
- whether to accept an E1, E2, E3, or opaque provider path instead of E4;
- whether to retain, disclose, preserve, or deliberately alter provenance evidence;
- the editorial message, factual claims, approvals, and final decision to publish;
- protection of provider credentials, watermark keys, local audit evidence, and
  sensitive prompts;
- interpretation of detector evidence in its context and avoidance of unsupported
  authorship or misconduct accusations.

Retonr provides evidence and controlled tools. It does not make the user's legal,
ethical, editorial, or disclosure decision.

### Required qualification notice

Every user-facing E3, E4, or E5 report includes substantially this notice:

> This result describes one identified generation and editing path. It is not a
> certificate that the text is watermark-free, human-authored, anonymous,
> original, legally compliant, or untraceable. It does not rule out learned signals
> in model weights, unknown or secret schemes, marks carried from source material,
> provider-side records, or changes made outside the reported boundaries.
> Statistical detector results can produce false positives, false negatives, and
> abstentions. You retain editorial control and responsibility for what you use,
> disclose, and publish.

## Release gates

A Retonr runtime profile may advertise `no_known_intentional_marker_enabled` only
when all E4 requirements pass in continuous qualification for the exact released
artifact set. Release checks must verify:

- one canonical artifact-set manifest and no mutable aliases as identity;
- complete insertion-point dispositions with no in-scope opaque or unresolved
  executable layer;
- stable-release binaries and locked dependencies;
- offline enforcement and trace evidence;
- byte-boundary, template, tokenizer, proxy, postprocessor, and export fixtures;
- speculation, adapter, plugin, middleware, tools, MCP, remote code, and fallback
  disabled unless each is separately qualified;
- known-scheme audit separation from product code;
- trigger detection that invalidates stale attestations;
- deterministic report generation and signature verification;
- policy checks that reject prohibited claims.

Ollama and a pinned `llama-server` sidecar are the baseline practical candidates
for this gate. Transformers can meet it for a tightly owned library wrapper. vLLM can
meet it in a fully pinned self-hosted environment. MLX LM needs a hardened owned
wrapper and complete environment control. LM Studio and remote providers remain
bounded or opaque until their unobservable layers are closed by stronger evidence.

The product outcome is intentionally modest: Retonr can show exactly what it ran,
what it inspected, what it observed, what it could not observe, and when that
evidence stopped applying. It cannot certify the metaphysical origin of prose.
