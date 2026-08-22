# Anthropic Claude text watermark watch

## Review status

Reviewed: August 21, 2026.

Research cutoff: August 21, 2026. This is a dated watch record, not a product
decision or a removal recipe.

Scope: Anthropic's public description of Claude text watermarking, its stated
relationship to SynthID-Text, and how that differs from Unicode, C2PA, and
classifier-based "AI detection." This record does not select a detector, claim that
a rewrite removes a mark, or authorize detector-guided editing.

## Provider statement

On 14 August 2026 Anthropic published
[How Claude's text watermark works](https://www.anthropic.com/news/claude-text-watermark).
The stated facts that matter to Retonr are:

- Future Claude models generate text that contains a watermark. Anthropic says this
  is to comply with the EU AI Act transparency marking duty. Anthropic also says it
  signed the EU Code of Practice on Transparency of AI-Generated Content, whose
  final text was published on 10 June 2026.
- The method is "a version of the SynthID-Text approach" from DeepMind's 2024
  *Nature* paper. It belongs to the keyed sampling family that traces to Scott
  Aaronson's 2022 proposal.
- Nothing is inserted into the text. There are no hidden characters. The watermark
  changes the *source of randomness* used to choose among tokens the model was
  already considering.
- Anthropic distinguishes this from C2PA content credentials attached to supported
  image files, which are metadata labels, not token-sampling marks.
- Detection, when offered, answers a likelihood that Claude was involved. It does
  not identify a user, prove human authorship, or survive a complete rewrite of
  every word. Sparse factual text, short samples, proofreading, and exact code have
  less or no usable mark.

These claims are `provider_statement`. The production key, exact tournament
parameters, coverage by model and surface, and detector operating point are not
independently verified here.

## What public removers usually confuse

Public "watermark remover" projects and posts around this announcement commonly
mix three different marks:

1. C2PA or other file-metadata credentials. These are not the Claude text
   watermark. Stripping sidecar bytes does not touch keyed sampling.
2. Unicode tag, homoglyph, or invisible-character payloads. Anthropic states that
   the Claude text watermark adds no hidden characters.
3. Statistical keyed-sampling marks (SynthID-Text tournament sampling and related
   schemes). These live in token choices. They are not a string that a regex, NFC
   pass, or "remove special characters" tool can delete.

A tool that claims to "remove Anthropic watermarks" by stripping Unicode or C2PA is
solving the wrong problem. A tool that paraphrases until a detector goes quiet is
an evasion recipe, not fidelity-preserving reconstruction. Retonr must not adopt
either as a product path.

## Retonr implication

Retonr remains a local-first fidelity-gated editorial engine. The hard gates stay
the acceptance authority. Watermark or source-classification diagnostics must not
guide generation, retry, ranking, or acceptance.

Staying current here means:

- Treat Claude text marks as keyed tournament sampling, not Unicode.
- Keep the existing research-only watermark-refusal corpus as folklore refusal, not
  as a detector.
- Revalidate this record if Anthropic ships a public detector API, changes the
  sampling family, or documents a different coverage rule.
- Do not implement a remover, a detector-guided rewrite, or a claim that a
  Retonr transaction "clears" a Claude mark.

A later local reconstruction can change token-level sampling evidence as a side
effect of rewriting. That is not a guarantee, not a feature, and not a compliance
argument.

## Open questions

- Exact production sampling parameters and key management remain undisclosed.
- Detector API details, false-positive behavior, and edit-distance tolerance are
  not yet a public, independently measured operating point.
- Coverage of older Claude models, translations, and lightly edited human text is
  described qualitatively by Anthropic and needs revalidation when a detector is
  actually available.

## Sources

- Anthropic,
  ["How Claude's text watermark works"](https://www.anthropic.com/news/claude-text-watermark),
  14 August 2026.
- Dathathri et al.,
  ["Scalable watermarking for identifying large language model outputs"](https://www.nature.com/articles/s41586-024-08025-4),
  *Nature* (2024), SynthID-Text.
- European Commission,
  [Code of Practice on Transparency of AI-Generated Content](https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content),
  final text published 10 June 2026.
