# Local model evaluation protocol

Research date and evidence cutoff: August 13, 2026

Status: proposed development protocol. No model, runtime, quantization, hardware
class, or operating system is qualified by this document. No model was downloaded
or run for this review, and external API spend was zero.

Post-plan status: the checked-in development foundation has grown to 49 deterministic
fidelity and structure cases plus 120 synthetic editorial cases across five groups.
The 25-case core and 39-case editorial references below describe the frozen initial
projection at this document's evidence cutoff, not current repository totals. The
recorded local artifact observations have expired and cannot support a new run without
fresh approved identity and runtime evidence.

## Decision summary

Retonr should evaluate the complete editorial transaction, not select a model from a
generic leaderboard. The primary harness should drive the Retonr CLI and native
runtime adapter against versioned Retonr cases. Inspect AI is the preferred
experiment coordinator when its logs and custom-task model are useful. Lighteval is
an optional secondary check for standard tasks. Neither tool replaces Retonr's
deterministic fidelity, structure, provenance, and resource measurements.

The first development bakeoff should spend no money. It should begin with the two
currently installed Ollama artifacts and add the previously observed compact artifact
only if it is found locally and revalidated or acquisition is separately approved:

| Cohort | Current local candidate | Recorded Ollama digest | Purpose |
| --- | --- | --- | --- |
| Compact | Ministral 3 8B Q4_K_M | `1922accd5827ebe6829e536369195db25eaf664528dc66206d646ea3bb386b71`, previously observed and requiring a fresh inventory recheck | Establish the quality and latency floor |
| Workstation | Gemma 4 26B Q4_K_M | `5571076f3d70050487b26b341705799e0ab29b808164f90d20d4cf84f699d251` | Test whether a larger cross-family artifact improves restraint and fidelity |
| Workstation | Qwen 3.6 27B Q4_K_M | `a50eda8ed977ab48a12431878896b27ffd5cef552c17af3317d9623b939a7f1e` | Test a second large family under the same runtime contract |

These are mutable runtime references paired with previously observed inventory
digests, not independently reconstructed source-artifact identities. If any current
identity differs, the old observation is not reused. A 2B to 4B candidate and a 12B
to 20B candidate should be added only after a separate model review selects exact
artifacts and the owner authorizes acquisition.

The RTX 4090 is a development accelerator, not the product baseline. Results from
CUDA, CPU, Metal, Vulkan, ROCm, or hybrid execution are separate evidence strata.
Slow execution is acceptable when the workflow remains bounded, cancellable, and
correct. Hardware classes compete on quality first and resource behavior second.

## Research questions

The initial program answers these questions in order:

1. Can an exact model and runtime stack produce the required candidate envelope on
   every smoke case without leaking protected placeholders or control text?
2. Does the accepted set preserve protected literals and structure under deterministic
   gates, and preserve claims, polarity, modality, attribution, and conditions under
   independently produced typed semantic evidence with uncertainty?
3. Does it leave already-direct text alone more often than a cheaper baseline?
4. After fidelity acceptance, does it resolve declared editorial findings without
   introducing new findings or flattening the owner's position?
5. Does a larger artifact provide materially better accepted coverage or owner
   preference than the smallest viable artifact?
6. How do quantization, runtime, backend, context pressure, batching, and hardware
   affect critical decisions and repeated outputs?
7. What is the smallest exact stack that works acceptably in each declared resource
   class?

The program does not ask which model has the highest general knowledge score. It
also does not infer authorship, human origin, watermark absence, legal compliance,
or model-family behavior from an artifact result.

## Current official tooling snapshot

The following observations are `official_implementation` evidence under the
[research evidence vocabulary](README.md):

