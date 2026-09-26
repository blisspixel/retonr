//! Application service for deterministic editorial quality and anti-slop inspection.

use rewrite_engine::lint;
use rewrite_types::{EditorialComparison, EditorialFinding};

/// Service for deterministic editorial quality and anti-slop inspection.
#[derive(Clone, Copy, Debug, Default)]
pub struct EditorialLintService;

impl EditorialLintService {
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
