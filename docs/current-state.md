# Current implementation state

## Checkpoint

The public repository completed the milestone 0.1 technical evidence and is
implementing milestone 0.2 under the `Retonr` project identity. 0.1 is not a tagged
milestone release; INV-Q04 applies from the first completed 0.2 closeout. External
contracts remain provisional. Public source availability does not freeze package,
protocol, or stored-data contracts.

## Implemented

| Component | Current behavior |
| --- | --- |
| `rewrite-types` | Versioned document, candidate, gate, status, reason, edit, rewrite-record v2, redacted generation provenance, content-redacted typed claim-evidence contracts, and an inert extractor manifest |
| `rewrite-text-adapter` | Bounded UTF-8 parsing, optional BOM retention, newline fingerprints, exact no-edit output, apply, reparse, verification, and a content-redacted pre-model inventory of encoding, controls, and possible unstructured-text Content Credential wrappers |
| `rewrite-engine` | Cancellation, typed value protection, sentinel integrity, hard gates, closed structure and semantic evidence boundaries, deterministic claim comparison, an informational shadow claim-comparison gate with no eligibility authority, reason priority, lexicographic selection, and document-atomic abstention |
| `rewrite-model` | Separate immutable single-file and canonical artifact-set identities; strict package-source and transformation evidence; complete runtime-package and model-package manifests with distinct entrypoint, generation-worker, forbidden-utility, helper, dependency, resource, configuration, license, provenance, and transformation roles; inert native-load observations; typed runtime-build and effective-state derivation from package and load evidence; additive effective-package v2 member uses with shared-byte purpose-union validation; strict bounded inert generation-qualification cluster, case, suite, repetition, stable generation-system, planned-attempt, qualification-plan, prelaunch precursor, managed-evidence, cleanup, bundle, readback, receipt, completed-or-failed attempt, pre-output selection-policy, repetition-scoped receipt-set, ordered receipt-pair, deterministic report-relationship, and portable deterministic-evaluation records with exact manifest, ordinal, source, request, output-ceiling, runtime, model, effective-state, effective-package, candidate, semantic-suite-order, report-summary, and status relationship validation; frozen qualification v1 authority; and a distinct inert claim-extraction qualification v2 record and ID that cannot enter v1 activation |
| `rewrite-model-store` | Durable SQLite schema 12 preserving every schema 1 through 11 artifact, lifecycle, activation, package, evidence, preregistration, generation-system, portable plan, case, and terminal-evidence contract; atomic generation-qualification policy and complete-projection preregistration with typed canonical readback; atomic effective-package V2 and generation-system foundation persistence; atomic portable plan persistence with bounded recursive cold readback; exact pretraffic candidate precursor checkpoint; atomic completed-or-failed candidate execution closure over managed evidence, cleanup, bundle, root-bound storage reference, bounded readback, receipt, and attempt records; additive schema 11 terminal-evidence tables for platform evidence, license evidence, attempt-ledger manifests, candidate-generation-failure repeatability results, repeatability, resource, and human manifests, operation receipts, and phase interruptions, with an atomic cohort writer and no live authority; additive schema 12 judge-execution tables for candidate judge plans, schedules, request aggregates, response aggregates, observation batches, managed local judge receipts, and candidate judge joins, with no judge-execution writer and no qualification, activation, or live-use authority; an outcome-only probe that is not yet integrated into activation recovery; exact supported-schema inspection; single-epoch exact-handle backup and atomic migration from schemas 1 through 11; recursive relationship checks; immutable-conflict detection; fail-closed recovery; and bounded coherent artifact-state inventory |
| `rewrite-inference` | Backend-neutral bounded discovery, adapter-admitted output-contract digests, candidate generation, a distinct claim-output contract, structured completion, and a provider-neutral local-judge attempt contract with exact choices, sorted rubric clauses, and bounded cited byte spans; content-redacted debug and error surfaces, cancellation, deadlines, and deterministic fakes |
| `rewrite-grounded` | Structured masked prompt envelope, exact inference policy, proposal-only candidates, and redacted generation provenance |
| `rewrite-ollama` | IP-literal loopback-only native API adapter with bounded bodies, explicit parameters, exact candidate-contract discovery, candidate and structured completion, terminal-stop enforcement, concurrency, cancellation, pre-call and post-call identity checks, coherent read-only runtime, inventory, model-description, and residency preflight; a caller-supplied retained HTTP/1 session with one preflight, an absolute 4 MiB UTF-8 completion-input ceiling enforced before wire serialization or completion traffic, structured completions, connection callbacks, monotonically ordered response checkpoints, nonserializable content-free request and response receipts, and an opt-in v0.32.15 nine-response completion profile that binds two equal post-generation runtime-reported residency observations while proving neither handler, model use, resident-page identity, effective identity, nor qualification; a separate caller-stream runtime-only probe limited to one 1 KiB-capped `GET /api/version`, exact typed version equality, and non-authoritative version evidence; no connector, pool, retry, reconnect, or fallback path; inert exact-version cloud-disable declaration and startup-marker evidence whose production reviewed-runtime allowlist is empty; and a separate exact version, package, and backend-conditional worker generation policy whose production allowlist is also empty |
| `rewrite-ollama-package` | Strict bounded parsing of the accepted Ollama manifest-v2 layer shape and GGUF v3 metadata and tensor table, followed by deterministic reconstruction of one canonical six-member model artifact set and semantic model-package manifest; inert full LocalArchive foundation verification that re-derives source, descriptor, logical-binding, artifact-set, model-package, provenance, and logical license-member relationships from exact streams, plus a narrower binding recheck that is valid only after an independent complete member rehash; bounded reconstruction of one reviewed Linux x86_64 GNU libc Ollama runtime package from a layout and exact member bytes, including exactly one generation worker, an isolation helper that must not be code-loaded, and an exact transformation record whenever the package differs from its source artifact set; the unchanged schema-1 exact package-review disposition; a typed canonical controlled-build input manifest that requires the source, module, checksum, toolchain, compiler, assembler, linker, build-script, native-package, standard-library, helper, and license closure plus an explicit offline CPU-only policy, cleared environment, and ordered arguments, independently hashes every component stream, and derives a content-free build plan; a typed two-attempt build report compiler and verifier that hash layout, SBOM, provenance, transformation, isolation, and bounded stream evidence, reconstruct both runtime trees, and independently derive byte-identical or different output disposition; and additive schema-2 controlled-source-build compilation and verification that accept only paths, evidence classes, and control results, hash every bounded evidence record, consume the verified input closure, reconstruct the runtime even when the result is blocked, derive identity fields from bytes, and reject incomplete or inconsistent dispositions. These contracts have no admitted production record or policy authority; LocalArchive verification makes no upstream-authenticity claim, and the crate performs no network, qualification, activation, load, or execution |
| `rewrite-runtime-isolation` | Linux managed prelaunch isolation with an expected-identity helper copied into an executable sealed anonymous file before the first probe; fresh user, network, PID, and private mount namespaces; recursively private propagation; loopback-only networking; a private `/dev` exposing exactly one retained usable `/dev/null`; fresh PID-namespace procfs; ambient descriptor close-on-exec sealing with verified stage-two closure before target launch; and no-new-privileges plus complete capability reduction. The target-inherited seccomp policy permits only `AF_INET` and `AF_INET6` through `socket()`, denies every other socket family, denies `io_uring_setup`, namespace and mount escape APIs, device creation, `bpf`, `clone3`, and namespace-bearing `clone` flags, and requires seccomp mode 2 during target reobservation. The retained lease binds strict READY2 device evidence, mount and namespace identities, bounded mount-info parsing, usable null-device identity, exact device entries, private proc identity, retained-handle target launch, bounded startup streams, one namespace-local loopback and socket-diagnostics capability, repeated reobservation, and process-tree teardown. This is bounded device visibility evidence, not formal CPU-placement proof. A distinct retained controlled-build launch uses fresh user, network, PID, and private mount namespaces, recursively private propagation, and a fresh 16 GiB, 262,144-inode tmpfs at `/tmp`. The byte quota reserves 4 GiB of bounded scratch beyond the 4 GiB private-input and 8 GiB committed-output ceilings. Capability ABI 2 transfers exact path, byte-count, SHA-256, and retained-file declarations, copies at most 4 GiB into a normalized private input tree, verifies every copy, and remounts the complete tree read-only without exposing the caller input root. It exposes a distinct empty private tmpfs directory as the fixed output alias; the target never receives or mounts the retained host output. The parent independently observes the mount namespace. The helper revalidates the retained program, mapped input files, private aliases, and host output; the builder separately revalidates the capability ABI, input and private-output descriptor-to-alias identities, and output working directory. Landlock ABI 3 permits input reads and execution, exact read and write access to `/dev/null`, and writes only to the private output while denying ambient host reads and other device files. Stage two completes only after the coordinator is reaped and descendant-held diagnostic streams drain. The guardian then commits at most 4,096 output entries and 8 GiB of regular-file bytes, exports from retained private-file handles into the initially empty retained host output, rehashes during copy, and requires an exact host-tree recommitment. Cleared environment, bounded diagnostic tails and outer streams, bounded wall time, cancellation, and process-tree teardown remain enforced; deterministic unsupported results on Windows and macOS |
| `rewrite-runtime-source-builder` | Static offline coordinator for the reviewed Ollama v0.32.15 Linux CPU source build. Under capability ABI 2 it revalidates the inherited input and private-output descriptors against their fixed aliases and requires the private output as its working directory; extracts only the exact frozen archives; applies the retained transformations; invokes the frozen toolchain; assembles the bounded runtime tree; and writes layout, SBOM, provenance, and transformation evidence. Two developer-only preparation binaries normalize already-fetched local source trees and compile the frozen component manifest and review evidence. They do not fetch inputs or grant admission, qualification, activation, generation, or user-facing `retonr` CLI authority |
| `rewrite-runtime-attestor` | Safe bounded facade over native attached-listener and exact established-connection evidence: Windows owner-PID tables plus retained process and executable handles; Linux bounded `NETLINK_SOCK_DIAG` dump and exact retained-cookie queries plus a proc-root-relative holder scan anchored by pidfds, strict bounded effective-UID status parsing, relative descriptor inspection, namespace evidence, and a retained executable object; a Linux managed observer that consumes exact launch facts and a namespace-local diagnostics capability; Linux object-bound native-load observation; a distinct retained generation-worker observer that binds the exact descendant, parent chain, namespaces, zero-capability privilege state, reviewed v0.32.15 CPU command, static native closure, and byte-identical private GGUF mapping through final reobservation; a distinct canonical `authority: none` external-component discovery report that cannot be passed as an allowlist; a frozen-set verifier that binds exact discovery and separate review-evidence bytes before exposing expected components; deterministic unsupported results for exact native-load binding on Windows and for attached, managed, discovery, native-load, and worker observation on macOS; redacted inert evidence only |
| `rewrite-app` | Model-free candidate check; provisional grounded path; exact offline single-file and artifact-set lifecycle; schema-8 migration; whole-tree runtime and model package leases; retained handles for every runtime code member, including the worker, and every model-package member; bounded, cancellable, cursor-independent revalidation of exact retained model objects; static package attestation; and an opaque lease-bound managed Ollama input plan. The plan requires the exact six-member package, source, format, descriptors, roles, and explicit model reference; reconstructs the original manifest and content-addressed blob aliases without reopening host paths; retains the unique model-weight object for later worker observation; and exposes only redacted mapping evidence and the target-visible model identity. The closed v0.32.15 Linux composition requires a noncloneable verified launch capability that consumes the exact input plan and an independently approved LocalGeneration model-license authority bound to the same specialized lease. The production license approval policy is empty. Structural authorization performs no model rehash; launch revalidates immediately before private input materialization, fixes `ollama serve`, loopback, the cleared offline environment, and CPU-only visibility, and retains exact private runtime, launch, model, and license subjects without exposing raw handles. Cleanup uses an independent cancellation token and preserves combined isolation and authority failures. Exact private GGUF mapping supports bounded model-load evidence, not model use. The app-owned effective-package v2 path derives exact model, runtime, license, embedded-component, shared-byte purpose-union, exclusion, and isolation relationships only from retained capabilities. It releases typed evidence only after managed cleanup plus independent final model and runtime package revalidation, and rejects equal portable facts from a different live subject. The crate also provides inert installed-Ollama model reconstruction, managed import, package persistence, and readback; inert reviewed Linux Ollama runtime-package reconstruction, managed import, schema-6 package tables preserved unchanged by schema 8, and readback, with extra tree files failing closed and no cloud-disable allowlist change; a read-only controlled-build bundle verifier that validates ceilings before source access, pins a separate manifest and exact component tree, rejects indirect, multiply linked, missing, extra, changed, or digest-drifting input, retains the verified handles, and exposes a content-free deterministic build plan; a Linux-only executor that rejects source or attempt-root overlap, runs two fresh retained controlled builds, requires successful nontruncated results, independently rehashes each guardian-exported host tree, requires exact equality with the helper tree commitment, and retains member handles, fingerprints, and byte digests before return. That evidence seal detects substitution and later drift but is not an operating-system write lock. The executor revalidates every boundary, rejects unexpected or aliased output entries, derives app-owned isolation and stream evidence, and compiles a canonical independently verifiable two-attempt report; a durable evidence publisher that copies the complete frozen input set, both output trees, managed evidence, and report into one bounded application-owned tree, derives an exact content manifest, verifies and synchronizes staging, publishes without replacement, and supports independent retained reacquisition and full rehash; a build-stage review compiler that uses only that retained root, independently verifies its canonical schema-2 bytes, keeps all six semantic controls unrun, and grants no partial approval; and a non-authoritative two-operation runtime-admission runner. Discovery launches only the retained entrypoint, reparses canonical native discovery, and always cleans up with a fresh cancellation token. Final verification consumes the separately verified frozen native set, repeats exact managed process and package binding, performs the retained version probe and final reobservation, independently re-derives canonical native-load and managed-final records, and returns opaque foundation-bound native-closure, managed-startup, and cloud-disable passed controls. Neither operation mutates policy or grants admission. A separate managed-judge precursor closes the portable plan, schedule, request aggregate, judge system, runtime, model, static interpretation, characterized package, exact prepared isolation, and 14 policy bindings before releasing a noncloneable runner handoff. Its phase observer enforces the exact seven-response preflight and nine-response attempt profile, observes and reobserves server and worker state, and seals each request, response, complete residency receipt, contiguous ordinal span, and effective-state subject before schedule advancement. Its schedule-wide authority consumes the completed sequence, revalidates every attempt and ordinal span, and derives exact residency, process, native-load, connection, and effective-state aggregates plus a dedicated effective-state join. A distinct schedule-wide effective-package plan rejects a singular final-attempt surrogate, validates the exact prepared-isolation subject and every real retained attempt state, derives fresh V2 evidence from the common state, and releases the full authority only after cleanup plus independent model, runtime, evidence, and authority revalidation. The single-entry managed schedule executor applies one absolute deadline, executes every exact attempt, preserves cleanup and final-validation failures, derives a durable receipt with resident response ordinals `8..(7 + 9 * N)`, compiles canonical compatibility triage, and returns a nonforgeable opaque join with mandatory fresh full-record revalidation. An older inert managed-process attestor with caller-supplied loaded-component evidence, a cancellable pair-extraction service, and an informational shadow join retain their prior semantics. The controlled-build tools include developer-only binaries and an example runner, but no user-facing `retonr` command or admitted production runtime |
| `retonr` | Provisional `check` command with file or multiline standard-input documents, an explicit non-replacing output policy, opt-in `--in-place` (`-i`) with an implied sibling backup for a regular file, escaped interactive terminal rendering, a terminal raw-output double opt-in, escaped `--diff`, `--dry-run`, and redacted `--trace`; a pre-model `inspect` command that inventories one file or directory: encoding, BOM, newline kind, control-class counts, sibling sidecar presence, and skipped child reasons, without stripping bytes, following links, or validating a Content Credential; `--recursive` is a bounded walk that skips hidden names, `target`, and `node_modules`; a `rewrite` command that validates one source, optionally inspects `--data-dir` for an active generation binding and an exact `--artifact-id`, then attaches in-process fake-backend conformance when that recovered qualification names the retained fake backend, or fails closed otherwise; dedicated `version` and read-only `doctor` recovery commands that name migrate or removal-recovery follow-up without mutation; generated `completions` scripts and a section-1 `man` page from the live CLI definition; plus an explicit-root offline model-artifact CLI for single-file import, exact artifact-set folder import, read-only `list` of registered single-file installations, read-only `inspect` of one registered artifact's declared facts, inventory, set inventory, pending-operation inspection, confirmed repository migration, selected reconciliation, selected set reconciliation, inactive removal, exact removal recovery, inactive set removal, exact set-removal recovery, and optional read-only `device-evidence` (`fitr`) of `fitr.retonr.evidence.v1` without qualification or a repository |
| `rewrite-eval` | A 49-case versioned positive and hard-negative suite with exact expectation results and transformation coverage, four baseline contracts with an offline no-rewrite CLI and recovered fake-conformance attach for generative kinds, 120 cases across five balanced synthetic editorial groups, a writing-sample library, a research-only watermark-refusal corpus, an independent claim-shadow calibration runner, a strict exact generation deterministic-case contract that binds source identity and every predeclared expectation, retained exact generation case material, a versioned hybrid scorecard library and CLI that bind and execute exact deterministic suite pairs before normalizing blinded order-swapped triage observations, a typed retained-session local-judge executor that runs both orders after hard gates and returns a separate limited transport receipt, a version-gated v0.32.15 static installed-package-to-inventory binding that consumes an opaque nonserializable exact-runner receipt, a versioned non-generative Ollama observe or verify preflight, a separate native attached-process preflight that remains response-unbound, a retained-connection preflight with repeated native attribution, a Linux-only managed preflight library that binds retained runtime-package, isolation, process, connection, provider-declaration, read-only API, and native-load evidence and can additionally return an inert package-declared typed `RuntimeBuildIdentity`, and a one-shot managed generation bracket that independently requires reviewed cloud-disable status and an exact package-and-worker generation-path review; both production allowlists are empty and block the bracket before launch, the scorecard remains caller-declared and triage-only, model artifact and inventory identities remain distinct, the managed candidate-attempt executor returns either nonforgeable completed authority or an exact portable failed record while retaining its typed primary cause and every cleanup or revalidation cause behind redacted debug output, the completed receipt join produces a noncloneable and nonserializable verified candidate batch, the semantic-order batch-set join rederives its portable receipt set internally, the qualified deterministic compiler derives the portable hard-gate record from live batch-set and case-material authority with mandatory final revalidation, and the offline candidate-to-judge preparation join derives and retains the exact judge plan, two-order schedule, request aggregate, selected authorities, and compatibility hard-gate projection without runtime or network inputs. Its no-launch pairing boundary consumes and revalidates both real runner handoffs, requires complete plan, schedule, request-aggregate, and judge-system equality, and privately retains the paired authority. The consuming Active operation is the sole public high-level managed judge entry. It consumes exact preregistered repetitions in order, validates the complete operation scope, and supplies the retained Prepared deadline and one lifecycle to the crate-private executor. That executor consumes the paired authority once, runs the complete schedule, derives a cleanup-gated eval-owned durable receipt with the exact ordinal span `8..(7 + 9 * N)`, compiles canonical content-free compatibility triage, and returns a noncloneable, nonserializable opaque join. Public revalidation reruns exact triage from current candidate authorities and observations, rebuilds the complete join record, and requires full equality inside mandatory initial and fresh terminal authority validation. The chain has no CLI, its records remain inert, and its judge evidence remains probabilistic, triage-only, and unqualified |
| Fuzz targets | Protection round trips and plain-text no-edit byte identity |

