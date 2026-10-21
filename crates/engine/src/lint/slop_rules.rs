//! Deterministic rule matchers for synthetic AI slop patterns.

use std::ops::Range;

use super::collector::FindingCollector;

use super::rules::{RuleCatalog, add_finding, is_inside_quotes};

pub(crate) fn check_prefabricated_scene_setting(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "In today's rapidly evolving digital landscape";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::PrefabricatedSceneSetting,
            pattern,
            pos,
            "Prefabricated introductory scene-setting cliche",
        );
    }
    super::patterns::scene_variants(text, quotes, findings);
}

pub(crate) fn check_contrastive_reframe(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "not merely a formatting tool; it is a catalyst";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::ContrastiveReframe,
            pattern,
            pos,
            "Artificial contrastive reframing",
        );
    }
    super::patterns::contrastive_variants(text, quotes, findings);
}

pub(crate) fn check_stacked_tricolon(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "fast, flexible, and future-ready. It informs, inspires, and empowers";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::StackedTricolon,
            pattern,
            pos,
            "Stacked tricolons and parallel list accumulation",
        );
    }
    super::patterns::tricolon_variants(text, quotes, findings);
}

pub(crate) fn check_promotional_puffery(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "groundbreaking, game-changing platform unlocks unparalleled productivity";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::PromotionalPuffery,
            pattern,
            pos,
            "Promotional puffery and unsubstantiated superlatives",
        );
    }
    super::patterns::puffery_variants(text, quotes, findings);
}

pub(crate) fn check_ornamental_abstraction(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "delve into the rich tapestry of a multifaceted landscape";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::OrnamentalAbstraction,
            pattern,
            pos,
            "Ornamental metaphorical abstraction",
        );
    }
    super::patterns::ornamental_variants(text, quotes, findings);
}

pub(crate) fn check_reader_segmentation(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "Whether you're a seasoned professional or just starting out";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::ReaderSegmentation,
            pattern,
            pos,
            "Formulaic reader segmentation cliche",
        );
    }
    super::patterns::reader_variants(text, quotes, findings);
}

pub(crate) fn check_performative_question_answer(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "The result? A seamless, robust, and intuitive experience";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::PerformativeQuestionAnswer,
            pattern,
            pos,
            "Performative rhetorical question-and-answer pair",
        );
    }
    super::patterns::question_variants(text, quotes, findings);
}

pub(crate) fn check_empty_significance(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "represents a pivotal step forward and marks a significant milestone";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::EmptySignificance,
            pattern,
            pos,
            "Empty claim of historic significance",
        );
    }
    super::patterns::significance_variants(text, quotes, findings);
}

pub(crate) fn check_summary_pileup(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let markers = ["In summary", "Ultimately", "In conclusion"];
    let mut matches = Vec::new();
    for marker in markers {
        for (occurrence, (abs_pos, _)) in text.match_indices(marker).enumerate() {
            if !is_inside_quotes(abs_pos, abs_pos + marker.len(), quotes) {
                matches.push((
                    abs_pos,
                    marker,
                    u16::try_from(occurrence).unwrap_or(u16::MAX),
                ));
                if matches.len() >= 3 && matches.len() > findings.remaining() {
                    findings.refuse();
                    return;
                }
            }
        }
    }
    if matches.len() >= 3 {
        matches.sort_by_key(|&(pos, _, _)| pos);
        for (position, marker, occurrence) in matches {
            super::rules::push_finding(
                findings,
                text,
                RuleCatalog::SummaryPileup,
                marker,
                position,
                occurrence,
                "Excessive density of summary markers",
            );
        }
    }
}

pub(crate) fn check_generic_positive_close(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "The possibilities are endless, and the future is bright";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::GenericPositiveClose,
            pattern,
            pos,
            "Ungrounded generic optimistic ending",
        );
    }
    super::patterns::close_variants(text, quotes, findings);
}

pub(crate) fn check_transition_pileup(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let markers = ["Moreover", "Furthermore", "Additionally"];
    let mut matches = Vec::new();
    for marker in markers {
        for (occurrence, (abs_pos, _)) in text.match_indices(marker).enumerate() {
            if !is_inside_quotes(abs_pos, abs_pos + marker.len(), quotes) {
                matches.push((
                    abs_pos,
                    marker,
                    u16::try_from(occurrence).unwrap_or(u16::MAX),
                ));
                if matches.len() >= 3 && matches.len() > findings.remaining() {
                    findings.refuse();
                    return;
                }
            }
        }
    }
    if matches.len() >= 3 {
        matches.sort_by_key(|&(pos, _, _)| pos);
        for (position, marker, occurrence) in matches {
            super::rules::push_finding(
                findings,
                text,
                RuleCatalog::TransitionPileup,
                marker,
                position,
                occurrence,
                "Pile-up of sentence-initial transitional adverbs",
            );
        }
    }
}

pub(crate) fn check_performative_collaboration(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let pattern = "Let's embark on this journey together and unlock the power";
    if let Some(pos) = text.find(pattern)
        && !is_inside_quotes(pos, pos + pattern.len(), quotes)
    {
        add_finding(
            findings,
            text,
            RuleCatalog::PerformativeCollaboration,
            pattern,
            pos,
            "Performative invitation to embark on a journey",
        );
    }
    super::patterns::collaboration_variants(text, quotes, findings);
}
