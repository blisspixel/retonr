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

fn occurrence_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../fixtures/lint_occurrence_regression.json"
    ))
    .expect("occurrence regression fixture")
}

#[test]
fn repeated_findings_above_u16_limit_do_not_panic_or_lose_multiplicity() {
    let fixture = occurrence_fixture();
    let repetitions =
        usize::try_from(fixture["repetitions"].as_u64().expect("repetitions")).expect("count");
    let pattern = fixture["pattern"].as_str().expect("pattern");
    let source = pattern.repeat(repetitions);
    let candidate = pattern.repeat(repetitions - 1);
    let comparison = compare_editorial_findings(&source, &candidate);
    assert_eq!(comparison.source_findings.len(), repetitions);
    assert_eq!(comparison.candidate_findings.len(), repetitions - 1);
    assert_eq!(comparison.retained_findings.len(), repetitions - 1);
    assert_eq!(comparison.resolved_findings.len(), 1);
    assert!(comparison.introduced_findings.is_empty());
    assert_eq!(comparison.source_findings[65535].occurrence, u16::MAX);
    assert_eq!(comparison.source_findings[65536].occurrence, u16::MAX);
    assert!(comparison.is_strict_improvement());
}

#[test]
fn repeated_defect_reduction_and_addition_use_exact_occurrences() {
    let fixture = occurrence_fixture();
    let source = fixture["source"].as_str().expect("source");
    let candidate = fixture["candidate"].as_str().expect("candidate");
    let reduced = compare_editorial_findings(source, candidate);
    assert_eq!(reduced.resolved_findings.len(), 2);
    assert_eq!(reduced.retained_findings.len(), 1);
    assert!(reduced.is_strict_improvement());
    let increased = compare_editorial_findings(candidate, source);
    assert_eq!(increased.introduced_findings.len(), 2);
    assert!(increased.has_introduced_defects());
}

#[test]
fn quoted_matches_still_count_toward_occurrence_but_are_not_findings() {
    let fixture = occurrence_fixture();
    for field in ["quoted", "nested_quotes"] {
        let findings = lint_text(fixture[field].as_str().expect("quoted text"));
        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].occurrence,
            if field == "quoted" { 1 } else { 2 }
        );
    }
}

#[test]
fn density_thresholds_preserve_every_unquoted_match_and_its_ordinal() {
    for pattern in ["\u{2014}", "\u{2705}"] {
        let quoted = format!("\"{pattern}\" ");
        assert!(lint_text(&format!("{quoted}{}", pattern.repeat(2))).is_empty());
        let findings = lint_text(&format!("{quoted}{}", pattern.repeat(3)));
        assert_eq!(findings.len(), 3);
        assert_eq!(
            findings
                .iter()
                .map(|finding| finding.occurrence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }
}
