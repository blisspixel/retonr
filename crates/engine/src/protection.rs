use std::{collections::BTreeMap, sync::LazyLock};

use regex::Regex;
use thiserror::Error;

static URL_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^\s<>\"']+"#).expect("static URL regex must compile"));
static EMAIL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
        .expect("static email regex must compile")
});
static NUMBER_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[$€£]?\d[\d,]*(?:\.\d+)?%?").expect("static number regex must compile")
});
static SENTINEL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{PROTECTED_[A-Z_]+_\d{4}\}\}").expect("static sentinel regex must compile")
});

/// Category assigned to a protected source value.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProtectedKind {
    /// Caller-declared exact term.
    DeclaredTerm,
    /// HTTP or HTTPS URL.
    Url,
    /// Email address.
    Email,
    /// Numeric literal, optionally including currency or percent markers.
    Number,
}

impl ProtectedKind {
    fn token_label(self) -> &'static str {
        match self {
            Self::DeclaredTerm => "TERM",
            Self::Url => "URL",
            Self::Email => "EMAIL",
            Self::Number => "NUMBER",
        }
    }
}

/// Source value replaced by a typed sentinel during generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectedValue {
    /// Engine-issued sentinel token.
    pub token: String,
    /// Exact original surface value.
    pub surface: String,
    /// Value category.
    pub kind: ProtectedKind,
}

/// Immutable protection result for one source unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtectionPlan {
    /// Source text with protected values replaced by sentinels.
    pub masked_source: String,
    /// Protected values in source order.
    pub values: Vec<ProtectedValue>,
}

/// Protected-sentinel planning or restoration failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProtectionError {
    /// Source already contains a token in the engine-reserved sentinel namespace.
    #[error("source contains a reserved protected-sentinel token")]
    ReservedTokenInSource,
    /// A raw candidate changed the number of occurrences of a protected value.
    #[error("candidate changed protected value occurrence count")]
    ProtectedOccurrenceCount,
    /// A masked candidate removed or duplicated an issued sentinel.
    #[error("candidate changed issued sentinel occurrence count")]
    SentinelOccurrenceCount,
    /// A masked candidate introduced a sentinel that was never issued.
    #[error("candidate introduced an unknown sentinel")]
    UnknownSentinel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MatchSpan {
    start: usize,
    end: usize,
    kind: ProtectedKind,
}

