# External change watch and revalidation

## Purpose

Retonr depends on facts that change outside the repository: provider marking and
retention behavior, watermark research, transparency law, provenance standards,
model and runtime releases, agent protocols, Rust, operating systems, and security
advisories. These inputs require a permanent evidence and revalidation process.

The watch is not an automatic policy feed. An external announcement can open a
review, invalidate evidence, narrow support, or block a release. It cannot silently
change candidate selection, profiles, network behavior, format policy, or user
output.

## Watch domains

| Domain | Primary inputs | Impacted records |
| --- | --- | --- |
| Provider text marking | Official product documentation, system cards, release notes, detector documentation, cloud-partner notices | Provider claim ledger, runtime capability matrix, public claims |
| Provider records and privacy | Official retention, abuse-monitoring, zero-data-retention, training, and deletion documentation | Privacy notices, remote-adapter policy, qualification |
| Watermark science | Peer-reviewed proceedings, primary preprints, official implementations, independent reproductions, attacks | Research taxonomy, threat model, evaluation fixtures, limitations |
| Provenance standards | C2PA, XMP, IPTC, Unicode, PDF, OOXML, trust lists, validator releases | Carrier matrix, adapters, signature policy, regression fixtures |
| Law and codes | Primary legislation, official guidelines, harmonized standards, signed codes, enforcement decisions | Project role review, distribution gates, user-facing limitations |
| Local runtimes and models | Official releases, source, defaults, build flags, model cards, licenses, tokenizer and template changes | Artifact sets, output-policy assurance, qualification records |
| Agent and knowledge protocols | MCP specification and SDKs, Agent Plugins, Open Knowledge Format, conformance suites, client behavior | CLI machine contract, MCP server, plugin package, knowledge projections, compatibility matrix |
| Rust and dependencies | Stable Rust, Cargo, Clippy, rustdoc, crates, advisories, licenses, maintainer changes | Toolchain pin, lockfile, supply-chain review, CI |
| Operating systems and packaging | Signing, notarization, accessibility, permissions, installers, native UI behavior | Desktop decision, packages, release qualification |

Secondary reporting can identify a candidate change. A support or legal claim is
not updated until a primary source or reproducible artifact is reviewed. When no
primary evidence is available, the ledger records `unknown`.

## Required review points

The external watch runs:

- At entry to every milestone that depends on an external technology or claim
- Before freezing any release candidate
- Before qualifying a new model, runtime, provider surface, language, format, or
  distribution territory
- When an automated advisory, release, or primary-source monitor reports a relevant
  change
- When a provider claim contradicts an earlier claim or observed behavior
- When a supported artifact, runtime, tokenizer, template, processor, protocol,
  validator, trust list, standard, or law changes
- When a user or maintainer provides reproducible contrary evidence
- After a relevant vulnerability, incident, enforcement action, or research attack

These are logical and event-driven gates, not implementation-duration estimates.

## Evidence states

Every item receives one state from the research vocabulary:

- `standard`
- `peer_reviewed`
- `official_implementation`
- `provider_statement`
- `preprint`
- `local_observation`
- `inference`
- `unknown`

Operational triage also assigns:

- `new`: not yet compared with current claims
- `corroborates`: strengthens an existing bounded claim
- `contradicts`: conflicts with a current source or retained observation
- `invalidates`: changes an artifact or assumption that current qualification needs
- `narrows`: removes a provider surface, language, length, platform, or threat model
  from a defensible claim
- `expands_candidate`: may support broader behavior after full qualification
- `no_product_impact`: relevant history with no current contract change

A signed commitment is not implementation evidence. A provider statement with an
undisclosed mechanism is not independently reproducible. A preprint does not become
a release guarantee. No public documentation means unknown, not absent.

## Change record

Every material watch item records:

- Stable ID, discovery date, effective date, and research cutoff
- Domain, provider or project, product surface, region, and exact version
- Primary URL, immutable archive or digest where permitted, and access date
- Evidence state and reproducibility grade
- Narrow claim supported by the source
- Exact quoted fragment only when needed and within source-use limits
- Earlier claims corroborated, contradicted, superseded, or left unresolved
- Product invariants, decisions, schemas, adapters, fixtures, and releases affected
- Security, privacy, fidelity, format, accessibility, and legal consequences
- Required revalidation and its owner
- Resolution, evidence bundle, and release where the decision became effective