- [Inspect AI model documentation](https://inspect.aisi.org.uk/models.html) lists
  local Hugging Face, vLLM, Ollama, llama-cpp-python, SGLang, and other providers,
  and supports custom model API extensions. Its task and scorer model fits a
  Retonr-specific harness better than a fixed leaderboard suite.
- [Inspect AI evaluation logs](https://inspect.aisi.org.uk/eval-logs.html) retain the
  task, model, plan, status, samples, scores, and selected model API traffic. Raw
  request logging needs an explicit data-handling decision because source text can
  appear in logs. The current PyPI release observed at the cutoff was
  [Inspect AI 0.3.249](https://pypi.org/project/inspect-ai/).
- [Lighteval](https://github.com/huggingface/lighteval) supports Inspect, vLLM, and
  Accelerate paths and sample-level results. Its maintainers describe the Inspect
  entry point as preferred. The current published release observed at the cutoff
  was [0.13.0](https://github.com/huggingface/lighteval/releases/tag/v0.13.0).
  Its official repository says Windows is not tested or supported, so it cannot be
  Retonr's cross-platform source of truth.
- [vLLM](https://docs.vllm.ai/en/stable/) is the preferred high-throughput local GPU
  evaluator where the model, platform, and architecture are supported. The dated
  runtime matrix records the exact current release selected for preregistration. vLLM
  states that results are not reproducible by default and that
  reproducibility is bounded to the same hardware and vLLM version even after its
  documented controls are enabled. Its
  [batch-invariance mode](https://docs.vllm.ai/en/stable/features/batch_invariance/)
  is beta and has model and NVIDIA hardware constraints.
- [Hugging Face Transformers offline mode](https://huggingface.co/docs/transformers/installation)
  supports exact local paths, `HF_HUB_OFFLINE=1`, and `local_files_only=True`.
  Transformers plus Accelerate is a separately selected execution stratum for CPU or
  architectures that vLLM cannot load. Defaults from `generation_config.json` must not
  silently enter a qualified run.
- [Hugging Face Hub environment controls](https://huggingface.co/docs/huggingface_hub/main/en/package_reference/environment_variables)
  provide offline, telemetry-disable, implicit-token-disable, and update-check
  controls. Retonr still needs an operating-system network-deny test because an
  environment variable is configuration evidence, not isolation proof.

The [local runtime matrix](2026-08-13-local-runtime-matrix.md) is authoritative for
current runtime selection: attach to a user-managed Ollama service first, then add a
pinned Retonr-managed `llama-server` sidecar. The prior
[provider-neutral review](2026-08-12-provider-neutral-runtimes.md) remains background.
vLLM and Transformers are valuable evaluation backends, but they do not define the
cross-platform product contract.

## Evaluation architecture

Use four separate lanes and join their records by immutable identifiers:

| Lane | Responsibility | Authority |
| --- | --- | --- |
| Retonr product lane | Run the real prompt, adapter, candidate envelope, validators, selection, transaction, and report path | Source of truth for product behavior |
| Experiment lane | Schedule cases, enforce limits, collect transcripts, and invoke scorers | Inspect AI when helpful, otherwise a small pinned runner |
| Deterministic evidence lane | Check byte, structure, literal, schema, lint, and resource facts; deterministically compare independently extracted typed claims | Source of truth for exact observations; extraction remains probabilistic |
| Human review lane | Judge ambiguous fidelity, owner preference, restraint, and channel fit | Required for release semantic and style claims |

Lighteval may provide a fifth, auxiliary lane for selected public instruction-following
or language tasks. Its results appear in an appendix and never compensate for a
Retonr fidelity failure.

The harness must invoke Retonr rather than duplicate its prompts and validators in
Python. A direct backend call is useful only as a diagnostic control. If direct and
product paths differ, report the difference and treat the product path as the
candidate under test.

## Candidate cohorts

Model size is a scheduling hint, not a capability claim. Disk bytes, resident
memory, active parameters, context cache, and offload can differ substantially at
the same advertised parameter count.

| Cohort | Initial artifact range | Intended device envelope | Minimum family diversity |
| --- | --- | --- | --- |
| Small | 1B to 4B instruction artifacts, usually quantized | CPU-only systems, 8 GB class GPUs, and lower-memory unified systems | Two unrelated model families |
| Medium | 7B to 14B instruction artifacts, with 12B to 20B candidates when memory permits | Mainstream 8 GB to 16 GB accelerators and modern Apple unified memory | Two unrelated model families |
| Large | 20B to 32B dense artifacts or sparse models with a comparable measured footprint | 24 GB workstation GPUs or larger unified-memory systems | Two unrelated model families |

Each cohort includes a no-rewrite baseline and the strongest cheaper qualified
candidate. A larger model advances only when it improves accepted fidelity coverage,
restraint, or blind preference enough to justify its additional memory and latency.

Do not pool dense and mixture-of-experts parameter counts. Record total parameters,
active parameters when authoritatively available, weight bytes, quantization,
resident memory, and observed execution placement independently.

### Candidate eligibility

Before any generation, a candidate record must contain:

- Exact model artifact or artifact-set manifest and SHA-256 digests
- Immutable upstream revision and local acquisition record when available
- Model, tokenizer, chat template, generation configuration, adapter, and license
  identities
- Runtime package, executable, libraries, build features, and adapter contract
- Effective context, output limit, sampling parameters, structured-output method,
  and reasoning or thinking controls
- Operating system, architecture, hardware backend, device class, driver, precision,
  offload, and memory policy
- Offline, bind-address, proxy, authentication, telemetry, and update-check state
- Case-suite, prompt-template, strategy, validator, lint catalog, and Retonr build
  digests

Unknown values remain `unknown`. A display name, mutable tag, endpoint model field,
or successful load is not a substitute for identity.

## Suite design

Every suite has one immutable manifest, content digest, declared use, and maximum
case count. The runner rejects undeclared files, cases, prompts, scorers, or network
access.

### Stage 0: manifest and isolation preflight

This stage performs no generation. It verifies identities, licenses, file bounds,
local paths, free storage, runtime reachability, loopback binding, update state,
telemetry state, and outbound-network denial. It records a capability result rather
than silently lowering context, changing precision, or offloading unexpectedly.

Failure stops that exact candidate run. It does not create a general model-family or
platform claim.

### Stage 1: eight-case smoke

Run one case from each required behavior before scale:

1. Valid candidate-envelope JSON
2. Obvious editorial padding with protected facts
3. Already-direct text that should remain byte-identical
4. Quantity, date, unit, and named-entity preservation
5. Negation and modality preservation
6. URL, path, code, quotation, and sentinel preservation
7. Embedded instruction treated as document content rather than a command
8. Cancellation, timeout, and bounded-output behavior

Use one candidate, temperature 0, one declared seed, and one attempt. If the product
policy permits a retry, record raw first-attempt validity, retry count, retry cause,
and final product result separately. A retry must never disappear from the score.

Scale only when the exact stack completes all eight cases and produces a readable
evidence bundle. Passing is installation evidence, not model qualification.

### Stage 2: bounded development bakeoff

The originally proposed first bakeoff projected 39 then-current synthetic editorial
cases: 15 cases from `editorial_quality_v1.json` and 24 paired finding and clean-control
cases from `editorial_slop_v1.json`. A future generation-suite projection must freeze
the prompt, source, protected terms, reference revision, expected findings, and
adjudication form. The then-25-case `core.json` was a deterministic validator
regression suite; it was not 25 independent model generations.

Run one generation per case for every candidate that passed smoke. Review all hard
failures and a blinded, randomized comparison of the common accepted subset. Do not
tune prompts per model during the comparison. A prompt or template repair creates a
new experiment revision and reruns every candidate.

For the two leading candidates, repeat the eight smoke cases three times in fixed,
reversed, and shuffled order. This gives an inexpensive scheduling-sensitivity and
output-repeatability check without multiplying the entire development run.

### Stage 3: fidelity and claim challenge

After typed claim evidence exists, add a balanced generated-output suite covering:

- Agent, action, object, recipient, and attribution swaps
- Numbers, units, currencies, versions, dates, timezones, and ranges
- Negation, modality, obligation, permission, and uncertainty
- Conditions, exceptions, exclusions, comparisons, thresholds, and temporal order
- Quotations, citations, links, paths, identifiers, code, and cross-unit references
- Prompt injection, sentinel imitation, schema text, and adversarial Unicode

Every acceptable and corrupted pair receives independent labels before model runs.
Model judges may triage disputes but cannot supply the sole release label. The
generator under test cannot be its only semantic evaluator.

### Stage 4: context and document scaling

This stage begins only after the chunk planner and relevant format adapter exist.
Exercise 25, 50, and 75 percent of the exact qualified context budget, then the
product's declared maximum unit size. Do not infer usable context from model metadata
alone.

For each supported format, measure the complete extract, plan, generate, validate,
apply, reopen, and report transaction. Future document strata include:

- Plain text and Markdown with newline, code, link, table, HTML, and reference cases
- DOCX packages with runs, fields, relationships, sections, comments, and rejected
  features
- Spreadsheet cells with formulas, types, merged regions, comments, and text-only
  edit eligibility
- JSON and HTML with exact non-text structure preservation
- Streaming input with fixed chunk boundaries, backpressure, cancellation, and
  atomic failure behavior

The output report states changed editable text, unchanged bytes or package parts,
word-count delta, accepted and rejected units, and structure checks. Page count may
be compared only under an exact renderer and version. Similar word count does not
prove stable pagination.

### Stage 5: locked qualification

Locked release evidence is deferred until the behavior, prompts, schemas, and
thresholds are mature. During early 0.x development, the bakeoff informs design and
finds regressions; it does not prematurely declare broad support.

When qualification begins, use a fresh, access-controlled suite sized by a
predeclared power analysis. Freeze one confirmatory candidate or a multiplicity
procedure before opening results. Publish negative, null, abstained, and excluded
outcomes with confidence intervals.

## Metrics and decision rule

Do not collapse the result into one quality score. Compare candidates in this order:

1. Critical invariant and structure violations
2. Accepted-set semantic error and corruption acceptance
3. Eligible-candidate and transformation coverage
4. Clean-control exact no-op rate and unnecessary edit cost
5. Declared editorial findings resolved, retained, and introduced
6. Blind owner or reviewer preference on the common accepted subset
7. Latency, memory, throughput, cancellation, disk, and optional energy observations

A candidate cannot compensate for a changed fact with better prose or faster
throughput. Ties remain ties when the evidence cannot distinguish them.

### Exact and fidelity metrics

Record at least:

- Raw and final structured-output parse success
- Candidate count, truncation, retry, timeout, cancellation, and adapter-error rates
- Protected literal, entity, quantity, claim, polarity, modality, attribution, and
  cross-unit outcomes by category
- Validator accept, abstain, and reject outcomes with exact reason IDs
- Byte identity outside editable spans and structural-fingerprint equality
- Exact unchanged-output rate on clean controls
- Accepted-set semantic error and eligible-candidate coverage
- Word, Unicode-scalar, and byte edit ratios within editable spans
- Word-count and character-count deltas

Tokenizer-specific edit distance may be reported only as a diagnostic because it
changes with the model tokenizer.

### Editorial-quality metrics

After all hard gates pass, record:

- Source findings resolved, retained, introduced, suppressed, and uncertain
- Results per rule, channel, language, document unit, and clean-control class
- Reference-revision distance as a diagnostic, not an exact-answer score
- Blind pairwise preference, restraint, channel fit, and main-point preservation
- Whether the output added generic framing, promotional language, false certainty,
  excessive headings, repeated conclusions, emojis, or dash-heavy punctuation

These are editorial observations. They are not an AI detector and never become an
authorship probability.

### Resource metrics

Measure cold load and warm execution separately:

- Load time and first-token latency
- Completed-candidate and end-to-end latency at p50, p95, and maximum
- Prompt and output tokens, tokens per second, and validated units per minute
- Peak process resident memory and runtime-reported model residency
- Peak device memory where a platform can measure it reliably
- Model storage, temporary storage, and retained evidence size
- Cancellation latency, timeout recovery, and subsequent-request health
- Optional energy and power observations with the exact measurement tool named

Never compare timings from concurrent and isolated runs as though they were the same
condition. Warm-up count, process lifetime, batch size, request order, and other GPU
workloads belong in the record.

## Repeatability and backend comparison

Deterministic settings do not establish universal deterministic output. Floating
point kernels, schedulers, runtime versions, templates, and device backends can
change token selection.

For every run:

- Set every generation parameter explicitly. Do not inherit model or server
  generation defaults.
- Record output text and token-ID digests when the runtime exposes token IDs.
- Repeat finalists in multiple request orders and compare exact output plus critical
  accept or abstain decisions.
- Keep vLLM version and hardware fixed for a reproducibility claim. If beta batch
  invariance is enabled, name that fact and its model and hardware limits.
- Run cross-runtime differential tests only when weights, tokenizer, template,
  precision, and generation settings can be shown equivalent.
- Treat a quantized artifact as a new candidate. Compare it against a higher-precision
  reference with predeclared non-inferiority criteria before qualifying it.

Output diversity can be evaluated later with a fixed nonzero-temperature profile
and predeclared seeds. It must be a separate experiment from the deterministic
development bakeoff.

## Hardware matrix

The first reference run uses the RTX 4090 host and Ollama because the model bytes are
already present. Portability evidence then proceeds through independent classes:

| Execution class | Primary path | Compatibility path | Required distinction |
| --- | --- | --- | --- |
| NVIDIA workstation | vLLM for high-throughput research; Ollama or pinned `llama-server` for product tests | Transformers and Accelerate | Native Windows, WSL, and Linux are separate environments |
| Apple silicon | Ollama or pinned Metal `llama-server` | MLX LM experiment | Unified memory size and Metal or MLX runtime are part of identity |
| CPU x86-64 or ARM64 | Pinned `llama-server` GGUF | Transformers and Accelerate | ISA, thread count, BLAS backend, RAM, and offload state are recorded |
| AMD or Intel GPU | Pinned runtime-specific build | Ollama, llama.cpp, or Transformers where supported | ROCm, Vulkan, SYCL, and other backends are separate classes |

Name vLLM or Transformers and Accelerate before a run. If vLLM does not support the
exact architecture, mark that vLLM stratum unsupported and schedule a separate pinned
Transformers and Accelerate run. Never switch backend within an active run. Do not make
vLLM a Windows or Apple product requirement. Lighteval's lack of official Windows
support also prevents it from being the only runner.

## Recommended first bakeoff

The first useful run is deliberately narrow:

1. Revalidate the exact Ollama version, executable or package evidence, `/api/show`
   projection, `/api/ps` placement, model inventory digests, prompt template, context,
   and local-only configuration for the three recorded artifacts.
2. Run the eight-case smoke serially with no other model resident on the GPU.
3. Advance every clean smoke result to the 39-case development projection.
4. Use one candidate, temperature 0, top-p 1, seed 7, an 8,192-token requested
   context, and a 256-token output ceiling unless the frozen strategy requires a
   smaller bound. Record effective rather than merely requested values.
5. Blind and randomize all generated candidates for review. The no-rewrite source
   and reference revision are controls, not assumed winners.
6. Repeat the eight smoke cases three times and in varied request order for the two
   leading exact candidates.
7. Publish one local evidence bundle with all failures and no support claim.

This run is 24 smoke generations at first pass, at most 117 development generations
if all three candidates advance, and 48 finalist-repeat generations. It is bounded
at 189 generations before any optional rerun. A failed or cancelled request counts
toward the ceiling. Prompt changes start a new run rather than extending the budget.

The comparison answers whether the installed 26B or 27B artifacts improve on the 8B
artifact enough to justify their observed cost. It does not answer the small-device
question. The next authorized acquisition should fill the 2B to 4B cohort, followed
by one 12B to 20B candidate only if it adds family or resource coverage not already
represented.

## Proposed command shape

These commands describe the intended interface. The Retonr generation projection
and `local` subcommand do not exist yet and must not be presented as implemented.
Use POSIX shell examples in documentation and screenshots; paths remain ordinary
platform inputs.

Validate the current deterministic suites with implemented commands:

```bash
cargo run --locked -p rewrite-eval -- crates/eval/fixtures/core.json
cargo run --locked -p rewrite-eval -- \
  --editorial-corpus crates/eval/fixtures/editorial_quality_v1.json
cargo run --locked -p rewrite-eval -- \
  --editorial-corpus crates/eval/fixtures/editorial_slop_v1.json
```

Proposed product-harness shape:

```bash
retonr eval local \
  --plan eval/plans/local-smoke-v1.json \
  --runtime ollama \
  --artifact-id sha256:<artifact-set-id> \
  --offline-required \
  --limit 8 \
  --output eval-runs/<run-id>
```

Proposed Inspect coordinator shape after a pinned wrapper exists:

```bash
uv run --frozen inspect eval eval/retonr_local.py \
  --model vllm/local \
  -M model_path="$RETONR_MODEL_ROOT/<artifact-set-id>" \
  --limit 8 \
  --log-dir eval-runs/<run-id>/inspect
```

For an unsupported vLLM architecture, mark that vLLM run unsupported. Schedule a
separate pinned Transformers and Accelerate run with an exact local path. That run
must set offline controls, disable telemetry and implicit credentials, reject remote
code, and write the complete resolved environment to the run manifest.

## Storage and acquisition boundary

No acquisition occurs as part of evaluation. The owner requested that any later
development-machine downloads use `E:\models`; this protocol does not create or
modify that directory. When authorized, use a dedicated root such as
`E:\models\retonr-eval` and pass it through a task-specific `RETONR_MODEL_ROOT`
setting. Portable manifests store artifact IDs and normalized relative paths, never
the machine-specific absolute root.

Acquisition is a separate network-enabled operation that:

1. Resolves an immutable upstream revision.
2. Reviews license and redistribution terms.
3. Downloads into a staging directory with explicit size limits.
4. Rejects executable remote code and unsafe weight formats.
5. Hashes every required file and creates a canonical artifact-set manifest.
6. Moves the verified bytes into owner-controlled storage atomically.
7. Records tool versions, source URLs, revision, file sizes, and digests.

Evaluation begins later under an outbound-network deny policy. `HF_HUB_OFFLINE=1`,
`HF_HUB_DISABLE_TELEMETRY=1`, `HF_HUB_DISABLE_IMPLICIT_TOKEN=1`, and exact local
paths are defense in depth, not substitutes for the deny test.

## Evidence bundle

Each run produces one content-addressed directory containing:

- Plan and suite manifests
- Retonr, adapter, runtime, environment, artifact, template, and parameter identities
- Case order and randomization seed
- Per-case raw status, bounded source and output records, deterministic findings,
  timing, memory, and error data
- Human-review assignments and adjudication results when authorized
- Aggregate tables derived from immutable per-case records
- Command transcript, network-isolation evidence, and a run completion state
- Explicit exclusions, failures, deviations, and invalidation triggers

Synthetic development text may remain in the local bundle. Private owner text needs
separate consent, retention, encryption, deletion, and log-minimization decisions.
Inspect's model API logging must be disabled or redirected to an approved protected
store for private cases. A digest of private text is not anonymization and can permit
guessing attacks against predictable content.

The report names the exact independent unit and does not pool cases that share a
template as though they were independent. Development counts are descriptive.
Qualification intervals and thresholds are defined only after power and governance
review.

## Stop conditions

Stop and retain the partial record when:

- Runtime, artifact, template, tokenizer, parameter, or environment identity drifts
- Any undeclared network connection occurs
- The runtime loads a remote-backed model, plugin, adapter, draft model, or code
- Effective context, precision, or offload differs from the plan
- A sentinel, protected literal, structure, formula, or non-text package component
  changes outside the declared editable scope
- Evidence storage exceeds its bound or cannot be written atomically
- Cancellation fails, a timeout leaves the runtime unhealthy, or another workload
  invalidates resource observations
- The fixed generation ceiling is reached

Do not delete inconvenient output. Mark the run incomplete or failed and preserve
the bounded evidence needed to diagnose it.

## Logical implementation order

1. Define versioned evaluation-plan, artifact-set, runtime-environment, per-case, and
   aggregate-report schemas in the Rust type boundary.
2. Project the frozen 39-case editorial subset into a generation suite without
   changing its existing corpus role.
3. Add a native Retonr local-eval command that runs the real Ollama adapter serially,
   enforces ceilings, and writes atomically.
4. Add exact identity capture, offline preflight, resource sampling, cancellation,
   and resume-safe run completion.
5. Complete and integrate the typed evidence contract, qualify extraction completeness
   and provenance, then add the balanced fidelity challenge suite.
6. Add blinded review export and adjudication import without requiring private text
   to leave the machine.
7. Add a pinned Inspect custom task as an optional coordinator and a preselected vLLM
   research stratum. Schedule Transformers and Accelerate as a separate pinned stratum.
8. Run the installed-artifact RTX 4090 bakeoff and publish a local development report.
9. After explicit acquisition approval, fill the small and medium cohort gaps.
10. Differentially test the leading artifact through pinned `llama-server`, CPU, and
    Apple Metal before making any portability claim.
11. Add Markdown, DOCX, spreadsheet, JSON, HTML, and streaming strata only after each
    adapter has deterministic preservation fixtures.
12. Freeze calibration and locked qualification suites late in 0.x, when prompts,
    schemas, and product behavior are stable enough that a gate is meaningful.

This order creates useful development evidence early without turning an exploratory
RTX 4090 result into a premature support promise.