impl ProtectionPlan {
    /// Extracts declared terms and typed literals, then masks non-overlapping spans.
    ///
    /// # Errors
    ///
    /// Returns [`ProtectionError`] if source text already uses the reserved token
    /// namespace.
    pub fn build(source: &str, declared_terms: &[String]) -> Result<Self, ProtectionError> {
        if SENTINEL_PATTERN.is_match(source) {
            return Err(ProtectionError::ReservedTokenInSource);
        }

        let mut spans = Vec::new();
        for term in declared_terms.iter().filter(|term| !term.is_empty()) {
            spans.extend(source.match_indices(term).map(|(start, value)| MatchSpan {
                start,
                end: start + value.len(),
                kind: ProtectedKind::DeclaredTerm,
            }));
        }
        add_regex_matches(&mut spans, source, &URL_PATTERN, ProtectedKind::Url);
        add_regex_matches(&mut spans, source, &EMAIL_PATTERN, ProtectedKind::Email);
        add_regex_matches(&mut spans, source, &NUMBER_PATTERN, ProtectedKind::Number);

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
                selected.push(span);
            }
        }

        let mut masked = String::with_capacity(source.len());
        let mut values = Vec::with_capacity(selected.len());
        let mut cursor = 0;
        for (ordinal, span) in selected.into_iter().enumerate() {
            masked.push_str(&source[cursor..span.start]);
            let token = format!(
                "{{{{PROTECTED_{}_{:04}}}}}",
                span.kind.token_label(),
                ordinal + 1
            );
            masked.push_str(&token);
            values.push(ProtectedValue {
                token,
                surface: source[span.start..span.end].to_owned(),
                kind: span.kind,
            });
            cursor = span.end;
        }
        masked.push_str(&source[cursor..]);

        Ok(Self {
            masked_source: masked,
            values,
        })
    }

    /// Converts restored candidate text to the issued sentinel representation.
    ///
    /// # Errors
    ///
    /// Returns [`ProtectionError`] if any protected surface occurs a different
    /// number of times than it did in the source.
    pub fn mask_raw_candidate(&self, candidate: &str) -> Result<String, ProtectionError> {
        if SENTINEL_PATTERN.is_match(candidate) {
            return Err(ProtectionError::UnknownSentinel);
        }
        let mut grouped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for value in &self.values {
            grouped
                .entry(value.surface.as_str())
                .or_default()
                .push(value.token.as_str());
        }

        let mut replacements = Vec::new();
        for (surface, tokens) in grouped {
            let matches: Vec<usize> = candidate
                .match_indices(surface)
                .map(|(index, _)| index)
                .collect();
            if matches.len() != tokens.len() {
                return Err(ProtectionError::ProtectedOccurrenceCount);
            }
            for (index, token) in matches.into_iter().zip(tokens) {
                replacements.push((index, index + surface.len(), token));
            }
        }
        replacements.sort_by_key(|(start, end, _token)| (*start, *end));
        if replacements.windows(2).any(|items| items[0].1 > items[1].0) {
            return Err(ProtectionError::ProtectedOccurrenceCount);
        }

        let mut masked = candidate.to_owned();
        for (start, end, token) in replacements.into_iter().rev() {
            masked.replace_range(start..end, token);
        }
        self.validate_masked(&masked)?;
        Ok(masked)
    }

    /// Validates that every issued sentinel appears exactly once and no unknown
    /// sentinel was introduced.
    ///
    /// # Errors
    ///
    /// Returns [`ProtectionError`] for missing, duplicated, or unknown tokens.
    pub fn validate_masked(&self, candidate: &str) -> Result<(), ProtectionError> {
        for value in &self.values {
            if candidate.matches(&value.token).count() != 1 {
                return Err(ProtectionError::SentinelOccurrenceCount);
            }
        }
        for matched in SENTINEL_PATTERN.find_iter(candidate) {
            if !self
                .values
                .iter()
                .any(|value| value.token == matched.as_str())
            {
                return Err(ProtectionError::UnknownSentinel);
            }
        }
        Ok(())
    }

    /// Restores every validated sentinel to its exact source surface.
    ///
    /// # Errors
    ///
    /// Returns [`ProtectionError`] if sentinel integrity is invalid.
    pub fn restore(&self, candidate: &str) -> Result<String, ProtectionError> {
        self.validate_masked(candidate)?;
        let mut restored = candidate.to_owned();
        for value in &self.values {
            restored = restored.replace(&value.token, &value.surface);
        }
        Ok(restored)
    }
}

fn add_regex_matches(
    spans: &mut Vec<MatchSpan>,
    source: &str,
    pattern: &Regex,
    kind: ProtectedKind,
) {
    spans.extend(pattern.find_iter(source).map(|matched| MatchSpan {
        start: matched.start(),
        end: matched.end(),
        kind,
    }));
}

#[cfg(test)]
mod tests {
    use super::{ProtectedKind, ProtectionError, ProtectionPlan};

    #[test]
    fn masks_typed_literals_and_declared_terms() {
        let plan = ProtectionPlan::build(
            "Email Ada at ada@example.com before https://example.com and pay $12.50.",
            &["Ada".to_owned()],
        )
        .expect("fixture has no reserved token");
        assert_eq!(plan.values.len(), 4);
        assert_eq!(plan.values[0].kind, ProtectedKind::DeclaredTerm);
        assert!(plan.masked_source.contains("{{PROTECTED_EMAIL_0002}}"));
        assert!(plan.masked_source.contains("{{PROTECTED_URL_0003}}"));
        assert!(plan.masked_source.contains("{{PROTECTED_NUMBER_0004}}"));
    }

    #[test]
    fn raw_candidate_round_trips_exact_values() {
        let source = "Version 2 costs $10 at https://example.com.";
        let plan = ProtectionPlan::build(source, &[]).expect("valid fixture");
        let raw = "Version 2 costs $10. Visit https://example.com.";
        let masked = plan.mask_raw_candidate(raw).expect("same literal counts");
        assert_eq!(plan.restore(&masked).expect("issued sentinels"), raw);
    }

    #[test]
    fn rejects_changed_or_unknown_values() {
        let plan = ProtectionPlan::build("Version 2", &[]).expect("valid fixture");
        assert_eq!(
            plan.mask_raw_candidate("Version 3"),
            Err(ProtectionError::ProtectedOccurrenceCount)
        );
        assert_eq!(
            plan.validate_masked("Version {{PROTECTED_NUMBER_9999}}"),
            Err(ProtectionError::SentinelOccurrenceCount)
        );
    }

    #[test]
    fn rejects_reserved_source_tokens() {
        assert_eq!(
            ProtectionPlan::build("{{PROTECTED_URL_0001}}", &[]),
            Err(ProtectionError::ReservedTokenInSource)
        );
    }
}
