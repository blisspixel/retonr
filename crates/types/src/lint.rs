//! Domain contracts for editorial lint findings and comparative assessments.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Diagnostic finding identified by deterministic editorial rules.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct EditorialFinding {
    /// Identifier of the violated rule.
    pub rule_id: String,
    /// Exact matched substring in the text.
    pub evidence: String,
    /// Zero-based occurrence of this evidence substring, saturated at `u16::MAX`.
    /// Repeated saturated findings retain their full multiplicity in comparisons.
    pub occurrence: u16,
    /// Human-readable explanation of why this pattern is flagged.
    pub message: String,
}

impl EditorialFinding {
    /// Creates a new editorial finding.
    #[must_use]
    pub fn new(
        rule_id: impl Into<String>,
        evidence: impl Into<String>,
        occurrence: u16,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            evidence: evidence.into(),
            occurrence,
            message: message.into(),
        }
    }
}

/// Comparative assessment of editorial findings between source and candidate.
#[derive(Clone, Debug, Default, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
pub struct EditorialComparison {
    /// Findings observed in the source text.
    pub source_findings: Vec<EditorialFinding>,
    /// Findings observed in the candidate replacement.
    pub candidate_findings: Vec<EditorialFinding>,
    /// Findings present in the source that were eliminated in the candidate.
    pub resolved_findings: Vec<EditorialFinding>,
    /// New findings introduced by the candidate that did not exist in the source.
    pub introduced_findings: Vec<EditorialFinding>,
    /// Findings that persisted unchanged between source and candidate.
    pub retained_findings: Vec<EditorialFinding>,
}

impl EditorialComparison {
    /// Computes resolved, introduced, and retained findings from source and candidate.
    #[must_use]
    pub fn compute(
        source_findings: Vec<EditorialFinding>,
        candidate_findings: Vec<EditorialFinding>,
    ) -> Self {
        let mut candidate_remaining = finding_counts(&candidate_findings);
        let mut source_remaining = finding_counts(&source_findings);
        let mut resolved = Vec::new();
        let mut retained = Vec::new();
        for source in &source_findings {
            if consume_finding(&mut candidate_remaining, source) {
                retained.push(source.clone());
            } else {
                resolved.push(source.clone());
            }
        }
        let mut introduced = Vec::new();
        for candidate in &candidate_findings {
            if !consume_finding(&mut source_remaining, candidate) {
                introduced.push(candidate.clone());
            }
        }

        Self {
            source_findings,
            candidate_findings,
            resolved_findings: resolved,
            introduced_findings: introduced,
            retained_findings: retained,
        }
    }

    /// Returns whether the candidate introduced any new editorial defects.
    #[must_use]
    pub fn has_introduced_defects(&self) -> bool {
        !self.introduced_findings.is_empty()
    }

    /// Returns whether the candidate improved editorial quality by resolving defects
    /// without introducing new ones.
    #[must_use]
    pub fn is_strict_improvement(&self) -> bool {
        !self.resolved_findings.is_empty() && self.introduced_findings.is_empty()
    }
}

fn finding_counts(findings: &[EditorialFinding]) -> BTreeMap<(&str, &str, u16), usize> {
    let mut counts = BTreeMap::new();
    for finding in findings {
        *counts
            .entry((
                finding.rule_id.as_str(),
                finding.evidence.as_str(),
                finding.occurrence,
            ))
            .or_default() += 1;
    }
    counts
}

fn consume_finding<'a>(
    counts: &mut BTreeMap<(&'a str, &'a str, u16), usize>,
    finding: &'a EditorialFinding,
) -> bool {
    let key = (
        finding.rule_id.as_str(),
        finding.evidence.as_str(),
        finding.occurrence,
    );
    if let Some(count) = counts.get_mut(&key)
        && *count > 0
    {
        *count -= 1;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editorial_finding_round_trips() {
        let finding = EditorialFinding::new(
            "conversational_residue",
            "Certainly!",
            0,
            "Conversational residue opening",
        );
        let serialized = serde_json::to_string(&finding).expect("serialize");
        let deserialized: EditorialFinding =
            serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(finding, deserialized);
    }

    #[test]
    fn editorial_comparison_computes_partitions() {
        let f1 = EditorialFinding::new("r1", "e1", 0, "m1");
        let f2 = EditorialFinding::new("r2", "e2", 0, "m2");
        let f3 = EditorialFinding::new("r3", "e3", 0, "m3");

        let source = vec![f1.clone(), f2.clone()];
        let candidate = vec![f2.clone(), f3.clone()];

        let comparison = EditorialComparison::compute(source, candidate);
        assert_eq!(comparison.resolved_findings, vec![f1]);
        assert_eq!(comparison.retained_findings, vec![f2]);
        assert_eq!(comparison.introduced_findings, vec![f3]);
        assert!(comparison.has_introduced_defects());
        assert!(!comparison.is_strict_improvement());
    }

    #[test]
    fn strict_improvement_when_defects_resolved_and_none_introduced() {
        let f1 = EditorialFinding::new("r1", "e1", 0, "m1");
        let source = vec![f1];
        let candidate = Vec::new();

        let comparison = EditorialComparison::compute(source, candidate);
        assert_eq!(comparison.resolved_findings.len(), 1);
        assert!(comparison.introduced_findings.is_empty());
        assert!(!comparison.has_introduced_defects());
        assert!(comparison.is_strict_improvement());
    }
    #[test]
    fn occurrence_identity_and_duplicate_multiplicity_are_preserved() {
        let first = EditorialFinding::new("r", "e", 0, "m");
        let second = EditorialFinding::new("r", "e", 1, "m");
        let saturated = EditorialFinding::new("r", "e", u16::MAX, "m");
        let comparison = EditorialComparison::compute(
            vec![
                first.clone(),
                second.clone(),
                saturated.clone(),
                saturated.clone(),
            ],
            vec![second.clone(), saturated.clone()],
        );
        assert_eq!(
            comparison.retained_findings,
            vec![second, saturated.clone()]
        );
        assert_eq!(comparison.resolved_findings, vec![first, saturated]);
        assert!(comparison.introduced_findings.is_empty());
        assert!(comparison.is_strict_improvement());
    }
}
