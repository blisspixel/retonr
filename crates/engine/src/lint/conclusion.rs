//! Repeated conclusions: an opener whose tail repeats an earlier four-word run.
//!
//! A lone closing sentence is not a finding. The evidence runs from the opener
//! through the sentence terminator, which keeps the development corpus phrase.

use std::ops::Range;

use super::collector::FindingCollector;
use super::patterns::accept;
use super::residue::{char_limited_end, match_folded, next_char_len, word_bounded};
use super::rules::{RuleCatalog, add_finding};

const CONCLUSION_OPENERS: [&str; 3] = ["in conclusion", "to conclude", "in closing"];

pub(crate) fn repeated_conclusions(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
) {
    let words = conclusion_words(text);
    let mut index = 0;
    while index < text.len() {
        if let Some((opener_end, sentence_end)) = conclusion_at(text, index)
            && let Some(evidence_end) = repeated_tail(&words, index, opener_end, sentence_end)
            && accept(
                text,
                quotes,
                findings,
                RuleCatalog::RepeatedConclusion,
                index,
                evidence_end,
            )
        {
            add_finding(
                findings,
                text,
                RuleCatalog::RepeatedConclusion,
                &text[index..evidence_end],
                index,
                "Verbatim repetition of an earlier statement as a conclusion",
            );
            index = evidence_end;
            continue;
        }
        index += next_char_len(text, index);
    }
}

struct ConclusionWord {
    start: usize,
    end: usize,
    folded: String,
}

fn conclusion_words(text: &str) -> Vec<ConclusionWord> {
    let mut words = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let Some(character) = text[index..].chars().next() else {
            break;
        };
        if character.is_alphanumeric() {
            let start = index;
            while let Some(next) = text[index..].chars().next()
                && next.is_alphanumeric()
            {
                index += next.len_utf8();
            }
            words.push(ConclusionWord {
                start,
                end: index,
                folded: text[start..index].to_ascii_lowercase(),
            });
            continue;
        }
        index += character.len_utf8();
    }
    words
}

fn conclusion_at(text: &str, index: usize) -> Option<(usize, usize)> {
    if !at_sentence_start(text, index) {
        return None;
    }
    let opener_end = CONCLUSION_OPENERS
        .iter()
        .find_map(|opener| match_folded(text, index, opener))?;
    if !word_bounded(text, index, opener_end) {
        return None;
    }
    let limited = char_limited_end(text, index, 160);
    let sentence_end = text[index..limited]
        .find(['.', '?', '!'])
        .map_or(limited, |relative| index + relative + 1);
    Some((opener_end, sentence_end))
}

fn at_sentence_start(text: &str, index: usize) -> bool {
    let before = text[..index].trim_end_matches([' ', '\t']);
    before.is_empty() || before.ends_with(['.', '?', '!', '\n', '\r'])
}

fn repeated_tail(
    words: &[ConclusionWord],
    opener_start: usize,
    opener_end: usize,
    sentence_end: usize,
) -> Option<usize> {
    let tail: Vec<&str> = words
        .iter()
        .filter(|word| word.start >= opener_end && word.start < sentence_end)
        .map(|word| word.folded.as_str())
        .collect();
    if tail.len() < 4 {
        return None;
    }
    let earlier: Vec<&str> = words
        .iter()
        .filter(|word| word.end <= opener_start)
        .map(|word| word.folded.as_str())
        .collect();
    let repeated = tail
        .windows(4)
        .any(|window| earlier.windows(4).any(|previous| previous == window));
    repeated.then_some(sentence_end)
}
