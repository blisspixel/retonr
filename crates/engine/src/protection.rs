use std::{
    collections::{BTreeMap, HashMap},
    sync::LazyLock,
};

use aho_corasick::{AhoCorasickBuilder, MatchKind};
use regex::Regex;
use thiserror::Error;

mod extract;

use extract::{
    collect_source_spans, is_exact_typed_span, select_non_overlapping, selected_typed_spans,
    typed_counts,
};

static SENTINEL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\{\{PROTECTED_[A-Z_]+_\d{4}\}\}").expect("static sentinel regex must compile")
});

/// Maximum protected values extracted from one rewrite unit.
pub const MAX_PROTECTED_OCCURRENCES: usize = 4_096;
/// Maximum UTF-8 bytes accepted or produced by protection processing.
pub const MAX_PROTECTED_TEXT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum exact terms accepted in one rewrite policy.
pub const MAX_PROTECTED_TERMS: usize = 32;
/// Maximum UTF-8 byte length of one protected term.
pub const MAX_PROTECTED_TERM_BYTES: usize = 256;
/// Maximum combined UTF-8 bytes across protected terms.
pub const MAX_PROTECTED_TERM_TOTAL_BYTES: usize = 2 * 1024;

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
    masked_source: String,
    /// Protected values in source order.
    values: Vec<ProtectedValue>,
    /// Independent typed-literal multiset extracted from the source.
    typed_counts: BTreeMap<(ProtectedKind, String), usize>,
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
    /// The bounded multi-pattern matcher could not be constructed.
    #[error("protected-value matcher could not be constructed")]
    MatcherBuild,
    /// Protection processing exceeded its occurrence, match, or output limit.
    #[error("protected-value resource limit exceeded")]
    ResourceLimit,
    /// Declared terms violated count, byte, uniqueness, or text-safety limits.
    #[error("declared protected terms violate the bounded policy")]
    InvalidDeclaredTerms,
    /// Selected protected surfaces cannot map uniquely to their source occurrences.
    #[error("protected source surfaces have an ambiguous occurrence mapping")]
    AmbiguousSurfaceMapping,
}

impl ProtectionPlan {
    /// Returns the source text with protected values replaced by sentinels.
    #[must_use]
    pub fn masked_source(&self) -> &str {
        &self.masked_source
    }

    /// Returns the validated protected values in source order.
    #[must_use]
    pub fn values(&self) -> &[ProtectedValue] {
        &self.values
    }

    /// Consumes the validated plan into its masked source and protected values.
    #[must_use]
    pub fn into_parts(self) -> (String, Vec<ProtectedValue>) {
        (self.masked_source, self.values)
    }

    /// Extracts declared terms and typed literals, then masks non-overlapping spans.
    ///
    /// # Errors
    ///
    /// Returns [`ProtectionError`] if source text already uses the reserved token
    /// namespace.
    pub fn build(source: &str, declared_terms: &[String]) -> Result<Self, ProtectionError> {
        if source.len() > MAX_PROTECTED_TEXT_BYTES {
            return Err(ProtectionError::ResourceLimit);
        }
        if !protected_terms_are_valid(declared_terms) {
            return Err(ProtectionError::InvalidDeclaredTerms);
        }
        if SENTINEL_PATTERN.is_match(source) {
            return Err(ProtectionError::ReservedTokenInSource);
        }

        let selected = select_non_overlapping(collect_source_spans(source, declared_terms)?)?;
        let typed_counts = typed_counts(source)?;

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
        if masked.len() > MAX_PROTECTED_TEXT_BYTES {
            return Err(ProtectionError::ResourceLimit);
        }

        let plan = Self {
            masked_source: masked,
            values,
            typed_counts,
        };
        match plan.mask_raw_candidate(source) {
            Ok(remasked) if remasked == plan.masked_source => Ok(plan),
            Ok(_) | Err(ProtectionError::ProtectedOccurrenceCount) => {
                Err(ProtectionError::AmbiguousSurfaceMapping)
            }
            Err(error) => Err(error),
        }
    }

