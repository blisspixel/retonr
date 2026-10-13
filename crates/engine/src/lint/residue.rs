//! Case-folded assistant residue that the exact corpus sentences do not catch.

use std::ops::Range;

use super::collector::FindingCollector;
use super::rules::{RuleCatalog, add_finding, is_inside_quotes};

const EXACT_SENTENCES: &[&str] = &[
    "Certainly!",
    "Let me know if you would like more detail.",
    "It is important to note that",
    "In today's rapidly evolving digital landscape",
    "groundbreaking, game-changing platform unlocks unparalleled productivity",
    "delve into the rich tapestry of a multifaceted landscape",
];

struct FoldedPattern {
    pattern: &'static str,
    rule: RuleCatalog,
    message: &'static str,
    line_start: bool,
    whole_word: bool,
}

pub(crate) fn check_folded_residue(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let covered = exact_covers(text, quotes);
    for pattern in folded_patterns() {
        scan_pattern(text, quotes, &covered, findings, pattern);
    }
    scan_todays_world(text, quotes, &covered, findings);
}

fn folded_patterns() -> &'static [FoldedPattern] {
    &[
        FoldedPattern {
            pattern: "certainly",
            rule: RuleCatalog::ConversationalResidue,
            message: "Conversational residue opening",
            line_start: true,
            whole_word: true,
        },
        FoldedPattern {
            pattern: "sure",
            rule: RuleCatalog::ConversationalResidue,
            message: "Conversational residue opening",
            line_start: true,
            whole_word: true,
        },
        FoldedPattern {
            pattern: "great question",
            rule: RuleCatalog::ConversationalResidue,
            message: "Conversational residue opening",
            line_start: true,
            whole_word: true,
        },
        FoldedPattern {
            pattern: "i hope this helps",
            rule: RuleCatalog::ConversationalResidue,
            message: "Conversational residue closing",
            line_start: false,
            whole_word: false,
        },
        FoldedPattern {
            pattern: "let me know if",
            rule: RuleCatalog::ConversationalResidue,
            message: "Conversational residue closing",
            line_start: false,
            whole_word: false,
        },
        FoldedPattern {
            pattern: "here's a revised version",
            rule: RuleCatalog::ConversationalResidue,
            message: "Conversational residue opening",
            line_start: false,
            whole_word: false,
        },
        FoldedPattern {
            pattern: "here is a revised version",
            rule: RuleCatalog::ConversationalResidue,
            message: "Conversational residue opening",
            line_start: false,
            whole_word: false,
        },
        FoldedPattern {
            pattern: "as an ai language model",
            rule: RuleCatalog::ConversationalResidue,
            message: "Conversational residue opening",
            line_start: false,
            whole_word: false,
        },
        FoldedPattern {
            pattern: "it's important to note",
            rule: RuleCatalog::ThroatClearing,
            message: "Non-substantive throat-clearing preamble",
            line_start: false,
            whole_word: false,
        },
        FoldedPattern {
            pattern: "it is important to note",
            rule: RuleCatalog::ThroatClearing,
            message: "Non-substantive throat-clearing preamble",
            line_start: false,
            whole_word: false,
        },
        FoldedPattern {
            pattern: "delve",
            rule: RuleCatalog::OrnamentalAbstraction,
            message: "Ornamental metaphorical abstraction",
            line_start: false,
            whole_word: true,
        },
        FoldedPattern {
            pattern: "rich tapestry",
            rule: RuleCatalog::OrnamentalAbstraction,
            message: "Ornamental metaphorical abstraction",
            line_start: false,
            whole_word: false,
        },
        FoldedPattern {
            pattern: "game-changer",
            rule: RuleCatalog::PromotionalPuffery,
            message: "Promotional puffery and unsubstantiated superlatives",
            line_start: false,
            whole_word: true,
        },
        FoldedPattern {
            pattern: "game changer",
            rule: RuleCatalog::PromotionalPuffery,
            message: "Promotional puffery and unsubstantiated superlatives",
            line_start: false,
            whole_word: true,
        },
    ]
}

