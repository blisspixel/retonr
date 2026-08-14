use std::{collections::BTreeMap, sync::LazyLock};

use regex::Regex;

use super::{MAX_PROTECTED_OCCURRENCES, ProtectedKind, ProtectionError};

static URL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s<>\"']+"#).expect("static URL regex must compile"));
static EMAIL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
        .expect("static email regex must compile")
});
static NUMBER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[$€£]?(?:\.\d+|(?:\d{1,3}(?:,\d{3})+|\d+)(?:\.\d+)?)%?")
        .expect("static number regex must compile")
});

const MAX_PROTECTION_MATCH_SPANS: usize = MAX_PROTECTED_OCCURRENCES * 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MatchSpan {
    pub start: usize,
    pub end: usize,
    pub kind: ProtectedKind,
}

pub(super) fn collect_source_spans(
    source: &str,
    declared_terms: &[String],
) -> Result<Vec<MatchSpan>, ProtectionError> {
    let mut spans = Vec::new();
    for term in declared_terms.iter().filter(|term| !term.is_empty()) {
        for (start, value) in source.match_indices(term) {
            add_match_span(
                &mut spans,
                MatchSpan {
                    start,
                    end: start + value.len(),
                    kind: ProtectedKind::DeclaredTerm,
                },
            )?;
        }
    }
    add_typed_spans(&mut spans, source)?;
    Ok(spans)
}

pub(super) fn typed_counts(
    text: &str,
) -> Result<BTreeMap<(ProtectedKind, String), usize>, ProtectionError> {
    let mut counts = BTreeMap::new();
    for span in selected_typed_spans(text)? {
        *counts
            .entry((span.kind, text[span.start..span.end].to_owned()))
            .or_default() += 1;
    }
    Ok(counts)
}

pub(super) fn selected_typed_spans(text: &str) -> Result<Vec<MatchSpan>, ProtectionError> {
    let mut spans = Vec::new();
    add_typed_spans(&mut spans, text)?;
    select_non_overlapping(spans)
}

pub(super) fn select_non_overlapping(
    mut spans: Vec<MatchSpan>,
) -> Result<Vec<MatchSpan>, ProtectionError> {
    spans.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| right.end.cmp(&left.end))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    let mut selected = Vec::new();
    for span in spans {
        if selected
            .last()
            .is_none_or(|previous: &MatchSpan| span.start >= previous.end)
        {
            if selected.len() == MAX_PROTECTED_OCCURRENCES {
                return Err(ProtectionError::ResourceLimit);
            }
            selected.push(span);
        }
    }
    Ok(selected)
}

pub(super) fn is_exact_typed_span(
    spans: &[MatchSpan],
    start: usize,
    end: usize,
    kind: ProtectedKind,
) -> bool {
    spans
        .iter()
        .any(|span| span.start == start && span.end == end && span.kind == kind)
}

fn add_typed_spans(spans: &mut Vec<MatchSpan>, source: &str) -> Result<(), ProtectionError> {
    add_url_matches(spans, source)?;
    add_regex_matches(spans, source, &EMAIL_PATTERN, ProtectedKind::Email)?;
    add_regex_matches(spans, source, &NUMBER_PATTERN, ProtectedKind::Number)?;
    Ok(())
}

fn add_url_matches(spans: &mut Vec<MatchSpan>, source: &str) -> Result<(), ProtectionError> {
    for matched in URL_PATTERN.find_iter(source) {
        let end = trim_url_end(source, matched.start(), matched.end());
        if end <= matched.start() {
            continue;
        }
        let trimmed = &source[matched.start()..end];
        if !URL_PATTERN
            .find(trimmed)
            .is_some_and(|inner| inner.start() == 0 && inner.end() == trimmed.len())
        {
            continue;
        }
        add_match_span(
            spans,
            MatchSpan {
                start: matched.start(),
                end,
                kind: ProtectedKind::Url,
            },
        )?;
    }
    Ok(())
}

fn add_regex_matches(
    spans: &mut Vec<MatchSpan>,
    source: &str,
    pattern: &Regex,
    kind: ProtectedKind,
) -> Result<(), ProtectionError> {
    for matched in pattern.find_iter(source) {
        add_match_span(
            spans,
            MatchSpan {
                start: matched.start(),
                end: matched.end(),
                kind,
            },
        )?;
    }
    Ok(())
}

fn add_match_span(spans: &mut Vec<MatchSpan>, span: MatchSpan) -> Result<(), ProtectionError> {
    if spans.len() == MAX_PROTECTION_MATCH_SPANS {
        return Err(ProtectionError::ResourceLimit);
    }
    spans.push(span);
    Ok(())
}

fn trim_url_end(text: &str, start: usize, mut end: usize) -> usize {
    while end > start {
        let span = &text[start..end];
        let Some(last) = span.chars().next_back() else {
            break;
        };
        let trim = matches!(last, '.' | ',' | ';' | ':' | '!' | '?')
            || (matches!(last, ')' | ']' | '}') && !span_has_opener(span, last));
        if !trim {
            break;
        }
        end -= last.len_utf8();
    }
    end
}

fn span_has_opener(span: &str, closer: char) -> bool {
    let opener = match closer {
        ')' => '(',
        ']' => '[',
        '}' => '{',
        _ => return false,
    };
    let without_closer = &span[..span.len() - closer.len_utf8()];
    without_closer.contains(opener)
}
