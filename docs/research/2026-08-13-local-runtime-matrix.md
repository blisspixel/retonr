# Local runtime compatibility and conformance matrix

Research date: August 13, 2026

Evidence cutoff: 2026-08-13 14:17 PDT. External API spend and model download
spend were zero.

## Decision summary

Retonr should keep two local runtime paths on the critical path to 1.0:

1. Attach to an existing user-managed Ollama service through its native API.
2. Launch a pinned `llama-server` build as a Retonr-managed sidecar.

Both paths can cover Windows, macOS, and Linux. Together they test the two
ownership modes Retonr needs: attaching to an installed runtime and controlling
the complete runtime process. Neither runtime family should receive a blanket
support claim. Support belongs to an exact qualified tuple of runtime build,
model artifact set, configuration, operating system, architecture, hardware
backend, and execution class.

LM Studio, Jan, LocalAI, vLLM, and MLX LM warrant experimental adapters after the
common driver contract and conformance suite exist. Their useful interfaces do
not remove their identity, update, cancellation, offline, or platform gaps. An
unknown OpenAI-compatible endpoint remains transport-only and cannot become a
qualified generation backend without a runtime-specific identity driver.

Agent Plugins 1.0 and MCP belong at the agent integration boundary. They can
package or expose Retonr's CLI and MCP tools. They must not define, discover, or
execute model runtime adapters. Runtime adapters remain trusted application code
with a smaller attack surface and an explicit release process.

## Evidence status and version snapshot

The version table is a dated review input, not a request to track `latest` at
runtime and not a support claim.

