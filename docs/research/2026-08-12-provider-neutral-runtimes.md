# Provider-neutral, user-controlled model runtimes

Research date: August 12, 2026

## Decision summary

Retonr should qualify two local generation paths before expanding the runtime
catalog:

1. An existing user-managed Ollama service through its native API.
2. A Retonr-managed, pinned `llama-server` sidecar using an exact local GGUF
   artifact.

Those paths cover Windows, macOS, and Linux, and they exercise two different
ownership models: attach to an installed runtime, and own the complete runtime
process. They are enough to prove that the inference port is genuinely neutral.
Adding more adapters before both paths pass the same qualification suite would
increase surface area without increasing confidence.

LM Studio, vLLM, and MLX LM are useful candidates after the core paths work.
They must not be called supported merely because they expose an
OpenAI-compatible endpoint. Protocol compatibility does not establish runtime
identity, artifact identity, effective settings, offline behavior, or output
policy.

For 1.0, generic OpenAI-compatible transport should be restricted to local
experimental use unless a runtime-specific identity driver can bind the exact
server, artifact set, template, parameters, and execution class. Authenticated
remote and local-network inference is a separate trust mode and should remain
after the local-first 1.0 path.

## Support vocabulary

Use these terms consistently in CLI output, documentation, schemas, and tests:

| Term | Meaning |
| --- | --- |
| Catalogued | Retonr has reviewed metadata for a runtime or model artifact, but has not established that it works. |
| Candidate | The combination is eligible for evaluation. It has no product support claim. |
| Qualified | One exact runtime, artifact set, policy, operating system, hardware class, and execution class passed all predeclared gates. |
| Activated | The user selected a currently valid qualified record for one role on this installation. |
| Invalidated | Evidence no longer authorizes use because an identity, policy, dependency, or relevant environment input changed. |

A runtime family, model family, mutable tag, repository name, API dialect, or
successful load is never qualified by itself.

## Recommended runtime matrix

