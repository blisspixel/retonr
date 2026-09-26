use super::*;

#[test]
fn clean_text_produces_no_findings() {
    let clean = "Retonr verifies each rewrite unit against structural and layout constraints.";
    assert!(lint_text(clean).is_empty());
}

#[test]
fn detects_conversational_residue() {
    let text = "Certainly! We can help with that.";
    let findings = lint_text(text);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].rule_id, "conversational_residue");
    assert_eq!(findings[0].evidence, "Certainly!");
}

#[test]
fn compares_source_and_candidate_defects() {
    let source = "Certainly! In today's rapidly evolving digital landscape, we build software.";
    let candidate = "We build software with deterministic fidelity verification.";
    let comparison = compare_editorial_findings(source, candidate);

    assert_eq!(comparison.source_findings.len(), 2);
    assert!(comparison.candidate_findings.is_empty());
    assert_eq!(comparison.resolved_findings.len(), 2);
    assert!(comparison.introduced_findings.is_empty());
    assert!(comparison.is_strict_improvement());
}
