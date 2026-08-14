use proptest::prelude::*;

use crate::{
    MAX_PROTECTED_OCCURRENCES, MAX_PROTECTED_TERMS, MAX_PROTECTED_TEXT_BYTES, ProtectedKind,
    ProtectionError, ProtectionPlan,
};

#[test]
fn masks_typed_literals_and_declared_terms() {
    let plan = ProtectionPlan::build(
        "Email Ada at ada@example.com before https://example.com and pay $12.50.",
        &["Ada".to_owned()],
    )
    .expect("fixture has no reserved token");
    assert_eq!(plan.values().len(), 4);
    assert_eq!(plan.values()[0].kind, ProtectedKind::DeclaredTerm);
    assert!(plan.masked_source().contains("{{PROTECTED_EMAIL_0002}}"));
    assert!(plan.masked_source().contains("{{PROTECTED_URL_0003}}"));
    assert!(plan.masked_source().contains("{{PROTECTED_NUMBER_0004}}"));
}

#[test]
fn trailing_list_commas_are_not_part_of_a_number() {
    let source = "It costs $10, which is fair.";
    let plan = ProtectionPlan::build(source, &[]).expect("valid fixture");
    assert_eq!(plan.values().len(), 1);
    assert_eq!(plan.values()[0].surface, "$10");
    let rewritten = "It costs $10 which is fair.";
    let masked = plan
        .mask_raw_candidate(rewritten)
        .expect("removing a list comma must not count as changing $10");
    assert_eq!(plan.restore(&masked).expect("issued sentinels"), rewritten);
}

#[test]
fn grouped_thousands_remain_one_number() {
    let plan = ProtectionPlan::build("Pay 1,234.50 now.", &[]).expect("valid fixture");
    assert_eq!(plan.values().len(), 1);
    assert_eq!(plan.values()[0].surface, "1,234.50");
}

#[test]
fn raw_candidate_round_trips_exact_values() {
    let source = "Version 2 costs $10 at https://example.com.";
    let plan = ProtectionPlan::build(source, &[]).expect("valid fixture");
    let raw = "Version 2 costs $10. Visit https://example.com.";
    let masked = plan.mask_raw_candidate(raw).expect("same literal counts");
    assert_eq!(plan.restore(&masked).expect("issued sentinels"), raw);
}

#[test]
fn raw_candidate_ignores_shorter_values_nested_in_protected_surfaces() {
    let source =
        "Email ada@example.com about version 2 at https://example.com before paying $12.50.";
    let plan = ProtectionPlan::build(source, &[]).expect("valid fixture");
    let masked = plan
        .mask_raw_candidate(source)
        .expect("nested protected surfaces must not be counted twice");
    assert_eq!(plan.restore(&masked).expect("issued sentinels"), source);
}

#[test]
fn fuzz_regression_rejects_ambiguous_overlapping_extracted_surfaces() {
    let source = "\x12n4\0\0\0ada@exalpme.con a\x19out.comada@\x08g $12.ada@exalpme.con a\x19out.comada@\x08g $150.\n";
    assert_eq!(
        ProtectionPlan::build(source, &[]),
        Err(ProtectionError::AmbiguousSurfaceMapping)
    );
}

#[test]
fn duplicate_standalone_and_nested_values_round_trip() {
    let source = "Version 2 costs $12.50, while version 2 also costs $12.50.";
    let plan = ProtectionPlan::build(source, &[]).expect("valid fixture");
    let masked = plan
        .mask_raw_candidate(source)
        .expect("duplicate values must receive distinct sentinels");
    assert!(
        plan.values()
            .iter()
            .all(|value| masked.matches(&value.token).count() == 1)
    );
    assert_eq!(plan.restore(&masked).expect("issued sentinels"), source);
}

#[test]
fn rejects_added_or_removed_standalone_nested_value() {
    let plan = ProtectionPlan::build("Version 2 costs $12.50.", &[]).expect("valid fixture");
    assert_eq!(
        plan.mask_raw_candidate("Version costs $12.50."),
        Err(ProtectionError::ProtectedOccurrenceCount)
    );
    assert_eq!(
        plan.mask_raw_candidate("Version 2 and 2 costs $12.50."),
        Err(ProtectionError::ProtectedOccurrenceCount)
    );
}