Controlled-build seccomp denies `socket()`, `io_uring_setup`, pathname and stream
local socket pairs, and every `socketpair()` tuple except the
exact anonymous `AF_UNIX`, `SOCK_SEQPACKET | SOCK_CLOEXEC`, protocol-zero channel
required by the pinned Rust process launcher. An active canary verifies the exact
exception and the surrounding denials.

Managed judge response portability no longer depends on provider response-ID
uniqueness. Each `CandidateJudgeResponseV1` identity binds the exact plan, schedule,
zero-based schedule index, request identity at that position, and nested Ollama
transport response identity. Construction and decoding rederive the first four
associations from the separately loaded plan, schedule, and request aggregate.
Observation normalization, the response and observation aggregates, the managed
receipt relationship, and the portable join use that same schedule-indexed identity.
Repeated equal provider response IDs therefore remain valid nested evidence while
the two presentation records remain distinct. The underlying Ollama response ID
also binds the complete structured request, including its presentation-specific
seed, so equal candidate and output bytes do not normally produce equal provider IDs
in a valid admitted request aggregate. Neither identity is execution, model-use,
semantic, or qualification proof.

The model layer also contains the implemented strict bounded phase-evidence
contracts: the attempt-ledger manifest, repetition-scoped repeatability result,
repeatability-evidence manifest, resource-evidence manifest, and
human-adjudication manifest. They validate their typed scope, ordering, status, and
  subordinate-record relationships but remain inert. The five portable operation
  prerequisites are now implemented as well: operation policy, complete request
  projection, platform evidence, license evidence, and operation receipt. The inert
  `GenerationQualificationPhaseInterruptionRecordV1` companion is also implemented.
  Their
