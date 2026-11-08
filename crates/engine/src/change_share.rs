//! Word-occurrence change share for a source and a candidate.
//!
//! The share compares word occurrences. It does not treat a pure reordering as
//! a change. Alphanumeric fidelity gates still reject a token-sequence change.
//! Whitespace separates words and is not part of the measured text. Rounding is
//! half up, and both percentages saturate at 100.

use std::collections::HashMap;

use rewrite_types::{GateResult, ReasonCode, RewriteOptions, RewriteUnit};

/// Reported share of words and word characters that differ.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChangeShare {
    /// Rounded percentage of word occurrences that differ.
    pub changed_token_pct: u8,
    /// Rounded percentage of characters inside those words that differ.
    pub changed_char_pct: u8,
    /// Word occurrences that differ, taking the larger of the two sides.
    ///
    /// A substitution counts once. A pure reordering counts as zero.
    pub changed_word_count: u64,
}

/// Measures the word-occurrence share of `candidate` against `source`.
#[must_use]
pub fn change_share(source: &str, candidate: &str) -> ChangeShare {
    let source_counts = word_counts(source);
    let candidate_counts = word_counts(candidate);
    let mut removed_tokens = 0_u64;
    let mut added_tokens = 0_u64;
    let mut removed_chars = 0_u64;
    let mut added_chars = 0_u64;
    for (word, count) in &source_counts {
        let count = *count;
        let shared = count.min(count_of(&candidate_counts, word));
        let removed = count.saturating_sub(shared);
        removed_tokens = removed_tokens.saturating_add(removed);
        removed_chars = removed_chars.saturating_add(removed.saturating_mul(char_len(word)));
    }
    for (word, count) in &candidate_counts {
        let count = *count;
        let shared = count.min(count_of(&source_counts, word));
        let added = count.saturating_sub(shared);
        added_tokens = added_tokens.saturating_add(added);
        added_chars = added_chars.saturating_add(added.saturating_mul(char_len(word)));
    }
    ChangeShare {
        changed_token_pct: rounded_percent(
            removed_tokens.saturating_add(added_tokens),
            total_count(&source_counts).saturating_add(total_count(&candidate_counts)),
        ),
        changed_char_pct: rounded_percent(
            removed_chars.saturating_add(added_chars),
            total_chars(&source_counts).saturating_add(total_chars(&candidate_counts)),
        ),
        changed_word_count: removed_tokens.max(added_tokens),
    }
}

pub(super) fn validate_change_budget(
    unit: &RewriteUnit,
    restored: &str,
    options: &RewriteOptions,
) -> Option<(GateResult, ReasonCode)> {
    let budget = options.change_budget?;
    let share = change_share(&unit.text, restored);
    if share.changed_token_pct > budget.max_change_percent
        || share.changed_char_pct > budget.max_change_percent
    {
        return Some((
            GateResult::fail(
                "change_share",
                "change_budget_exceeded",
                "candidate change share exceeds the requested percentage",
            ),
            ReasonCode::ChangeBudgetExceeded,
        ));
    }
    Some((
        GateResult::pass("change_share"),
        ReasonCode::ChangeBudgetExceeded,
    ))
}

fn word_counts(text: &str) -> HashMap<&str, u64> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word).or_insert(0) += 1;
    }
    counts
}

fn count_of(counts: &HashMap<&str, u64>, word: &str) -> u64 {
    counts.get(word).copied().unwrap_or(0)
}

fn total_count(counts: &HashMap<&str, u64>) -> u64 {
    counts.values().copied().fold(0, u64::saturating_add)
}

fn total_chars(counts: &HashMap<&str, u64>) -> u64 {
    counts.iter().fold(0, |total, (word, count)| {
        total.saturating_add(count.saturating_mul(char_len(word)))
    })
}

fn char_len(word: &str) -> u64 {
    u64::try_from(word.chars().count()).unwrap_or(u64::MAX)
}

fn rounded_percent(numerator: u64, denominator: u64) -> u8 {
    if denominator == 0 || numerator == 0 {
        return 0;
    }
    let rounded = numerator
        .saturating_mul(100)
        .saturating_add(denominator / 2)
        / denominator;
    u8::try_from(rounded).unwrap_or(100).min(100)
}

#[cfg(test)]
mod tests {
    use super::change_share;

    fn case_edit(changes: usize) -> (String, String) {
        let mut source = String::new();
        let mut candidate = String::new();
        for index in 0..100 {
            if index > 0 {
                source.push(' ');
                candidate.push(' ');
            }
            source.push_str("word");
            if index < changes {
                candidate.push_str("Word");
            } else {
                candidate.push_str("word");
            }
        }
        (source, candidate)
    }

    #[test]
    fn fixtures_at_zero_two_and_ten_percent_are_exact() {
        for (changes, percent) in [(0, 0), (2, 2), (10, 10)] {
            let (source, candidate) = case_edit(changes);
            let share = change_share(&source, &candidate);
            assert_eq!(share.changed_token_pct, percent, "{changes} tokens");
            assert_eq!(share.changed_char_pct, percent, "{changes} chars");
            assert_eq!(
                share.changed_word_count,
                u64::try_from(changes).unwrap_or(u64::MAX)
            );
        }
    }

    #[test]
    fn empty_text_and_pure_reordering_measure_as_no_change() {
        assert_eq!(change_share("", "").changed_token_pct, 0);
        assert_eq!(change_share("", "").changed_char_pct, 0);
        assert_eq!(change_share("", "word").changed_token_pct, 100);
        let reordered = change_share("alpha beta", "beta alpha");
        assert_eq!(reordered.changed_token_pct, 0);
        assert_eq!(reordered.changed_char_pct, 0);
    }
}
