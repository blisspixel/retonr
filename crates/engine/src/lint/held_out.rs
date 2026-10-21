//! Independent held-out lint cases and density floors.
//!
//! Recall is the share of family positives whose expected evidence is present.
//! The false-positive rate is the share of clean, quoted, and near-miss controls
//! that produce any finding. Both figures are checks on this fixture. They are
//! not an authorship score.

use std::collections::BTreeSet;

use rewrite_types::TextPoint;
use serde_json::Value;

use super::{RuleCatalog, lint_text, lint_text_excluding};

const HELD_OUT: &str = include_str!("../../../eval/fixtures/lint_held_out.json");

const DEVELOPMENT: &[&str] = &[
    include_str!("../../fixtures/lint_bounded_regression.json"),
    include_str!("../../fixtures/lint_occurrence_regression.json"),
    include_str!("../../../eval/fixtures/editorial_quality_v1.json"),
    include_str!("../../../eval/fixtures/editorial_slop_v1.json"),
    include_str!("../../../eval/fixtures/editorial_prose_v1.json"),
    include_str!("../../../eval/fixtures/editorial_model_impressions_v1.json"),
    include_str!("../../../eval/fixtures/editorial_assistant_residue_v1.json"),
];

struct HeldOut {
    intentionally_exact: Vec<String>,
    families: Vec<FamilyCase>,
}

struct FamilyCase {
    rule_id: String,
    positive: Positive,
    clean: String,
    quoted: String,
    near_miss: String,
}

struct Positive {
    text: String,
    evidence: Vec<String>,
}

#[test]
fn held_out_recall_is_one_and_control_false_positive_rate_is_zero() {
    let held_out = held_out();
    assert!(held_out.intentionally_exact.is_empty());
    let mut seen = BTreeSet::new();
    let mut hits = 0;
    let mut controls = 0;
    let mut false_positives = 0;
    for family in &held_out.families {
        assert!(seen.insert(family.rule_id.as_str()));
        assert!(
            !DEVELOPMENT
                .iter()
                .any(|fixture| fixture.contains(&family.positive.text)),
            "{} positive is already in a development fixture",
            family.rule_id
        );
        let findings = lint_text(&family.positive.text);
        assert!(
            findings
                .iter()
                .all(|finding| finding.rule_id == family.rule_id),
            "{} unexpected finding: {findings:?}",
            family.rule_id
        );
        assert!(
            evidence_present(&findings, family),
            "{} missing evidence: {findings:?}",
            family.rule_id
        );
        assert_spans(&family.positive.text, &findings);
        hits += 1;
        for control in [&family.clean, &family.quoted, &family.near_miss] {
            controls += 1;
            let unexpected = lint_text(control);
            if !unexpected.is_empty() {
                false_positives += 1;
            }
            assert!(
                unexpected.is_empty(),
                "{} control {control:?} flagged {unexpected:?}",
                family.rule_id
            );
        }
    }
    for rule in RuleCatalog::ALL {
        assert!(seen.contains(rule.id()), "{}", rule.id());
    }
    assert_eq!(seen.len(), RuleCatalog::ALL.len());
    assert_eq!(hits, held_out.families.len());
    assert_eq!(false_positives, 0);
    assert_eq!(controls, held_out.families.len() * 3);
}

#[test]
fn disabling_one_rule_drops_only_that_rules_held_out_positive() {
    let held_out = held_out();
    for disabled in RuleCatalog::ALL {
        for family in &held_out.families {
            let findings = lint_text_excluding(&family.positive.text, disabled);
            let present = evidence_present(&findings, family);
            if disabled.id() == family.rule_id {
                assert!(
                    !present
                        && findings
                            .iter()
                            .all(|finding| finding.rule_id != disabled.id()),
                    "disabling {} still matched {}: {findings:?}",
                    disabled.id(),
                    family.rule_id
                );
            } else {
                assert!(
                    present,
                    "disabling {} removed {}: {findings:?}",
                    disabled.id(),
                    family.rule_id
                );
            }
        }
    }
}