| Runtime path | Current disposition | Operating systems | Interface | Artifact and offline path | Hardware path | Identity and control assessment | Runtime license |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Ollama native API | Implemented candidate. First external-runtime qualification target. | Native Windows, macOS, and Linux packages are documented. | Native `/api/*` API. Prefer it over the compatibility API because it exposes version, installed models, model details, running state, context, and residency data. | Imports GGUF, Safetensors models, and adapters. Local-only mode requires cloud features to be explicitly disabled. Model pulls are network actions and are never part of rewrite execution. | CPU plus NVIDIA, AMD ROCm, Vulkan, and Apple Metal paths, with platform-specific limits. | Better discovery than a generic compatibility server, but still incomplete without a runtime executable digest, a verified mapping from the Ollama package digest to Retonr's artifact set, all effective model configuration, and `/api/ps` checks. Windows and macOS automatic updates can invalidate an active binding. | The public Ollama repository is MIT licensed. Model licenses are separate. Review the exact distributed package and notices rather than inferring package terms solely from the source repository. |
| Pinned `llama-server` sidecar | Planned candidate. Preferred portable and second qualification target. | Official builds and build instructions cover Windows, macOS, and Linux. | Native server endpoints plus OpenAI-compatible endpoints. Use the smallest stable native subset needed for health, properties, tokenization, and bounded generation. | Exact local GGUF path with `--offline`; no `-hf` shortcut, router mode, model directory, autoload, remote media, or writable slot state. Retonr stages and hashes the artifact before launch. | CPU, Metal, CUDA, HIP, Vulkan, SYCL, OpenVINO, and other builds exist. Each exact build and execution class is a separate candidate. | Strongest controllable path because Retonr can pin and hash the executable, libraries, model, template, arguments, environment, bind address, and process lifetime. `/props` reports useful state but does not replace Retonr's own hashes. | MIT. Model licenses and accelerator library redistribution terms remain separate. |
| Generic OpenAI-compatible local endpoint | Transport candidate only. Never qualified without a runtime-specific identity driver. | Depends on the server. | A deliberately small non-streaming chat or completion subset with JSON Schema where independently confirmed. | Acquisition and artifact storage are outside the protocol. | Depends on the server. | The protocol normally exposes a caller-selected model name, not authoritative artifact bytes or effective runtime state. An alias can point at different weights. Treat this as untrusted transport, not a support boundary. | Depends on the server and model. |
| LM Studio native v1 API | Later external-runtime candidate. Do not bundle. | Official requirements list Apple silicon macOS, Windows x64 and ARM, and Linux x64 and ARM64, with narrower tested Linux coverage. | Prefer native `/api/v1/*` discovery and load endpoints; use OpenAI-compatible inference only where the native endpoint cannot meet the bounded request contract. | Can operate offline after models and runtimes are present. `lms import` supports move, copy, hard link, and symbolic link. Qualification must use a private copied artifact or independently hash and monitor the resolved target. | LM Studio selects separately downloadable runtimes, including llama.cpp and MLX paths. The selected runtime and version are part of identity. | The v1 model list exposes key, size, format, quantization, loaded instance, and load configuration, but no documented content digest. Runtime selection and updates are mutable. This is insufficient for exact qualification unless Retonr owns and rechecks the artifact bytes and obtains a stable runtime identity. | Proprietary application terms permit personal and internal business use through published interfaces. The terms restrict redistribution and other uses. Obtain a release-specific legal review before advertising or packaging integration. Model and underlying runtime licenses are separate. |
| vLLM OpenAI-compatible server | Later Linux workstation or user-managed server candidate. Not a cross-platform default. | Main GPU support requires Linux and does not natively support Windows. WSL and community forks are not equivalent support. The project documents a community-maintained vLLM Metal plugin for Apple silicon as a distinct path. | Mature OpenAI-compatible serving API with structured output support, but runtime-specific startup and identity evidence are still required. | Use an exact staged local model snapshot, local tokenizer, explicit revisions, `trust_remote_code=false`, and offline Hub settings. Do not allow the server to resolve a repository name during a qualified run. | Strong NVIDIA, AMD, Intel, CPU, and server-class paths. Apple support uses the separate community-maintained plugin. | Suitable when Retonr launches an exact environment or can attest a self-hosted deployment. Defaults are too dynamic for qualification: model generation configuration is loaded automatically, model implementation can fall back, load formats can use plugins, and custom logits or IO processors can be loaded. | Apache-2.0. Model, CUDA, ROCm, plugin, image, and transitive licenses remain separate. |
| MLX LM server | Experimental Apple-focused candidate. Not a 1.0 dependency. | MLX LM describes itself as an Apple silicon package. The lower-level MLX framework now also documents Linux CUDA and CPU packages, but that does not by itself qualify MLX LM serving on those platforms. No Windows path is documented. | OpenAI-like HTTP server. Its own documentation says it has only basic security checks and is not recommended for production. | A model argument can be a local path or a Hub repository. Repository identifiers may download. Qualified experiments must use staged local paths and prevent network access. | Best fit is Apple silicon. Evaluate lower-level MLX Linux support separately if MLX LM publishes matching support evidence. | Server model identifiers are paths or aliases, and no robust content identity contract is documented. The server should remain an opt-in experiment until it has a hardened identity and conformance wrapper. | MIT. Model licenses are separate. |

Primary platform and interface evidence:

- [Ollama operating systems and hardware](https://docs.ollama.com/gpu),
  [Windows](https://docs.ollama.com/windows),
  [macOS](https://docs.ollama.com/macos), and
  [Linux](https://docs.ollama.com/linux)
- [Ollama local-only configuration](https://docs.ollama.com/faq),
  [installed model inventory](https://docs.ollama.com/api/tags),
  [running model state](https://docs.ollama.com/api/ps), and
  [model import](https://docs.ollama.com/import)
- [`llama-server` options and API](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md),
  [llama.cpp build backends](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md), and
  [llama.cpp releases](https://github.com/ggml-org/llama.cpp/releases)
- [LM Studio system requirements](https://lmstudio.ai/docs/app/system-requirements),
  [offline operation](https://lmstudio.ai/docs/app/offline),
  [native model inventory](https://lmstudio.ai/docs/developer/rest/list),
  [model load state](https://lmstudio.ai/docs/developer/rest/load), and
  [`lms import`](https://lmstudio.ai/docs/cli/local-models/import)
- [vLLM GPU requirements](https://docs.vllm.ai/en/latest/getting_started/installation/gpu/),
  [CPU support](https://docs.vllm.ai/en/latest/getting_started/installation/cpu/),
  and [engine arguments](https://docs.vllm.ai/en/stable/configuration/engine_args/)
- [MLX LM](https://github.com/ml-explore/mlx-lm),
  [MLX LM server warning and API](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/SERVER.md),
  and [MLX installation targets](https://ml-explore.github.io/mlx/build/html/install.html)

## Provider-neutral adapter shape

Do not implement a single permissive `OpenAiCompatibleBackend` and treat every
server as interchangeable. Split backend integration into three explicit parts:

1. A transport dialect sends bounded requests and decodes bounded responses.
2. An identity driver discovers the exact runtime, artifact set, effective
   configuration, and execution class.
3. An acquisition driver describes whether Retonr downloads product-managed
   bytes, invokes an explicit runtime import, or only attaches to user-managed
   state.

The inference port can remain backend-neutral. Qualification must bind the full
driver set. Reusing an OpenAI-compatible transport implementation is useful;
reusing its identity assumptions is not.

Each backend driver needs to report at least:

- Backend implementation ID and adapter contract version
- Runtime version, build revision, executable or package digest, loaded native
  libraries, and relevant build features
- Artifact-set ID and every member digest
- Template, tokenizer, system prompt, preset, adapter, draft model, parser,
  renderer, and generation-configuration digests
- Effective context, output, sampling, reasoning, grammar, and structured-output
  settings
- Operating system, architecture, hardware backend, device identities without a
  stable telemetry identifier, offload class, and CPU, GPU, or hybrid execution
- Offline and network policy, endpoint scope, authentication mode, proxy policy,
  and redirect policy
- Acquisition owner, license decision, and whether runtime or artifact updates
  can happen automatically
- Capability evidence and the qualification records that currently authorize
  each role

An adapter must reject a required field it cannot establish. It must not fill
unknown identity with a convenient model name.

## Artifact identity model

The current one-file `ArtifactId` concept is appropriate for a GGUF weight file,
but not sufficient for every runtime. Add an artifact-set manifest before
qualifying directory-based models or composed runtime packages.

An artifact-set ID should be the digest of a canonical manifest that contains:

- Normalized relative path, byte length, and SHA-256 digest for every required
  file
- Exact upstream origin and immutable full revision when available
- Primary weights, shards, indexes, tokenizer files, model configuration,
  generation configuration, and chat template
- Optional adapter, projector, draft model, and custom vocabulary members
- Reviewed license records for the model, tokenizer, code, adapters, and bundled
  runtime components
- Conversion and quantization tool identities, arguments, logs, and source
  artifact-set ID
- Explicit exclusions, with evidence that the runtime cannot load an excluded
  executable or configuration file

For a single GGUF, retain both `weight_digest` and `effective_package_id`. The
effective package includes every input that can change token selection. This
prevents a stable weight hash from hiding a changed template, system prompt,
adapter, grammar, or generation preset.

For an Ollama model, keep these identities separate:

- Retonr's source artifact or artifact-set digest
- The Ollama inventory digest returned for the installed model
- A digest of the complete effective model description returned by `/api/show`
- The mutable runtime reference or tag used only to address the package

The official API documents a `digest` field but does not define it as the digest
of the original imported GGUF bytes. Retonr must not equate those identities
without a verified import mapping. Hash all output-affecting `/api/show` fields,
including parameters, template, system, messages, renderer, parser, requirements,
remote model fields, and projectors when present. Reject any remote-backed model
for local qualification.

For a Hugging Face snapshot, resolve a full commit hash and create Retonr's own
canonical file manifest after download. Hub cache revisions and links are useful
acquisition evidence, not the active artifact identity. Hugging Face documents
full commit revisions, snapshot downloads, and a cache whose files may be links
to shared blobs. Copy the approved members into private staging, validate paths,
hash them, and activate the immutable result. Prefer Safetensors. Do not load
pickle-based weights or unreviewed remote code.

Sources:

- [Hugging Face file and snapshot downloads](https://huggingface.co/docs/huggingface_hub/guides/download)
- [Hugging Face cache and revision behavior](https://huggingface.co/docs/huggingface_hub/en/package_reference/file_download)
- [Hugging Face pickle security guidance](https://huggingface.co/docs/hub/security-pickle)
- [Safetensors format](https://huggingface.co/docs/safetensors/main/en/index)

## Offline import and activation rules

Offline import is a first-class acquisition path, not a fallback after a failed
network request.

1. Inspect the source without executing it. Reject special files, device files,
   alternate data streams, path traversal, links escaping the source, excessive
   members, and unsupported formats.
2. Copy required bytes into a private staging directory. Do not activate hard
   links or symbolic links because another process can mutate their targets.
3. Hash every member and the canonical artifact-set manifest. Compare any
   supplied digest, but never substitute an upstream checksum for Retonr's own
   read.
4. Review all applicable license records before invoking a runtime import.
5. Import or launch under an outbound-network deny test. A local path is not
   proof that the runtime will not resolve missing files or plugins remotely.
6. Inspect the resulting runtime package and effective configuration. Bind it to
   the staged source identity rather than trusting a new display name.
7. Run smoke and device suites. Qualification remains a separate action.
8. Activate with one atomic pointer update only after current qualification and
   all identities are rechecked.
9. Recheck source bytes and runtime state immediately before and after every
   generation batch. Any drift discards the entire batch and invalidates the
   binding.

Runtime-specific consequences:

- Ollama cloud models and web search must be unavailable in local-only mode.
  Require the documented cloud-disable setting and retain a no-network test.
- `llama-server` must receive `--offline` and one exact `-m` path. Do not use
  `-hf`, router autoload, a model directory, remote media, or automatic fitting
  in a qualified launch.
- LM Studio import must not use its default move behavior, hard links, or
  symbolic links for a Retonr-managed qualified artifact. Runtime downloads,
  selection, and app updates are separate explicit operations that invalidate
  prior evidence when identity changes.
- vLLM must use exact local model and tokenizer paths, explicit load format,
  `trust_remote_code=false`, an explicit model implementation, no custom loader,
  IO, or logits-processor plugins, and no automatic generation configuration.
  Set `--generation-config vllm` and send every qualified generation parameter.
- MLX LM experiments must use an exact local path. Its documented behavior of
  downloading a Hub model when absent is forbidden during qualification and
  generation.

## Large-document and context-window contract

A runtime's context number is a capacity input, not a document support claim.
Ollama chooses default context from available VRAM and documents that larger
contexts require more memory. `llama-server` can load context size from model
metadata and exposes context-scaling controls. vLLM derives context from model
configuration by default and can automatically choose a value that fits memory.
LM Studio reports both model maximum and loaded-instance context. These values
describe different things and can change without changing the visible model
name.

Retonr must record four separate values:

| Value | Definition |
| --- | --- |
| Native model context | The upstream or artifact-declared training or supported limit. This is untrusted metadata until tested. |
| Effective runtime context | The value observed after this exact runtime, artifact, hardware backend, and load policy start. |
| Qualified context envelope | The largest prompt plus output range that passed fidelity, position, memory, and cancellation gates for the exact tuple. |
| Per-request source budget | The conservative source allowance after reserving tokens for templates, instructions, schemas, protected facts, document plan, neighborhood context, output, and a safety margin. |

The per-request source budget is always smaller than the qualified context
envelope. It is calculated with the exact qualified tokenizer. When Retonr does
not own a matching tokenizer, it uses a prequalified conservative byte envelope
and abstains above it. No runtime truncation, context shifting, summarization,
or overflow recovery may silently change the admitted source.

Nominal fit also says nothing about reliable use. The primary study
[Lost in the Middle](https://arxiv.org/abs/2307.03172) found substantial
position-dependent degradation even in explicitly long-context models. Runtime
qualification therefore includes document-shaped tests at multiple admitted
lengths with protected facts and cross-references at the beginning, middle,
end, and chunk boundaries. Passing a short prompt suite does not qualify a long
context envelope.

Runtime sources:

- [Ollama context allocation and memory guidance](https://docs.ollama.com/context-length)
- [`llama-server` context and scaling controls](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [vLLM model context derivation and automatic fitting](https://docs.vllm.ai/en/stable/configuration/engine_args/)
- [LM Studio model and loaded-instance context](https://lmstudio.ai/docs/developer/rest/list)

### Required multi-pass document flow

Large TXT, Markdown, DOCX, and future document formats use one format-neutral
transaction over adapter-owned units:

1. The document adapter parses the source, records its complete digest, assigns
   stable unit IDs, inventories structure, and identifies bounded eligible prose
   spans. Unsupported or ambiguous structure is protected or rejects the
   transaction according to the format policy.
2. A planning pass builds a versioned document plan from bounded observations.
   It records outline, intended audience, declared style, terminology, entities,
   protected values, claims, cross-references, repeated phrases, and constraints.
   The plan cites source unit IDs and may express uncertainty. It is evidence,
   not a semantic proof.
3. The planner partitions eligible prose into deterministic target units and
   bounded context windows. A window may include read-only neighboring units and
   relevant document-plan entries, but the model may emit edits only for the
   named target units. Window overlap never creates two writers for one byte
   range.
4. Each generation request uses the same activated runtime tuple and carries the
   document-plan digest, target unit IDs, local context, protected sentinels, and
   explicit output contract. Chunk size follows the qualified budget and risk,
   not a fixed character count or the runtime's advertised maximum.
5. Unit validation checks literal invariants, claims, edit bounds, style rules,
   and output syntax. Passing units remain staged and are not written into the
   source.
6. A document-level consistency pass checks terminology, names, tense, voice,
   repeated definitions, headings, links, references, numbering, citations, and
   relationships across every staged unit, including units that were not
   rewritten.
7. The format adapter applies non-overlapping edits to a staged copy, reparses
   the result, verifies structural fingerprints and all non-target bytes, and
   then runs final document verification.
8. Commit is atomic at the selected scope. Runtime drift, cancellation, failed
   consistency, failed reassembly, or any stale source digest returns the exact
   original and keeps staged diagnostics separate.

The document plan must not become a lossy substitute for source context. A unit
receives exact local text plus only the global evidence it needs. Any claim that
depends on distant content requires the exact cited source units in its read-only
window or causes abstention.

### File and folder safety

- The default file workflow writes to standard output, an explicit new path, or
  a sibling staged result. It does not modify the input in place.
- An explicit replacement mode writes a verified temporary sibling, flushes it,
  checks that the source digest is unchanged, and uses an atomic replacement
  primitive only where that primitive is qualified on the current filesystem.
- A folder job freezes an input manifest before work. It rejects path traversal,
  symlink and junction escapes, recursive output inclusion, duplicate canonical
  paths, unsupported file types, and case-collision hazards.
- Folder output defaults to a separate explicitly named root. Relative paths and
  untouched files are preserved according to a declared copy policy. Retonr
  never deletes a source tree as part of rewriting.
- Folder atomicity is explicit. `document` mode may commit independently verified
  files. `folder` mode exposes no final output unless every selected document and
  the complete output manifest pass. The default must not be inferred from file
  count.
- Resumption uses source, plan, unit, runtime, artifact, and policy digests.
  Staged candidates with any mismatch are discarded, never adapted silently.
- Parallel rewriting is allowed only within the qualified concurrency envelope.
  Results are ordered by stable unit ID, and document consistency remains a
  deterministic final gate.

## Strict qualification invariants

The following invariants apply to every backend. A runtime-specific adapter may
strengthen them but may not weaken them.

### Identity and immutability

- A qualification record identifies exact adapter, runtime, runtime package,
  artifact set, tokenizer, template, output contract, request policy, operating
  system, hardware backend, and execution class.
- A mutable tag, alias, path, model key, display name, or served model name is an
  address only. It never serves as artifact identity.
- Runtime version text without an executable or package digest is insufficient
  for release qualification when Retonr manages the runtime. For an external
  runtime, the weaker identity must be explicit and must pass a separately
  approved policy.
- Every file or effective configuration field capable of changing token
  selection is hashed or rejected as unknown.
- Auto-update, hot-swap, just-in-time model loading, autoload, auto-eviction with
  substitution, and fallback to another model, runtime, device, or backend are
  disabled for qualified runs.
- Identity is rechecked before and after generation. Drift discards all
  candidates from the batch and invalidates activation.

### Network and privacy

- Local mode accepts only an IP-literal loopback endpoint, disables proxies and
  redirects, and proves no outbound connection during model load and generation.
- Model acquisition is explicit, consented, bounded, checksummed, cancellable,
  and separate from rewrite execution.
- A server may not download missing weights, tokenizers, templates, code,
  adapters, plugins, or runtime libraries during qualification or generation.
- Input and output content are absent from logs, metrics labels, traces, crash
  reports, and error values by default.
- A local-network or remote endpoint is a distinct privacy mode. It requires
  explicit user selection, authenticated encrypted transport, host pinning or an
  equivalent identity policy, retention disclosure, and separate qualification.

### Effective generation state

- Context, output limit, temperature, top-p, seed, candidate count, stop policy,
  reasoning policy, schema or grammar, template, and system content are explicit.
- Any undocumented default that can affect output makes the combination
  unqualified.
- The adapter confirms the effective context and execution state after load, not
  only the requested state.
- Automatic memory fitting, device fallback, quantization substitution,
  tokenizer substitution, model-implementation fallback, and hidden prompt
  augmentation are rejected.
- Structured output constrains syntax only. Retonr parses and validates every
  result as untrusted input through the same validation cascade.
- Seed support is capability evidence, not a claim of bitwise reproduction
  across runtime versions, hardware backends, driver versions, or thread counts.
- Runtime-advertised, artifact-declared, configured, observed, and qualified
  context values remain distinct. Only the qualified envelope admits work.
- Qualification reserves prompt and output headroom and includes position and
  cross-window fidelity fixtures. Maximum nominal context is never the default
  source budget.

### Resources and failure behavior

- Request, response, queue, concurrency, context, candidate, memory, disk,
  process, and deadline limits are explicit and tested at boundaries.
- Cancellation reaches load and generation work, and incomplete output is never
  returned as a candidate.
- Out-of-memory, runtime crash, device loss, timeout, malformed output,
  truncation, or unsupported configuration preserves the exact original.
- CPU, GPU, and hybrid execution are different qualification records even when
  the model bytes match.
- Windows, macOS, and Linux claims require independent clean-install and
  offline-after-import evidence.

### License and supply chain

- Runtime, model, tokenizer, adapter, conversion tool, accelerator library, and
  redistribution decisions are separate records.
- Use "open source model" only when the applicable license supports that claim.
  Use "open-weight model" for weights whose license is source-available or has
  field-of-use restrictions.
- Source repository license, downloadable application terms, container image
  contents, and bundled binary notices are independently reviewed.
- Runtime and model packages are fetched from approved immutable revisions,
  verified before staging, scanned without execution, and rehashed locally.
- Unreviewed model code, pickle deserialization, plugins, and arbitrary custom
  loaders are forbidden in qualified generation.

### Output-policy and watermark boundary

- Retonr never enables a known statistical watermark, watermarking generation
  configuration, output-signature processor, or opaque postprocessor in a
  qualified generation path.
- Qualification inventories every configured logits processor, sampler
  extension, renderer, parser, template, system prompt, adapter, and output
  postprocessor. Unknown or dynamically loaded components reject qualification.
- A model or runtime that requires an undisclosed output watermark is rejected
  for the generation role. A user may still inspect it as unsupported state.
- Source and build review establishes what the configured stack is intended to
  do. Controlled output tests detect regressions. Neither proves that model
  weights contain no learnable statistical signature or that every future
  detector will return a particular result.
- The strongest accurate claim is: "The qualified Retonr generation stack does
  not intentionally add a known provider watermark in the reviewed runtime,
  configuration, or postprocessing path." Do not claim universally
  "watermark-free" output.
- Detector scores and watermark observations remain research diagnostics. They
  never rank candidates, lower fidelity gates, or establish human authorship.

This boundary matters because watermarking can be an inference-time operation,
not only a property of hosted providers. Transformers documents both standard
and SynthID text watermarking configurations that modify generation, and vLLM
supports runtime-loaded logits processors that can alter token probabilities.
An open runtime or locally stored model is therefore not sufficient evidence by
itself.

Sources:

- [Transformers watermarking generation utilities](https://huggingface.co/docs/transformers/en/internal/generation_utils)
- [vLLM logits processor design](https://docs.vllm.ai/en/latest/design/logits_processors/)
- [vLLM engine plugin and generation arguments](https://docs.vllm.ai/en/stable/configuration/engine_args/)

## Current code implications

The existing `InferenceBackend` boundary is a good base: it is object-safe,
backend-neutral, bounded, cancellable, and carries an explicit artifact digest,
sampling policy, reasoning policy, output contract, and context limits. Keep it.

At code-review level, the Ollama adapter accepts only IP-literal loopback endpoints,
configures reqwest to bypass proxies and not follow redirects, uses the native API,
rechecks the inventory digest around generation, bounds responses, and treats
generated JSON as untrusted. It is an implemented candidate, not yet a qualified
backend, for these reasons:

- `RuntimeIdentity.digest` is currently `None`; `/api/version` text cannot detect
  a replaced build with the same reported version.
- Inventory maps the Ollama-reported tag digest directly to `ArtifactId`. The API
  documentation does not establish that this is the original source weight
  digest or Retonr's complete effective artifact-set digest.
- The `/api/show` wire type and details digest omit documented output-affecting
  fields such as parameters, system content, messages, Modelfile, renderer,
  parser, runtime requirements, remote model state, and projectors.
- Generation fetches model details but only checks for the `completion`
  capability. It does not compare template, parameter, license, metadata, or
  other effective-state digests to the qualification record.
- The adapter does not yet inspect `/api/ps`, so it does not verify loaded context,
  VRAM residency, or the effective CPU, GPU, or hybrid execution class.
- Backend-wide structured-output, seed, and reasoning capability declarations
  are assumed rather than earned by an exact runtime and artifact conformance
  record.
- Windows and macOS Ollama installations can update automatically. Any runtime
  change must invalidate activation before more generation occurs.

The next contract revision should add an effective runtime snapshot and
artifact-set identity rather than adding Ollama-specific fields to the core
generation request.

## Logical rollout order

### 1. Repair identity before adding adapters

- Add artifact-set and effective-package identities.
- Extend qualification evidence to bind runtime package, output-affecting
  configuration, operating system, hardware backend, and execution class.
- Define identity strength and refuse release qualification when required
  evidence is unavailable.
- Add a common backend conformance suite for drift, defaults, offline behavior,
  bounded responses, cancellation, and redacted failures.

### 2. Finish the Ollama path

- Capture every documented `/api/show` field and reject remote-backed models.
- Separate Ollama inventory digest from source artifact digest.
- Verify local-only configuration through behavior, not configuration text
  alone.
- Inspect `/api/ps` before and after generation and bind effective context and
  execution class.
- Establish runtime package identity on all three operating systems, or narrow
  the external-runtime claim explicitly.
- Qualify only exact imported packages and explicit settings.

### 3. Deliver the pinned `llama-server` path

- Select exact release builds per operating system, architecture, and hardware
  backend, with locally verified package and executable digests.
- Launch one exact GGUF offline on IP-literal loopback with a minimal argument and
  environment allowlist.
- Verify health, properties, tokenization, template, context, structured output,
  execution class, and process identity.
- Run the same fixtures and qualification policy used for Ollama.
- Compare accepted, rejected, and abstained decisions across runtimes. Narrow
  support where they diverge materially.

### 4. Prove bounded multi-pass document execution

- Freeze the document-plan schema, unit identity, context-window policy, source
  budget calculation, and staged transaction record.
- Add beginning, middle, end, cross-window, repeated-term, and cross-reference
  fixtures at several qualified context utilization levels.
- Prove no silent tokenizer or runtime truncation and no overlapping writers.
- Prove document-atomic and folder-atomic failure paths on Windows, macOS, and
  Linux, including cancellation, stale source, path collision, and resume drift.
- Keep full-document one-shot generation as a benchmark candidate, never as an
  implicit fallback.

### 5. Freeze a runtime driver contract

- Extract reusable transport, identity, and acquisition interfaces from the two
  proven implementations.
- Publish the capability negotiation and conformance fixture format.
- Keep backend defaults out of the application service and prompts out of the
  adapters.
- Make third-party adapters possible without making arbitrary dynamic plugins a
  1.0 security dependency.

### 6. Evaluate local OpenAI-compatible transport

- Implement the smallest common non-streaming request and response subset.
- Require a named identity driver for qualification.
- Treat an unknown compatible endpoint as experimental and preserve the original
  on every unsupported field or identity gap.
- Do not enable remote hosts, DNS names, proxies, redirects, or user credentials
  as a side effect of this transport work.

### 7. Evaluate LM Studio as an external runtime

- Use its native v1 inventory and load APIs to capture the strongest available
  state.
- Require copied local artifacts, fixed runtime selection, no JIT substitution,
  no MCP integration, no CORS, loopback binding, and authentication where the
  server supports it.
- Resolve the missing content-digest and runtime-package identity problem before
  qualification. If it cannot be resolved, retain a useful experimental adapter
  without a support claim.

### 8. Evaluate vLLM for controlled workstations

- Start with Retonr-launched Linux instances using exact local Safetensors
  snapshots and an environment lock, not arbitrary remote endpoints.
- Disable remote code, dynamic loaders, IO plugins, logits processors, automatic
  model implementation fallback, automatic generation configuration, remote
  media, request-content logs, and observability export.
- Qualify each CUDA, ROCm, Intel, CPU, and Apple plugin path independently.
- Consider authenticated self-hosted endpoints only after the remote privacy and
  identity contract exists.

### 9. Reconsider MLX LM when its server contract matures

- Keep direct MLX and MLX LM work out of the critical 1.0 path.
- Re-evaluate when official server documentation provides strong model identity,
  stable capability discovery, effective setting inspection, cancellation, and
  production-grade local security.
- Prefer the portable `llama-server` Metal build on macOS until MLX LM shows a
  measurable quality or resource advantage under the same qualification gates.

## 1.0 runtime boundary

The appropriate 1.0 claim is narrow:

> Retonr supports exact qualified local model and runtime combinations. The
> initial runtime paths are an attached local Ollama service and a pinned local
> `llama-server` sidecar on Windows, macOS, and Linux. Model installation is
> explicit, offline import is supported, and rewrite execution does not contact
> a model provider.

Do not promise all Ollama models, all GGUF files, every OpenAI-compatible server,
or every hardware backend. Publish the exact passing matrix and retain the test
evidence.

LM Studio, vLLM, MLX LM, and unknown compatible servers can appear in
experimental discovery only after that status is unmistakable in human and
machine output. Candidate presence must never be presented as qualified support.

## Primary license references

- [Ollama source license](https://github.com/ollama/ollama/blob/main/LICENSE)
- [llama.cpp license](https://github.com/ggml-org/llama.cpp/blob/master/LICENSE)
- [LM Studio application terms](https://lmstudio.ai/app-terms)
- [vLLM project and license](https://github.com/vllm-project/vllm)
- [MLX LM license](https://github.com/ml-explore/mlx-lm/blob/main/LICENSE)