#[test]
fn rejects_incompatible_partial_overlaps() {
    let plan = ProtectionPlan::build("abc bcd", &["abc".to_owned(), "bcd".to_owned()])
        .expect("valid fixture");
    assert_eq!(
        plan.mask_raw_candidate("abcd"),
        Err(ProtectionError::ProtectedOccurrenceCount)
    );
}

#[test]
fn match_flood_fails_closed() {
    let source = (1..=256)
        .map(|length| "1".repeat(length))
        .collect::<Vec<_>>()
        .join(" ");
    let plan = ProtectionPlan::build(&source, &[]).expect("valid fixture");
    assert_eq!(
        plan.mask_raw_candidate(&"1".repeat(4_096)),
        Err(ProtectionError::ProtectedOccurrenceCount)
    );
}

#[test]
fn dense_supported_occurrences_round_trip() {
    let source = "1 ".repeat(MAX_PROTECTED_OCCURRENCES);
    let plan = ProtectionPlan::build(&source, &[]).expect("fixture is at the occurrence limit");
    assert_eq!(plan.values().len(), MAX_PROTECTED_OCCURRENCES);
    let masked = plan
        .mask_raw_candidate(&source)
        .expect("dense supported input must mask in one pass");
    assert_eq!(plan.restore(&masked).expect("issued sentinels"), source);
}

#[test]
fn rejects_occurrence_and_masked_output_overflow() {
    let excessive = "1 ".repeat(MAX_PROTECTED_OCCURRENCES + 1);
    assert_eq!(
        ProtectionPlan::build(&excessive, &[]),
        Err(ProtectionError::ResourceLimit)
    );

    let source = format!("{} 1", "a".repeat(MAX_PROTECTED_TEXT_BYTES - 2));
    assert_eq!(source.len(), MAX_PROTECTED_TEXT_BYTES);
    assert_eq!(
        ProtectionPlan::build(&source, &[]),
        Err(ProtectionError::ResourceLimit)
    );
}

#[test]
fn direct_constructor_rejects_unbounded_declared_terms() {
    let terms = (0..=MAX_PROTECTED_TERMS)
        .map(|index| format!("nonmatching-{index}"))
        .collect::<Vec<_>>();
    assert_eq!(
        ProtectionPlan::build("bounded source", &terms),
        Err(ProtectionError::InvalidDeclaredTerms)
    );
}

#[test]
fn declared_term_order_does_not_change_the_plan() {
    let source = "abc ab";
    let forward =
        ProtectionPlan::build(source, &["abc".to_owned(), "ab".to_owned()]).expect("valid fixture");
    let reverse =
        ProtectionPlan::build(source, &["ab".to_owned(), "abc".to_owned()]).expect("valid fixture");
    assert_eq!(forward, reverse);
}

#[test]
fn rejects_changed_or_unknown_values() {
    let plan = ProtectionPlan::build("Version 2", &[]).expect("valid fixture");
    assert_eq!(
        plan.mask_raw_candidate("Version 3"),
        Err(ProtectionError::ProtectedOccurrenceCount)
    );
    assert_eq!(
        plan.validate_masked("Version {{PROTECTED_NUMBER_9999}}"),
        Err(ProtectionError::UnknownSentinel)
    );
}

#[test]
fn rejects_reserved_source_tokens() {
    assert_eq!(
        ProtectionPlan::build("{{PROTECTED_URL_0001}}", &[]),
        Err(ProtectionError::ReservedTokenInSource)
    );
}

proptest! {
    #[test]
    fn generated_duplicate_and_nested_numbers_round_trip(value in 0_u16..10_000) {
        let source = format!("Version {value} and version {value} cost ${value}.50.");
        let plan = ProtectionPlan::build(&source, &[]).expect("generated source is valid");
        let first = plan.mask_raw_candidate(&source).expect("source values are intact");
        let second = plan.mask_raw_candidate(&source).expect("matching is deterministic");
        prop_assert_eq!(&first, &second);
        prop_assert_eq!(plan.restore(&first).expect("issued sentinels"), source);
    }
}
