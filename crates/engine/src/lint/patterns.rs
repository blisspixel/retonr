//! Bounded variants for families that began as one corpus sentence.
//!
//! Each exact corpus phrase is recorded first. A variant skips any span already
//! claimed by the same rule, so the development evidence stays a single finding.
//! Windows are character counts, not a claim about sentence grammar.

use std::ops::Range;

use super::collector::FindingCollector;
use super::density::{count_words, rate_met};
use super::residue::{char_limited_end, match_folded, next_char_len, word_bounded};
use super::rules::{RuleCatalog, add_finding, is_inside_quotes};

const SCENE_CUES: [&str; 3] = ["landscape", "era", "climate"];
const CONTRASTIVE_OPENERS: [&str; 3] = ["not just", "not merely", "not only"];
const CONTRASTIVE_CLOSERS: [&str; 3] = ["; it is", ", but", "; but"];
const QUESTION_OPENERS: [&str; 3] = ["the result?", "the answer?", "the upshot?"];
const QUESTION_BUZZWORDS: [&str; 4] = ["seamless", "robust", "intuitive", "effortless"];
const PUFFERY: [&str; 6] = [
    "truly groundbreaking",
    "truly unparalleled",
    "incredibly groundbreaking",
    "incredibly unparalleled",
    "extremely groundbreaking",
    "extremely unparalleled",
];

pub(crate) fn scene_variants(text: &str, quotes: &[Range<usize>], findings: &mut FindingCollector) {
    emit_windows(
        text,
        quotes,
        findings,
        RuleCatalog::PrefabricatedSceneSetting,
        "Prefabricated introductory scene-setting cliche",
        scene_at,
    );
}

pub(crate) fn contrastive_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_density_windows(
        text,
        quotes,
        findings,
        RuleCatalog::ContrastiveReframe,
        "Artificial contrastive reframing",
        Rate {
            floor: 1,
            per_thousand: 2,
        },
        contrastive_at,
    );
}

pub(crate) fn tricolon_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_density_windows(
        text,
        quotes,
        findings,
        RuleCatalog::StackedTricolon,
        "Stacked tricolons and parallel list accumulation",
        Rate {
            floor: 2,
            per_thousand: 2,
        },
        tricolon_at,
    );
}

pub(crate) fn puffery_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::PromotionalPuffery,
        "Promotional puffery and unsubstantiated superlatives",
        &PUFFERY,
    );
}

pub(crate) fn ornamental_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::OrnamentalAbstraction,
        "Ornamental metaphorical abstraction",
        &["nestled within", "the fabric of", "a vibrant tapestry"],
    );
}

pub(crate) fn reader_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_windows(
        text,
        quotes,
        findings,
        RuleCatalog::ReaderSegmentation,
        "Formulaic reader segmentation cliche",
        reader_at,
    );
}

pub(crate) fn question_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_windows(
        text,
        quotes,
        findings,
        RuleCatalog::PerformativeQuestionAnswer,
        "Performative rhetorical question-and-answer pair",
        question_at,
    );
}

pub(crate) fn significance_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::EmptySignificance,
        "Empty claim of historic significance",
        &[
            "pivotal moment",
            "significant milestone",
            "marks a new era",
            "historic turning point",
        ],
    );
}

pub(crate) fn close_variants(text: &str, quotes: &[Range<usize>], findings: &mut FindingCollector) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::GenericPositiveClose,
        "Ungrounded generic optimistic ending",
        &[
            "the sky is the limit",
            "the best is yet to come",
            "possibilities are endless",
        ],
    );
}

pub(crate) fn collaboration_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::PerformativeCollaboration,
        "Performative invitation to embark on a journey",
        &["join me on this journey", "embark on this journey"],
    );
}

pub(crate) fn throat_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::ThroatClearing,
        "Non-substantive throat-clearing preamble",
        &[
            "it should be noted that",
            "it is worth noting that",
            "it's worth noting that",
        ],
    );
}

