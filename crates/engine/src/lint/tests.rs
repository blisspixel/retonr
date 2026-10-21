use super::*;

#[test]
fn bounded_findings_match_full_results_and_refuse_every_insufficient_ceiling() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/lint_bounded_regression.json"))
            .expect("bounded regression");
    for case in fixture["cases"].as_array().expect("cases") {
        let text = case["text"].as_str().expect("text");
        let expected = lint_text(text);
        assert_eq!(
            expected.len(),
            usize::try_from(case["findings"].as_u64().expect("count")).expect("bounded count")
        );
        for limit in 0..=expected.len() + 1 {
            let actual = lint_text_bounded(text, limit);
            if limit < expected.len() {
                assert_eq!(actual, Err(FindingLimitExceeded));
            } else {
                assert_eq!(actual.expect("complete result"), expected);
            }
        }
    }
}

#[test]
fn bounded_dense_matching_refuses_without_collecting_the_whole_document() {
    for pattern in ["\u{2014}", "\u{2705}", "!!!", "Ultimately ", "Moreover "] {
        let text = pattern.repeat(100_000);
        assert_eq!(lint_text_bounded(&text, 8), Err(FindingLimitExceeded));
    }
}

#[test]
fn marker_ordinals_count_quoted_matches_and_saturate_without_quadratic_rescanning() {
    let text = format!("\"Ultimately\" {}", "Ultimately ".repeat(65_537));
    let findings = lint_text(&text);
    assert_eq!(findings.len(), 65_537);
    assert_eq!(findings[0].occurrence, 1);
    assert_eq!(findings.last().expect("last").occurrence, u16::MAX);
}

#[test]
fn finding_span_follows_the_line_that_contains_the_evidence() {
    let text = "caf\u{00e9}\nCertainly! The file stayed.\n";
    let findings = lint_text(text);
    let finding = findings
        .iter()
        .find(|item| item.evidence == "Certainly!")
        .expect("residue");
    let span = finding.span.expect("span");
    assert_eq!((span.line, span.column), (2, 1));
    let start = usize::try_from(span.start).expect("start");
    let end = usize::try_from(span.end).expect("end");
    assert_eq!(&text[start..end], "Certainly!");
    let point = rewrite_types::TextPoint::from_byte_index(text, start).expect("point");
    assert_eq!((span.line, span.column), (point.line, point.column));
}

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

#[test]
fn folded_residue_catches_common_assistant_lines_and_leaves_ordinary_prose() {
    let caught = [
        (
            "I hope this helps! Let me know if you'd like any further changes.",
            "conversational_residue",
        ),
        (
            "In today's fast-paced world, things change.",
            "prefabricated_scene_setting",
        ),
        ("It's important to note that we grew.", "throat_clearing"),
        ("Let's delve into the details.", "ornamental_abstraction"),
        (
            "This is a rich tapestry of ideas.",
            "ornamental_abstraction",
        ),
        (
            "Here's a revised version of your draft.",
            "conversational_residue",
        ),
        (
            "As an AI language model, I cannot do that.",
            "conversational_residue",
        ),
        (
            "Great question! The answer is yes.",
            "conversational_residue",
        ),
        ("certainly! Done.", "conversational_residue"),
        (
            "This is a game-changer for the team.",
            "promotional_puffery",
        ),
        (
            "This is a game changer for the team.",
            "promotional_puffery",
        ),
    ];
    for (text, rule) in caught {
        let findings = lint_text(text);
        assert!(
            findings.iter().any(|finding| finding.rule_id == rule),
            "{text:?} missing {rule}: {findings:?}"
        );
    }

    let clean = [
        "Surely the plan is sound.",
        "Historical certainty is rare.",
        "I am sure the gate held.",
        "We are certain the count matches.",
        "We delved into the archive yesterday.",
        "The game-changing update shipped.",
        "A seventeenth-century tapestry survived.",
        "Digital advertising landscape shifted last year.",
        "She wrote, \"Certainly!\" in the margin.",
        "The witness wrote, \"It is important to note that timing matters.\"",
        "The quoted review called it a \"game-changing platform\" and stopped.",
        "Retonr verifies each rewrite unit against structural and layout constraints.",
    ];
    for text in clean {
        let findings = lint_text(text);
        assert!(
            findings.is_empty(),
            "{text:?} should stay clean: {findings:?}"
        );
    }
}