bounded canonical decoders reconstruct from exact typed relationships and trusted
expected inputs; serialized policy, request, assessment, and runner fields do not
become their own authority. The receipt accepts peak-zero `NotRequired` for
`Cancelled`, `DeadlineExceeded`, or `Failed` before managed authority acquisition,
and for `Completed` only with an exact rejected platform or license gate and skipped
phase manifests. Peak one requires passed or failed live finalization, and
  `Completed` cannot combine with failed finalization. The interruption record binds
  one noncompleted receipt to its exact operation policy, target, plan, suite, derived
  phase policy, phase, checkpoint, optional planned attempt, and closed reason. It is
  not a phase-manifest item and grants no execution, persistence, activation,
  qualification, or live-use authority. The compact
`GenerationQualificationRecordV1` is implemented with a strict canonical
nine-field encoding, internally derived status, complete trusted relationship
revalidation, and explicit rejection of incomplete all-completed or all-passed
prefixes as policy decisions. It is inert. The eval-owned
`VerifiedPassedRepeatabilityJoins` authority now binds each caller-supplied,
internally consistent Passed result to one distinct live opaque join in exact order,
retains those authorities, and performs mandatory fresh final validation across the
supplied subset. It grants no qualification authority. Canonical app-owned platform
and license assessment-policy authorities are
implemented with strict bounded encodings, empty production roots, and retained
structural denial states. Model-license control verification now produces a separate
structural live proof before explicit production promotion, and its root can be
assessed without minting launch authority. The app-owned platform and license
assessment compilers are implemented. Platform assessment consumes a noncloneable,
nonserializable, double-sampled current-host authority backed by the canonical
privacy-bounded `HostEnvironmentV1` and four typed projections; intrinsically
unsupported tuples and reviewed policy denial stay host-free. The model layer also
defines the typed English, plain UTF-8, whole-document generation-case request profile,
and the grounded crate exposes the shared byte-exact prompt renderer. The app also
implements the deterministic one-at-a-time request builder with preplanning
derivation, stable component and request identities, exact post-plan reconstruction,
and no public raw prompt or protection getter. Its streaming projection authority
compiles and retains one bounded request at a time. The eval-owned Draft -> Projected
-> Prepared state machine captures the absolute operation deadline before policy
construction, consumes the stream and app assessment authorities, materializes the
exact schema 8 generation-system and schema 9 plan foundations, then cold-validates
the plan under the schema 7 preregistration write lock. Prepared retains the complete
recursively checked foundation and both preregistration records as a nonserializable
pretraffic authority. It grants no launch, network, model, or
request-traffic API. A traffic-eligible Prepared state can now be consumed once into
the noncloneable, nonserializable `ActiveGenerationQualificationOperation`. The Active
owner retains the exact Prepared authority and one process-local live lifecycle and
brackets activation and revalidation with both normal deadline-aware checks and fresh
mandatory-finalizer checks. Its sole high-level strict judge method consumes exact
preregistered repetitions in order, validates the complete target, baseline, plan,
suite, eval, and app scope, and supplies the Prepared deadline and single lifecycle to
the managed schedule. Its strict candidate method consumes target and baseline
  attempts in exact plan and request-projection order under that same deadline and
  lifecycle. Active first checkpoints the exact prelaunch precursor and requires a new
  insertion before traffic; an exact replay is inert. Target attempts require Approved
  resource observation, baseline attempts use the compatibility observation shape, and
  failed attempts become durable before their ordinal is consumed and the operation
  terminalizes without retry. Active owns completed execution while it is pending.
  App-root-bound publication, bounded fresh readback, receipt compilation, atomic
  terminal metadata, and in-transaction bundle reacquisition all succeed before Active
  advances a successful durable ordinal. A private process-local subject binds Active
outcomes, batches, batch sets, paired judge handoffs, and judge joins, which rejects
same-scope authority replay across Active owners. Active retains target records only,
seals their exact plan-order prefix into one noncloneable attempt-ledger closure, and
refuses judge traffic until candidate settlement is complete and the ledger is sealed.
Rejected Prepared operations now terminalize without acquiring
managed authority or traffic into four exact skipped manifests, a peak-zero
`NotRequired` receipt, and the compact rejected qualification record. Canonical
resource and human-adjudication phase policies now have strict bounded structural
verification, domain-and-length-framed identities, and empty application-controlled
production roots. Distinct typed source-denial records bind the exact target, plan,
suite, and phase policy and can be the sole subordinate identity of a failed resource
or human manifest without claiming that either phase ran. App-owned denied-phase
compilers consume the exact verified policy once, retain the fixed operation and
borrowed scope, and freshly revalidate the resulting inert record and one-item
`Failed` manifest. These authorities grant no execution or traffic. A final compiler
may consume the resulting manifest only when phase ordering has legally reached that
phase. The Prepared authority also has a consuming fail-fast transition for a
source-denied resource or human policy. It retains both exact policy authorities,
the applicable standalone denial records, four empty `Skipped` manifests, and a
peak-zero `Failed` plus `NotRequired` receipt. It exposes no qualification record or
traffic method. The model layer now also has the first inert positive
resource-attempt result, which binds one completed target attempt and receipt to
full-precision provider, monotonic, worker high-water, payload-footprint, and ordered
exceeded-limit fields. The Linux runtime observer can obtain a
pidfd-and-start-token-bracketed kernel `VmHWM` value, and runtime attestation exposes
the exact verified total payload size separately from executable-code bytes. The
Ollama opt-in path inseparably owns the exact response, resident receipt, six-field
provider observation, response-head checkpoint, and retained-session subject. The
Approved-only app measurement authority consumes that composite, the exact worker
observation, cleanup-gated package, and freshly revalidated package leases. It derives
close-only and attempt timing, checked installed footprint, and strict ordered policy
exceedances, then retains only borrowed completion views behind a self-validating
nonportable snapshot. A distinct strict eval runner enforces the pretraffic clock
sequence, retains the exact worker and app authorities through cleanup, and constructs
the portable attempt result only after the candidate receipt exists. The ordinary
runner remains compatibility-preserving and rejects an uncompiled strict resource
closure. Strict batch sets reject mixed or missing observations and preserve exact
semantic-order results through A/B judge presentation and every freshly revalidated
repeatability join. The stronger complete Passed boundary now binds the exact Passed
attempt ledger and its operation-policy digest to one result for every frozen
repetition before revalidating the complete live join set. The resource compiler
accepts only that closure plus an app-owned Approved policy, independently rederives
strict exceedances, and emits no manifest for missing or invalid observations. The
strict runner preserves Prepared's one absolute operation deadline across launch,
preflight, generation, final observation, joins, and mandatory finalization. It is
not production-complete, and
production Approved roots remain empty. The final
`VerifiedGenerationQualification` compiler is not implemented.

