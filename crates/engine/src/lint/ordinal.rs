//! Exact-evidence ordinals for generalized phrase and window matches.
//!
//! Each exact evidence byte string keeps one running count. A later hit reads
//! that count instead of searching the preceding text again. Quoted and other
//! unemitted matches still advance the count. The count saturates at `u16::MAX`.

use std::collections::HashMap;
use std::ops::Range;

#[cfg(test)]
use std::cell::Cell;

use super::collector::FindingCollector;
use super::patterns::accept;
use super::rules::{RuleCatalog, push_finding};

#[cfg(test)]
thread_local! {
    static PHRASE_ORDINAL_NOTES: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_phrase_ordinal_notes() {
    PHRASE_ORDINAL_NOTES.with(|notes| notes.set(0));
}

#[cfg(test)]
pub(crate) fn phrase_ordinal_notes() -> usize {
    PHRASE_ORDINAL_NOTES.with(Cell::get)
}

#[cfg(test)]
fn record_phrase_ordinal_note() {
    PHRASE_ORDINAL_NOTES.with(|notes| notes.set(notes.get().saturating_add(1)));
}

struct ExactRun {
    count: u16,
    next: usize,
}

pub(super) struct ExactOrdinals {
    runs: HashMap<String, ExactRun>,
}

impl ExactOrdinals {
    pub(super) fn new() -> Self {
        Self {
            runs: HashMap::new(),
        }
    }

    /// Returns the 0-based non-overlapping occurrence of `evidence` at `start`.
    ///
    /// Overlapping starts do not advance the count. That matches a forward
    /// `find` that resumes after each match.
    pub(super) fn note(&mut self, evidence: &str, start: usize) -> u16 {
        #[cfg(test)]
        record_phrase_ordinal_note();
        let run = self
            .runs
            .entry(evidence.to_owned())
            .or_insert(ExactRun { count: 0, next: 0 });
        if start < run.next {
            return run.count;
        }
        let occurrence = run.count;
        run.count = run.count.saturating_add(1);
        run.next = start.saturating_add(evidence.len());
        occurrence
    }
}

pub(super) fn emit_phrases(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
    rule: RuleCatalog,
    message: &str,
    phrases: &[&str],
) {
    let mut ordinals = ExactOrdinals::new();
    for phrase in phrases {
        if findings.exceeded() {
            return;
        }
        let mut index = 0;
        while index < text.len() {
            if let Some(end) = super::residue::match_folded(text, index, phrase) {
                let occurrence = ordinals.note(&text[index..end], index);
                if accept(text, quotes, findings, rule, index, end) {
                    push_finding(
                        findings,
                        text,
                        rule,
                        &text[index..end],
                        index,
                        occurrence,
                        message,
                    );
                    if findings.exceeded() {
                        return;
                    }
                    index = end;
                    continue;
                }
            }
            index += super::residue::next_char_len(text, index);
        }
    }
}

pub(super) fn emit_windows(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
    rule: RuleCatalog,
    message: &str,
    detect: fn(&str, usize) -> Option<(usize, usize)>,
) {
    let mut ordinals = ExactOrdinals::new();
    let mut index = 0;
    while index < text.len() {
        if let Some((start, end)) = detect(text, index) {
            let occurrence = ordinals.note(&text[start..end], start);
            if accept(text, quotes, findings, rule, start, end) {
                push_finding(
                    findings,
                    text,
                    rule,
                    &text[start..end],
                    start,
                    occurrence,
                    message,
                );
                if findings.exceeded() {
                    return;
                }
            }
            index = end;
            continue;
        }
        index += super::residue::next_char_len(text, index);
    }
}

#[derive(Clone, Copy)]
pub(super) struct Rate {
    pub(super) floor: usize,
    pub(super) per_thousand: usize,
}

pub(super) fn emit_density_windows(
    text: &str,
    quotes: &[Range<usize>],
    findings: &mut FindingCollector,
    rule: RuleCatalog,
    message: &str,
    rate: Rate,
    detect: fn(&str, usize) -> Option<(usize, usize)>,
) {
    if findings.exceeded() {
        return;
    }
    let mut ordinals = ExactOrdinals::new();
    let mut hits = Vec::new();
    let mut index = 0;
    while index < text.len() {
        if let Some((start, end)) = detect(text, index) {
            let occurrence = ordinals.note(&text[start..end], start);
            if accept(text, quotes, findings, rule, start, end) {
                hits.push((start, end, occurrence));
            }
            index = end;
            continue;
        }
        index += super::residue::next_char_len(text, index);
    }
    if !super::density::rate_met(
        hits.len(),
        super::density::count_words(text),
        rate.floor,
        rate.per_thousand,
    ) {
        return;
    }
    for (start, end, occurrence) in hits {
        push_finding(
            findings,
            text,
            rule,
            &text[start..end],
            start,
            occurrence,
            message,
        );
        if findings.exceeded() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ExactOrdinals;

    #[test]
    fn exact_evidence_ordinals_saturate_without_a_second_scan() {
        let mut ordinals = ExactOrdinals::new();
        let mut expected = 0_u16;
        for index in 0..70_000 {
            let occurrence = ordinals.note("abcd", index * 4);
            assert_eq!(occurrence, expected);
            expected = expected.saturating_add(1);
        }
        assert_eq!(ordinals.note("abcd", 70_000 * 4), u16::MAX);
        assert_eq!(ordinals.note("abcd", 1), u16::MAX);
    }
}
