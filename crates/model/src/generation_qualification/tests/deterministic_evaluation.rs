use super::super::codec::{append_digest, append_u32, append_u64};
use super::*;

pub(super) mod fixture;

use fixture::*;

#[test]
fn suite_pair_framing_vector_is_stable() {
    let mut canonical = CANDIDATE_DETERMINISTIC_SUITE_PAIR_DIGEST_DOMAIN.to_vec();
    append_digest(&mut canonical, &Digest::sha256(b"suite"));
    append_digest(&mut canonical, &Digest::sha256(b"material"));
    append_u64(&mut canonical, 1);
    append_u32(&mut canonical, 0);
    for bytes in [br#"{"candidate":"a"}"#.as_slice(), br#"{"candidate":"b"}"#] {
        append_u64(
            &mut canonical,
            u64::try_from(bytes.len()).expect("suite JSON length"),
        );
        canonical.extend_from_slice(bytes);
    }
    assert_eq!(
        Digest::sha256(&canonical).as_str(),
        "745aee30c1251dcaad6d38cf56d02412cb087e870cf3bdd9901c9ce1c5c4b254"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the stable vector test verifies every public record projection together"
)]
fn identities_aggregates_and_order_are_stable() {
    let fixture = pair_fixture("pair failure policy");
    let reports = report_relationship(0);
    let case_material = digest("case material");
    let suite_pair = digest("suite pair");
    let pair_id =
        CandidateReceiptPairSetId::from_receipt_sets(&fixture.candidate_a, &fixture.candidate_b)
            .expect("pair ID");
    let reversed =
        CandidateReceiptPairSetId::from_receipt_sets(&fixture.candidate_b, &fixture.candidate_a)
            .expect("reversed pair ID");
    assert_ne!(pair_id, reversed);
    let record = CandidateDeterministicEvaluationRecordV1::new(
        &fixture.candidate_a,
        &fixture.candidate_b,
        input(&case_material, &suite_pair, &reports),
    )
    .expect("evaluation record");
    assert_eq!(record.schema_version(), 1);
    assert_eq!(record.candidate_receipt_pair_set_id(), &pair_id);
    assert_eq!(
        record.candidate_a_receipt_set_id(),
        fixture.candidate_a.receipt_set_id()
    );
    assert_eq!(
        record.candidate_b_receipt_set_id(),
        fixture.candidate_b.receipt_set_id()
    );
    assert_eq!(
        record.suite_manifest_id(),
        fixture.candidate_a.suite_manifest_id()
    );
    assert_eq!(record.repetition_id(), fixture.candidate_a.repetition_id());
    assert_eq!(
        record.candidate_a_generation_system_id(),
        fixture.candidate_a.generation_system_id()
    );
    assert_eq!(
        record.candidate_b_generation_system_id(),
        fixture.candidate_b.generation_system_id()
    );
    assert_eq!(record.case_material_set_digest(), &case_material);
    assert_eq!(record.suite_pair_digest(), &suite_pair);
    assert_eq!(
        record.deterministic_policy_digest(),
        &candidate_deterministic_policy_digest()
    );
    assert_eq!(
        record.candidate_a_report_digest(),
        reports.candidate_a_report_digest()
    );
    assert_eq!(
        record.candidate_b_report_digest(),
        reports.candidate_b_report_digest()
    );
    assert_eq!(record.report_pair_digest(), reports.report_pair_digest());
    assert_eq!((record.total(), record.passed()), (2, 1));
    assert_eq!(record.transformation_coverage(), coverage(1, 1));
    assert_eq!(
        record.status(),
        CandidateDeterministicEvaluationStatusV1::Failed
    );
    assert_eq!(
        (
            pair_id.digest().as_str(),
            reports.candidate_a_report_digest().as_str(),
            reports.candidate_b_report_digest().as_str(),
            reports.report_pair_digest().as_str(),
            record.deterministic_evaluation_id().digest().as_str(),
        ),
        (
            "86d4171528d6b35dfa0ec32e9fdf2b8058141150768fe3125f869b67ad0415f9",
            "2fc6263797a7e42d5cdb5f6c45fb14c3b34ae759e6a5f0ed5ae9ca50a6cf7bca",
            "ea2551da56b72357097fa3574a8d746e19b7a85f948d01b50e06034d5b0389c0",
            "b9c938d086f99ff2d2be470a05974ca79ffbe035e7cc67d847e3f05639502763",
            "5173db6c01c04bf46668c5b592304e9dc4cfbafbb18bd6c10c0efe62d4732be2"
        )
    );
    assert_eq!(
        CANDIDATE_RECEIPT_PAIR_SET_ID_DOMAIN,
        b"retonr:candidate-receipt-pair-set:v1\0"
    );
    assert_eq!(
        CANDIDATE_DETERMINISTIC_EVALUATION_ID_DOMAIN,
        b"retonr:candidate-deterministic-evaluation:v1\0"
    );
    assert_eq!(
        CANDIDATE_DETERMINISTIC_REPORT_DIGEST_DOMAIN,
        b"retonr:candidate-deterministic-report:v1\0"
    );
    assert_eq!(
        CANDIDATE_DETERMINISTIC_SUITE_PAIR_DIGEST_DOMAIN,
        b"retonr:candidate-deterministic-suite-pair:v1\0"
    );
    assert_eq!(
        CANDIDATE_DETERMINISTIC_REPORT_PAIR_DIGEST_DOMAIN,
        b"retonr:hybrid-scorecard-report-pair:v1\0"
    );
    assert_eq!(
        candidate_deterministic_policy_digest().as_str(),
        "761e147db1918db3712cb203cfd77e8a002efbcae5479be99f4364d758a938da"
    );
    let encoded = serde_json::to_vec(&record).expect("record JSON");
    assert_eq!(
        CandidateDeterministicEvaluationRecordV1::from_json_bytes(
            &encoded,
            &fixture.candidate_a,
            &fixture.candidate_b,
            input(&case_material, &suite_pair, &reports),
        )
        .expect("record decode"),
        record
    );
    let debug = format!("{record:?}");
    assert!(debug.contains(record.deterministic_evaluation_id().digest().as_str()));
    assert!(!debug.contains(record.report_pair_digest().as_str()));
}