### Historical local real source-build validation

On 2026-08-26, the current workspace completed two fresh controlled builds of the
frozen Ollama v0.32.15 Linux CPU candidate. Each exported tree contains 42 files and
112,538,256 regular-file bytes. Independent file-record manifests are equal and hash
to `cf64e48d470105ee2eaaea41c5d58946952e2fe039e5f0aecafdc37c51d008df`.
Independent Linux path, type, mode, and size manifests are equal and hash to
`54ac56462915b9f61c18bcd3aed5eab1217660d2cb6c9d8dffc527b289ce4f8f`.

The source-input artifact-set ID is
`2433b585dccd20c05a08635ead686a46c68aa1761ce9392cd9ec29e56bfc1317`.
The build-plan digest is
`a2335e020eab27f979a49068be91b1126eaf91265ffae6df87cda00f4818142c`.
The byte-identical runtime artifact-set ID is
`73b9a15f695836743572813537cafc73748366c91b0db5a440aaa3a959d04092`,
and its reconstructed runtime-package manifest ID is
`60c3aa6bdd7ff9bec4274eb9c0ad88e0e88b19a636322a1946712384095062a4`.
The durable evidence artifact-set ID is
`54c861ef10e4e4f912bfa1759c697a62937b69b79c9922407112dd9ef6ddf440`.
The publisher reacquired the evidence after no-replace publication, and a separately
built read-only verifier reacquired and fully rehashed it again. A later workspace
build-cache cleanup removed that local ext4 image and its target-owned bundle and
outputs. A hardened v16 attempt then recovered the exact 17-file, 1,935,956,001-byte
frozen input tree from its private namespace, but failed closed when the cleanup
removed its retained host output directories. No current durable build evidence is
available for admission. A fresh typed-receipt build, publication, and independent
reacquisition are required. Both production allowlists remain empty.

The literal semantic evaluator accepts only an identical case-folded alphanumeric
and newline-token sequence. Punctuation can change. Lexical content, newline
placement, unsafe controls, protected values, or structure cannot.

Typed claim evidence binds each bounded extraction to an exact unit, text digest,
extractor-manifest digest, completion state, confidence policy, and canonical evidence
digest. The deterministic comparator accepts only complete compatible sets, rejects an
empty nontrivial source extraction, retains unknown and below-threshold counts, and
binds the aggregate to both exact evidence sets. Extraction is not implemented and
remains probabilistic when added; comparison evidence is not semantic proof.

Inference capability discovery now lists sorted, unique schema digests instead of a
generic structured-output Boolean. Grounded generation and evaluation require an
exact digest match before backend work. The structured-completion port returns only
one bounded complete JSON value after exact artifact checks; its request and response
debug views omit prompt and generated content. The model domain now has a distinct,
inert claim-extraction artifact role. The current Ollama implementation still admits
only generation and the existing candidate schema. The claim-output contract and
an inert extractor manifest now exist. Ollama discovery still refuses that
digest. An application pair-extraction service can call a backend twice and
compare the results. Completed comparison evidence can be joined onto the
engine's informational shadow gate for candidate-check and grounded
transactions. The join is skipped when the backend does not admit the claim
contract, the payload is unusable, or extraction is incomplete. A claim
conflict cannot reject a candidate that already passed the hard gates. The
join has no activation path or product authority. An independent
`rewrite-eval` calibration runner assigns fixture claim identities, compares
them separately from generation, and records whether attaching that
informational shadow changed hard-gate acceptance. It cannot promote a claim
result into a hard gate. Qualification schema v1 explicitly rejects claim
extraction, and its activation APIs cannot accept the separate v2 identifier or record.

The model domain now also represents a canonical, path-bounded artifact set;
content-addressed runtime-build and effective-state records; and an effective-package
evidence records that join those exact identities. Version 1 requires one canonical
purpose set for every artifact member. Additive version 2 distinguishes effective,
evidence-only, dual-use, and positively excluded members with typed roles. When
multiple logical paths share one `ArtifactId`, version 2 requires every effective
path to carry the same union of output-affecting purposes while evidence roles remain
path-specific. Both versions bind completeness, acquisition, license review,
transformation disposition, runtime load closure, and exclusion and isolation
evidence. Their private fields, byte-bounded relationship-aware decoding, closed
vocabularies, fixed canonical encoding, and frozen digests make identity comparisons
portable across Windows, macOS, and Linux. These records are inert evidence
vocabulary. They do not attest a live process, prove that supplied evidence is true
or complete, authorize a role, or upgrade a v1 qualification.

Additive semantic package contracts now distinguish an exact runtime package from
an exact model package. Runtime members have complete static roles and load policy;
model members bind weights, tokenizer, prompt templates, parameters, license, source,
and transformation evidence. Evidence roles alone do not imply output influence;
shared byte identities still inherit every actual output-affecting purpose. A
native-load observation separately binds one retained process witness to
the exact reviewed runtime package and observed executable mappings. Linux can build
that observation from retained package-member file objects and `/proc/PID/map_files`
under strict limits. Windows returns unsupported because its admitted public mapping
APIs do not bind a mapped section to an exact retained file object. macOS returns
unsupported. These records remain evidence, not activation or qualification.

Qualification v2 is a separate, inert, content-addressed record for exactly the
claim-extraction role. It binds the artifact set, effective-package evidence, runtime
build, effective state, source and context ceilings, prompt, claim-output and
claim-operation contracts, request and threshold policies, language policy, hardware
envelope, qualification suite, retained result evidence, license decision, and
qualification outcome. Its bounded decoder reloads and rechecks all four subject
records. It has no `authorizes` method and cannot enter existing v1 activation or
recovery. SQLite schema 4 persists the five immutable evidence records
in separate tables and adds a bounded installed-set table with a unique portable
set-root key and distinct positive generation. Schema 5 adds a separate
artifact-set removal journal. Schema 6 adds immutable runtime-package,
model-package, and native-load tables with relationship foreign keys. Schema 7 adds
only the immutable generation-qualification operation-policy and request-projection
preregistration tables and preserves every earlier record byte-for-byte. Migration
creates all additive tables empty and never infers package, load, installation, or
authority evidence from legacy state. Every dependent
write and read reloads canonical record bytes,
recomputes indexed identities, and recursively cross-checks the complete subject. The
v1 tables and serialized records remain unchanged, and migration grants no authority.
The installed-set record is structural persistence only. It does not prove that the
root or member bytes exist, grant a lease, attest a runtime, qualify a package, or
authorize claim extraction. A lease is a separate verified operation that reads that
record and independently reverifies the managed bytes. The application now writes it only after an exact local
folder import verifies and publishes every manifest member under the content-derived
managed root. That observation does not turn the durable record into authority.
Package leases and the inert installed-Ollama import consume managed-set state, but
no user-facing generation path consumes it yet.
The application and CLI now expose migration only as an explicit confirmed operation
against an initialized repository. A current schema is an exact no-op. A supported
older schema is inspected under both exclusive lifecycle locks and one retained
SQLite write reservation. SQLite copies that locked logical state, including committed
WAL frames, into a bounded rollback-mode snapshot and serializes it into the exact held
repository file. Retonr re-reads and integrity-checks the same file handle, synchronizes
the file and directory, and then commits the supported migration within that same
reservation. The result reports the exact source and target schemas plus an opaque
retained backup key. Ordinary repository commands remain exact-schema and never
migrate implicitly.

## Verification

The complete current trust-chain slice is held to these repository gates:

```console
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features --no-fail-fast
cargo llvm-cov --locked --workspace --all-features --fail-under-lines 80
cargo doc --locked --workspace --all-features --no-deps
cargo clippy -p rewrite-runtime-attestor --all-targets --target x86_64-unknown-linux-gnu -- -D warnings
cargo clippy -p rewrite-runtime-attestor --all-targets --target aarch64-apple-darwin -- -D warnings
cargo deny check
cargo audit --db target/advisory-db-clean
npm run lint:markdown
pwsh -NoProfile -File scripts/check-repository.ps1
cargo +nightly check --locked --manifest-path fuzz/Cargo.toml --bins
cargo build --locked --workspace --release
```