    /// Converts restored candidate text to the issued sentinel representation.
    ///
    /// # Errors
    ///
    /// Returns [`ProtectionError`] if any protected surface occurs a different
    /// number of times than it did in the source.
    pub fn mask_raw_candidate(&self, candidate: &str) -> Result<String, ProtectionError> {
        if candidate.len() > MAX_PROTECTED_TEXT_BYTES
            || self.values.len() > MAX_PROTECTED_OCCURRENCES
        {
            return Err(ProtectionError::ResourceLimit);
        }
        if SENTINEL_PATTERN.is_match(candidate) {
            return Err(ProtectionError::UnknownSentinel);
        }
        let typed_spans = selected_typed_spans(candidate)?;
        let mut candidate_counts = BTreeMap::new();
        for span in &typed_spans {
            *candidate_counts
                .entry((span.kind, candidate[span.start..span.end].to_owned()))
                .or_default() += 1;
        }
        if candidate_counts != self.typed_counts {
            return Err(ProtectionError::ProtectedOccurrenceCount);
        }
        let mut grouped: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for value in &self.values {
            grouped
                .entry(value.surface.as_str())
                .or_default()
                .push(value.token.as_str());
        }
        if grouped.is_empty() {
            return Ok(candidate.to_owned());
        }

        let patterns: Vec<&str> = grouped.keys().copied().collect();
        let matcher = AhoCorasickBuilder::new()
            .match_kind(MatchKind::LeftmostLongest)
            .build(&patterns)
            .map_err(|_error| ProtectionError::MatcherBuild)?;
        let mut matched_counts = vec![0_usize; patterns.len()];
        let mut replacements = Vec::with_capacity(self.values.len());
        for matched in matcher.find_iter(candidate) {
            let pattern_index = matched.pattern().as_usize();
            let Some(surface) = patterns.get(pattern_index).copied() else {
                return Err(ProtectionError::MatcherBuild);
            };
            let Some(tokens) = grouped.get(surface) else {
                return Err(ProtectionError::MatcherBuild);
            };
            let Some(matched_count) = matched_counts.get_mut(pattern_index) else {
                return Err(ProtectionError::MatcherBuild);
            };
            let token_index = *matched_count;
            let Some(token) = tokens.get(token_index) else {
                return Err(ProtectionError::ProtectedOccurrenceCount);
            };
            let kind = self
                .values
                .iter()
                .find(|value| value.token == *token)
                .map(|value| value.kind)
                .ok_or(ProtectionError::MatcherBuild)?;
            if kind != ProtectedKind::DeclaredTerm
                && !is_exact_typed_span(&typed_spans, matched.start(), matched.end(), kind)
            {
                return Err(ProtectionError::ProtectedOccurrenceCount);
            }
            replacements.push((matched.start(), matched.end(), *token));
            *matched_count += 1;
        }
        if patterns.iter().zip(matched_counts).any(|(surface, count)| {
            grouped
                .get(surface)
                .is_none_or(|tokens| tokens.len() != count)
        }) {
            return Err(ProtectionError::ProtectedOccurrenceCount);
        }

        let masked_len =
            replacements
                .iter()
                .try_fold(candidate.len(), |length, (start, end, token)| {
                    length
                        .checked_sub(end - start)
                        .and_then(|shorter| shorter.checked_add(token.len()))
                });
        let Some(masked_len) = masked_len.filter(|length| *length <= MAX_PROTECTED_TEXT_BYTES)
        else {
            return Err(ProtectionError::ResourceLimit);
        };
        let mut masked = String::with_capacity(masked_len);
        let mut cursor = 0;
        for (start, end, token) in replacements {
            masked.push_str(&candidate[cursor..start]);
            masked.push_str(token);
            cursor = end;
        }
        masked.push_str(&candidate[cursor..]);
        Ok(masked)
    }

