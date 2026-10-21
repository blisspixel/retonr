//! Per-1,000-word density for marks that are ordinary once and a tell when repeated.
//!
//! A document under 1,000 words uses the absolute floor, so the existing short
//! fixtures stay put. A longer document reports a mark only when its count per
//! 1,000 words reaches the rate. Emoji matching is a bounded block list, not
//! the Unicode emoji property: skin-tone modifiers, variation selectors, and
//! joiners are not separate marks.

use std::collections::BTreeMap;
use std::ops::Range;

use super::collector::FindingCollector;
use super::rules::{RuleCatalog, is_inside_quotes};

const SHORT_DOCUMENT_WORDS: usize = 1_000;
const MARK_FLOOR: usize = 3;
const MARK_PER_THOUSAND: usize = 3;

pub(crate) fn dash_density(text: &str, quotes: &[Range<usize>], findings: &mut FindingCollector) {
    emit_scanned(
        text,
        quotes,
        findings,
        RuleCatalog::ExcessiveDashDensity,
        "Excessive dash density",
        scan_dashes,
    );
}

pub(crate) fn emoji_density(text: &str, quotes: &[Range<usize>], findings: &mut FindingCollector) {
    emit_scanned(
        text,
        quotes,
        findings,
        RuleCatalog::ExcessiveEmojiDensity,
        "Excessive emoji density",
        scan_emoji,
    );
}

pub(crate) fn rate_met(unquoted: usize, words: usize, floor: usize, per_thousand: usize) -> bool {
    if unquoted < floor {
        return false;
    }
    words < SHORT_DOCUMENT_WORDS || unquoted.saturating_mul(1_000) / words >= per_thousand
}

fn emit_scanned(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
    rule: RuleCatalog,
    message: &str,
    scan: fn(&str, &mut dyn FnMut(usize, usize) -> bool),
) {
    let words = count_words(text);
    if words < SHORT_DOCUMENT_WORDS {
        let mut pending = Vec::new();
        let mut eligible = 0_usize;
        let mut seen = BTreeMap::<String, u16>::new();
        scan(text, &mut |start, end| {
            let Some(evidence) = text.get(start..end) else {
                return true;
            };
            let occurrence = note_seen(&mut seen, evidence);
            if is_inside_quotes(start, end, quotes) {
                return true;
            }
            eligible += 1;
            let finding = spanned(findings, text, rule, evidence, start, occurrence, message);
            if eligible < MARK_FLOOR {
                pending.push(finding);
                return true;
            }
            for previous in pending.drain(..) {
                findings.push(previous);
            }
            findings.push(finding);
            !findings.exceeded()
        });
        return;
    }
    let mut eligible = 0_usize;
    scan(text, &mut |start, end| {
        if !is_inside_quotes(start, end, quotes) {
            eligible += 1;
        }
        true
    });
    if !rate_met(eligible, words, MARK_FLOOR, MARK_PER_THOUSAND) {
        return;
    }
    let mut seen = BTreeMap::<String, u16>::new();
    scan(text, &mut |start, end| {
        let Some(evidence) = text.get(start..end) else {
            return true;
        };
        let occurrence = note_seen(&mut seen, evidence);
        if is_inside_quotes(start, end, quotes) {
            return true;
        }
        let finding = spanned(findings, text, rule, evidence, start, occurrence, message);
        findings.push(finding);
        !findings.exceeded()
    });
}

fn spanned(
    findings: &FindingCollector,
    text: &str,
    rule: RuleCatalog,
    evidence: &str,
    start: usize,
    occurrence: u16,
    message: &str,
) -> rewrite_types::EditorialFinding {
    let mut finding =
        rewrite_types::EditorialFinding::new(rule.id(), evidence, occurrence, message);
    finding.span = findings.locate(text, start, start.saturating_add(evidence.len()));
    finding
}

fn note_seen(seen: &mut BTreeMap<String, u16>, evidence: &str) -> u16 {
    let occurrence = seen.get(evidence).copied().unwrap_or(0);
    seen.insert(evidence.to_owned(), occurrence.saturating_add(1));
    occurrence
}

pub(crate) fn count_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| token.chars().any(char::is_alphanumeric))
        .count()
}

fn scan_dashes(text: &str, visit: &mut dyn FnMut(usize, usize) -> bool) {
    let mut index = 0;
    while index < text.len() {
        let rest = &text[index..];
        let end = if rest.starts_with('\u{2014}') {
            index + '\u{2014}'.len_utf8()
        } else if rest.starts_with('\u{2013}') {
            index + '\u{2013}'.len_utf8()
        } else if rest.starts_with(" -- ") {
            index + 4
        } else {
            index += super::residue::next_char_len(text, index);
            continue;
        };
        if !visit(index, end) {
            return;
        }
        index = end;
    }
}

fn scan_emoji(text: &str, visit: &mut dyn FnMut(usize, usize) -> bool) {
    for (index, character) in text.char_indices() {
        if !is_density_emoji(character) {
            continue;
        }
        if !visit(index, index + character.len_utf8()) {
            return;
        }
    }
}

fn is_density_emoji(character: char) -> bool {
    if matches!(character, '\u{1F3FB}'..='\u{1F3FF}') {
        return false;
    }
    matches!(
        character,
        '\u{2705}'
            | '\u{1F300}'..='\u{1F5FF}'
            | '\u{1F600}'..='\u{1F64F}'
            | '\u{1F680}'..='\u{1F6FF}'
            | '\u{1F900}'..='\u{1F9FF}'
            | '\u{1FA70}'..='\u{1FAFF}'
    )
}