Exact test and coverage totals are intentionally not copied from the previous
schema-5 checkpoint because the trust-chain slice adds target-specific crates and
fixtures. The release evidence is the passing workflow for the merged revision, not
a count embedded before that workflow finishes. Linux-native CI exercises SOCK_DIAG,
managed namespaces, retained launch, namespace-local attestation, and native-load
fixtures. An uncontrolled hosted test may accept only the exact typed access-denied
result when proc policy blocks the required observation. A separate mandatory
networkless container gate drops all capabilities, sets no-new-privileges, runs as the
caller UID, and requires the managed attestor tests to exercise their native success
path. A separate networkless live-worker gate starts as root with only `CAP_SETPCAP`
and `CAP_CHECKPOINT_RESTORE`; the latter permits exact `/proc/<pid>/map_files`
observation. It drops both capabilities before worker execution and requires the exact worker,
static native closure, private GGUF mapping, stable reobservation, and post-exit
rejection path. A privileged networkless gate also executes the retained two-attempt build,
durable no-replace publication, independent reacquisition, blocked schema-2 review
compilation and verification, and drift rejection path. The coverage job runs both
attestor paths and the privileged path with the same LLVM profile as the workspace suite before enforcing
the 80 percent line floor. Windows and macOS exercise their exact supported and
unsupported contracts. Warnings are treated as errors. The local nightly check only
type-checks fuzz targets on Windows; Linux CI runs the bounded sanitizer-backed fuzz
smoke.