pub(crate) fn canned_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::CannedTransition,
        "Canned conversational transition",
        &["that being said", "with that said"],
    );
}

pub(crate) fn attribution_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::VagueAttribution,
        "Vague or unreferenced attribution formula",
        &["research shows", "studies show", "many believe"],
    );
}

pub(crate) fn qualifier_variants(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    emit_phrases(
        text,
        quotes,
        findings,
        RuleCatalog::RedundantQualifier,
        "Redundant modifying qualifier",
        &[
            "close proximity",
            "past history",
            "advance planning",
            "unexpected surprise",
            "added bonus",
            "basic fundamentals",
        ],
    );
}

fn emit_phrases(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
    rule: RuleCatalog,
    message: &str,
    phrases: &[&str],
) {
    for phrase in phrases {
        let mut index = 0;
        while index < text.len() {
            if let Some(end) = match_folded(text, index, phrase)
                && accept(text, quotes, findings, rule, index, end)
            {
                add_finding(findings, text, rule, &text[index..end], index, message);
                index = end;
                continue;
            }
            index += next_char_len(text, index);
        }
    }
}

fn emit_windows(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
    rule: RuleCatalog,
    message: &str,
    detect: fn(&str, usize) -> Option<(usize, usize)>,
) {
    let mut index = 0;
    while index < text.len() {
        if let Some((start, end)) = detect(text, index)
            && accept(text, quotes, findings, rule, start, end)
        {
            add_finding(findings, text, rule, &text[start..end], start, message);
            index = end;
            continue;
        }
        index += next_char_len(text, index);
    }
}

#[derive(Clone, Copy)]
struct Rate {
    floor: usize,
    per_thousand: usize,
}

fn emit_density_windows(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
    rule: RuleCatalog,
    message: &str,
    rate: Rate,
    detect: fn(&str, usize) -> Option<(usize, usize)>,
) {
    let mut hits = Vec::new();
    let mut index = 0;
    while index < text.len() {
        if let Some((start, end)) = detect(text, index) {
            if accept(text, quotes, findings, rule, start, end) {
                hits.push((start, end));
            }
            index = end;
            continue;
        }
        index += next_char_len(text, index);
    }
    if !rate_met(hits.len(), count_words(text), rate.floor, rate.per_thousand) {
        return;
    }
    for (start, end) in hits {
        add_finding(findings, text, rule, &text[start..end], start, message);
        if findings.exceeded() {
            return;
        }
    }
}

pub(super) fn accept(
    text: &str,
    quotes: &[Range<usize>],
    findings: &FindingCollector,
    rule: RuleCatalog,
    start: usize,
    end: usize,
) -> bool {
    word_bounded(text, start, end)
        && !is_inside_quotes(start, end, quotes)
        && !findings.overlaps_rule(rule.id(), start, end)
}

fn scene_at(text: &str, index: usize) -> Option<(usize, usize)> {
    let opener_end = match_folded(text, index, "in today's")?;
    if !word_bounded(text, index, opener_end) {
        return None;
    }
    let window_end = char_limited_end(text, opener_end, 80);
    let cue_end = SCENE_CUES
        .iter()
        .filter_map(|cue| earliest_word(text, opener_end, window_end, cue))
        .min()?;
    Some((index, cue_end))
}

fn contrastive_at(text: &str, index: usize) -> Option<(usize, usize)> {
    let opener_end = CONTRASTIVE_OPENERS
        .iter()
        .find_map(|opener| match_folded(text, index, opener))?;
    if !word_bounded(text, index, opener_end) {
        return None;
    }
    let window_end = char_limited_end(text, opener_end, 80);
    let closer_end = CONTRASTIVE_CLOSERS
        .iter()
        .filter_map(|closer| earliest_ending(text, opener_end, window_end, closer))
        .min()?;
    Some((index, closer_end))
}

