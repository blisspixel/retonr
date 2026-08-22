# Planning documents

These documents define the product direction and constrain implementation.

| Document | Purpose |
| --- | --- |
| [Product](product.md) | Users, jobs, positioning, hypotheses, scope, and success criteria |
| [Product and engineering invariants](invariants.md) | Permanent product, execution, interface, and quality boundaries |
| [Editorial sovereignty](governance/editorial-sovereignty.md) | Viewpoint neutrality, user responsibility, and project legal boundary |
| [Provenance and derivative handling](provenance.md) | Inspection, preservation, sanitation, signatures, markings, and reports |
| [Current state](current-state.md) | Implemented behavior, verification evidence, limitations, and next operations |
| [Naming](naming.md) | Public identity, namespace evidence, and remaining release gates |
| [Architecture](architecture.md) | Component boundaries, data flow, contracts, and failure semantics |
| [Design](design.md) | CLI, native desktop, editorial brief, API, MCP, and screenshot experience |
| [Input and integration surfaces](interfaces.md) | Multiline input, clipboard, local API, MCP, skills, and compatibility boundaries |
| [Language and format preservation](language-and-format.md) | Multilingual qualification and preservation contracts by input surface |
| [Document transactions](document-transactions.md) | Non-destructive files, folders, large-document passes, staging, and change reports |
| [Guided editorial brief](editorial-brief.md) | Document-specific clarification and time-aware preference evolution |
| [Editorial lint](editorial-lint.md) | Explainable anti-slop findings, quality-loop boundaries, and reporting |
| [Evaluation corpora](evaluation-corpora.md) | Separate synthetic editorial-quality fixtures from known-watermark research fixtures |
| [Writing-sample library](evaluation-style-library.md) | Licensed pre-AI human controls, synthetic model-style impressions, and style-is-not-a-watermark refusals |
| [Model and runtime support](model-support.md) | Hardware discovery, runtime selection, model evaluation, and fallback rules |
| [Optional fitr evidence](fitr.md) | Device-measurement handoff from the sister project; not a qualification |
| [Installation and distribution](distribution.md) | Bootstrap installers, artifact verification, release targets, updates, and removal |
| [Snapshot testing guide](testing-snapshot.md) | What a development snapshot build does today and what feedback is useful |
| [Technology](technology.md) | Current recommended stack and deferred decisions |
| [Evaluation](evaluation.md) | Baselines, corpora, metrics, statistical reporting, and release gates |
| [Security](security.md) | Assets, trust boundaries, threats, privacy, and provenance handling |
| [Quality](quality.md) | Rust, testing, coverage, accessibility, CI, and refinement standards |
| [Roadmap](roadmap.md) | Dependency-ordered version plan through 1.0 |
| [Phase execution plans](planning/README.md) | Detailed work packages, decisions, tests, and handoff gates for 0.2 through 1.0 |
| [Superseded next-phase research ledger](research/2026-08-11-next-phases.md) | Historical August 11 assumptions retained for decision provenance |
| [Research integrity and synthesis contract](research/README.md) | Evidence labels, reproducibility rules, and paper-style publication threshold |
| [External change watch](external-change-watch.md) | Recurring provider, research, standards, runtime, protocol, and toolchain revalidation |
| [Anthropic Claude text watermark watch](research/2026-08-21-anthropic-text-watermark.md) | Dated provider and primary-source review of Claude text watermarking and Retonr's non-evasion boundary |
| [Watermark and editorial re-expression report](research/2026-08-12-editorial-reexpression-report.md) | Paper-style synthesis of provenance, quality, assurance, evaluation, limitations, and research agenda |
| [Text watermarking literature map](research/2026-08-12-watermark-literature-map.md) | Systematic primary-source map, evidence labels, benchmark incompatibilities, open gaps, and citation audit |
| [Rust engineering research](research/2026-08-12-rust-engineering.md) | Current Rust, testing, unsafe, compatibility, and release evidence standards |
| [Agent integration research](research/2026-08-12-agent-integrations.md) | Agent Plugins, Agent Skills, MCP, conformance, and packaging gates |
| [Open Knowledge Format research](research/2026-08-12-open-knowledge-format.md) | OKF v0.2 knowledge bundles, trust, portability, attestation, and Retonr boundaries |
| [Provider-neutral runtime research](research/2026-08-12-provider-neutral-runtimes.md) | Local runtime matrix, artifact identity, output policy, and long-input qualification |
| [Current local runtime matrix](research/2026-08-13-local-runtime-matrix.md) | Dated cross-platform runtime status, exact identity requirements, and Agent Plugin and MCP boundary |
| [Main readiness and next 0.2 slice](research/2026-08-20-main-readiness-and-next-slice.md) | Public-main evidence, live read-only Ollama preflight, documentation drift, and ordered trust work |
| [Attached Ollama process witness](research/2026-08-20-attached-process-witness.md) | Platform ownership mechanisms, bounded witness design, limitations, and response-binding next step |
| [Effective runtime trust chain](research/2026-08-21-effective-runtime-trust-chain/context.md) | Frozen baseline evidence, selected SOCK_DIAG, package, load, provider, and managed-isolation designs, and live implementation status |
| [Hybrid rewrite evaluation](research/2026-08-21-hybrid-rewrite-evaluation.md) | 169-case development foundation, hard-gate score ordering, neutral local-judge execution, limited transport receipts, and locked-run plan |
| [Local model tiers](research/2026-08-13-local-model-tiers.md) | Small, medium, and workstation candidates, exact revisions, local inventory, and qualification gates |
| [Local model evaluation protocol](research/2026-08-13-local-model-evaluation.md) | Zero-download bakeoff, hardware strata, metrics, execution limits, and logical implementation order |
| [Editorial pattern graph decision](research/2026-08-13-editorial-pattern-graph.md) | Separate product editorial relationships, personal profiles, and source-signal research |
| [Editorial pattern mathematics](research/2026-08-13-editorial-pattern-mathematics.md) | Ledger-first graph math, fixed-point scoring, statistical boundaries, and deterministic Rust direction |
| [Editorial pattern evaluation](research/2026-08-13-editorial-pattern-evaluation.md) | Preregistered matcher, actionability, flat-versus-graph, fidelity, preference, and drift protocol |
| [Local model smoke comparison](research/2026-08-12-local-model-smoke.md) | Three-family local editing smoke run, clean control, exact identities, and limitations |
| [Local watermark assurance](research/2026-08-12-local-watermark-assurance.md) | Intentional insertion points, exact-stack evidence levels, bounded claims, and requalification |
| [Text watermark science](research/2026-08-12-text-watermark-science.md) | Mechanisms, detectors, attacks, trade-offs, limits, and product implications |
| [Watermark evaluation protocol](research/2026-08-12-watermark-evaluation-protocol.md) | Preregistered calibration, power, attacks, quality, isolation, and reproducibility |
| [Provider marking practices](research/2026-08-12-provider-marking-practices.md) | Dated Anthropic, Google, OpenAI, Microsoft, Meta, Mistral, Cohere, and AWS evidence |
| [Provenance policy research](research/2026-08-12-provenance-policy.md) | C2PA, document carriers, current policy, preservation, and derivative handling |
| [Decision records](decisions/README.md) | Durable technical and product decisions |
| [Evaluation data policy](governance/data-policy.md) | Proposed authorization, retention, revocation, and deletion rules |
| [User research protocol](governance/user-research.md) | Proposed research, consent, annotation, and adjudication workflow |
| [0.1 refinement record](reviews/2026-08-12-0.1-refinement.md) | Evidence and open findings from the required refinement passes |

## Document status

Working decisions remain provisional through 0.8 and may be refined or superseded as
evidence improves. The 0.9 phase freezes the compatibility surface. Decisions that
affect public APIs, stored data, supported formats, security properties, or package
names require an architecture decision record before release qualification, not
before reversible exploration.

The repository uses `Retonr` as its public project identity. Package and installer
publication remain blocked on the release evidence and legal-review gates recorded
in the naming decision.

The roadmap defines what each version means. The phase execution plans define the
ordered implementation and evidence needed to reach it. The current-state document
is the only source that describes a planned capability as implemented.
