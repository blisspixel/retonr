//! Deterministic editorial lint evaluation against annotated corpora.

use rewrite_engine::lint;
pub use rewrite_engine::lint::RuleCatalog;
pub use rewrite_types::EditorialFinding;
use serde::Serialize;

use crate::editorial_corpus::{EditorialCaseKind, EditorialCorpus};

#[cfg(test)]
mod tests;

/// Content-free aggregate evaluation report comparing the linter against an annotated corpus.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EditorialLintEvaluationReport {
    /// Total cases evaluated.
    pub total_cases: usize,
    /// Total cases where all expectations matched perfectly.
    pub passed_cases: usize,
    /// Total cases with mismatches.
    pub failed_cases: usize,
    /// Detailed failures by case ID.
    pub failures: Vec<EditorialLintFailure>,
}

impl EditorialLintEvaluationReport {
    /// Whether all evaluated cases passed with zero mismatches.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        self.failed_cases == 0
    }
}

/// A specific case failure where observed findings differed from expectations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EditorialLintFailure {
    /// Case identifier.
    pub id: String,
    /// Category or channel.
    pub channel: String,
    /// Description of the mismatch.
    pub error: String,
}

/// Lints source text against the comprehensive editorial rule catalog.
#[must_use]
pub fn lint_text(text: &str) -> Vec<EditorialFinding> {
    lint::lint_text(text)
}

/// Evaluates an editorial corpus against the deterministic linter.
#[must_use]
pub fn evaluate_corpus_against_linter(corpus: &EditorialCorpus) -> EditorialLintEvaluationReport {
    let mut total_cases = 0;
    let mut passed_cases = 0;
    let mut failed_cases = 0;
    let mut failures = Vec::new();

    for case in &corpus.cases {
        total_cases += 1;
        let findings = lint_text(&case.source);

        let mut case_passed = true;
        let mut case_error = String::new();

        match case.kind {
            EditorialCaseKind::Finding => {
                for expected in &case.expected_source_findings {
                    let matched = findings.iter().any(|f| {
                        f.rule_id == expected.rule_id
                            && f.evidence == expected.evidence
                            && f.occurrence == expected.occurrence
                    });
                    if !matched {
                        case_passed = false;
                        case_error = format!(
                            "missing expected finding: rule={}, evidence='{}', occurrence={}",
                            expected.rule_id, expected.evidence, expected.occurrence
                        );
                        break;
                    }
                }
            }
            EditorialCaseKind::CleanControl => {
                for targeted_rule in &case.target_rules {
                    if let Some(unexpected) = findings.iter().find(|f| f.rule_id == *targeted_rule)
                    {
                        case_passed = false;
                        case_error = format!(
                            "unexpected finding on clean control: rule={}, evidence='{}'",
                            unexpected.rule_id, unexpected.evidence
                        );
                        break;
                    }
                }
            }
        }

        if case_passed {
            passed_cases += 1;
        } else {
            failed_cases += 1;
            failures.push(EditorialLintFailure {
                id: case.id.clone(),
                channel: case.channel.clone(),
                error: case_error,
            });
        }
    }

    EditorialLintEvaluationReport {
        total_cases,
        passed_cases,
        failed_cases,
        failures,
    }
}