#[test]
fn development_sentences_keep_one_finding_and_their_original_evidence() {
    let stacked = lint_text(
        "The platform is fast, flexible, and future-ready. It informs, inspires, and empowers every team.",
    );
    let stacked: Vec<_> = stacked
        .iter()
        .filter(|finding| finding.rule_id == "stacked_tricolon")
        .map(|finding| finding.evidence.as_str())
        .collect();
    assert_eq!(
        stacked,
        ["fast, flexible, and future-ready. It informs, inspires, and empowers"]
    );

    let repeated = lint_text(
        "The tool preserves the source file. In conclusion, the tool preserves the source file.",
    );
    assert_eq!(repeated.len(), 1);
    assert_eq!(
        repeated[0].evidence,
        "In conclusion, the tool preserves the source file."
    );
    assert!(repeated[0].span.is_some());
}

#[test]
fn density_uses_a_short_document_floor_and_a_per_thousand_word_rate() {
    assert!(lint_text("The draft is clear, short, and specific.").is_empty());
    let en_dash = lint_text("North \u{2013} south \u{2013} east \u{2013} west.");
    assert_eq!(en_dash.len(), 3);
    assert!(en_dash.iter().all(|finding| finding.evidence == "\u{2013}"));
    let mixed = lint_text("A \u{2014} B \u{2013} C -- D.");
    assert_eq!(mixed.len(), 3);

    let quiet_dashes = format!("{}{}", "alpha ".repeat(1_001), "\u{2014}".repeat(3));
    assert!(lint_text(&quiet_dashes).is_empty());
    let loud_dashes = format!("{}{}", "alpha ".repeat(1_001), "\u{2014}".repeat(6));
    assert_eq!(
        lint_text(&loud_dashes)
            .iter()
            .filter(|finding| finding.rule_id == "excessive_dash_density")
            .count(),
        6
    );

    let sentence = "The patch is not only a rename, but a stricter read. ";
    assert!(
        lint_text(&format!("{}{sentence}", "alpha ".repeat(1_100)))
            .iter()
            .all(|finding| finding.rule_id != "contrastive_reframe")
    );
    assert!(
        lint_text(&format!("{}{}", "alpha ".repeat(1_100), sentence.repeat(3)))
            .iter()
            .any(|finding| finding.rule_id == "contrastive_reframe")
    );

    let pair = "The draft is clear, short, and specific. It names, checks, and records. ";
    assert!(
        lint_text(&format!("{}{pair}", "alpha ".repeat(2_000)))
            .iter()
            .all(|finding| finding.rule_id != "stacked_tricolon")
    );
}

fn held_out() -> HeldOut {
    let value: Value = serde_json::from_str(HELD_OUT).expect("held-out lint fixture");
    let intentionally_exact = value["intentionally_exact"]
        .as_array()
        .expect("intentionally_exact")
        .iter()
        .map(|item| item.as_str().expect("exact id").to_owned())
        .collect();
    let families = value["families"]
        .as_array()
        .expect("families")
        .iter()
        .map(|family| FamilyCase {
            rule_id: family["rule_id"].as_str().expect("rule").to_owned(),
            positive: Positive {
                text: family["positive"]["text"]
                    .as_str()
                    .expect("positive")
                    .to_owned(),
                evidence: family["positive"]["evidence"]
                    .as_array()
                    .expect("evidence")
                    .iter()
                    .map(|item| item.as_str().expect("evidence text").to_owned())
                    .collect(),
            },
            clean: family["clean"].as_str().expect("clean").to_owned(),
            quoted: family["quoted"].as_str().expect("quoted").to_owned(),
            near_miss: family["near_miss"].as_str().expect("near miss").to_owned(),
        })
        .collect();
    HeldOut {
        intentionally_exact,
        families,
    }
}

fn evidence_present(findings: &[rewrite_types::EditorialFinding], family: &FamilyCase) -> bool {
    let mut remaining: Vec<&str> = findings
        .iter()
        .filter(|finding| finding.rule_id == family.rule_id)
        .map(|finding| finding.evidence.as_str())
        .collect();
    for evidence in &family.positive.evidence {
        let Some(index) = remaining.iter().position(|item| *item == evidence) else {
            return false;
        };
        remaining.remove(index);
    }
    true
}

fn assert_spans(text: &str, findings: &[rewrite_types::EditorialFinding]) {
    for finding in findings {
        let span = finding.span.expect("phrase span");
        let start = usize::try_from(span.start).expect("start");
        let end = usize::try_from(span.end).expect("end");
        assert_eq!(&text[start..end], finding.evidence);
        let point = TextPoint::from_byte_index(text, start).expect("point");
        assert_eq!(span.line, point.line);
        assert_eq!(span.column, point.column);
    }
}
