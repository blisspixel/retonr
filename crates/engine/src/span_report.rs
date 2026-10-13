//! Redacted locations for protected, structural, and unsafe failures.

use std::collections::{BTreeMap, VecDeque};

use rewrite_types::{GateEvidenceDetails, SpanChange, SpanFailure, TextPoint};

use crate::protection::{ProtectedKind, ProtectedOccurrence, protected_occurrences};

const MAX_FAILURES: usize = rewrite_types::MAX_SPAN_FAILURES;

pub(crate) fn protection_details(
    source: &str,
    candidate: &str,
    declared_terms: &[String],
) -> Option<GateEvidenceDetails> {
    let source_spans = protected_occurrences(source, declared_terms).ok()?;
    let candidate_spans = protected_occurrences(candidate, declared_terms).ok()?;
    let failures = diff_occurrences(
        source,
        candidate,
        declared_terms,
        &source_spans,
        &candidate_spans,
    );
    details(failures)
}

pub(crate) fn structure_details(source: &str, candidate: &str) -> Option<GateEvidenceDetails> {
    details(structure_failures(source, candidate))
}

pub(crate) fn unsafe_details(source: &str, candidate: &str) -> Option<GateEvidenceDetails> {
    details(unsafe_failures(source, candidate))
}

fn details(failures: Vec<SpanFailure>) -> Option<GateEvidenceDetails> {
    if failures.is_empty() {
        return None;
    }
    let details = GateEvidenceDetails::SpanFailures { failures };
    rewrite_types::GateEvidence::new("span_failures", "redacted span locations", Some(details))
        .ok()
        .and_then(|evidence| evidence.details)
}

#[derive(Clone, Copy)]
struct Side<'a> {
    kind: ProtectedKind,
    surface: &'a str,
    start: usize,
}

fn diff_occurrences<'a>(
    source: &'a str,
    candidate: &'a str,
    declared_terms: &[String],
    source_spans: &[ProtectedOccurrence],
    candidate_spans: &[ProtectedOccurrence],
) -> Vec<SpanFailure> {
    let source_sides = sides(source, source_spans);
    let candidate_sides = sides(candidate, candidate_spans);
    let mut queues: BTreeMap<(ProtectedKind, &str), VecDeque<usize>> = BTreeMap::new();
    for (index, side) in candidate_sides.iter().enumerate() {
        queues
            .entry((side.kind, side.surface))
            .or_default()
            .push_back(index);
    }
    let mut matched = vec![false; candidate_sides.len()];
    let mut unmatched_source = Vec::new();
    for side in &source_sides {
        if let Some(index) = queues
            .get_mut(&(side.kind, side.surface))
            .and_then(VecDeque::pop_front)
        {
            matched[index] = true;
        } else {
            unmatched_source.push(*side);
        }
    }
    let unmatched_candidate = candidate_sides
        .iter()
        .enumerate()
        .filter(|(index, _)| !matched[*index])
        .map(|(_, side)| *side)
        .collect::<Vec<_>>();
    pair_unmatched(
        source,
        candidate,
        declared_terms,
        &unmatched_source,
        &unmatched_candidate,
    )
}

fn sides<'a>(text: &'a str, spans: &[ProtectedOccurrence]) -> Vec<Side<'a>> {
    spans
        .iter()
        .filter_map(|span| {
            text.get(span.start..span.end).map(|surface| Side {
                kind: span.kind,
                surface,
                start: span.start,
            })
        })
        .collect()
}

fn pair_unmatched(
    source: &str,
    candidate: &str,
    declared_terms: &[String],
    unmatched_source: &[Side<'_>],
    unmatched_candidate: &[Side<'_>],
) -> Vec<SpanFailure> {
    let mut failures = Vec::new();
    for kind in [
        ProtectedKind::DeclaredTerm,
        ProtectedKind::Url,
        ProtectedKind::Email,
        ProtectedKind::Number,
    ] {
        let left = unmatched_source.iter().filter(|side| side.kind == kind);
        let mut right = unmatched_candidate.iter().filter(|side| side.kind == kind);
        for source_side in left {
            if failures.len() == MAX_FAILURES {
                return failures;
            }
            let candidate_side = right.next();
            let change = if candidate_side.is_some() {
                SpanChange::Altered
            } else {
                SpanChange::Missing
            };
            push_failure(
                &mut failures,
                source,
                candidate,
                declared_terms,
                kind,
                Some(source_side),
                candidate_side,
                change,
            );
        }
        for candidate_side in right {
            if failures.len() == MAX_FAILURES {
                return failures;
            }
            push_failure(
                &mut failures,
                source,
                candidate,
                declared_terms,
                kind,
                None,
                Some(candidate_side),
                SpanChange::Added,
            );
        }
    }
    failures
}

#[expect(
    clippy::too_many_arguments,
    reason = "each span field stays visible at the two call sites"
)]
fn push_failure(
    failures: &mut Vec<SpanFailure>,
    source: &str,
    candidate: &str,
    declared_terms: &[String],
    kind: ProtectedKind,
    source_side: Option<&Side<'_>>,
    candidate_side: Option<&Side<'_>>,
    change: SpanChange,
) {
    let kind_name = kind_name(kind);
    let ordinal = (kind == ProtectedKind::DeclaredTerm)
        .then(|| {
            let surface = source_side.or(candidate_side).map(|side| side.surface);
            surface.and_then(|surface| {
                declared_terms
                    .iter()
                    .position(|term| term == surface)
                    .and_then(|index| u32::try_from(index.saturating_add(1)).ok())
            })
        })
        .flatten();
    if kind == ProtectedKind::DeclaredTerm && ordinal.is_none() {
        return;
    }
    let failure = SpanFailure {
        kind: kind_name.to_owned(),
        ordinal,
        source: source_side.and_then(|side| point_at(source, side.start)),
        candidate: candidate_side.and_then(|side| point_at(candidate, side.start)),
        change,
    };
    if failure.source.is_some() || failure.candidate.is_some() {
        failures.push(failure);
    }
}

