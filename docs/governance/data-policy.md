# Evaluation data authorization, retention, and deletion policy

## Status

Status: proposed. No non-synthetic collection is authorized until the project owner
records approval, the consent materials are attached, and the evaluation manifest
names this policy revision by digest.

This policy applies to evaluation, calibration, user research, red-team work, and
model qualification. It does not authorize general product telemetry or collection
from production documents.

## Governing principles

- Collect the minimum data needed for a predeclared question.
- Keep identity, contact details, writing, labels, and derived representations
  separated by least privilege.
- Treat style features and embeddings as sensitive personal data.
- Do not describe hashing, redaction, or local storage as anonymization.
- Never use a locked evaluation case as a prompt example, training item, or threshold
  tuning input.
- Make consent revocation and deletion invalidate the complete application-controlled
  derivation closure.

## Authorized data classes

### Synthetic fixtures

Maintainer-written text that contains no copied personal, confidential, or licensed
source material may be committed for development and regression use. The fixture
manifest records its authoring method, intended risk category, and review status.

### Licensed public data

Public data may be used only after a manifest records the exact source revision,
license, permitted purpose, attribution requirement if any, redistribution limits,
and reviewer decision. Public availability alone is not authorization.

### Participant-contributed data

Participant writing or study responses require specific informed consent for the
named study. Consent states the data collected, purpose, access roles, retention,
derived features, model or human review, withdrawal method, deletion limits, and
whether any de-identified aggregate may remain after withdrawal.

Participants confirm that they own the writing or are authorized to provide it.
Third-party private messages, employer-confidential material, credentials, legal
privilege, and regulated records are excluded unless a later approved protocol
explicitly establishes authority and safeguards.

### Security test data

Red-team fixtures use synthetic secrets, identities, and canaries. Real credentials,
tokens, private keys, private documents, or copied production incidents are not
admitted to the repository.

## Prohibited collection and use

- Scraping personal writing without study-specific consent or an approved licensed
  public-data decision
- Content from minors in the initial research program
- Passwords, authentication tokens, private keys, payment-card data, or government
  identifiers
- Raw production documents, prompts, candidates, audio, or profiles in diagnostics,
  analytics, crash reports, or support bundles by default
- Training or fine-tuning a model on participant data unless a separate consent and
  approval explicitly authorizes it
- Reusing participant data for a materially different question without renewed
  authorization
- Sending research data to a remote model or service under the local-only protocol
- Allowing a model-generated candidate to become owner evidence without explicit
  owner editing and confirmation

## Required manifest

Every non-synthetic corpus records:

- Corpus and policy schema versions
- Content digest and immutable source revision
- Purpose, predeclared questions, included fields, and minimization decision
- Collection authority, consent revision, and participant population
- License and redistribution decision
- Data controller and approved access roles
- Storage locations, encryption, key ownership, and backup scope
- Retention trigger and deletion procedure
- Derivation types, including labels, features, vectors, retrieval snapshots,
  compiled profiles, exports, and caches
- Split, cluster, topic, participant, and leakage controls
- Approval, amendment, incident, revocation, and deletion records

Unknown, incomplete, or mismatched manifests are rejected before ingestion.

## Storage and access

- Direct identifiers and contact records are stored separately from corpus IDs.
- Access is deny by default, role-scoped, reviewed, and logged.
- Locked cases are sealed from prompt construction and development access.
- Local research storage uses operating-system protections and encryption at rest
  where the selected store supports it.
- Portable exports omit contact data, credentials, configuration, and encryption
  keys.
- Repository fixtures contain only approved redistributable content.

## Retention

Retention is set per approved study before collection. A manifest cannot use
`indefinite` for participant raw text without a separate recorded justification.

When the study-specific period ends, application-controlled raw text, labels tied to
that text, vectors, caches, exports, and intermediate files are deleted unless a
renewed approval records a narrower retained purpose. Synthetic fixtures and
redistributable public corpora follow their manifest and license.

The project stores only aggregate results that cannot reasonably be linked back to a
participant after source deletion. Small cells, rare phrases, unique n-grams, and
free-text notes are reviewed before an aggregate is retained.

## Exclusion, revocation, and deletion semantics

The system represents distinct operations:

1. Retrieval-ineligible keeps authorized evidence and approved aggregate influence,
   but invalidates retrieval snapshots containing that evidence.
2. Profile-influence exclusion keeps authorized evidence for inspection while
   invalidating its permitted derivation closure in observations, vectors,
   retrieval, and compiled profiles.
3. Consent revocation blocks new use and invalidates the complete
   application-controlled derivation closure, subject only to a documented legal
   retention obligation.
4. Deletion removes application-controlled source, metadata, labels, features,
   vectors, snapshots, compiled views, caches, exports, and backups when they reach
   their documented deletion cycle.

Every operation appends a non-content audit event with request, scope, policy,
completion, exception, and verification identities. The audit event does not retain
the deleted content or a reversible content identifier.

Deletion claims are limited to application-controlled buffers, files, databases,
caches, exports, and backups. They do not claim physical erasure from device wear
leveling, operating-system swap, crash dumps, third-party backups, or storage outside
project control. These limits are disclosed before collection.

## Incident and disclosure handling

Unexpected exposure freezes affected collection and locked-set use. The owner
records scope, affected identities, access evidence, containment, notification
decision, retained legal obligations, and whether a corpus must be retired. Broad
disclosure of locked content permanently disqualifies that version from future
locked release evidence.

## Approval and change control

Initial approval requires the project owner, privacy reviewer, and research owner.
Security review is also required when new storage, network, model, audio, or export
boundaries are introduced.

Material changes create a new policy revision and corpus manifest. Prior consent is
not silently broadened. The approved revision, reviewer identities, date, and digest
are retained with study evidence.
