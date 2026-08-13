# Guided editorial brief and evolving preferences

## Product outcome

Retonr can act like a careful copy editor before it rewrites. It inspects a document
without changing it, identifies the few unresolved choices that could materially
affect the result, and asks the user a short set of answerable questions.

This serves a principal-editor workflow: an executive, author, or owner can provide
the main point, audience, stance, and non-negotiables without line-editing a long
draft. Retonr then applies that direction through bounded document transactions and
reports what changed.

The workflow has two distinct inputs:

- A long-lived personal profile describes relatively stable writing preferences and
  time-aware evidence.
- A document brief describes the purpose and choices for this document, audience,
  channel, and moment.

A document answer does not silently become a permanent preference. A profile rule
does not silently override an explicit document brief.

## Example workflow

```console
retonr brief report.docx --profile executive --interactive
retonr plan report.docx --profile executive --brief report-brief.json
retonr rewrite report.docx --profile executive --brief report-brief.json --output-dir rewritten
```

The interactive flow may ask:

1. What is the one conclusion the reader should retain?
2. Who is the decision-maker, and what do they already know?
3. Should the draft recommend, explain, reassure, challenge, or request action?
4. Which claims, phrases, caveats, or commitments are non-negotiable?
5. Is light copy editing enough, or may Retonr also tighten repetition and
   transitions inside the approved edit budget?

Questions are derived from the actual document and profile. They are not a fixed
onboarding survey. The user can answer, edit, skip, defer, or mark a question
irrelevant. Non-interactive operation requires an existing brief or safe defaults and
never prompts.

## Brief schema

A versioned brief contains explicit, typed fields:

- Owner and optional delegated editor role
- Document digest and adapter capability version
- Audience and assumed reader knowledge
- Primary point, desired action, and success condition
- Stance, tone, formality, directness, and warmth
- Allowed edit level and maximum change budget
- Protected claims, terms, caveats, quotations, and commitments
- Terminology and naming decisions
- Channel, locale, and publication context
- Questions asked, answers, skipped items, and remaining uncertainty
- Source of every field: user answer, active profile rule, document observation, or
  explicit default
- Creation, activation, supersession, and expiration state

Document observations and model inferences remain provisional. Only user-confirmed
answers or approved defaults become active brief instructions. A brief cannot expand
adapter eligibility, weaken fidelity, authorize new files, or grant profile mutation.

## Question selection

Retonr asks a question only when all of these conditions hold:

1. The answer could change an eligible rewrite decision or abstention.
2. The uncertainty cannot be resolved from an explicit profile rule, protected
   source text, or already confirmed brief field.
3. The question has a small, clear answer space or a concise free-text answer.
4. The user can understand why the answer matters.
5. Expected editorial value exceeds interruption cost under a predeclared policy.

The system ranks candidate questions lexicographically:

1. Prevent a fidelity or commitment mistake.
2. Resolve a direct conflict between source, profile, and brief.
3. Clarify the primary point, audience, or requested action.
4. Resolve a choice likely to affect many eligible units.
5. Improve style only after higher-risk uncertainty is closed.

This adapts the useful idea from active preference learning that information gain is
not enough by itself. A technically informative question can still be difficult,
annoying, or impossible for a person to answer. Retonr therefore measures question
answerability, interruption burden, skip rate, correction rate, and downstream edit
value alongside information gain.

The default interaction is a small bounded set. The user can request more questions
or choose `use safe defaults`. A skipped answer never becomes a negative preference.

## Document analysis boundary

The analysis pass may identify:

- Candidate thesis or requested action
- Audience assumptions and unexplained terms
- Competing tones or inconsistent formality
- Repeated definitions and phrases
- Ambiguous pronouns or references
- Sections whose stance conflicts
- Claims, caveats, quotations, dates, names, and commitments that need protection
- Places where a brief answer would affect several units

These are untrusted observations linked to exact source units. They are not facts
about the author and do not become profile evidence. A local model may suggest an
observation, but deterministic parsing, source links, explicit confirmation, and the
shared validation cascade control its use.

Questions never ask the user to approve a lossy summary in place of the document.
For a high-impact answer, the interface shows the relevant source excerpts and the
planned effect without requiring the user to inspect every line.

## Precedence and conflicts

The instruction order is:

1. Protected source facts and format invariants
2. Explicit active document brief
3. Explicit active channel and profile rules
4. Authorized profile observations with confidence and context
5. Document-level provisional guidance
6. Strategy defaults

Higher levels can narrow lower levels but cannot bypass fidelity or format gates. A
brief conflict is shown before generation. Retonr does not invent a compromise
between `must retain` and `remove`.

