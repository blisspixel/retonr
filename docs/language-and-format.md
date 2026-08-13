# Language and format preservation

## Two independent support claims

Retonr separates language support from document-format support:

- Language support means a qualified model and validation policy can process the
  declared language, locale, mode, and content class at the published risk bound.
- Format support means an adapter can identify eligible text, apply accepted edits,
  and verify the advertised structure and unchanged regions.

A language can be qualified for plain text without being qualified for Markdown or
DOCX. A format can be structurally supported while a particular language remains
unsupported. Interfaces report both capability decisions explicitly.

## Multilingual contract

Each rewrite unit carries:

- User-declared or detected BCP 47 language and locale
- Detection confidence and source
- Script and text direction where relevant
- Tokenizer and normalization observations
- Qualified generator, evaluator, and policy identities
- Any mixed-language spans that must remain protected

The user can declare a document language, declare languages by region, or choose
automatic detection. Automatic detection never invents support. Low-confidence or
unsupported units remain unchanged, and document atomicity returns the complete
original when a required unit cannot be processed.

Mixed-language documents are planned as region-aware input. Code, identifiers,
proper names, quoted source material, citations, and embedded language switches can
be protected independently. Cross-language profile transfer is off by default.
Profile evidence is scoped by language and channel, with an explicit reviewed
fallback when evidence is sparse.

The first 1.0 language matrix must include English, one additional Latin-script
language, and one non-Latin-script language. The exact set is a qualification
decision, not a marketing decision. Right-to-left content receives explicit parser,
terminal-safety, diff, desktop layout, clipboard, and document-adapter testing before
it is advertised.

## Multilingual evaluation

Every supported language receives positive paraphrases and hard negatives for:

- Entities, roles, attribution, and coreference
- Quantities, currency, units, dates, time, duration, and locale conventions
- Negation, modality, evidentiality, politeness, formality, and honorifics where used
- Conditions, exceptions, comparisons, thresholds, and temporal order
- Segmentation, punctuation, quotation, casing, inflection, and agreement
- Unicode normalization, combining marks, grapheme clusters, confusables, and bidirectional controls
- Code switching, borrowed words, names, URLs, paths, and product terms

Qualification reports results per language and risk category. Pooled multilingual
scores cannot hide a weak language, script, or content class. Human adjudicators and
profile owners must be fluent in the language they assess.

## Formatting contract by surface

| Input surface | Preservation contract |
| --- | --- |
| Direct argument | Exact Unicode scalar content is accepted only within the documented shell and size limits; multiline use should prefer stdin or a file |
| Standard input | Read bytes until end of file without trimming; preserve supported byte order mark, newline kind, blank lines, and final-newline state |
| Plain-text clipboard | Read and write explicit plain text only; do not claim that fonts, colors, links, or other rich clipboard flavors are preserved |
| TXT file | Preserve source bytes outside accepted text edits and return the complete original on document-atomic abstention |
| Markdown | Rewrite only eligible source ranges, preserve all other bytes, reparse output, and compare a versioned structural fingerprint |
| DOCX | Patch only the declared WordprocessingML subset, verify package topology and untouched parts, and abstain on ambiguous formatting or unsupported features |
| XLSX | Post-1.0 only: rewrite declared prose cells while protecting formulas, types, references, names, validation, styles, charts, macros, and workbook structure |
| API or MCP | Require an explicit media type and encoding; return capabilities, unsupported features, and preservation evidence in the outcome |

No path flattens a structured document into plain text and then claims formatting was
preserved. Rich clipboard input is a separate future adapter with explicit HTML or
RTF capability and sanitization rules. Until it graduates, clipboard operations are
plain text and labeled accordingly.

## Plain text

The adapter records decoding, optional byte order mark, newline sequence, final
newline state, and original byte ranges. It never normalizes line endings, Unicode,
or whitespace as an incidental side effect. Any deliberate normalization is a
separate transformation that requires an explicit option and a distinct rewrite
record.

Multiline input is one document, not a sequence of shell arguments. Empty lines,
leading and trailing whitespace, and the absence of a final newline remain
meaningful adapter state.

## Markdown

Markdown uses source splicing rather than serialize-after-parse. The initial
qualified subset is plain prose in paragraphs and headings. Code, raw HTML, link
destinations, autolinks, front matter, unsupported inline constructs, and ambiguous
nested ranges are protected or cause abstention.

The output must:

1. Change only approved non-overlapping UTF-8 byte ranges.
2. Escape generated content for the exact source context.
3. Preserve byte identity outside those ranges.
4. Reparse under the same declared dialect and extensions.
5. Match the required structural fingerprint.
6. Introduce no new executable HTML, destination, or unsupported construct.

Tables, task lists, footnotes, and other extensions graduate separately with their
own language and structure fixtures.

## DOCX

The initial subset is unencrypted `.docx` main-story paragraphs and table cells with
homogeneous effective run formatting. The adapter preserves package relationships,
content types, untouched decompressed part bytes, and non-target content. It does not
promise character-level formatting preservation across arbitrary run-boundary
rewrites.

Tracked changes, fields, signatures, encryption, macros, content controls, drawings,
equations, embedded objects, ambiguous mixed formatting, and other undeclared
features are protected or rejected according to the capability matrix. Reopening a
file is necessary evidence, not sufficient evidence of preservation.

## Verification and abstention

Completed output is verified by the owning adapter after all edits are applied.
Validation failure, unsupported content, mixed-language uncertainty, or format drift
produces a typed unsupported or abstained result. In document-atomic mode, output is
the original bytes. Unit and region atomicity are allowed only after independence and
completed-document checks pass.

The full interface behavior is defined in [Input and integration surfaces](interfaces.md).

## Long documents and folders

Format support composes with the
[non-destructive document transaction](document-transactions.md). Each adapter owns
stable units and protected state while the application owns source-linked document
guidance, bounded context, unit and region validation, staging, atomicity, recovery,
and reporting.

A directory is a manifest of independent documents, not a new format. Discovery
freezes canonical paths, source digests, capabilities, bounds, destinations,
collisions, and link policy before model work. One file cannot expand authority to an
adjacent path.

Page counts are reported only from the same named, versioned, qualified renderer,
fonts, locale, operating system, and page settings. Retonr can enforce a configured
drift bound in that environment but cannot promise universal pagination across
applications and machines.

## Post-1.0 SpreadsheetML

Spreadsheet rewriting begins with an opt-in declared subset of prose cells. It never
edits formula text as prose. Formula elements, cached-value policy, cell types,
references, defined names, tables, validation, conditional formatting, charts,
macros, external links, styles, calculation settings, sheets, and workbook structure
remain protected.

Shared strings require special handling because several cells can reference one
string entry. A selected cell rewrite must create or reuse an isolated replacement
without changing unselected cells. Qualification requires formula and structure
fingerprints, shared-string reference checks, package verification, recalculation
policy, and reopen fixtures. A generic XML substitution cannot make an XLSX support
claim.