| Runtime or standard | Observed release or status | Primary evidence | Retonr disposition |
| --- | --- | --- | --- |
| Ollama | `v0.32.9`, released August 11, 2026 | [Ollama v0.32.9 release](https://github.com/ollama/ollama/releases/tag/v0.32.9) | Implemented candidate. First external-runtime qualification target. |
| llama.cpp | `b10417`, commit `2606220`, released August 13, 2026 | [llama.cpp b10417 release](https://github.com/ggml-org/llama.cpp/releases/tag/b10417) | Planned candidate. Preferred Retonr-managed portable path. |
| LM Studio | Stable download page displayed `0.4.21`; native v1 API introduced in `0.4.0` | [LM Studio download](https://lmstudio.ai/download), [native v1 API](https://lmstudio.ai/docs/developer/rest) | Experimental external-runtime candidate. Do not bundle. |
| vLLM | `v0.27.1`, released August 11, 2026 | [vLLM v0.27.1 release](https://github.com/vllm-project/vllm/releases/tag/v0.27.1) | Experimental Linux workstation or self-hosted candidate. |
| MLX LM | `v0.31.3`, released April 22, 2026; MLX framework `v0.32.0` | [MLX LM v0.31.3 release](https://github.com/ml-explore/mlx-lm/releases/tag/v0.31.3), [MLX v0.32.0 release](https://github.com/ml-explore/mlx/releases/tag/v0.32.0) | Experimental Apple silicon candidate. Not a portable default. |
| LocalAI | `v4.8.2`, released August 7, 2026 | [LocalAI v4.8.2 release](https://github.com/mudler/LocalAI/releases/tag/v4.8.2) | Secondary experimental candidate after direct runtimes. |
| Jan | `v0.8.4`, released July 23, 2026 | [Jan v0.8.4 release](https://github.com/janhq/jan/releases/tag/v0.8.4) | Secondary experimental desktop candidate after direct llama.cpp. |
| Agent Plugins | Specification `1.0.0`, Working Draft | [Agent Plugins specification](https://agent-plugins.org/specification) | Package the agent-facing integration only. |
| MCP | Released protocol revision `2026-07-28` | [MCP transports](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports), [MCP tools](https://modelcontextprotocol.io/specification/2026-07-28/server/tools) | Expose bounded Retonr tools; keep inference behind the application service. |

Hugging Face Text Generation Inference is not a new adapter target. Its latest
release is `v3.3.7`, the release notes moved the project into maintenance mode,
and the repository was archived on March 21, 2026. Retonr should spend its server
qualification effort on vLLM instead. Existing user-managed TGI endpoints can be
inspected as unsupported OpenAI-compatible transport, but should not receive a
new support commitment. See the
[TGI v3.3.7 release](https://github.com/huggingface/text-generation-inference/releases/tag/v3.3.7)
and [archived repository](https://github.com/huggingface/text-generation-inference).

## The identity model

Retonr must separate five concepts that compatible APIs often collapse:

| Concept | Meaning | Example |
| --- | --- | --- |
| Transport dialect | The request, response, streaming, and error wire contract. | Ollama native `/api/chat`, OpenAI-compatible chat completions, or LM Studio `/api/v1/chat`. |
| Runtime implementation | The executable code that interprets the request and runs inference. | Ollama `v0.32.9`, llama.cpp `b10417`, or one LM Studio runtime package. |
| Artifact set | Every immutable file that can affect output. | Weights, tokenizer, template, model configuration, adapters, projector, and generation configuration. |
| Effective configuration | The output-affecting values actually applied after defaults and runtime normalization. | Context, samplers, seed, reasoning policy, grammar, template, and stop tokens. |
| Execution class | The operating system, architecture, accelerator backend, device class, and CPU, GPU, or hybrid placement. | Windows x64 with CUDA on one RTX 4090, or macOS arm64 with Metal. |

A caller-selected `model` string is an address, not an artifact identity. A
successful request proves only that one transport exchange completed. It does not
prove which runtime binary, weights, tokenizer, template, defaults, plugins,
postprocessors, or network services produced the result.

Qualification therefore binds this complete tuple:

```text
adapter contract version
+ transport dialect and negotiated capability set
+ runtime family, version, build revision, and package digest
+ artifact-set manifest digest
+ effective configuration digest
+ operating system and architecture
+ hardware backend and execution class
+ offline-policy evidence
+ conformance-suite revision and result digest
```

Any changed or unknown member invalidates the qualification record. A mutable
tag, endpoint alias, package channel, automatic update, or runtime-selected engine
is an invalidation input, even when the visible model name is unchanged.

## Driver boundary

Do not implement one permissive `OpenAiCompatibleBackend` and use it as the
support boundary. A runtime integration has three independent responsibilities:

1. A transport driver encodes requests, decodes bounded responses, and handles
   streaming, deadlines, and errors.
2. An identity driver produces preflight, pre-generation, and post-generation
   snapshots of runtime, artifact, configuration, and execution state.
3. A lifecycle driver stages artifacts or attaches to user-managed state, then
   loads, unloads, cancels, and shuts down only what it owns.

Several runtime integrations can reuse one OpenAI-compatible transport driver.
They cannot reuse an identity conclusion. A generic endpoint without a recognized
identity driver may be useful for experiments, but it must report its identity as
unknown and cannot enter a qualified workflow.

The backend-neutral application port should expose capabilities instead of
runtime-specific fields. The capability result for one exact tuple is one of:

- `unsupported`: the runtime contract does not provide the feature.
- `declared`: official documentation or runtime metadata claims the feature.
- `observed`: the exact tuple passed the relevant conformance fixture.
- `qualified`: the observed evidence passed the locked release gates and remains
  current.

`declared` is never silently promoted to `observed` or `qualified`.

## Required adapter operations

Every candidate adapter should implement the following small operations or return
an explicit unsupported result:

| Operation | Required behavior |
| --- | --- |
| `probe` | Verify loopback endpoint policy, transport dialect, runtime identity, and bounded health response without generating text. |
| `inventory` | Enumerate locally available models with runtime-native identifiers and retain unknown identity fields as unknown. |
| `inspect` | Capture complete artifact, tokenizer, template, configuration, capability, residency, and execution evidence available from the runtime. |
| `tokenize` | Use the exact runtime tokenizer when a stable endpoint exists, or invoke a separately qualified tokenizer. Never infer fit from character count alone. |
| `load` | Load one exact staged artifact set with explicit settings. Reject automatic download, fallback, substitution, or remote model resolution. |
| `generate_stream` | Produce bounded typed events, enforce idle and total deadlines, and never expose an incomplete candidate. |
| `cancel` | Stop admission, signal the runtime, and prove that generation and its occupied slot or process ended within the qualified deadline. |
| `snapshot` | Recheck runtime, artifact, effective configuration, and execution state before and after every generation batch. |
| `unload` | Release only the model instance or process owned by the operation. An attached adapter must not disrupt unrelated user work. |
| `shutdown` | Terminate only a Retonr-owned sidecar after graceful cancellation and bounded drain. |

## Cross-platform runtime matrix

### Primary and first-wave candidates

| Runtime path | Windows | macOS | Linux | Ownership and format | Assessment |
| --- | --- | --- | --- | --- | --- |
| Ollama native API | Native release assets include x64 and arm64. GPU support varies by vendor and package. | Native application package. Apple GPU execution uses Metal. | Native amd64 and arm64 packages with CPU and several accelerator variants. | User-managed service. Imports GGUF, supported Safetensors models, and adapters into Ollama packages. | Best first attached-runtime path. Native inventory, detail, version, and running-state endpoints provide useful evidence, but do not establish executable or complete effective-package identity. Automatic application updates and cloud features require explicit controls. |
| Retonr-managed `llama-server` | Official `b10417` assets include x64 and arm64 CPU, x64 CUDA, Vulkan, OpenVINO, SYCL, and ROCm variants. | Official Apple silicon and Intel assets. Metal is enabled by default in normal macOS builds. | Official CPU and several accelerator assets across x64 and arm64. Build support includes CUDA, HIP, Vulkan, SYCL, OpenVINO, and others. | Retonr-owned sidecar with one exact local GGUF and a minimal argument allowlist. | Strongest portable identity and isolation path. Retonr can hash the executable, libraries, GGUF, template, arguments, environment, and process. Rapid upstream releases require exact pinning rather than a floating build number. |
| LM Studio or `llmster` | Official support for x64 and ARM. x64 requires AVX2. | Apple silicon only, macOS 14 or newer. Intel Macs are not supported. | x64 and arm64; Ubuntu 20.04 or newer, with official documentation warning that versions newer than Ubuntu 22 are not well tested. | User-managed desktop application or headless daemon. GGUF and MLX runtime packages are selected separately. Local import can move, copy, hard-link, or symlink. | Useful external-runtime candidate. Native v1 inventory and load APIs are materially better than an anonymous compatible endpoint. Runtime package selection, just-in-time loading, app updates, and missing content digests prevent broad qualification. Proprietary application terms require separate review. |
| vLLM | No native Windows support. WSL and community forks are distinct execution classes. | Core vLLM is Linux-first. Apple GPU support uses the community-maintained vLLM-Metal plugin on Apple silicon. | Primary supported platform, with NVIDIA, AMD, Intel, and CPU paths. | Prefer a Retonr-launched exact Python environment or container using a staged local Safetensors artifact set. | Strong Linux workstation and server candidate, especially for throughput. Too dynamic for a default desktop path. Disable usage reporting, remote resolution, remote code, plugins, custom processors, and automatic generation configuration during qualification. |
| MLX LM server | No documented Windows path. | MLX LM explicitly targets Apple silicon. Large-model memory wiring requires macOS 15 or newer. | The lower-level MLX framework has Linux packages, but that does not establish a supported MLX LM server path. | User-managed Python server using an MLX model directory or Hugging Face repository. | Apple-only experiment. The server warns that it is not recommended for production and implements only basic security checks. Prefer llama.cpp Metal until MLX LM exposes stronger identity, cancellation, and server-hardening contracts. |

Primary platform evidence:

- [Ollama Windows](https://docs.ollama.com/windows),
  [macOS](https://docs.ollama.com/macos),
  [Linux](https://docs.ollama.com/linux), and
  [hardware support](https://docs.ollama.com/gpu)
- [llama.cpp b10417 release assets](https://github.com/ggml-org/llama.cpp/releases/tag/b10417)
  and [build backends](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md)
- [LM Studio system requirements](https://lmstudio.ai/docs/app/system-requirements)
  and [headless operation](https://lmstudio.ai/docs/developer/core/headless)
- [vLLM GPU installation and platform requirements](https://docs.vllm.ai/en/latest/getting_started/installation/gpu/)
- [MLX LM project scope](https://github.com/ml-explore/mlx-lm)
  and [MLX LM server warning](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/SERVER.md)

### Secondary candidates

| Runtime path | Cross-platform position | Reason to retain | Reason not to prioritize |
| --- | --- | --- | --- |
| LocalAI `v4.8.2` | Containers are documented as the all-platform route. Native packages are documented for Linux and macOS. | Broad OpenAI-compatible API, many inference backends, CPU and several accelerator images, local or on-premises deployment. | Its automatic backend detection, gallery acquisition, multi-backend surface, agents, distributed mode, and broad media feature set enlarge the identity and security boundary. A direct llama.cpp or vLLM driver is easier to qualify. |
| Jan `v0.8.4` | Desktop releases cover Windows, macOS, and Linux. | Open source desktop distribution, required local API key, default loopback bind, GGUF import, and a llama.cpp router. | Jan adds a mutable engine manager and router above llama.cpp. Imported files may be linked in place, the engine can auto-update, and its compatible endpoint does not by itself bind the complete runtime tuple. Direct llama.cpp remains the stronger first path. |
| Generic OpenAI-compatible endpoint | Runs wherever its unknown server runs. | Low-friction experimentation and a useful transport test double. | No authoritative runtime, artifact, configuration, execution, lifecycle, offline, or cancellation identity. It can never be described as qualified without a named identity driver. |

Sources:

- [LocalAI installation paths](https://localai.io/docs/installation/index.html)
  and [LocalAI quickstart](https://localai.io/docs/basics/getting_started/)
- [Jan local API server](https://jan.ai/docs/api-server),
  [model management](https://jan.ai/docs/manage-models), and
  [llama.cpp engine management](https://www.jan.ai/docs/desktop/local-engine/llama-cpp)

## Capability matrix

The table records documented behavior at the evidence cutoff. Each item still
requires conformance testing on the exact tuple.

| Runtime | Structured output | Streaming | Cancellation | Offline and acquisition | Identity evidence |
| --- | --- | --- | --- | --- | --- |
| Ollama native | Native `format` accepts JSON or a JSON Schema. Retonr must validate the returned JSON independently. | Native endpoints stream newline-delimited JSON by default and accept `stream: false`. | No explicit per-request cancellation endpoint is documented. Transport abort and slot release must be measured. | Local-only mode is documented through `OLLAMA_NO_CLOUD=1` or `disable_ollama_cloud`. Imports supported local GGUF, Safetensors, and adapter forms. Pull and push remain separate network operations. | `/api/version`, `/api/tags`, `/api/show`, and `/api/ps` expose useful state. The Ollama model digest is not documented as Retonr's source-artifact or complete effective-package digest. |
| `llama-server` | Native `json_schema` and grammar support, plus OpenAI-compatible `response_format`. Supported JSON Schema features must be frozen and tested. | Native completion supports server-sent events; chat completions supports synchronous and streaming responses. | Current server documentation has no stable per-request cancellation endpoint. A June 2026 upstream issue reported generation continuing after disconnect on an earlier main revision. Managed-process termination is the only reliable fallback until the exact build proves finer cancellation. | `--offline` forces cache use and prevents network access. Qualification uses one exact `-m` path and forbids Hub shortcuts, router downloads, remote media, and writable slot state. | `/props`, tokenization, slots, and process state help, but Retonr-owned package and artifact hashes are authoritative for qualification. |
| LM Studio | OpenAI-compatible chat completions documents JSON Schema output. Syntax still requires independent parse and schema validation. | Native v1 chat and compatible endpoints support streaming. Native v1 exposes load and prompt progress events. | The TypeScript SDK documents immediate prediction cancellation. The HTTP v1 documentation reviewed here exposes no stable request-cancel operation for a Rust client, so disconnect behavior and slot release remain an observed gate. | Core inference can run offline once model and runtime packages are present. Search, download, runtime download, and update checks use the network. Use `lms import --copy`; default move, hard links, and symlinks are not acceptable for a Retonr-owned qualified artifact. Disable just-in-time loading. | Native v1 model inventory and `echo_load_config` expose more state than the compatible API. They do not supply a documented content digest for every model and runtime input. |
| vLLM | OpenAI-compatible JSON Schema, regex, choice, and grammar constraints are documented. Reasoning models can require additional structured-output configuration. | OpenAI-compatible endpoints support streaming. | vLLM has internal asynchronous abort machinery, but support must be established through black-box disconnect, timeout, queued-request, and slot-release tests on the exact server build. | Use exact local model and tokenizer directories with Hub offline mode. Disable usage statistics explicitly because vLLM documents anonymous usage collection as enabled by default. Reject remote code and dynamic plugins. | A model alias from `/v1/models` is weak. Strong identity requires a Retonr-launched environment or attested container, exact package lock, local artifact manifest, server arguments, environment, and effective engine configuration. |
| MLX LM | The reviewed server contract does not document JSON Schema constrained output. Treat it as unsupported until an exact server proves a bounded equivalent. | The server documents optional streaming, and the Python API exposes `stream_generate`. | No server cancellation contract was found in the reviewed primary documentation. Treat cancellation as unsupported until proven. | A local model path is accepted, but a Hub identifier downloads when absent. Qualification must use a staged local directory, Hub offline settings, and an outbound deny harness. Never enable unreviewed tokenizer remote code. | Server responses identify a path or repository name, not immutable artifact contents or the complete effective configuration. A wrapper would need to provide identity. |
| LocalAI | Backend-dependent grammar or schema behavior cannot be generalized across its runtime surface. A named backend must pass the exact schema subset. | OpenAI-compatible streaming is a transport candidate. | No family-wide cancellation conclusion is justified across its multiple backends. Test the selected backend and wrapper together. | Galleries, URL, Hugging Face, Ollama, and OCI acquisition are documented. Qualified execution must use a staged local package with all automatic acquisition disabled. | Automatic backend selection is useful for convenience and hostile to stable identity. Bind the LocalAI build, selected backend, backend build, model files, configuration, and device path separately. |
| Jan | The reviewed Jan API reference does not establish a JSON Schema contract. Underlying llama.cpp capability is not automatically a Jan API guarantee. | Chat completions documents streaming. | No stable request cancellation contract was found in the reviewed API documentation. | Desktop use is offline-capable after setup. Local GGUF import supports linking or duplication; use duplicated, independently hashed artifacts for managed qualification. | Record Jan, router, llama.cpp engine, model, settings, and execution state separately. Disable engine auto-update for an active binding. |

Capability sources:

- [Ollama structured outputs](https://docs.ollama.com/capabilities/structured-outputs),
  [streaming](https://docs.ollama.com/api/streaming),
  [local-only mode](https://docs.ollama.com/faq), and
  [model import](https://docs.ollama.com/import)
- [llama-server API and options](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
  and [reported disconnect limitation](https://github.com/ggml-org/llama.cpp/issues/24496)
- [LM Studio native API](https://lmstudio.ai/docs/developer/rest),
  [streaming events](https://lmstudio.ai/docs/developer/rest/streaming-events),
  [structured output](https://lmstudio.ai/docs/developer/openai-compat/structured-output),
  [prediction cancellation](https://lmstudio.ai/docs/typescript/llm-prediction/cancelling-predictions),
  [offline behavior](https://lmstudio.ai/docs/app/offline), and
  [local import](https://lmstudio.ai/docs/cli/local-models/import)
- [vLLM structured outputs](https://docs.vllm.ai/en/latest/features/structured_outputs/),
  [engine arguments](https://docs.vllm.ai/en/latest/configuration/engine_args/), and
  [usage-statistics opt-out](https://docs.vllm.ai/en/latest/usage/usage_stats/)
- [MLX LM server](https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/SERVER.md)

## Offline-after-setup contract

"Local" is not an adequate privacy or offline claim. A local runtime may still
perform model discovery, pull missing files, check for updates, report usage,
invoke remote tools, or fall back to a cloud model. Retonr should qualify offline
behavior, not infer it from a loopback URL.

The runtime conformance suite must enforce all of these conditions:

1. Acquisition and rewrite execution are separate commands and transactions.
   Rewrite execution never downloads a runtime, model, tokenizer, adapter,
   template, plugin, or schema.
2. Retonr copies every required artifact into private staging, validates paths and
   file types, hashes every byte, and atomically activates a canonical artifact-set
   manifest. Symlinks and hard links are not active managed artifacts.
3. Qualified execution uses local paths rather than repository names, mutable
   tags, model galleries, router autoload, or just-in-time loading.
4. Runtime cloud features, remote tools, web search, update checks, telemetry, and
   implicit model acquisition are disabled before launch.
5. The conformance run executes under an operating-system outbound deny boundary
   while preserving only the required IP-literal loopback connection.
6. Proxy discovery is disabled, redirects are rejected, DNS names are rejected for
   local qualification, and credentials are never sent to an unexpected origin.
7. A missing artifact, cache member, tokenizer file, or runtime component fails
   closed. It does not trigger a network attempt or substitute another model.
8. Runtime logs and retained test evidence prove which connection attempts were
   made. Absence of a documented cloud feature is not evidence of no network use.

Ollama requires both its documented cloud-disable setting and behavioral network
denial. vLLM requires its usage-statistics opt-out in addition to Hub offline mode.
LM Studio requires preinstalled model and runtime packages, disabled just-in-time
loading, and a test that update and catalog features do not affect execution.
`llama-server` receives `--offline` even though the process is also sandboxed.

## Structured-output contract

Structured output constrains syntax. It does not establish factual fidelity,
semantic equivalence, authorship, legal compliance, or safety.

Retonr should define one small versioned JSON Schema subset for candidate
envelopes and probe it per exact runtime and artifact. The initial subset should
use closed objects, required properties, strings, integers, booleans, bounded
arrays, enums, and explicit length or count limits. Unsupported schema keywords
must be rejected before inference rather than silently dropped.

Every response remains untrusted:

1. Bound status line, headers, frame count, frame size, aggregate bytes, token
   claims, and decompression.
2. Decode the runtime-specific stream with arbitrary frame fragmentation tests.
3. Assemble exactly one terminal candidate envelope.
4. Reject duplicate terminal events, trailing data, malformed UTF-8, malformed
   JSON, unknown fields, missing fields, and schema violations.
5. Apply Retonr's deterministic structure, protected-content, fidelity, and edit
   budget validation after schema validation.
6. Preserve the original on any failure or uncertainty.

No streamed fragment may be written into the user's output document. Streaming is
an internal transport and progress facility until the complete candidate passes
the transaction's validation cascade.

## Streaming and cancellation contract

Streaming should be first-class in the adapter even when a command presents only
the final validated result. It improves bounded memory use, progress, time to first
token measurement, and the opportunity to cancel runaway generation.

The common stream event model should contain only typed bounded events:

- runtime and model load progress
- prompt processing progress
- candidate text delta
- separated reasoning delta when the runtime cannot suppress it
- usage update
- terminal completion
- terminal failure

Reasoning content is never merged into the candidate. It is discarded by default
and excluded from normal logs. Progress events carry no raw document text.

Cancellation qualification must prove more than a client future returning:

- cancellation before connection performs no runtime work
- cancellation while queued removes the request from the queue
- cancellation during prompt processing releases its slot
- cancellation during generation stops token production
- cancellation during a stalled stream reaches a bounded terminal state
- cancellation during model load does not activate a partial model
- a cancelled operation never returns or commits a partial candidate
- the next request can use the released capacity
- an attached runtime retains unrelated user work

If a runtime cannot prove request-scoped cancellation, an attached adapter reports
that limitation and cannot qualify for unattended batch rewriting. A
Retonr-managed single-purpose sidecar may use bounded graceful shutdown followed
by process termination as a fallback. That fallback must be recorded as
process-scoped cancellation and tested for cleanup of temporary files, child
processes, ports, and device memory.

## Conformance suite

One data-driven suite should run against every transport and identity driver.
Runtime-specific tests can add evidence but cannot replace the shared gates.

### Discovery and identity

- Reject DNS, wildcard, non-loopback, redirected, proxied, and user-info endpoints
  in the local trust mode.
- Bound health, version, inventory, detail, tokenizer, running-state, and error
  responses.
- Distinguish runtime display version from executable and package digests.
- Distinguish runtime inventory digest from Retonr artifact-set digest.
- Detect changes to weights, tokenizer, template, system prompt, adapter, grammar,
  generation configuration, runtime build, backend library, and execution class.
- Reject an unknown required identity instead of substituting a display name.

### Generation and validation

- Exercise every keyword in Retonr's JSON Schema subset, including rejected
  keywords and adversarial nesting.
- Split every stream delimiter and multi-byte UTF-8 sequence at every practical
  boundary.
- Enforce prompt, output, response-byte, frame, candidate, context, idle, and total
  limits at the exact boundary and one unit beyond it.
- Verify explicit sampling, reasoning, stop, seed, candidate-count, and template
  settings against effective runtime state.
- Preserve exact original input on malformed, truncated, overlong, duplicated,
  or semantically rejected output.
- Record tokenizer counts and runtime usage separately; disagreeing values are
  evidence, not a reason to silently choose one.

### Offline and lifecycle

- Import from a read-only local source while outbound traffic is denied.
- Reject missing members, escaping links, special files, alternate data streams,
  unexpected executable content, and mutation during hashing or import.
- Start with empty runtime caches and prove that rewrite execution cannot acquire
  missing content.
- Verify load, warm generation, unload, restart, crash recovery, and stale process
  cleanup.
- Run update and model-substitution drift tests without changing the visible alias.
- Prove that automatic fallback, remote models, remote media, tools, and plugins
  cannot activate in the qualified configuration.

### Platform and resource evidence

- Run clean-install evidence independently on Windows, macOS, and Linux.
- Treat CPU, CUDA, Metal, ROCm, Vulkan, SYCL, and hybrid placement as separate
  execution classes.
- Record cold load, warm latency, time to first token, throughput, peak system RAM,
  peak device memory, context use, cancellation latency, and cleanup latency.
- Include CPU-only and low-memory paths. Slow throughput is acceptable when the
  workflow remains correct, bounded, and candid about expected completion.
- Re-run cross-window fidelity and cancellation fixtures at several context
  utilization bands. Nominal context size is not a qualified source budget.

## Qualification record

A passing run should emit a signed or digest-addressed record containing:

- conformance schema and suite revisions
- adapter and transport driver versions
- runtime family, version, full build revision, package digest, executable digest,
  dependent native-library digests, and relevant build features
- canonical artifact-set manifest and license decision references
- tokenizer, template, prompt template, system prompt, adapter, projector, draft
  model, grammar, parser, renderer, and generation-configuration digests
- requested and effective context, sampling, reasoning, structured-output, stop,
  candidate, and seed settings
- operating system, architecture, hardware backend, device class, and CPU, GPU, or
  hybrid placement without a stable telemetry identifier
- acquisition owner, endpoint policy, authentication mode, offline controls,
  proxy and redirect policy, and observed network-denial result
- structured-output subset, streaming behavior, cancellation class, and limit
  envelope that passed
- fixture manifest, accepted, rejected, and abstained counts, and complete failure
  evidence

The record authorizes only its exact tuple and declared roles. It is provenance
and conformance evidence, not proof that all future outputs are correct or free of
every undisclosed statistical signal.

## Agent Plugins and MCP boundary

The Agent Plugins `1.0.0` Working Draft defines skills and MCP servers as its two
portable component types. It does not define a model-runtime adapter type. Retonr
should respect that boundary:

```text
agent client
  -> Agent Plugin package or direct MCP configuration
  -> Retonr MCP server or CLI tool
  -> Retonr application service and transaction
  -> qualified inference port
  -> built-in runtime adapter
  -> local model runtime
```

The agent-facing layer may expose bounded operations such as inspect, plan,
rewrite-to-staging, validate, report, commit, and abort. It receives the same
non-destructive transaction semantics as the CLI. It does not receive raw runtime
credentials, arbitrary endpoint access, plugin-defined inference code, or a way to
bypass artifact qualification.

An Agent Plugin package can include a Retonr skill and an MCP server entry that
launches the installed Retonr executable. The package must not bundle mutable
models, inference runtimes, or adapter libraries. Runtime selection remains an
explicit user-controlled Retonr configuration and policy decision.

## Logical implementation order

### 1. Freeze the runtime driver and evidence contracts

- Separate transport, identity, and lifecycle interfaces.
- Add typed capability states and explicit unsupported or unknown results.
- Define the effective runtime snapshot and artifact-set manifest.
- Define the shared stream event, cancellation result, and qualification record.
- Make unknown identity fail closed for qualified generation.

### 2. Complete Ollama as the first attached runtime

- Capture and canonicalize every documented output-affecting `/api/show` field.
- Keep Ollama inventory digest separate from source and effective-package digests.
- Add `/api/ps` residency, effective context, and execution-class checks.
- Establish runtime package identity per operating system or narrow the claim.
- Qualify structured output, fragmented streaming, cancellation, local-only mode,
  drift detection, and bounds on exact tuples.

### 3. Deliver the pinned `llama-server` sidecar

- Select exact release packages per supported operating system, architecture, and
  hardware backend.
- Stage and hash one local GGUF plus every required runtime library.
- Launch on an ephemeral IP-literal loopback port with `--offline`, an argument and
  environment allowlist, and no router, remote media, model download, or Web UI.
- Implement process-scoped cancellation fallback and prove cleanup.
- Run the same conformance fixtures used for Ollama and compare decisions.

### 4. Make streaming the application execution path

- Feed bounded adapter events into progress and cancellation without exposing
  unvalidated text.
- Buffer each candidate only inside the staged transaction.
- Integrate document-unit cancellation, resume evidence, and atomic final commit.
- Add concurrency and backpressure tests for long documents and folders.

### 5. Add experimental desktop adapters

- Add LM Studio native v1 discovery and load-state inspection before using its
  compatible inference endpoint.
- Add Jan only when the adapter can bind Jan, its router, llama.cpp engine, model,
  settings, and update state.
- Keep both adapters visibly experimental until exact cross-platform evidence
  passes. Do not infer capability from their underlying engine.

### 6. Add controlled workstation adapters

- Add vLLM first on Retonr-launched Linux CUDA with an exact environment lock and
  staged local Safetensors model.
- Evaluate ROCm, Intel, CPU, WSL, and vLLM-Metal as separate candidates.
- Add LocalAI only if users need a backend it uniquely provides and its layered
  identity can be established.
- Keep unknown compatible endpoints in an explicit experimental transport mode.

### 7. Reconsider direct MLX LM serving

- Measure it against pinned llama.cpp Metal on the same Apple silicon hardware and
  model capability class.
- Require a hardened identity wrapper, JSON Schema subset, request cancellation,
  offline behavior, and stable server contract.
- Do not add a direct adapter only to duplicate a weaker version of an already
  qualified local path.

### 8. Publish agent-facing packages after the CLI contract is stable

- Expose the same bounded application operations through MCP.
- Package the MCP server and skills according to Agent Plugins 1.0 without making
  runtime adapters plugin components.
- Run plugin and MCP conformance independently from inference qualification.
- Preserve CLI parity so agents do not become a privileged execution path.

## Revalidation triggers

Repeat this review and invalidate affected evidence when any of these changes:

- a target runtime release, package channel, update mechanism, or license
- runtime API fields, default values, streaming frames, cancellation behavior, or
  structured-output implementation
- supported operating systems, architectures, drivers, accelerator backends, or
  Python and container requirements
- model format, tokenizer, chat template, artifact packaging, or Hub behavior
- cloud fallback, remote tools, telemetry, proxy, authentication, or update checks
- Agent Plugins or MCP normative revision
- a security advisory involving request parsing, model loading, artifact import,
  local HTTP exposure, or native inference libraries
- a conformance fixture exposes runtime divergence or an identity field Retonr
  cannot bind

## Key conclusions

1. OpenAI compatibility is useful code reuse, not trustworthy identity.
2. Ollama plus a pinned `llama-server` sidecar is the smallest credible portable
   1.0 runtime surface.
3. Cancellation and slot release are the weakest common capabilities and must be
   release gates, especially for unattended long-document work.
4. Local execution is not automatically offline or private. Cloud features,
   downloads, update checks, telemetry, plugins, and remote resolution need both
   explicit controls and behavioral network-denial tests.
5. Structured output is a syntactic aid. Retonr's deterministic validation and
   original-preserving transaction remain authoritative.
6. LM Studio `0.4.x` is a stronger experimental candidate than its earlier API
   line because native v1 and `llmster` expose better lifecycle state. It still
   needs runtime and artifact identity evidence.
7. vLLM is the best next high-throughput Linux path, not the cross-platform
   default. MLX LM remains an Apple-specific experiment.
8. Jan and LocalAI are useful user-controlled ecosystems, but their extra routing
   and backend layers make them later identity-driver work rather than first-wave
   support.
9. Agent Plugins and MCP should distribute and expose Retonr's safe operations.
   They should not widen the trusted inference adapter boundary.