#[test]
fn pair_requires_same_plan_distinct_systems_and_semantic_closure() {
    let first = pair_fixture("first plan");
    let other_plan = pair_fixture("other plan");
    assert_eq!(
        CandidateReceiptPairSetId::from_receipt_sets(&first.candidate_a, &first.candidate_a),
        Err(GenerationQualificationContractError::ReceiptPairRelationshipMismatch),
    );
    assert_eq!(
        CandidateReceiptPairSetId::from_receipt_sets(&first.candidate_a, &other_plan.candidate_b),
        Err(GenerationQualificationContractError::ReceiptPairRelationshipMismatch),
    );
}

#[test]
fn report_framing_is_inert_and_limits_arithmetic_only() {
    let alleged_pass = br#"{"total":1,"passed":1}"#;
    let mismatched = CandidateDeterministicReportRelationshipV1::new(
        alleged_pass,
        alleged_pass,
        summary(0, coverage(0, 0)),
        summary(0, coverage(0, 0)),
    )
    .expect("model framing deliberately does not parse reports");
    assert_eq!(mismatched.candidate_a_summary().passed(), 0);
    assert_eq!(mismatched.candidate_b_summary().total(), 1);
    assert_eq!(
        mismatched.candidate_b_summary().transformation_coverage(),
        coverage(0, 0)
    );

    let invalid = GenerationQualificationContractError::InvalidDeterministicReportRelationship;
    assert_eq!(
        CandidateDeterministicTransformationCoverageV1::new(513, 0),
        Err(invalid)
    );
    assert_eq!(
        CandidateDeterministicTransformationCoverageV1::new(1, 2),
        Err(invalid)
    );
    assert_eq!(
        CandidateDeterministicReportSummaryV1::new(0, 0, coverage(0, 0)),
        Err(invalid)
    );
    assert_eq!(
        CandidateDeterministicReportSummaryV1::new(257, 0, coverage(0, 0)),
        Err(invalid)
    );
    assert_eq!(
        CandidateDeterministicReportSummaryV1::new(1, 2, coverage(0, 0)),
        Err(invalid)
    );
    assert_eq!(
        CandidateDeterministicReportSummaryV1::new(1, 1, coverage(2, 1)),
        Err(invalid)
    );
    assert_eq!(
        CandidateDeterministicReportRelationshipV1::new(
            b"",
            b"x",
            summary(1, coverage(0, 0)),
            summary(1, coverage(0, 0))
        ),
        Err(invalid)
    );
    assert_eq!(
        CandidateDeterministicReportRelationshipV1::new(
            b"x",
            b"",
            summary(1, coverage(0, 0)),
            summary(1, coverage(0, 0))
        ),
        Err(invalid)
    );
    let oversized = vec![b'x'; MAX_CANDIDATE_DETERMINISTIC_REPORT_JSON_BYTES + 1];
    assert_eq!(
        CandidateDeterministicReportRelationshipV1::new(
            &oversized,
            b"x",
            summary(1, coverage(0, 0)),
            summary(1, coverage(0, 0))
        ),
        Err(invalid)
    );

    let fixture = pair_fixture("summary mismatch");
    let reports = CandidateDeterministicReportRelationshipV1::new(
        b"a",
        b"b",
        CandidateDeterministicReportSummaryV1::new(2, 2, coverage(0, 0)).expect("bounded summary"),
        summary(1, coverage(0, 0)),
    )
    .expect("inert relationship");
    assert_eq!(
        CandidateDeterministicEvaluationRecordV1::new(
            &fixture.candidate_a,
            &fixture.candidate_b,
            input(&digest("material"), &digest("suites"), &reports),
        ),
        Err(GenerationQualificationContractError::DeterministicEvaluationRelationshipMismatch)
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the adversarial decoder matrix keeps all encoding classes visible"
)]
fn strict_decoder_rejects_untrusted_encodings_and_relationships() {
    let fixture = pair_fixture("decoder");
    let reports = report_relationship(1);
    let case_material = digest("decoder material");
    let suite_pair = digest("decoder suites");
    let input = input(&case_material, &suite_pair, &reports);
    let record = CandidateDeterministicEvaluationRecordV1::new(
        &fixture.candidate_a,
        &fixture.candidate_b,
        input,
    )
    .expect("record");
    assert_eq!(
        record.status(),
        CandidateDeterministicEvaluationStatusV1::Passed
    );
    let encoded = serde_json::to_vec(&record).expect("record JSON");
    assert_eq!(
        CandidateDeterministicEvaluationRecordV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_DETERMINISTIC_EVALUATION_JSON_BYTES + 1],
            &fixture.candidate_a,
            &fixture.candidate_b,
            input,
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        CandidateDeterministicEvaluationRecordV1::from_json_bytes(
            b"{",
            &fixture.candidate_a,
            &fixture.candidate_b,
            input,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let future = replace_once(&encoded, "\"schema_version\":1", "\"schema_version\":2");
    assert_eq!(
        CandidateDeterministicEvaluationRecordV1::from_json_bytes(
            &future,
            &fixture.candidate_a,
            &fixture.candidate_b,
            input,
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    for invalid in [
        replace_once(
            &encoded,
            "\"schema_version\":1",
            "\"schema_version\":1,\"unknown\":true",
        ),
        replace_once(
            &encoded,
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_version\":1",
        ),
        replace_once(&encoded, ",\"status\":\"passed\"", ""),
    ] {
        assert_eq!(
            CandidateDeterministicEvaluationRecordV1::from_json_bytes(
                &invalid,
                &fixture.candidate_a,
                &fixture.candidate_b,
                input,
            ),
            Err(GenerationQualificationContractError::InvalidEncoding)
        );
    }
    let substituted = replace_once(
        &encoded,
        case_material.as_str(),
        digest("other material").as_str(),
    );
    assert_eq!(
        CandidateDeterministicEvaluationRecordV1::from_json_bytes(
            &substituted,
            &fixture.candidate_a,
            &fixture.candidate_b,
            input,
        ),
        Err(GenerationQualificationContractError::DeterministicEvaluationRelationshipMismatch)
    );
    let reordered =
        serde_json::to_vec(&serde_json::from_slice::<serde_json::Value>(&encoded).expect("value"))
            .expect("reordered");
    assert_eq!(
        CandidateDeterministicEvaluationRecordV1::from_json_bytes(
            &reordered,
            &fixture.candidate_a,
            &fixture.candidate_b,
            input,
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let mut trailing = encoded;
    trailing.push(b' ');
    assert_eq!(
        CandidateDeterministicEvaluationRecordV1::from_json_bytes(
            &trailing,
            &fixture.candidate_a,
            &fixture.candidate_b,
            input,
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
}
