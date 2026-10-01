//! Application service for deterministic editorial quality and anti-slop inspection.

use rewrite_engine::lint;
use rewrite_types::{EditorialComparison, EditorialFinding};

/// Service for deterministic editorial quality and anti-slop inspection.
#[derive(Clone, Copy, Debug, Default)]
pub struct EditorialLintService;

impl EditorialLintService {
    /// Lints complete text with a finding ceiling enforced during matching.
    ///
    /// # Errors
    ///
    /// Returns the finding-limit error instead of partial editorial evidence.
    pub fn lint_bounded(
        text: &str,
        maximum: usize,
    ) -> Result<Vec<EditorialFinding>, lint::FindingLimitExceeded> {
        lint::lint_text_bounded(text, maximum)
    }

    /// Compares complete inputs under one combined finding-allocation ceiling.
    ///
    /// # Errors
    ///
    /// Refuses excessive findings before constructing a comparison.
    pub fn compare_bounded(
        source: &str,
        candidate: &str,
        maximum: usize,
    ) -> Result<EditorialComparison, lint::FindingLimitExceeded> {
        let source_findings = lint::lint_text_bounded(source, maximum)?;
        let candidate_findings =
            lint::lint_text_bounded(candidate, maximum.saturating_sub(source_findings.len()))?;
        Ok(EditorialComparison::compute(
            source_findings,
            candidate_findings,
        ))
    }
    /// Lints plain text against the deterministic editorial rule catalog.
    #[must_use]
    pub fn lint(text: &str) -> Vec<EditorialFinding> {
        lint::lint_text(text)
    }

    /// Evaluates source and candidate text and computes comparative editorial findings.
    #[must_use]
    pub fn compare(source: &str, candidate: &str) -> EditorialComparison {
        lint::compare_editorial_findings(source, candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_comparison_matches_full_evidence_and_uses_one_combined_ceiling() {
        let source = "Certainly! Source text.";
        let candidate = "Certainly! Candidate text.";
        assert_eq!(
            EditorialLintService::compare_bounded(source, candidate, 2)
                .expect("complete comparison"),
            EditorialLintService::compare(source, candidate)
        );
        assert!(EditorialLintService::compare_bounded(source, candidate, 1).is_err());
        assert!(EditorialLintService::compare_bounded(source, candidate, 0).is_err());
        assert!(EditorialLintService::lint_bounded(source, 0).is_err());
        assert!(
            EditorialLintService::compare_bounded("Clean source.", "Clean candidate.", 0)
                .expect("zero findings")
                .source_findings
                .is_empty()
        );
    }

    #[test]
    fn lint_service_detects_residue() {
        let text = "Certainly! This should be flagged.";
        let findings = EditorialLintService::lint(text);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_id, "conversational_residue");
    }

    #[test]
    fn lint_service_compares_source_and_candidate() {
        let source = "Certainly! In today's rapidly evolving digital landscape, we build software.";
        let candidate = "We build software with deterministic fidelity verification.";
        let comparison = EditorialLintService::compare(source, candidate);

        assert_eq!(comparison.resolved_findings.len(), 2);
        assert!(comparison.is_strict_improvement());
    }
}
