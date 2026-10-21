//! Deterministic editorial linting and style verification engine.

use rewrite_types::{EditorialComparison, EditorialFinding};

mod collector;
mod conclusion;
mod density;
#[cfg(test)]
mod held_out;
mod patterns;
mod residue;
mod rules;
mod slop_rules;
mod style_rules;
#[cfg(test)]
mod tests;

pub use rules::{RuleCatalog, apply_all_rules};

/// Complete findings cannot be represented within the requested ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("editorial finding limit exceeded")]
pub struct FindingLimitExceeded;

/// Lints complete text while bounding finding allocation during matching.
///
/// # Errors
///
/// Returns [`FindingLimitExceeded`] rather than partial findings when the
/// complete result exceeds `maximum`. Successful results match [`lint_text`].
pub fn lint_text_bounded(
    text: &str,
    maximum: usize,
) -> Result<Vec<EditorialFinding>, FindingLimitExceeded> {
    rules::apply_bounded_rules(text, maximum)
}

/// Lints text against the comprehensive editorial rule catalog.
#[must_use]
pub fn lint_text(text: &str) -> Vec<EditorialFinding> {
    rules::apply_all_rules(text)
}

/// Lints text with one catalog rule omitted.
///
/// Evaluation uses this to show that a fixture depends on that rule and that
/// every other fixture still matches. The lint command runs the full catalog.
#[must_use]
pub fn lint_text_excluding(text: &str, excluded: RuleCatalog) -> Vec<EditorialFinding> {
    rules::apply_rules_except(text, excluded)
}

/// Evaluates source and candidate text and computes comparative editorial findings.
#[must_use]
pub fn compare_editorial_findings(source: &str, candidate: &str) -> EditorialComparison {
    let source_findings = lint_text(source);
    let candidate_findings = lint_text(candidate);
    EditorialComparison::compute(source_findings, candidate_findings)
}