fn scan_pattern(
    text: &str,
    quotes: &[Range<usize>],
    covered: &[(usize, usize)],
    findings: &mut FindingCollector,
    pattern: &FoldedPattern,
) {
    let mut index = 0;
    while index < text.len() {
        if let Some(end) = match_folded(text, index, pattern.pattern)
            && accept_match(
                text,
                quotes,
                covered,
                index,
                end,
                pattern.line_start,
                pattern.whole_word,
            )
        {
            add_finding(
                findings,
                text,
                pattern.rule,
                &text[index..end],
                index,
                pattern.message,
            );
            index = end;
            continue;
        }
        index += next_char_len(text, index);
    }
}

fn scan_todays_world(
    text: &str,
    quotes: &[Range<usize>],
    covered: &[(usize, usize)],
    findings: &mut FindingCollector,
) {
    let mut index = 0;
    while index < text.len() {
        if let Some(phrase_end) = match_folded(text, index, "in today's")
            && let Some(world_end) = world_within(text, phrase_end)
            && accept_match(text, quotes, covered, index, world_end, false, false)
        {
            add_finding(
                findings,
                text,
                RuleCatalog::PrefabricatedSceneSetting,
                &text[index..world_end],
                index,
                "Prefabricated introductory scene-setting cliche",
            );
            index = world_end;
            continue;
        }
        index += next_char_len(text, index);
    }
}

fn world_within(text: &str, from: usize) -> Option<usize> {
    let window_end = char_limited_end(text, from, 80);
    let window = text.get(from..window_end)?;
    let mut offset = 0;
    while offset < window.len() {
        if let Some(end) = match_folded(window, offset, "world") {
            let absolute = from + offset;
            let absolute_end = from + end;
            if word_bounded(text, absolute, absolute_end) {
                return Some(absolute_end);
            }
        }
        offset += next_char_len(window, offset);
    }
    None
}

fn char_limited_end(text: &str, from: usize, limit: usize) -> usize {
    let mut end = from;
    for (seen, character) in text[from..].chars().enumerate() {
        if seen == limit {
            break;
        }
        end += character.len_utf8();
    }
    end
}

fn accept_match(
    text: &str,
    quotes: &[Range<usize>],
    covered: &[(usize, usize)],
    start: usize,
    end: usize,
    line_start: bool,
    whole_word: bool,
) -> bool {
    if is_inside_quotes(start, end, quotes) || overlaps(start, end, covered) {
        return false;
    }
    if line_start && !at_line_start(text, start) {
        return false;
    }
    if whole_word && !word_bounded(text, start, end) {
        return false;
    }
    true
}

fn exact_covers(text: &str, quotes: &[Range<usize>]) -> Vec<(usize, usize)> {
    let mut covered = Vec::new();
    for sentence in EXACT_SENTENCES {
        let mut offset = 0;
        while let Some(position) = text[offset..].find(sentence) {
            let start = offset + position;
            let end = start + sentence.len();
            if !is_inside_quotes(start, end, quotes) {
                covered.push((start, end));
            }
            offset = end;
        }
    }
    covered
}

fn match_folded(text: &str, start: usize, pattern: &str) -> Option<usize> {
    let mut end = start;
    for expected in pattern.chars() {
        let character = text[end..].chars().next()?;
        if fold_char(character) != expected {
            return None;
        }
        end += character.len_utf8();
    }
    Some(end)
}

fn fold_char(character: char) -> char {
    match character {
        '\u{2019}' | '\u{2018}' | '`' | '\u{00B4}' => '\'',
        other => other.to_ascii_lowercase(),
    }
}

fn at_line_start(text: &str, index: usize) -> bool {
    let line_start = text[..index].rfind('\n').map_or(0, |position| position + 1);
    text[line_start..index].chars().all(char::is_whitespace)
}

fn word_bounded(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    before.is_none_or(|character| !character.is_alphanumeric())
        && after.is_none_or(|character| !character.is_alphanumeric())
}

fn overlaps(start: usize, end: usize, covered: &[(usize, usize)]) -> bool {
    covered
        .iter()
        .any(|(covered_start, covered_end)| start < *covered_end && end > *covered_start)
}

fn next_char_len(text: &str, index: usize) -> usize {
    text[index..].chars().next().map_or(1, char::len_utf8)
}
