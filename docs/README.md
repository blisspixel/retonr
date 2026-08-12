# Planning documents

These documents define the product direction and constrain implementation.

| Document | Purpose |
| --- | --- |
| [Product](product.md) | Users, jobs, positioning, hypotheses, scope, and success criteria |
| [Current state](current-state.md) | Implemented behavior, verification evidence, limitations, and next operations |
| [Naming](naming.md) | Public identity, namespace evidence, and remaining release gates |
| [Architecture](architecture.md) | Component boundaries, data flow, contracts, and failure semantics |
| [Design](design.md) | CLI, desktop, voice, API, MCP, and screenshot experience |
| [Input and integration surfaces](interfaces.md) | Multiline input, clipboard, local API, MCP, skills, and compatibility boundaries |
| [Language and format preservation](language-and-format.md) | Multilingual qualification and preservation contracts by input surface |
| [Model and runtime support](model-support.md) | Hardware discovery, runtime selection, model evaluation, and fallback rules |
| [Installation and distribution](distribution.md) | Bootstrap installers, artifact verification, release targets, updates, and removal |
| [Technology](technology.md) | Current recommended stack and deferred decisions |
| [Evaluation](evaluation.md) | Baselines, corpora, metrics, statistical reporting, and release gates |
| [Security](security.md) | Assets, trust boundaries, threats, privacy, and provenance handling |
| [Quality](quality.md) | Rust, testing, coverage, accessibility, CI, and refinement standards |
| [Roadmap](roadmap.md) | Dependency-ordered version plan through 1.0 |
| [Phase execution plans](planning/README.md) | Detailed work packages, decisions, tests, and handoff gates for 0.2 through 1.0 |
| [Next-phase research ledger](research/2026-08-11-next-phases.md) | Dated primary-source assumptions behind model, integration, desktop, and voice planning |
| [Decision records](decisions/README.md) | Durable technical and product decisions |
| [Evaluation data policy](governance/data-policy.md) | Proposed authorization, retention, revocation, and deletion rules |
| [User research protocol](governance/user-research.md) | Proposed research, consent, annotation, and adjudication workflow |
| [0.1 refinement record](reviews/2026-08-12-0.1-refinement.md) | Evidence and open findings from the required refinement passes |

## Document status

All decisions are provisional until their roadmap gate is passed. Decisions that
affect public APIs, stored data, supported formats, security properties, or package
names require an architecture decision record before implementation.

The repository uses `Retonr` as its public project identity. Package and installer
publication remain blocked on the release evidence and legal-review gates recorded
in the naming decision.

The roadmap defines what each version means. The phase execution plans define the
ordered implementation and evidence needed to reach it. The current-state document
is the only source that describes a planned capability as implemented.