The current public-main baseline is
`6a9a00bc1af7181fae6489f5856653ccc7c5bb4b`. Its
[quality workflow](https://github.com/blisspixel/retonr/actions/runs/32615625101)
and [snapshot workflow](https://github.com/blisspixel/retonr/actions/runs/32615998005)
passed for that exact revision. These runs verify the implemented trust-chain and
snapshot contracts; they do not admit a runtime or qualify generation. They predate
the uncommitted controlled-build capability ABI 2 work described below and do not
verify that newer boundary.

## Runtime trust support matrix

| Capability | Linux | Windows | macOS |
| --- | --- | --- | --- |
| Attached listener and retained connection observation | Supported when socket, process, and descriptor visibility is complete; observation-only | Supported through documented owner-PID tables; observation-only | Unsupported |
| Managed prelaunch isolation and process attestation | Supported when unprivileged user, network, and PID namespaces and required process visibility are available | Unsupported | Unsupported |
| Controlled source-build execution | Supported only when user, network, PID, and mount namespaces, Landlock ABI 3, mount operations, and complete proc namespace visibility are available; the parent verifies and seals exact helper bytes before the first probe, and capability ABI 2 copies exact retained-file declarations into a digest-verified, read-only private input tree. It provides exact `/dev/null` access, uses a distinct quota-bounded private tmpfs output, denies target access to the caller input root and retained host output, and requires helper tree commitment, verified host export, and application rehash | Unsupported | Unsupported |
| Exact native-load binding to retained package objects | Supported for exact permitted packaged mappings plus independently verified frozen external components | Unsupported | Unsupported |
| Managed runtime-build and effective-state observation | Supported as inert evidence after successful managed package, process, native-load, retained generation, and exact leaf cross-validation. The portable identity binds reviewed runtime and model content, closed launch and input policy, actual server and worker component closures, normalized worker configuration, platform profile, CPU execution class, and effective context. A separate live join binds attempt-specific process, installation, request, response, residency, kernel, mapping, and reobservation evidence. Cleanup and both package revalidations complete before return | Unsupported | Unsupported |
| Qualified managed generation | Not implemented | Unsupported | Unsupported |

Support in this table describes the individual infrastructure contract, not model
qualification. Attached evidence is observation-only on every platform. Linux
managed evidence is also inert. The current read-only preflight composes the runtime
package, provider, isolation, native-load, and transport side and can construct a typed
runtime-build identity. A separate dual-gated operation retains that boundary through
one structured completion and joins the static model binding and runtime-reported
residency before cleanup. Empty cloud-disable and exact package-and-worker generation
allowlists both block the operation before launch. The operation constructs a
portable inert effective runtime state and a distinct attempt-bound live join before
cleanup, then releases them only after both package leases are revalidated. The
separate transport-only judge receipt cannot create the managed join. The managed
candidate-judge chain instead derives its durable receipt from the released app
schedule authority and full managed preflight. The resulting state, receipt, triage,
and opaque join prove neither model use, formal placement, handler execution,
semantics, nor qualification.

## Deliberate limitations

- The current CLI checks a supplied candidate and administers exact local artifact
  files. `inspect` inventories one source or directory before rewrite. It reports
  encoding, BOM, newline kind, control-class counts, and sibling `.c2pa` or
  `.xmp` presence. Hidden names, `target`, `node_modules`, and symlinks are
  skipped with reasons. Without `--recursive`, child directories are skipped.
  With `--recursive`, the walk is bounded, does not follow links, and uses
  portable `/` relative paths. A UTF-8 BOM plus variation selectors is
  `possible`, not a valid Content Credential. External references are
  `not_checked`. The command does not strip bytes. `model list` and `model inspect` are read-only: they do not qualify,
  activate, download, or treat a report as mutation authority.
  `model device-evidence` (`fitr`) reads optional
  `fitr.retonr.evidence.v1` without a repository. It reports device
  measurement only. `qualified` stays false. Host names, config paths, and
  result paths stay out of the report. The 64 KiB document boundary is joined
  by per-field text and collection bounds, unique served capabilities, positive
  memory measurements, and refusal of terminal-affecting forwarded text.
  Missing fitr is not an error.
  `rewrite` accepts one source file or standard input under the same
  output and inspection policy as `check`, including `--diff`, `--dry-run`,
  and `--trace`. A directory source is a dry-run destination manifest:
  `--output-dir` is required, `--recursive` is bounded, collisions are
  refused, and the output root cannot nest with the source. Existing ancestors
  are resolved before the root comparison, so a link or path-case alias cannot
  hide nesting. A dangling destination link is a collision rather than a
  planned output. No files are written. Optional `--data-dir` inspects an existing repository for an
  active generation binding. Optional `--artifact-id` must match that binding.
  When the recovered qualification names the retained fake backend, `rewrite`
  attaches in-process conformance, generates an identity candidate, and runs
  the common gates. It does not start a runtime, pull a model, or use the
  network. Any other recovered backend still fails closed. A recovered binding
  is not activation, a lease, or a qualification producer.
- `check` accepts either document from standard input, read to end of file without
  trimming, and preserves the byte order mark, newline kind, blank lines, surrounding
  whitespace, and final-newline state exactly. Both documents cannot share one stream.
  `--output` writes the accepted bytes, or the exact original after an abstention, to
  a new file that must not already exist, or to standard output, which moves the
  report to standard error. `--in-place` (`-i`) retains a sibling
  `<name>.retonr-backup` that must not already exist, then replaces a regular
  source file after same-directory staging. Standard input, `--output`, and
  symlinks are refused. A source with hard-link aliases is also refused so another
  path cannot be mutated indirectly. The commit rechecks the exact source bytes
  after validation, including the unchanged path, and checks the regular-file,
  single-link, and byte identity again immediately before replacement. Detected
  drift returns retryable `concurrent_modification` instead of reporting an
  unchanged or completed write. Unchanged accepted bytes leave the source untouched.
  A requested new output, backup, staging, or trace path treats every existing
  filesystem entry, including a dangling link, as reserved. A missing output parent
  is rejected before non-dry-run document work; dry-run output remains hypothetical.
  A terminal defaults to text; a pipe defaults to JSON. `-f` selects either.
  `--data-dir` is also `-D` or `RETONR_DATA_DIR`.
  Exact bytes reach a terminal only after the
  `--raw-terminal --yes` double opt-in and a warning. Without that double opt-in,
  a terminal receives escaped interactive rendering that neutralizes ANSI, OSC,
  C0, C1, carriage-return, hyperlink, clipboard, bidi, and invisible-control
  effects. Either flag alone stays escaped. `--diff` writes an escaped
  linear comparison of source and accepted output to standard error. `--dry-run`
  computes the report without creating `--output` or replacing the source.
  Untrusted inline report fields, including paths, backup names, artifact metadata,
  and optional device evidence, use single-line visible escapes. Structured JSON
  encodes the same terminal-affecting Unicode as JSON escapes without changing its
  decoded values.
  Cancellation is checked again at the final cooperative boundary before document
  output. `--trace` writes the redacted rewrite record to a new file. Before model or
  document work, a trace request rejects an existing path, a missing parent, and a
  path reserved by the same non-dry-run document transaction. Exclusive creation is
  still enforced when the trace is written because preflight does not eliminate a
  later filesystem race.
- The editorial corpus contract and 120 synthetic fixtures across five groups are
  implemented, but no lint scanner, rule catalog, or live anti-slop ranking path is
  implemented yet. A separate writing-sample library holds licensed pre-2018
  human excerpts and synthetic model-style impressions. A research-only watermark file
  refuses style-as-mark folklore and does not contain generated marks.
- `check` and `rewrite` accept only UTF-8 plain-text documents up to 16 MiB.
- Durable artifact lifecycle state, bounded single-file staging recovery, pinned single-file
  offline import, read-only managed-byte inventory, selected single-artifact orphan
  reconciliation, crash-recoverable inactive removal, and runtime artifact leases
  are implemented. Inventory verifies registered files,
  reports manifest-only state, and identifies orphan candidates and conflicts without
  mutation. Reconciliation requires one exact manifest, ignores earlier inventory
  evidence as authority, reacquires the exclusive lifecycle lock, and reverifies the
  current canonical file before atomically inserting any missing exact manifest and
  installation records or confirming that both existing records match. Removal
  selects one exact installation generation, rejects active or aliased
  bytes, journals preparation before deletion, resumes after interruption, and uses
  generation ordering so an old retry cannot delete a reinstall. The runtime lease
  boundary verifies current durable state and bytes, then retains the shared
  lifecycle lock and file handle until use ends. No user-facing runtime operation
  uses that single-file lease yet. Exact manifest-driven artifact-set folder import is implemented at the
  application boundary and exposed as offline `model import-set`. The command
  verifies the complete local source tree, publishes the whole content-derived set
  root, and records only inert structural installation state. It does not qualify,
  activate, lease, or execute the set. Read-only `inventory-set` inspects managed
  set roots under the shared lifecycle lock, reports registered tree status,
  manifest-only set state, verified orphan set roots, tree conflicts, oversized
  planned trees, and aggregate unexpected set-root counts, and never creates,
  repairs, or removes anything. The report does not grant a lease, qualify a
  package, or authorize a role. Selected `reconcile-set` accepts one exact set
  manifest, ignores earlier inventory evidence as authority, reacquires the
  exclusive lifecycle lock, and reverifies the current canonical set tree before
  atomically inserting any missing exact set-manifest and installation records or
  confirming that both existing records match. It does not copy, replace, repair,
  delete, qualify, or activate the set. Selected `remove-set` accepts one exact
  set installation generation, ignores earlier inventory evidence as authority,
  reacquires the exclusive lifecycle lock, and reverifies the current canonical
  set tree before journaling preparation, deleting the verified tree, and
  completing the journal. Exact prepared set removals resume through
  `recover-set-removal` without callbacks or cancellation. A later exact reimport
  uses the next generation so an old retry cannot delete the reinstall. Set
  removal does not qualify, activate, lease, or grant role authority.
  Single-file `inventory`, `reconcile`, and `remove`
  do not inspect or mutate managed sets. Set `inventory-set`, `reconcile-set`,
  `remove-set`, and `recover-set-removal`
  do not inspect or mutate single-file artifacts. A repository-owned
  artifact-set lease reverifies the complete registered tree under the shared
  repository and storage lifecycle locks and retains that boundary for its lifetime,
  so exclusive operations fail while it is live. Package attestation and the inert
  installed-Ollama import use managed-set boundaries, but no user-facing runtime
  operation does. Downloads,
  runtime-native pulls, bulk reconciliation, orphan deletion, runtime commands, and
  exact real-artifact qualification are not implemented. Effective-package evidence
  and qualification v2 have durable inert persistence but no production evidence
  producer, activation path, or CLI surface. An application-owned managed-process
  attestor can hash one live regular entrypoint, bind `RuntimeBuildIdentity` and
  `EffectiveRuntimeState`, and optionally persist those inert records. It does not
  activate a role, admit observed-only Ollama identity, or enable claim extraction.
  A newer static package service retains exact runtime code-member objects and model
  bytes but also grants no runtime authority. An offline installed-Ollama import
  reconstructs only the admitted manifest-v2 and GGUF-v3 package shape, publishes a
  canonical six-member managed set, and persists and reads back its inert model
  package. A separate offline reviewed-runtime import reconstructs one Linux x86_64
  GNU libc Ollama runtime package from a caller-supplied layout and member tree,
  requires the observed regular files to equal the declared members, publishes that
  set, and persists and reads back its inert runtime-package manifest. The layout
  requires one generation-worker executable and one isolation helper. The helper is
  stored as helper-executable evidence that must not be code-loaded. A transformed
  package must bind its exact source artifact set, tool, parameters, bounded log, and
  one transformation-record member. A direct bridge can also import only the exact
  byte-identical primary package from a retained controlled-build evidence closure. It
  rehashes only declared members, revalidates the retained closure immediately before
  no-replace publication and after durable package readback, and excludes build-only
  evidence from the managed runtime set. None of these imports has a CLI surface. None
  qualifies, activates,
  leases, loads, executes, or admits a runtime to the empty production cloud-disable
  allowlist. A separate evaluation library can bind one such model import to one exact
  verified idle Ollama v0.32.15 inventory and model-details observation. It requires
  the production backend identity `ollama_native` and reviewed source revision
  `b7871fc0d1d82fe109536efa3e0e8e411c766c75`. The version-scoped
  relationship checks the raw manifest digest, the exact config-plus-layers inventory
  size, GGUF, license, format, and a unique template match. The binding must consume
  the opaque, nonserializable receipt issued by the exact preflight runner for the
  plan and report. It proves only that static import-to-inventory relationship; model
  loaded, model used, application handler, effective identity, and qualification
  remain false. It also has no CLI surface. The
  CLI requires one explicit `--data-dir`; its fourteen repository model commands do not use the
  network. Only confirmed `model migrate` can apply a supported schema migration,
  and it first retains a verified repository-owned backup. The other thirteen commands
  remain exact-schema and non-migrating. `pending-operations` reads only bounded durable state and
  returns exact prepared single-file and artifact-set removal generations without
  opening or hashing model bytes. The current product ceilings are 256 GiB per artifact or set member, 4,096
  durable or storage entries, 4,096 set members, 8,192 set-tree entries, 512 GiB of
  aggregate inventory or set-import verification, and
  1 MiB per manifest. Removal is not secure
  erasure and does not affect external copies, caches, backups, or provider records.
  Only local, application-owned storage on the tested platform and filesystem
  configurations is
  within the current boundary; network filesystem semantics and other Windows
  filesystem drivers are not qualified.
- The Ollama adapter is fake-server tested and its read-only preflight has observed
  and verified existing local inventory without generation. The
  separate runtime-only probe consumes one retained caller-supplied stream, accepts
  at most 1 KiB from one `GET /api/version`, requires exact typed version equality,
  and returns non-authoritative version-only evidence. It has no model, inventory,
  generation, connector, retry, reconnect, package, or admission surface. The
  separate attached preflight brackets that HTTP work with point-in-time listener,
  process-incarnation, and executable evidence on Windows and Linux. macOS returns
  unsupported because no admitted public unprivileged listener-owner API exists.
  Linux now selects listener and connection rows through bounded SOCK_DIAG. Its
  holder scan retains the proc root, anchors each process with a pidfd, parses exactly
  one four-field `Uid:` row from each bounded status record for the effective UID, and inspects
  descriptor links relative to the retained process directory. A second anchored
  status read must confirm that the effective UID did not drift. It scans the complete
  admitted descriptor view even after a match. Once a pidfd exists, missing process
  state is accepted as exit only when that pidfd confirms exit. Access denial,
  resource exhaustion, malformed state, and incomplete visibility fail closed.
  That attached report uses independent requests, remains `response_bound: false`
  and `qualified: false`, and creates no runtime identity. The separate
  `--ollama-bound-preflight` command uses one retained direct HTTP/1 connection and
  checks exact reverse established-row attribution before traffic and after every
  fully drained response. Windows evidence is a context-binding PID, not exclusive
  socket ownership. Linux requires exactly one visible same-user descriptor holder,
  but cannot exclude holders hidden by UID, ptrace, proc-mount, PID-namespace, or
  security boundaries. macOS refuses before HTTP because no admitted public
  unprivileged tuple-to-process API is available. The bound report therefore states
  that exclusive socket ownership and application-handler execution are not proven.
  It remains `qualified: false` and creates no runtime, package, qualification,
  activation, or role identity. Executable bytes are not loaded-component closure.
  A separate development-only library preflight now joins a retained runtime-package
  lease, Linux managed launch and isolation, namespace-local process and connection
  evidence, cloud-disable declaration and startup marker, read-only API observation,
  and exact native-load evidence. It reobserves the retained boundaries and closes the
  process tree. Its report remains inert and explicitly leaves application-handler
  proof, exclusive socket ownership, model load or use, effective-runtime identity,
  and qualification false. It has no CLI surface and does not consume the inert
  model-package import. The production cloud-disable allowlist is empty, so its exact
  runtime disposition remains unreviewed. No runtime and model combination is
  qualified.
  The managed target inherits a seccomp socket allowlist installed before launch.
  `socket()` permits only `AF_INET` and `AF_INET6`; every other socket family and
  `io_uring_setup` are denied, and reobservation requires seccomp mode 2.
  An opt-in API returns the unchanged report plus a separate redacted, inert managed
  build binding. That binding constructs only a package-declared typed
  `RuntimeBuildIdentity` after the managed package, process, and native-load join.
  The exact entrypoint is joined to live process and load evidence, but target,
  revision, and other package semantics are not independently live-observed. Cleanup
  is complete and `process_retained_after_return` is false. It explicitly lacks a
  generation-bound provider snapshot, effective output configuration, platform,
  framework, and driver evidence, compute backend and device placement, effective
  context capacity, and a retained live runtime. Effective runtime state, model load
  or use, application-handler execution, and qualification remain false.
  A separate dual-gated one-shot API consumes the static v0.32.15 model binding. It
  requires an opaque admitted-runtime capability produced only from six exact passed
  controls and an independently verified canonical all-pass schema-2 review. It also
  requires a separate opaque generation-path capability that binds the exact package,
  version, source, worker, native record, and frozen external-component set through
  the production generation policy. The capability joins reject cross-operation
  control mixing, alternate frozen-set substitution, caller-authored review status,
  and unreviewed cloud-disable status. Both production allowlists are empty. After
  both independent reviews pass, it keeps the same managed process,
  runtime package lease, native observer, and direct HTTP/1 connection through one
  structured completion and the nine-response residency profile. The model's
  immutable artifact digest and mutable Ollama inventory digest
  are validated as separate identities. Successful redacted evidence records direct
  effective-context observation, equal runtime-reported residency, every connection
  attribution checkpoint, post-generation native load, and final package and
  isolation revalidation. The exact legacy managed-generation evidence schema 1
  remains unchanged. A distinct bracket-observation schema 1 names both retained
  package leases, the private input mapping and input-bound launch digest, the
  separately retained worker, its native closure, and its stable exact private GGUF
  mapping. Both package leases
  are revalidated immediately after generation and again after final observation; the
  model lease is checked again after cleanup. The bounded stable mapping supports
  `model_loaded_proven`, while model use, mapped-page identity and immutability, and
  handler execution remain false. Cleanup still completes before return. The bracket
  constructs and exactly cross-binds generation-bound provider, wire configuration,
  portable platform, actual server and worker closure, normalized worker
  configuration, compute backend, CPU placement, and effective-context observations
  into an inert portable effective runtime state plus a distinct live-attempt join.
  The API does not join the judge receipt, formally prove placement or driver absence,
  prove model weight use or handler execution, establish semantics, or qualify
  anything. It has no CLI surface.
- The grounded path can safely accept only literal-mode token-preserving changes
  under the current evaluator. Open-domain paraphrases and broader modes abstain.
- The typed claim contract and deterministic comparator are implemented. The engine
  can record independently produced comparison evidence on a separate informational
  shadow gate. The application can prepare that evidence from pair extraction and
  attach it to candidate-check or grounded transactions. That gate cannot authorize
  a rewrite or reject a candidate that already passed the hard gates. Literal-token
  failure still abstains. `rewrite-eval --claim-shadow-calibration` runs a
  checked-in fixture corpus through that same candidate-check path and fails if
  shadow evidence changes acceptance. No learned extractor or runtime-backed
  semantic evaluator is connected. The current
  synchronous semantic port is not the future runtime extraction boundary. The raw
  structured-completion port has no semantic authority. The Ollama adapter admits
  only the exact candidate contract currently advertised by discovery. This
  backend-wide admission does not qualify every inventoried artifact for that role.
- CPU-bound product paths are serial today: engine unit and candidate assessment,
  `rewrite-eval` suite cases, directory inspect walks, and artifact-set member
  hashing. The eval and Ollama preflight CLIs use a current-thread Tokio runtime
  because the retained HTTP/1 session has `max_concurrency: 1` and no connector
  pool. Compilation and ordinary tests already use host cores; Linux isolation and
  managed-attestor tests pin to one thread because they share process or namespace
  state. A bounded worker pool for independent hashing and deterministic-suite
  cases is planned after the concurrency envelope is specified. It must not
  parallelize retained inference, managed isolation, or exclusive lifecycle work,
  and it must not oversubscribe cores while a local runtime is generating.
- UTF-16, Markdown, DOCX, profiles, profile persistence, document briefs, complete
  multi-document mutation and recovery transactions, API, MCP, Agent Skills, Agent
  Plugins, and native desktop are not implemented yet. Schema-7 artifact, evidence,
  and atomic qualification-preregistration persistence, one-file checked replacement,
  and dry-run directory transaction planning are implemented.
- Positive human-adjudication authority is not implemented and must not use the
  structural V1 policy as a substitute. V1 omits the reviewer-directory trust
  snapshot, signature and key-epoch contract, rubric, outcome subject, tie and pass
  rules, reviewer visibility and assignment rules, adjudication replacement rule,
  and evidence-handling policy. Production human-policy roots remain empty until a
  reviewed V2 contract and explicit governance and retention decisions exist.
- Positive resource-phase derivation is implemented but unavailable in production.
  The inert target-attempt
  result, pidfd-bound Linux worker high-water observation, exact runtime and model
  payload sizes, inseparable provider completion, and Approved-only app measurement
  authority now close the live app boundary. The strict eval runner and receipt join
  consume that authority and compile the exact portable attempt result. Strict
  batch-set and repeatability propagation, the complete Passed closure, and the
  resource-manifest compiler are implemented. Production resource-policy roots remain
  empty, and the final qualification compiler must reload the full attempt closure.
  The strict candidate and judge routes now preserve a supplied original absolute
  deadline through every live boundary and mandatory finalization. The consuming
  Active operation is the sole public high-level candidate and judge entry. It
  supplies Prepared's retained clock and one lifecycle after exact plan-order request
  or repetition-scope validation and prevents judge traffic before candidate
  completion. The process-local subject, two-phase successful settlement, target-only
  attempt-record and completed-receipt collection, one-shot ledger sealing, exact
  receipt-to-record closure, and ledger-before-judge gate are implemented. Active
  also owns structured-response compilation, no-replace
  publication, fresh readback, receipt compilation, and exact settlement. A typed
  postprocessing failure consumes the ordinal, derives the exact failed-attempt
  record, and, when mandatory finalization passes, retains a fresh-revalidatable
  attempt-ledger interruption, skipped later manifests, and noncompleted operation
  receipt. Schema 10 makes the precursor checkpoint and exact completed-or-failed
  candidate closure durable. Completed closure binds app-root publication, canonical
  storage, bounded readback, receipt, and attempt metadata atomically. Bounded
  read-only reconciliation now runs before Prepared activation and allows only an
  entirely pristine plan. It does not retry, repair, promote, delete, or fabricate
  evidence. Schema 11 adds the terminal-evidence tables and an atomic cohort writer.
  That writer grants no live authority, so positive production execution remains
  disabled. Schema 12 adds seven inert judge-execution tables and no judge-execution
  writer. Those tables grant no qualification, activation, or live-use authority, and
  repeatability results remain limited to candidate-generation failure.
  Stored digests or caller-selected measurements cannot substitute for those typed
  observations.
- The model-free evaluator does not assess open-domain paraphrases and must abstain
  on them.
- No public API, schema, package, executable name, or configuration namespace is
  frozen.
- Rewrite records use unkeyed SHA-256 identity digests. These are not anonymization
  and can permit dictionary attacks on short predictable text. Stable local traces
  require an installation-keyed digest decision.
- The README includes one reproducible Linux-first rendering of verbatim output from
  the current release-optimized candidate-check binary. Model-backed rewrite,
  abstention, diff, trace and native desktop screenshots remain gated on their
  complete release-build behaviors under the screenshot policy.
- Evaluation data and user-research policies are proposed, not approved. No
  non-synthetic collection is authorized.
- The versioned hybrid scorecard executes two exact deterministic suites, requires
  their complete corpus and fixed policy digests to match the selected plan, and
  reports exact expectation results and transformation coverage. It normalizes
  blinded, order-swapped structured judge observations only after both suites pass.
  Its serializable report still labels judge observations caller-declared and
  triage-only. A separate typed executor now runs both orders over one already
  preflighted retained Ollama stream and returns a nonserializable receipt binding the
  plan, rubric, observation batch, retained preflight, exact request and response
  digests, and response ordinals. That receipt does not prove managed isolation,
  handler execution, model load or use, candidate generation, effective identity,
  semantics, or qualification. Neither the scorecard nor the receipt can override
  hard gates or replace human release adjudication. The executor has no CLI surface.
  Every retained-session completion rejects UTF-8 input above the absolute 4 MiB
  ceiling before wire serialization or completion traffic.
  A separate opt-in retained-session profile for reviewed Ollama v0.32.15 sends one
  structured completion followed by two exact, equal singleton `/api/ps` observations
  around final version, inventory, and details checks. Its nonserializable receipt
  proves only stable runtime-reported post-generation residency on that transport.
  Runtime memory size is not package inventory size. Handler execution, model use,
  resident-page identity, effective identity, and qualification remain false. This
  profile is joined only by the one-shot managed generation API. It is not joined to
  the local-judge executor.

## Next logical operations

The detailed handoff is in the
[0.2 grounded engine and CLI plan](planning/0.2-grounded-cli.md). The immediate order
is:

An in-memory terminal-evidence planner now derives one operation receipt and binds
a phase interruption only when interruption facts are supplied for a noncompleted
receipt. It does not persist, construct a qualification record,
or acquire live authority. Read-only attempt-ledger rederive rebuilds the
target manifest from schema-10 rows and does not write. Schema 11 adds
the operation-level terminal-evidence tables. The cohort writer stores those rows in one transaction and grants no live
authority. Schema 12 adds seven inert judge-execution tables for plans, schedules,
request aggregates, response aggregates, observation batches, managed local judge
receipts, and candidate judge joins. No judge-execution writer exists yet, and the
tables grant no qualification, activation, or live-use authority. Repeatability
results remain limited to candidate-generation failure.
Bounded read-only candidate-attempt reconciliation now runs before `Prepared`
activation. It inspects at most the plan's 1,024 attempts in plan order, classifies
each as not started, checkpoint-only, terminal failed, or terminal completed, and
compares durable metadata with the canonical application evidence root. Activation
proceeds only when every attempt is pristine. Checkpoints, published bundles without
terminal metadata, terminal metadata without matching storage, ambiguous terminal
commits, unexpected staging, corruption, or an exceeded bound fail closed without
retry, repair, promotion, deletion, or fabricated evidence. The existing pretraffic
checkpoint remains the concurrency barrier after this read-only admission check.

1. Preserve the schema-10 store, including the durable candidate execution closure,
   the schema-9 portable plan and case foundation,
   all unchanged schema-7 preregistration, and schema-6 lifecycle,
   migration, retained package-object, SOCK_DIAG, isolation, attestation, redaction,
   cancellation, and informational shadow boundaries.
2. Freeze and review one complete source-built Ollama runtime package. The historical
   official v0.32.15 archive review remains blocked because it cannot establish
   source-to-binary lineage or exact external platform component identity. The
   controlled-build contracts, retained Linux executor, static offline source builder,
   two-attempt comparison, sealed helper bootstrap, digest-verified private input
   snapshot, private output publication, and deterministic native fixture now exist.
   A historical frozen closure was built twice and independently reacquired on
   2026-08-26; both complete output trees and their portable ordinary Unix permission
   bits matched. That target-owned evidence was later removed and no longer supplies
   authority. Set-user-ID, set-group-ID, and sticky bits fail the controlled-build
   output boundary. The retained-file private model-root capability is now complete,
   including the helper protocol and live Linux validation. Produce and independently
   reacquire a fresh closure containing that final helper, then
   separately
   adjudicate source lineage, transformation, license, frozen native closure, managed startup,
   and cloud disable from typed retained evidence. Compile an all-pass schema-2 review
   before adding any exact runtime to the cloud-disable production allowlist. This
   does not authorize its generation worker.
3. Retain the completed dual-gated one-shot managed generation bracket. Admit an exact
   runtime and separately review its exact package-and-worker generation path. The
   bracket now observes and joins its generation-bound provider snapshot, complete
   closed configuration, platform and explicit driver class, actual server and worker
   component sets, normalized worker configuration, compute backend, CPU placement,
   and effective context. Managed Linux launch establishes and reobserves a private
   mount namespace, private procfs, reviewed device view, private read-only model
   root, retained worker, native closure, and exact GGUF mapping without exposing host
   paths. The portable effective-state identity excludes installation generations,
   frozen discovery provenance, PIDs, object identities, requests, responses, and
   residency. The separate live join binds those exact attempt facts. Cleanup and
   final revalidation of both runtime and model package leases must succeed before
   completed authority releases the state. After precursor consumption, a failure at
   a trustworthy typed closeout boundary returns an exact portable failed-attempt
   record while retaining its typed primary cause and every cleanup or revalidation
   cause behind redacted debug output. A runner failure before typed closeout or an
   ambiguous terminal commit can instead leave only durable partial state. Execution
   error is reserved for an inability to produce either trustworthy completed
   authority or an exact failed record. CPU-only observations remain bounded evidence,
   not formal placement proof. The completed managed attempt carries exact
   V2 managed evidence, successful cleanup, the retained response, and internally
   parsed candidates. App-owned publication derives candidate bytes from that
   response, publishes the exact evidence tree without replacement, independently
   reacquires it, and reloads every fixed record before deriving an inert completed
   receipt. The eval-owned join consumes that receipt and the nonforgeable completed
   attempt, reopens the bundle, and issues one `VerifiedCandidateBatch`. The
   semantic-order `VerifiedCandidateBatchSet` consumes one freshly revalidated batch
   per suite case and rederives its portable receipt set internally. Neither static
   inventory, a file mapping, nor API residency proves model weight use.
4. Keep attached Windows and Linux evidence as observation-only. Do not use it as a
   fallback for a failed managed launch. Windows exact native-load binding and
   managed isolation remain unsupported; macOS remains unsupported.
5. Preserve the distinct candidate-generation receipt over that same retained runtime
   and model boundary, the frozen selection policy, complete receipt-set closure,
   exact deterministic case contract, retained `VerifiedGenerationCaseMaterial`,
   live-authority batch set, portable deterministic evaluation record, and qualified
   deterministic compiler and offline candidate-to-judge preparation join with its
   exact two-order schedule, request aggregate, compatibility hard-gate projection,
   app-owned managed-judge precursor, exact no-launch pairing of both real runner
   handoffs, crate-private no-launch runner configuration with complete retained
   authority and exact preflight, limits, runtime, isolation,
   launch-plan-to-model-package lease identity, model, and installation-generation
   closure, phase-ordered observer with exact per-attempt request, response,
   residency-receipt, and effective-state sealing, pure
   judge-output normalizer, schedule-wide observation authority with five exact
   evidence aggregates and a dedicated effective-state join, and the schedule-wide
   cleanup-gated app package that rejects final-attempt substitution and retains the
   complete observer authority. Preserve the portable
   schedule-indexed response identity and its exact rederived request association;
   repeated nested provider response IDs must remain distinct presentation records.
   Preserve the implemented single-entry managed schedule executor, eval-owned
   cleanup-gated durable receipt with exact ordinals `8..(7 + 9 * N)`, canonical
   compatibility triage, and opaque join with mandatory full-record revalidation.
   Keep all judge evidence probabilistic and triage-only because these bindings do
   not prove semantics or qualification.
6. Preserve the implemented phase and operation records, compact
   `GenerationQualificationRecordV1`, ordered live-join authority, app assessment
   authorities, deterministic one-at-a-time request builder, streaming projection
   authority, staged Draft -> Projected -> Prepared compiler, the schema-7 atomic
   preregistration cohort, the schema-8 effective-package V2 plus generation-system
   foundation transaction, the schema-9 portable plan and case foundation, and the
   schema-10 candidate checkpoint and terminal closure.
   Preserve rejected pretraffic terminalization, the strict resource
   and human phase-policy verifiers, their typed inert source-denial records, and the
   app-owned denied-phase authorities. Preserve the completed fail-fast pretraffic
   phase-policy refusal so empty production roots cannot cause model traffic.
7. Preserve the eval-owned positive resource runner and receipt join over the
   Approved-only app measurement authority, inert attempt result, exact package
   payload sizes, pidfd-bound worker high-water observation, strict batch-set mode,
   repeatability resource collection, complete Passed closure, and resource-manifest
   compiler and consuming Active operation owner. Preserve Active as the sole strict
   candidate and judge entry with exact plan-order request and repetition scope,
   candidate-before-judge ordering, its Prepared deadline, single lifecycle,
   no-retry terminalization, and mandatory finalization. Preserve the process-local
   Active subject, two-phase completed candidate settlement, target-only record
   collection, exact target completed-receipt retention, one-shot attempt-ledger
   closure, receipt-to-record validation, ledger-before-judge gate, Active-owned
   postprocessing failure closure, and exact in-memory interruption evidence. Preserve
   foundation-gated preregistration with its retained recursively checked readback,
   newly inserted precursor checkpoint, Active-owned pending completion, app-root-bound
   publication and reacquisition, and atomic completed-or-failed terminal persistence.
   Bounded activation reconciliation now runs before Prepared activation and allows
   only an entirely pristine plan. It does not retry, repair, promote, delete, or
   fabricate evidence. An in-memory planner now derives that receipt and binds a phase interruption only
   when interruption facts are supplied for a noncompleted receipt. It does not
   persist, construct a qualification record, or acquire live
   authority. Read-only attempt-ledger rederive rebuilds the target manifest from schema-10 rows and does not write. Schema 11 adds
   the terminal-evidence tables. The cohort writer stores those rows in one transaction and grants no live authority.
   Schema 12 adds seven inert judge-execution tables for plans, schedules, request
   aggregates, response aggregates, observation batches, managed local judge receipts,
   and candidate judge joins. No judge-execution writer exists yet, and the tables
   grant no qualification, activation, or live-use authority. Repeatability results
   remain limited to candidate-generation failure.
   A positive human authority requires a reviewed V2
   policy and explicit reviewer-governance and evidence-retention decisions. Add each
   later dependency-complete schema cohort in order, then compile
   `VerifiedGenerationQualification` from fresh durable readbacks and every required
   live authority. Only after that compiler exists,
   project the 49 deterministic and 120 editorial development cases into
   preregistered smoke, calibration, and locked manifests. Deterministic gates remain
   the only machine acceptance authority, and human adjudication remains the release
   authority.
8. Finish remaining CLI recovery and packaging evidence, then capture model-backed
   screenshots only from the complete passing release path.

Later work follows the dependency order in the
[phase execution plan index](planning/README.md).
