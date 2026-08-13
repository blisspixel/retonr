# Open Knowledge Format and Retonr

## Review status

Reviewed: August 12, 2026.

Evidence cutoff: August 12, 2026.

The June 12 Google Cloud announcement introduced Open Knowledge Format v0.1. The
current official repository now specifies OKF v0.2, which supersedes v0.1. Retonr
therefore treats v0.2 as the current research target and the announcement as
historical context.

OKF remains a young 0.x specification. Retonr may qualify exact-version import and
export compatibility, but must not freeze its internal model around OKF or describe
the format as stable.

## What OKF is

OKF v0.2 is a vendor-neutral knowledge exchange format made from a directory of
UTF-8 Markdown files with YAML frontmatter. A non-reserved concept document requires
only a non-empty `type`. File paths identify concepts, ordinary Markdown links form
untyped graph edges, and optional `index.md` and `log.md` files support progressive
disclosure and change history.

The v0.2 optional families add:

- `sources` with stable IDs and per-source credibility signals
- `generated` and `verified` actor events
- Derived unverified, machine-confirmed, and human-reviewed trust tiers
- `status` and `stale_after` lifecycle fields
- Attested Computation concepts with runtimes, typed parameters, executors,
  receipts, and deterministic attesters
- An optional root `okf_version`

The specification intentionally does not define a runtime, service, fixed concept
taxonomy, domain schema, executor packaging format, attester ABI, sandbox, or access
control system. Its attestation runtime protocol and portability details remain
deferred.

## Relevant properties

| OKF property | Retonr opportunity | Required boundary |
| --- | --- | --- |
| Markdown and YAML files | Human-readable, diffable, offline bundles | Treat imported text and metadata as untrusted; preserve unknown fields |
| Path concept identity | Portable research and support ledgers | Contain paths and links within the selected bundle for local reads |
| `sources` and footnote IDs | Per-claim research and policy evidence | A source entry is a claim, not proof that the source supports the prose |
| `generated` and `verified` | Visible production and review history | Optional and privacy-sensitive; never infer consent, authority, or authorship |
| Trust tiers | Useful display hint for reviewed knowledge | Advisory only; never an authorization or fidelity gate |
| `status` and `stale_after` | Revalidation reminders | Staleness cannot silently mutate product behavior |
| Index hierarchy | Progressive disclosure for agents and long corpora | An index is navigation, not the complete source of truth |
| Normal Markdown links | Portable relationship graph | Edges are untyped and broken links are conformant; critical relations need Retonr validation |
| Attested Computation | Potential reproducible evaluation and qualification recipes | OKF does not package, sandbox, authorize, or execute the referenced code |

## Fit with Retonr

OKF complements rather than replaces the existing interfaces:

- Agent Plugins packages Retonr's portable Agent Skills and MCP server discovery.
- MCP carries live typed rewrite, check, and capability operations.
- OKF can exchange inspectable knowledge that an agent or person reads over time.
- SQLite remains the authoritative transactional store for profiles, consent,
  revocation, feedback, operations, and migrations.
- Retonr's versioned JSON schemas remain the authoritative machine contracts for
  commands, tools, transactions, and reports.

Candidate OKF bundle types include:

- `Retonr Research Claim`
- `Retonr Provider Claim`
- `Retonr Support Combination`
- `Retonr Style Rule Explanation`
- `Retonr Document Brief`
- `Retonr Preference Observation`
- `Retonr Evaluation Protocol`
- `Retonr Qualification Recipe`

These names are experimental producer-defined types, not additions to the OKF
specification.

## Recommended product use

### First compatibility spike

Build an isolated, optional OKF v0.2 import and export spike after the canonical
profile, brief, support, and research schemas exist.

The spike should:

1. Pin the exact specification revision and test bundle fixtures by digest.
2. Export a content-minimized research claim ledger and support matrix.
3. Export a redacted profile-policy view that excludes raw writing samples,
   embeddings, credentials, authorization state, private paths, and content hashes.
4. Import only inert knowledge proposals into a staging area.
5. Preview every proposed profile or policy effect before a separate authorized
   activation transaction.
6. Preserve unknown YAML keys and Markdown body bytes when round-tripping.
7. Validate UTF-8, frontmatter bounds, YAML resource limits, reserved filenames,
   path containment, links, and duplicate concept identities.
8. Reject executable interpretation of fenced code, links, executors, attesters,
   Skills, scripts, or referenced resources during import and inspection.