const fn kind_name(kind: ProtectedKind) -> &'static str {
    match kind {
        ProtectedKind::DeclaredTerm => "declared_term",
        ProtectedKind::Url => "url",
        ProtectedKind::Email => "email",
        ProtectedKind::Number => "number",
    }
}

fn structure_failures(source: &str, candidate: &str) -> Vec<SpanFailure> {
    let left = newline_events(source);
    let right = newline_events(candidate);
    let shared = left.len().min(right.len());
    for index in 0..shared {
        if left[index].kind != right[index].kind || left[index].at != right[index].at {
            let change = if left[index].kind == right[index].kind {
                SpanChange::Moved
            } else {
                SpanChange::Altered
            };
            return one_structure(
                source,
                candidate,
                change,
                Some(left[index].at),
                Some(right[index].at),
            );
        }
    }
    if left.len() > right.len() {
        return one_structure(
            source,
            candidate,
            SpanChange::Missing,
            Some(left[shared].at),
            None,
        );
    }
    if right.len() > left.len() {
        return one_structure(
            source,
            candidate,
            SpanChange::Added,
            None,
            Some(right[shared].at),
        );
    }
    first_difference(source, candidate)
        .map(|at| one_structure(source, candidate, SpanChange::Altered, Some(at), Some(at)))
        .unwrap_or_default()
}

fn one_structure(
    source: &str,
    candidate: &str,
    change: SpanChange,
    source_at: Option<usize>,
    candidate_at: Option<usize>,
) -> Vec<SpanFailure> {
    let failure = SpanFailure {
        kind: "structure".to_owned(),
        ordinal: None,
        source: source_at.and_then(|at| point_at(source, at)),
        candidate: candidate_at.and_then(|at| point_at(candidate, at)),
        change,
    };
    if failure.source.is_some() || failure.candidate.is_some() {
        vec![failure]
    } else {
        Vec::new()
    }
}

struct NewlineEvent {
    at: usize,
    kind: u8,
}

fn newline_events(text: &str) -> Vec<NewlineEvent> {
    let bytes = text.as_bytes();
    let mut events = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                events.push(NewlineEvent {
                    at: index,
                    kind: b'C',
                });
                index += 2;
            }
            b'\r' => {
                events.push(NewlineEvent {
                    at: index,
                    kind: b'R',
                });
                index += 1;
            }
            b'\n' => {
                events.push(NewlineEvent {
                    at: index,
                    kind: b'L',
                });
                index += 1;
            }
            _ => index += 1,
        }
    }
    events
}

fn unsafe_failures(source: &str, candidate: &str) -> Vec<SpanFailure> {
    if !source.chars().any(is_unsafe_text_character)
        && let Some(at) = first_unsafe(candidate)
    {
        return one_unsafe(source, candidate, SpanChange::Introduced, None, Some(at));
    }
    first_difference(source, candidate)
        .map(|at| one_unsafe(source, candidate, SpanChange::Altered, Some(at), Some(at)))
        .unwrap_or_default()
}

fn one_unsafe(
    source: &str,
    candidate: &str,
    change: SpanChange,
    source_at: Option<usize>,
    candidate_at: Option<usize>,
) -> Vec<SpanFailure> {
    let failure = SpanFailure {
        kind: "unsafe_text".to_owned(),
        ordinal: None,
        source: source_at.and_then(|at| point_at(source, at)),
        candidate: candidate_at.and_then(|at| point_at(candidate, at)),
        change,
    };
    if failure.source.is_some() || failure.candidate.is_some() {
        vec![failure]
    } else {
        Vec::new()
    }
}

fn first_unsafe(text: &str) -> Option<usize> {
    let mut offset = 0;
    for character in text.chars() {
        if is_unsafe_text_character(character) {
            return Some(offset);
        }
        offset += character.len_utf8();
    }
    None
}

fn first_difference(left: &str, right: &str) -> Option<usize> {
    let mut offset = 0;
    for (left_char, right_char) in left.chars().zip(right.chars()) {
        if left_char != right_char {
            return Some(offset);
        }
        offset += left_char.len_utf8();
    }
    if left.len() == right.len() {
        None
    } else {
        Some(offset.min(left.len()).min(right.len()))
    }
}

fn is_unsafe_text_character(character: char) -> bool {
    let codepoint = u32::from(character);
    (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        || matches!(
            codepoint,
            0x061C | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x206F | 0xFEFF
        )
}

fn point_at(text: &str, byte_index: usize) -> Option<TextPoint> {
    if byte_index > text.len() || !text.is_char_boundary(byte_index) {
        return None;
    }
    let prefix = &text[..byte_index];
    let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count())
        .ok()?
        .checked_add(1)?;
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let column = u32::try_from(byte_index - line_start)
        .ok()?
        .checked_add(1)?;
    Some(TextPoint { line, column })
}