fn reader_at(text: &str, index: usize) -> Option<(usize, usize)> {
    let opener_end = match_folded(text, index, "whether you're")
        .or_else(|| match_folded(text, index, "whether you are"))?;
    if !word_bounded(text, index, opener_end) {
        return None;
    }
    let window_end = char_limited_end(text, opener_end, 90);
    let or_just = earliest_word(text, opener_end, window_end, "or just")?;
    let next = next_word_end(text, or_just)?;
    Some((index, next))
}

fn question_at(text: &str, index: usize) -> Option<(usize, usize)> {
    let opener_end = QUESTION_OPENERS
        .iter()
        .find_map(|opener| match_folded(text, index, opener))?;
    if !word_bounded(text, index, opener_end) {
        return None;
    }
    let window_end = char_limited_end(text, opener_end, 60);
    let buzzword = QUESTION_BUZZWORDS
        .iter()
        .filter_map(|word| earliest_word(text, opener_end, window_end, word))
        .min()?;
    Some((index, buzzword))
}

fn tricolon_at(text: &str, index: usize) -> Option<(usize, usize)> {
    if !at_word_start(text, index) {
        return None;
    }
    let first_end = short_token(text, index)?;
    let second_start = comma_space(text, first_end)?;
    let second_end = short_token(text, second_start)?;
    let third_start = comma_space(text, second_end)?;
    let and_end = match_folded(text, third_start, "and ")?;
    let third_end = short_token(text, and_end)?;
    if !ends_item(text, third_end) {
        return None;
    }
    Some((index, third_end))
}

fn at_word_start(text: &str, index: usize) -> bool {
    text[..index]
        .chars()
        .next_back()
        .is_none_or(|character| !character.is_alphanumeric())
}

fn earliest_ending(text: &str, from: usize, to: usize, phrase: &str) -> Option<usize> {
    let limit = to.min(text.len());
    let mut index = from;
    while index < limit {
        if let Some(end) = match_folded(text, index, phrase)
            && end <= limit
            && text[end..]
                .chars()
                .next()
                .is_none_or(|character| !character.is_alphanumeric())
        {
            return Some(end);
        }
        index += next_char_len(text, index);
    }
    None
}

fn earliest_word(text: &str, from: usize, to: usize, phrase: &str) -> Option<usize> {
    let limit = to.min(text.len());
    let mut index = from;
    while index < limit {
        if let Some(end) = match_folded(text, index, phrase)
            && end <= limit
            && word_bounded(text, index, end)
        {
            return Some(end);
        }
        index += next_char_len(text, index);
    }
    None
}

fn next_word_end(text: &str, from: usize) -> Option<usize> {
    let rest = text.get(from..)?;
    let skipped = rest.len() - rest.trim_start().len();
    let start = from + skipped;
    let mut length = 0_usize;
    for character in text[start..].chars() {
        if !character.is_ascii_alphabetic() {
            break;
        }
        length += character.len_utf8();
        if length > 12 {
            return None;
        }
    }
    if (2..=12).contains(&length) {
        Some(start + length)
    } else {
        None
    }
}

fn short_token(text: &str, start: usize) -> Option<usize> {
    let mut length = 0_usize;
    let mut count = 0_usize;
    for character in text.get(start..)?.chars() {
        if count == 0 && !character.is_ascii_alphabetic() {
            return None;
        }
        if character.is_ascii_alphabetic() || character == '-' || character == '\'' {
            length += character.len_utf8();
            count += 1;
            if count > 24 {
                return None;
            }
            continue;
        }
        break;
    }
    (count >= 3).then_some(start + length)
}

fn comma_space(text: &str, index: usize) -> Option<usize> {
    text.get(index..)?.starts_with(", ").then_some(index + 2)
}

fn ends_item(text: &str, end: usize) -> bool {
    !text[end..]
        .trim_start()
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic())
}