    /// Validates that every issued sentinel appears exactly once and no unknown
    /// sentinel was introduced.
    ///
    /// # Errors
    ///
    /// Returns [`ProtectionError`] for missing, duplicated, or unknown tokens.
    pub fn validate_masked(&self, candidate: &str) -> Result<(), ProtectionError> {
        self.indexed_sentinel_matches(candidate).map(|_matches| ())
    }

    /// Restores every validated sentinel to its exact source surface.
    ///
    /// # Errors
    ///
    /// Returns [`ProtectionError`] if sentinel integrity is invalid.
    pub fn restore(&self, candidate: &str) -> Result<String, ProtectionError> {
        let matches = self.indexed_sentinel_matches(candidate)?;
        let restored_len =
            matches
                .iter()
                .try_fold(candidate.len(), |length, (start, end, value_index)| {
                    length.checked_sub(end - start).and_then(|shorter| {
                        shorter.checked_add(self.values[*value_index].surface.len())
                    })
                });
        let Some(restored_len) = restored_len.filter(|length| *length <= MAX_PROTECTED_TEXT_BYTES)
        else {
            return Err(ProtectionError::ResourceLimit);
        };
        let mut restored = String::with_capacity(restored_len);
        let mut cursor = 0;
        for (start, end, value_index) in matches {
            restored.push_str(&candidate[cursor..start]);
            restored.push_str(&self.values[value_index].surface);
            cursor = end;
        }
        restored.push_str(&candidate[cursor..]);
        Ok(restored)
    }

    fn indexed_sentinel_matches(
        &self,
        candidate: &str,
    ) -> Result<Vec<(usize, usize, usize)>, ProtectionError> {
        if candidate.len() > MAX_PROTECTED_TEXT_BYTES
            || self.values.len() > MAX_PROTECTED_OCCURRENCES
        {
            return Err(ProtectionError::ResourceLimit);
        }
        let mut issued = HashMap::with_capacity(self.values.len());
        for (index, value) in self.values.iter().enumerate() {
            if issued.insert(value.token.as_str(), index).is_some() {
                return Err(ProtectionError::SentinelOccurrenceCount);
            }
        }
        let mut counts = vec![0_u8; self.values.len()];
        let mut matches = Vec::with_capacity(self.values.len());
        for matched in SENTINEL_PATTERN.find_iter(candidate) {
            let Some(value_index) = issued.get(matched.as_str()).copied() else {
                return Err(ProtectionError::UnknownSentinel);
            };
            let Some(count) = counts.get_mut(value_index) else {
                return Err(ProtectionError::MatcherBuild);
            };
            if *count != 0 {
                return Err(ProtectionError::SentinelOccurrenceCount);
            }
            *count = 1;
            matches.push((matched.start(), matched.end(), value_index));
        }
        if counts.into_iter().any(|count| count != 1) {
            return Err(ProtectionError::SentinelOccurrenceCount);
        }
        Ok(matches)
    }
}

pub(crate) fn protected_terms_are_valid(terms: &[String]) -> bool {
    if terms.len() > MAX_PROTECTED_TERMS {
        return false;
    }
    let mut total = 0_usize;
    let mut unique = std::collections::BTreeSet::new();
    for term in terms {
        if term.is_empty()
            || term.len() > MAX_PROTECTED_TERM_BYTES
            || term.chars().any(is_disallowed_policy_character)
            || !unique.insert(term.as_str())
        {
            return false;
        }
        let Some(next_total) = total.checked_add(term.len()) else {
            return false;
        };
        total = next_total;
    }
    total <= MAX_PROTECTED_TERM_TOTAL_BYTES
}

fn is_disallowed_policy_character(character: char) -> bool {
    let codepoint = u32::from(character);
    character.is_control()
        || matches!(
            codepoint,
            0x061C
                | 0x200B..=0x200F
                | 0x2028..=0x202E
                | 0x2060..=0x206F
                | 0xFEFF
        )
}
