# Local model editing smoke comparison

## Status

Status: exploratory development evidence from 2026-08-12. This is not model
qualification, a recommendation, or a supported-hardware claim. External API spend
was zero.

The run asks a narrow question: can several local artifacts return the required
structured envelope, remove obvious editorial padding from one synthetic passage,
and avoid changing one already-direct control? It does not measure general fidelity,
style fit, long-document behavior, or watermark properties.

## Environment

- Operating system: Windows development host
- GPU: NVIDIA GeForce RTX 4090 with 24,564 MiB reported memory
- Driver: 610.47
- Runtime: Ollama 0.24.0 through IP-literal loopback endpoints
- Context: 8,192 tokens requested
- Output limit: 256 tokens
- Sampling: temperature 0, top-p 1, seed 7
- Reasoning output: disabled
- Output: one candidate using the adapter's candidate-envelope JSON Schema

Two Ollama processes shared one GPU during the run. The running-state endpoint later
reported the 27B Qwen artifact partly outside GPU memory while the 8B artifact was
resident. Timings therefore describe this run only and must not be compared as
single-model full-GPU benchmarks.

## Artifact identities

| Runtime reference | Ollama digest | Bytes | Reported family | Parameters | Quantization |
| --- | --- | ---: | --- | --- | --- |
| `ministral-3:8b` | `1922accd5827ebe6829e536369195db25eaf664528dc66206d646ea3bb386b71` | 6,022,236,616 | `mistral3` | 8.9B | Q4_K_M |
| `gemma4:26b` | `5571076f3d70050487b26b341705799e0ab29b808164f90d20d4cf84f699d251` | 17,987,581,215 | `gemma4` | 25.8B | Q4_K_M |
| `qwen3.6:27b` | `a50eda8ed977ab48a12431878896b27ffd5cef552c17af3317d9623b939a7f1e` | 17,420,432,739 | `qwen35` | 27.8B | Q4_K_M |

These are Ollama inventory digests, not independently rebuilt source-artifact
digests. No artifact is qualified by this record.

## Obvious-slop passage

Prompt SHA-256:
`727b4670e102abbceafdccb9f7d2aafc94e340f792933f10659e7f8f1d520fb9`.

The synthetic source states that a migration completed on May 6, 2026 and reduced
processing time from 14 minutes to 9 minutes. It surrounds those facts with scene
setting, promotional contrast, empty significance, and a repeated conclusion.

| Artifact | Candidate | Prompt tokens | Output tokens | Total seconds |
| --- | --- | ---: | ---: | ---: |
| Ministral 3 8B | On May 6, 2026, the migration was completed, reducing processing time from 14 minutes to 9 minutes. | 679 | 50 | 4.942 |
| Gemma 4 26B | The migration completed on May 6, 2026, reducing processing time from 14 minutes to 9 minutes. | 140 | 53 | 17.624 |
| Qwen 3.6 27B | The migration completed on May 6, 2026, reducing processing time from 14 minutes to 9 minutes. | 137 | 53 | 37.906 |

All three candidates retained the declared date and numeric relationship while
removing the targeted padding. This is one positive case, not a fidelity estimate.

## Already-direct control

Prompt SHA-256:
`59ea81a85c125da6d21cb195a65dbb1f788bbefc0ed87f57b31e5b3e6c25fa76`.

The control contains the same migration facts plus a June 1, 2026 review date and
explicitly asks for unchanged output when the prose is already direct.

| Artifact | Observed behavior | Prompt tokens | Output tokens | Total seconds |
| --- | --- | ---: | ---: | ---: |
| Ministral 3 8B | Rephrased the first two sentences and introduced Markdown emphasis around all dates and durations. | 652 | 66 | 1.214 |
| Gemma 4 26B | Returned the source unchanged. | 114 | 53 | 17.997 |
| Qwen 3.6 27B | Changed `fell` to `decreased` while preserving the remaining text. | 112 | 69 | 40.892 |

The control differentiates the artifacts more clearly than the obvious-slop case.
It supports retaining exact unchanged-output rewards, format gates, and edit-cost
ranking after fidelity acceptance. It does not prove that Gemma is generally better
or that a smaller model cannot qualify on lower-resource hardware.

## Portable tournament

The development tournament should compare exact artifacts in these measured classes:

| Class | Initial candidates | Purpose |
| --- | --- | --- |
| Minimal | Current 2B to 4B instruction artifacts and CPU quantizations | Establish whether low-memory or slow CPU hardware can meet the fidelity floor at useful coverage. |
| Compact | Ministral 3 8B and a current cross-family control | Exercise common 8 GB to 12 GB devices and Apple unified-memory systems. |
| Balanced | Current 12B to 20B dense or sparse artifacts | Find the smallest strong default for modern laptops and modest accelerators. |
| Workstation | Gemma 4 26B and Qwen 3.6 27B, with a cross-family control | Measure higher coverage or style gain without lowering fidelity. |

CPU, Metal, CUDA, HIP, Vulkan, and hybrid results are separate execution classes.
Slow tokens per second is acceptable when the declared workflow remains usable and
quality passes. A resource class is unsupported only when measured quality,
reliability, memory, or practical completion behavior fails its declared contract.

## Next evidence

1. Run the frozen smoke group through the native adapter and record complete request,
   runtime, artifact, template, parameter, and execution identities automatically.
2. Add exact unchanged-output, bounded-edit, and structured-output metrics.
3. Run paired positive, clean-control, and hard-negative cases across at least two
   model families per viable resource class.
4. Measure cold load, warm latency, memory, throughput, cancellation, and context
   degradation without pooling unlike execution classes.
5. Use independent human or deterministic labels. Model judges may surface
   disagreements but never become the sole expected-answer authority.
6. Publish support only after locked fidelity and accepted-set error evidence passes.