## Time-aware preference ledger

Style evolves. Retonr initially represents that evolution through an append-only
preference ledger and immutable profile versions, not a speculative graph database.

Each preference event records:

- Event ID and exact timestamp
- Profile, channel, audience, locale, document type, and optional project context
- Source type: declared rule, authorized sample, interview answer, correction,
  comparison, rejection, revocation, or deletion
- Provenance and consent state
- Structured subject, relation, value, and units where applicable
- Confidence, sample count, and supporting evidence IDs
- Valid-from, valid-until, supersedes, conflicts-with, and derived-from links
- Whether the value is explicit, observed, inferred, provisional, active, or revoked
- Contribution cap and retrieval eligibility

The active profile is a deterministic projection at a selected time and context. It
does not overwrite history. A recent situational preference does not automatically
erase a stable declared rule, and an old habit does not silently dominate a direct
current instruction.

The relational representation already forms a provenance and conflict graph. SQLite
tables and explicit edges are sufficient for the initial product. A dedicated graph
store, temporal embedding, or learned Temporal Knowledge Graph is admitted only if a
benchmark shows that it improves held-out owner preference or question quality over
the explicit ledger without weakening explainability, deletion, determinism,
portability, or resource use.

Temporal graph research demonstrates that facts and preferences can change over
time, but prediction performance in a recommendation task does not establish value
for personal writing style. Retonr treats that literature as a source of candidate
representations, not proof that a graph neural model belongs in the product.

## Feedback and refinement over time

After a rewrite, the user can provide structured feedback:

- Accept unchanged
- Accept after user edit
- Too formal, casual, verbose, terse, promotional, cautious, or indirect
- Wrong audience or main point
- Preferred one candidate over another
- Fidelity concern
- Correct or retire a profile rule
- Make a document answer a reusable preference

Feedback updates statistics or creates a provisional event. It does not alter the
active profile until the user previews and activates a new immutable version. User
edits are not automatically ingested as evidence. If the user elects to contribute
them, Retonr records lineage to the source, candidate, final edit, consent, and
contribution cap.

Reversing, revoking, or deleting an event invalidates its complete transitive
derivation closure and affected indexes. Reports distinguish logical removal from
application-controlled storage cleanup and external backup limitations.

## Typed first, voice later

The canonical brief contract is typed. CLI and native desktop typed workflows must
be complete before another input mode exists.

Local voice can later transcribe answers into the same editable fields. A transcript
must be reviewed and confirmed before it becomes a brief answer or profile event.
Voice adds no new instruction type, profile authority, or evidence shortcut. The
product remains complete without a microphone, speech runtime, or voice model.

This order keeps accessibility, deletion, cancellation, artifact licensing, and
cross-platform audio complexity from blocking the core editorial workflow.

## Agent behavior

An agent may:

- Request document analysis and proposed clarification questions
- Present those questions to the user
- Submit explicit answers through a bounded brief handle
- Run a plan or rewrite after the brief is active
- Receive the final validated output and change report

Routine agent authority cannot answer on the user's behalf, turn skipped questions
into preferences, activate a profile version, read raw profile evidence, or infer
consent from conversation state. A handle is scoped, expiring, revocable, and not an
authentication credential.

## Evaluation gates

Compare at least:

- No clarification
- Fixed generic questions
- Document-derived questions ranked by model confidence alone
- Document-derived questions under the value and answerability policy
- Full user-written brief

Measure:

- Blind owner preference and edit distance to the owner's final version
- Fidelity false acceptance and transformation coverage
- Main-point, audience, stance, and protected-commitment adherence
- Questions asked, skipped, revised, or judged irrelevant
- Answer time, interruption burden, and abandonment
- Marginal value of each question and diminishing returns
- Stability across documents, channels, topics, languages, and user experience levels
- Incorrect promotion of situational answers to durable preferences
- Revocation, deletion, projection, conflict, and time-travel reconstruction accuracy

The adaptive question system graduates only if it beats fixed questions or a simple
brief without a material fidelity, usability, privacy, or resource regression. A
Temporal Knowledge Graph graduates only if it beats the explicit time-aware ledger
under a predeclared incremental value threshold.

## Primary research references

- [Asking Easy Questions: A User-Friendly Approach to Active Reward Learning](https://iliad.stanford.edu/pdfs/publications/biyik2019asking.pdf)
- [Temporal Knowledge Graph Completion survey](https://arxiv.org/abs/2201.08236)
- [Temporal Knowledge Graph representation survey](https://arxiv.org/abs/2403.04782)
- [MetaTKG](https://aclanthology.org/2022.emnlp-main.487/)
