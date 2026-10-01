//! Deterministic rule matching for editorial quality and anti-slop patterns.

use std::ops::Range;

use rewrite_types::EditorialFinding;

use super::slop_rules;
use super::style_rules;

/// Catalog of supported editorial rule identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleCatalog {
    /// Prefabricated introductory scene-setting cliches.
    PrefabricatedSceneSetting,
    /// False or exaggerated contrastive reframes.
    ContrastiveReframe,
    /// Stacked tricolon lists and alliterative clusters.
    StackedTricolon,
    /// Unsubstantiated marketing superlatives and promotional puffery.
    PromotionalPuffery,
    /// Metaphorical ornamental abstractions.
    OrnamentalAbstraction,
    /// False reader segmentation formulas.
    ReaderSegmentation,
    /// Rhetorical question-answer pairs followed by buzzwords.
    PerformativeQuestionAnswer,
    /// Unsubstantiated claims of profound historical significance.
    EmptySignificance,
    /// Excessive piling-up of summary markers.
    SummaryPileup,
    /// Generic or ungrounded positive conclusions.
    GenericPositiveClose,
    /// Excessive consecutive sentence-initial transitional adverbs.
    TransitionPileup,
    /// Artificial calls for collaborative journeys.
    PerformativeCollaboration,
    /// Conversational residues like conversational openings or sign-offs.
    ConversationalResidue,
    /// Non-substantive throat-clearing preambles.
    ThroatClearing,
    /// Canned colloquial transitions.
    CannedTransition,
    /// Redundant repetition of full conclusion statements.
    RepeatedConclusion,
    /// Vague, unreferenced attribution formulas.
    VagueAttribution,
    /// Redundant modifying qualifiers.
    RedundantQualifier,
    /// Excessive exclamation marks.
    ExcessiveExclamation,
    /// Excessive em-dash punctuation density.
    ExcessiveDashDensity,
    /// Excessive emoji density.
    ExcessiveEmojiDensity,
}

impl RuleCatalog {
    /// Returns the stable string identifier for this rule.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::PrefabricatedSceneSetting => "prefabricated_scene_setting",
            Self::ContrastiveReframe => "contrastive_reframe",
            Self::StackedTricolon => "stacked_tricolon",
            Self::PromotionalPuffery => "promotional_puffery",
            Self::OrnamentalAbstraction => "ornamental_abstraction",
            Self::ReaderSegmentation => "reader_segmentation",
            Self::PerformativeQuestionAnswer => "performative_question_answer",
            Self::EmptySignificance => "empty_significance",
            Self::SummaryPileup => "summary_pileup",
            Self::GenericPositiveClose => "generic_positive_close",
            Self::TransitionPileup => "transition_pileup",
            Self::PerformativeCollaboration => "performative_collaboration",
            Self::ConversationalResidue => "conversational_residue",
            Self::ThroatClearing => "throat_clearing",
            Self::CannedTransition => "canned_transition",
            Self::RepeatedConclusion => "repeated_conclusion",
            Self::VagueAttribution => "vague_attribution",
            Self::RedundantQualifier => "redundant_qualifier",
            Self::ExcessiveExclamation => "excessive_exclamation",
            Self::ExcessiveDashDensity => "excessive_dash_density",
            Self::ExcessiveEmojiDensity => "excessive_emoji_density",
        }
    }
}

/// Applies all rule checks against input text.
#[must_use]
pub fn apply_all_rules(text: &str) -> Vec<EditorialFinding> {
    let quotes = quoted_spans(text);
    let mut findings = Vec::new();

    slop_rules::check_prefabricated_scene_setting(text, &quotes, &mut findings);
    slop_rules::check_contrastive_reframe(text, &quotes, &mut findings);
    slop_rules::check_stacked_tricolon(text, &quotes, &mut findings);
    slop_rules::check_promotional_puffery(text, &quotes, &mut findings);
    slop_rules::check_ornamental_abstraction(text, &quotes, &mut findings);
    slop_rules::check_reader_segmentation(text, &quotes, &mut findings);
    slop_rules::check_performative_question_answer(text, &quotes, &mut findings);
    slop_rules::check_empty_significance(text, &quotes, &mut findings);
    slop_rules::check_summary_pileup(text, &quotes, &mut findings);
    slop_rules::check_generic_positive_close(text, &quotes, &mut findings);
    slop_rules::check_transition_pileup(text, &quotes, &mut findings);
    slop_rules::check_performative_collaboration(text, &quotes, &mut findings);

    style_rules::check_conversational_residue(text, &quotes, &mut findings);
    style_rules::check_throat_clearing(text, &quotes, &mut findings);
    style_rules::check_canned_transition(text, &quotes, &mut findings);
    style_rules::check_repeated_conclusion(text, &quotes, &mut findings);
    style_rules::check_vague_attribution(text, &quotes, &mut findings);
    style_rules::check_redundant_qualifier(text, &quotes, &mut findings);
    style_rules::check_excessive_exclamation(text, &quotes, &mut findings);
    style_rules::check_excessive_dash_density(text, &quotes, &mut findings);
    style_rules::check_excessive_emoji_density(text, &quotes, &mut findings);

    findings
}

pub(crate) fn quoted_spans(text: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut in_ascii = None;
    let mut in_curly = None;
    for (idx, ch) in text.char_indices() {
        if ch == '"' {
            if let Some(start) = in_ascii.take() {
                spans.push(start..idx + ch.len_utf8());
            } else {
                in_ascii = Some(idx);
            }
        } else if ch == '\u{201C}' {
            in_curly = Some(idx);
        } else if ch == '\u{201D}'
            && let Some(start) = in_curly.take()
        {
            spans.push(start..idx + ch.len_utf8());
        }
    }
    spans.sort_by_key(|span| span.start);
    let mut maximum_end = 0;
    for span in &mut spans {
        maximum_end = maximum_end.max(span.end);
        span.end = maximum_end;
    }
    spans
}

pub(crate) fn is_inside_quotes(start: usize, end: usize, quotes: &[Range<usize>]) -> bool {
    let index = quotes.partition_point(|span| span.start <= start);
    index > 0 && end <= quotes[index - 1].end
}

pub(crate) fn add_finding(
    findings: &mut Vec<EditorialFinding>,
    text: &str,
    rule: RuleCatalog,
    evidence: &str,
    match_start: usize,
    message: &str,
) {
    let occurrence = count_previous_occurrences(text, evidence, match_start);
    findings.push(EditorialFinding::new(
        rule.id(),
        evidence,
        occurrence,
        message,
    ));
}

fn count_previous_occurrences(text: &str, evidence: &str, match_start: usize) -> u16 {
    let mut count = 0_u16;
    let mut offset = 0;
    while let Some(pos) = text[offset..].find(evidence) {
        let abs_pos = offset + pos;
        if abs_pos >= match_start {
            break;
        }
        count = count.saturating_add(1);
        offset = abs_pos + evidence.len();
    }
    count
}