9. Run offline and add no network resolution by default.
10. Compare the bundle with Retonr's canonical JSON export for information loss,
    privacy, portability, and agent usefulness.

The spike graduates only if OKF adds interoperability over canonical JSON without
weakening consent, deletion, authority, exact identity, or deterministic replay.

### Temporal preference use

OKF can provide a readable projection of a time-aware preference ledger. It is not
the ledger itself.

The projection may express concepts and links for rules, observations, channels,
sources, conflicts, supersession, and review history. Retonr still owns typed
relation semantics, valid time, transaction time, consent, revocation, derivation
closure, and immutable profile projections in its canonical schema.

OKF links are untyped relationships. A temporal knowledge graph or graph database is
not justified merely because OKF files form a graph. Each must prove incremental
value over the relational ledger and portable bundle baseline.

### Attested evaluation use

Attested Computation is promising for publishing human-readable qualification and
evaluation recipes. A Retonr bundle could name a pinned command, typed parameters,
expected receipt fields, and a deterministic result checker.

That is not yet a portable execution standard. Retonr must separately own package
identity, signatures, installation, permissions, sandboxing, network policy,
resource bounds, receipts, attester ABI, and authority. An OKF file never grants
permission to run referenced code.

## Privacy and authorship boundary

The actor convention and trust fields are optional. Export can reveal a person,
organization, model, process, review time, source location, usage pattern, and
workflow history.

Retonr therefore:

- Makes OKF export explicit and previews sensitive fields
- Uses local opaque actor IDs unless the user chooses a public identifier
- Does not interpret `human-reviewed` as human-authored
- Does not interpret `machine-confirmed` as correct
- Does not interpret `verified` as consent, authorization, ownership, or legal
  compliance
- Does not require generated-by attribution in edited user output
- Does not ingest OKF prose or agent-generated concepts as profile evidence without
  separate ownership, authorization, and admission
- Applies normal export deletion warnings because copied bundles are outside
  application-controlled storage

## Version and conformance boundary

OKF v0.2 deliberately tolerates unknown concept types, unknown frontmatter keys,
broken links, and missing optional fields. Retonr should preserve that format
permissiveness while applying stricter product rules to concepts that request an
effect.

`Conformant OKF` and `accepted Retonr import` are separate outcomes:

- Conformance checks the OKF bundle structure.
- Retonr import checks the selected Retonr concept schema, sensitivity, authority,
  versions, identifiers, references, bounds, and proposed effect.
- Activation remains a distinct authenticated transaction.

The v0.2 version rules encourage best-effort consumption of unknown versions. Retonr
may provide read-only generic inspection, but it must not perform a profile,
configuration, or executable effect from an unknown or unsupported version.

## Research and release gates

- Record the exact OKF specification version and repository revision.
- Maintain v0.1 migration fixtures for `timestamp` to `generated.at` and body
  citations to `sources` only if v0.1 import is claimed.
- Test unknown fields, types, versions, broken links, duplicate paths, reserved
  filenames, absolute and relative links, traversal, symlinks, YAML aliases,
  resource exhaustion, Unicode, and malformed frontmatter.
- Prove import, inspection, indexing, and export work with network access blocked.
- Prove import cannot execute or fetch an executor, attester, computation, link,
  Skill, or script.
- Prove imported trust fields cannot grant profile, file, network, model,
  administration, or publication authority.
- Test content-minimized exports for raw sample, embedding, credential, path, and
  short-hash leakage.
- Compare human and named-agent navigation through the generated indexes.
- Revalidate on every OKF version, conformance, actor, trust, lifecycle,
  attestation, path, or packaging change.

## Decision

Track OKF v0.2 as an experimental portable knowledge view and exchange format. Do
not make it a 1.0 storage dependency, replace Retonr's canonical JSON contracts, or
delay the reference CLI and agent tool work for it.

The earliest valuable implementation is a read-only exporter for research claims
and support matrices, followed by a redacted profile-policy export. Import and
Attested Computation integration remain experimental until authority, privacy,
round-trip, and execution boundaries pass their gates.

## Primary sources

- [Open Knowledge Format v0.2 specification](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
- [Official OKF repository and reference implementation](https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf)
- [Google Cloud introduction of OKF v0.1](https://cloud.google.com/blog/products/data-analytics/how-the-open-knowledge-format-can-improve-data-sharing/)
