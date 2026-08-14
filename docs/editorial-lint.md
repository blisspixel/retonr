# Editorial lint and the anti-slop quality loop

## Purpose

Retonr includes an explainable editorial-lint layer for recurring wording and
presentation problems. The informal project shorthand may be "slop detector," but
the product contract is narrower and testable: identify concrete editorial
anti-patterns, apply the user's declared style, and report what improved.

Editorial lint does not determine whether a person or model wrote a passage. Common
phrasing is weak source evidence, and human writing can contain every pattern in this
document. The product therefore reports a lint finding, not an AI-authorship verdict.

## Separation of concerns

Retonr maintains three distinct classes of analysis:

| Class | Example | Live rewrite authority |
| --- | --- | --- |
| Fidelity and safety gate | Changed quantity, broken link, unsupported document structure | May reject a candidate |
| Editorial lint | Canned transition, repeated conclusion, excessive punctuation, vague attribution | May guide and rank candidates after hard gates pass |
| Source-signal research | Provider classifier, published statistical watermark test, authorship model | Isolated diagnostics only; never guides or ranks a live candidate |

This boundary prevents an unreliable source classifier from becoming a hidden
optimization target. Editorial quality is a legitimate product objective. A lower
watermark or source-classification score is not.

## Finding model

Every finding has:

- A stable rule ID and version
- An exact source range or document node
- A category, severity, and concise explanation
- The observed evidence, without a claim about who or what wrote it
- Applicability by language, channel, document kind, and profile
- Exclusions for protected content, quotations, citations, code, and literal data
- A proposed action: retain, review, replace, combine, split, or remove
- The policy source: built-in baseline, selected house style, or explicit user rule

Deterministic findings are labeled deterministic. Learned findings disclose the
exact artifact and calibrated scope. Model self-assessment is not presented as
confidence.

## Initial rule families

The initial lint catalog should cover:

- Conversational residue such as assistant greetings, offers for further help, and
  meta-commentary about producing the answer
- Canned openings, transitions, scene-setting, and conclusions that add no claim
- Repeated thesis statements, recap paragraphs, and conclusion duplication
- Inflated sectioning, decorative headings, and unnecessary one-line fragments
- Formulaic contrast patterns and repeated rhetorical templates
- Excessive emoji, dash, exclamation, colon, bold, or parenthetical use relative to
  the selected profile and channel
- Vague attribution, anonymous authority, and unsupported confidence
- Redundant qualifiers, throat clearing, generic intensifiers, and abstract filler
- Uniform sentence or paragraph rhythm that conflicts with the user's evidence
- Phrase repetition within a document or across adjacent rewrite units
- Fabricated quotation styling or newly introduced quotation boundaries, which also
  trigger a fidelity gate

Individual words and punctuation marks are not inherently defective. Rules operate
on context, density, repetition, channel, and the user's explicit preferences. A
technical term, quotation, accessibility annotation, or intentional rhetorical
choice must not be rewritten merely because it matches a surface pattern.

## Relationship model

Some findings reinforce, exclude, precede, or form part of other findings. Retonr
will represent those relationships in a bounded, versioned editorial pattern graph
rather than a flat word ban list. The product graph contains only independently
qualified editorial rules. It does not contain model-family labels, detector scores,
watermark outcomes, or AI-authorship probabilities.

The scanner returns named findings and an activation vector, not one universal slop
score. A graph relationship earns product use only if it improves a predeclared
editorial outcome over the simpler flat-rule baseline without a material fidelity or
false-positive regression. Population excess ratios and source-style correlations
remain in a separate research graph that cannot guide live rewriting.

The detailed evidence and qualification order are recorded in the
[editorial pattern graph research decision](research/2026-08-13-editorial-pattern-graph.md)
and its [mathematical specification](research/2026-08-13-editorial-pattern-mathematics.md)
and [evaluation preregistration](research/2026-08-13-editorial-pattern-evaluation.md).

## Quality loop

1. Scan the eligible source and record its lint baseline.
2. Compile applicable user rules, profile tendencies, channel policy, and document
   brief into a versioned lint policy.
3. Give generation strategies only the findings and policies within their declared
   edit authority.
4. Run literal, structure, semantic, and other hard gates on each candidate.
5. Rescan eligible candidates with the same lint-policy version.
6. Among candidates that passed every hard gate, prefer declared-rule compliance,
   personal fit, fluency, and justified lint improvement in that order.
7. Report resolved, retained, introduced, suppressed, and uncertain findings.

A lint improvement can never compensate for a hard-gate failure. The engine does not
maximize change volume, phrase novelty, or distance from the source. When the safe
fix is uncertain, it retains the original or asks the user.

## CLI and agent contract

The reference CLI should expose:

```console
retonr lint <path|directory|-> --profile <name>
retonr lint rules --profile <name>
retonr lint explain <finding-id>
retonr check <path|-> --profile <name>
retonr rewrite <path|directory|-> --profile <name> --report <path>
```

Human output groups findings by severity and location. Versioned JSON returns stable
rule IDs, ranges, policy identity, exclusions, and proposed actions. Agent tools use
the same application service and cannot request a hidden authorship verdict or
detector-optimization mode.

## Change report

The transaction report includes:

- Source and output lint counts by stable rule ID
- Findings resolved, retained, introduced, suppressed, and uncertain
- Exact changed ranges and the policy authorizing each accepted edit
- Approximate word and character change ratios
- Profile and document-brief identities
- Hard-gate outcomes and abstentions
- Language or format strata for which no lint claim is available

The report says that a document has fewer named editorial findings. It does not say
that the document is human-written, undetectable, watermark-free, or legally exempt.

## Qualification

Each rule requires positive fixtures, context exclusions, adversarial near-matches,
and regression fixtures for every corrected defect. Rules are qualified separately
by language and channel. Release evaluation reports precision, recall where a
complete labeled set is meaningful, user acceptance of proposed fixes, introduced
finding rate, fidelity outcomes, and document-level repetition.

The strongest baseline is a transparent user-editable rule set. A learned lint rule
ships only when it improves a predeclared outcome over that baseline without a
material fidelity or false-positive regression.

Development, calibration, clean-control, and known-watermark fixtures follow the
separate [evaluation corpus contract](evaluation-corpora.md). Editorial cases never
receive human or AI authorship labels.