Mutable webpages are not treated as immutable evidence. Preserve a permitted local
snapshot, content digest, version, or dated archival reference when the claim is
material and the source offers no stable revision.

## Triage and response

1. Capture the primary evidence without changing shipping behavior.
2. Classify its evidence and operational states.
3. Compare it with the dated claim ledger and identify contradictions.
4. Map the exact support records and invariants it can affect.
5. Reproduce an implementation claim where a public artifact permits it.
6. Open a focused decision or defect when product behavior or claims may change.
7. Narrow or suspend the affected claim immediately when retained evidence is no
   longer sufficient.
8. Update research, active contracts, fixtures, qualification, and public
   documentation together.
9. Run the complete affected quality and release gates before restoring or
   expanding support.
10. Preserve the old decision and reason. Do not rewrite historical records to make
    the project appear continuously correct.

An urgent security or legal change may disable or narrow an optional feature, but it
still cannot rewrite completed local documents, mutate stored profiles, or enable a
network policy silently.

## Provider marking watch

Provider state is tracked per exact product surface and deployment path. Review at
least:

- Exact model IDs and release dates
- Consumer application, API, cloud reseller, local weights, and file export paths
- Text mark, metadata, Content Credential, visible label, detector, fingerprint,
  and provider record as independent mechanisms
- Minimum length, language, region, tokenizer, threshold, uncertainty, and supported
  edit limitations where disclosed
- Rollout, rollback, legacy-model, and unsupported-surface statements
- Detector access, terms, privacy, query retention, and versioning
- Model-level, inference-time, middleware, and postprocessing insertion points
- Contradictions between current help pages, system cards, commitments, and observed
  controlled behavior

Hosted provider statements never qualify a local runtime, and a local open-weight
review never establishes hosted provider behavior.

## Research watch

New watermark research is routed by contribution and evidence maturity:

- New embedding or detector family
- Calibration or false-positive correction
- Short, low-entropy, multilingual, code, or mixed-content result
- Paraphrase, translation, dilution, collision, stealing, spoofing, or oracle attack
- Public verification, multi-bit payload, privacy, or key-governance result
- Quality, factuality, latency, or capacity trade-off
- Formal limitation or impossibility result
- Independent reproduction, contradiction, retraction, or implementation drift

A paper changes Retonr's product only if it affects a named threat, claim, test, or
architecture assumption. A new attack normally adds or revises an isolated research
fixture and limitation. It does not add a detector to the live rewrite loop.

## Automation boundary

Automation may:

- Poll official release and advisory feeds
- Detect changed pages, tags, digests, schemas, dependencies, and model manifests
- Open a review item containing links and mechanical diffs
- Run pinned reproducibility, conformance, and regression jobs
- Mark an existing qualification stale when an exact identity changes

Automation may not:

- Accept a legal interpretation
- Convert a provider commitment into verified implementation
- Promote a preprint claim into product support
- Update public wording without review
- Download or activate a model, runtime, detector, trust list, or policy silently
- Query a provider detector with user content
- Change a live threshold, prompt, style rule, or output policy
- Close a contradiction merely because one source is newer

## Repository workflow

Use one focused issue or change record per externally caused decision. Labels should
distinguish provider marking, provenance standard, watermark research, runtime,
protocol, dependency, legal review, security advisory, and platform change.

Every resolved item names:

- The exact evidence reviewed
- Whether support stayed unchanged, narrowed, expanded, or was removed
- Which qualification records became stale
- Which fixtures and documentation changed
- Which checks and platforms passed
- The first release containing the decision

Do not create long-lived provider-specific branches. Changes land through focused,
short-lived branches and return to one passing main line. Each milestone release
contains a dated external-dependency and provider-claim snapshot.

## Release gate

A release cannot inherit an earlier external review merely because code did not
change. Before release freeze:

- Resolve or explicitly accept every material `contradicts` and `invalidates` item.
- Confirm every provider and runtime statement against its dated evidence record.
- Re-run exact artifact and output-policy qualification where an identity changed.
- Re-run affected carrier and format fixtures when a standard or validator changed.
- Re-run Agent Plugins, MCP, and OKF compatibility independently when any of their
  exact specification or schema revisions change.
- Review current advisories, licenses, toolchain, protocol, packaging, and signing
  requirements.
- State remaining unknowns and unsupported surfaces publicly.
- Record the watch cutoff and links in the release evidence.

If evidence is insufficient, narrow the claim or feature. A release date is not a
reason to preserve a statement the project can no longer support.
