use rewrite_types::Digest;

use super::{
    GenerationCaseManifestV1, GenerationCaseManifestV1Input, GenerationClusterRecordV1,
    GenerationQualificationContractError, GenerationQualificationPlanV1,
    GenerationQualificationPlanV1Input, GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
    case, digest, fixture, limits,
};
use crate::ArtifactId;

#[test]
fn case_rejects_source_and_cluster_substitution() {
    let fixture = fixture();
    let source_digest = digest("source");
    assert_eq!(
        GenerationCaseManifestV1::new(
            &fixture.cluster,
            GenerationCaseManifestV1Input {
                case_key: "changed-source".to_owned(),
                source_artifact_id: ArtifactId::from_digest(source_digest),
                source_digest: digest("different source"),
                source_byte_count: 1,
                case_contract_digest: digest("contract"),
                language_digest: digest("language"),
                mode_digest: digest("mode"),
                format_digest: digest("format"),
            },
        ),
        Err(GenerationQualificationContractError::InvalidSourceBinding)
    );

    let other_cluster =
        GenerationClusterRecordV1::new("other", digest("other policy")).expect("other cluster");
    let encoded = serde_json::to_vec(&fixture.cases[0]).expect("case encodes");
    assert_eq!(
        GenerationCaseManifestV1::from_json_bytes(&encoded, &other_cluster),
        Err(GenerationQualificationContractError::ClusterMismatch)
    );
}

#[test]
fn suite_rejects_duplicate_ids_keys_and_cross_order_substitution() {
    let fixture = fixture();
    assert_eq!(
        GenerationSuiteManifestV1::new(
            digest("protocol"),
            &[fixture.cases[0].clone(), fixture.cases[0].clone()],
        ),
        Err(GenerationQualificationContractError::DuplicateEntry)
    );

    let same_key = case(
        &fixture.cluster,
        fixture.cases[0].case_key(),
        "other source",
    );
    assert_ne!(same_key.case_id(), fixture.cases[0].case_id());
    assert_eq!(
        GenerationSuiteManifestV1::new(digest("protocol"), &[fixture.cases[0].clone(), same_key],),
        Err(GenerationQualificationContractError::DuplicateEntry)
    );

    let mut reversed = fixture.cases.clone();
    reversed.reverse();
    let encoded = serde_json::to_vec(&fixture.suite).expect("suite encodes");
    assert_eq!(
        GenerationSuiteManifestV1::from_json_bytes(&encoded, &reversed),
        Err(GenerationQualificationContractError::CaseMismatch)
    );
}

#[test]
fn semantic_case_order_changes_identity_and_attempt_reorder_is_rejected() {
    let fixture = fixture();
    let mut reversed_cases = fixture.cases.clone();
    reversed_cases.reverse();
    let reversed_suite = GenerationSuiteManifestV1::new(digest("protocol"), &reversed_cases)
        .expect("semantic case order remains valid");
    assert_ne!(
        reversed_suite.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );

    let mut reversed_attempts = fixture.attempts.clone();
    reversed_attempts.reverse();
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &fixture.repetitions,
            &fixture.systems,
            &reversed_attempts,
            GenerationQualificationPlanV1Input {
                limits: limits(4),
                selection_policy_digest: digest("selection policy"),
                failure_policy_digest: digest("failure policy"),
            },
        ),
        Err(GenerationQualificationContractError::AttemptOrdinalMismatch)
    );
}

#[test]
fn generation_system_set_requires_strict_digest_order_and_uniqueness() {
    let fixture = fixture();
    let mut reversed = fixture.systems.clone();
    reversed.reverse();
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &fixture.repetitions,
            &reversed,
            &fixture.attempts,
            GenerationQualificationPlanV1Input {
                limits: limits(4),
                selection_policy_digest: digest("selection"),
                failure_policy_digest: digest("failure"),
            },
        ),
        Err(GenerationQualificationContractError::NonCanonicalSetOrder)
    );
    let one = fixture.systems[0].clone();
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &fixture.repetitions,
            &[one.clone(), one],
            &fixture.attempts,
            GenerationQualificationPlanV1Input {
                limits: limits(4),
                selection_policy_digest: digest("selection"),
                failure_policy_digest: digest("failure"),
            },
        ),
        Err(GenerationQualificationContractError::NonCanonicalSetOrder)
    );
}

#[test]
fn plan_rejects_duplicate_attempts_and_plan_local_ceiling_mismatch() {
    let fixture = fixture();
    let attempt = fixture.attempts[0].clone();
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &fixture.repetitions,
            &fixture.systems,
            &[attempt.clone(), attempt],
            GenerationQualificationPlanV1Input {
                limits: limits(2),
                selection_policy_digest: digest("selection"),
                failure_policy_digest: digest("failure"),
            },
        ),
        Err(GenerationQualificationContractError::DuplicateEntry)
    );
    assert_eq!(
        GenerationQualificationPlanV1::new(
            &fixture.suite,
            &fixture.repetitions,
            &fixture.systems,
            &fixture.attempts,
            GenerationQualificationPlanV1Input {
                limits: limits(1),
                selection_policy_digest: digest("selection"),
                failure_policy_digest: digest("failure"),
            },
        ),
        Err(GenerationQualificationContractError::InvalidCollectionSize)
    );
}

#[test]
fn repetition_and_plan_reject_another_suite() {
    let fixture = fixture();
    let other_case = case(&fixture.cluster, "other-case", "other source");
    let other_suite = GenerationSuiteManifestV1::new(digest("other protocol"), &[other_case])
        .expect("other suite");

    let repetition = serde_json::to_vec(&fixture.repetition).expect("repetition encodes");
    assert_eq!(
        GenerationRepetitionRecordV1::from_json_bytes(&repetition, &other_suite),
        Err(GenerationQualificationContractError::SuiteMismatch)
    );
    let plan = serde_json::to_vec(&fixture.plan).expect("plan encodes");
    assert_eq!(
        GenerationQualificationPlanV1::from_json_bytes(
            &plan,
            &other_suite,
            &fixture.repetitions,
            &fixture.systems,
            &fixture.attempts,
        ),
        Err(GenerationQualificationContractError::SuiteMismatch)
    );
}

#[test]
fn every_content_field_changes_its_record_identity() {
    let fixture = fixture();
    let changed_cluster =
        GenerationClusterRecordV1::new("editorial-core", digest("changed policy"))
            .expect("changed cluster");
    assert_ne!(changed_cluster.cluster_id(), fixture.cluster.cluster_id());

    let changed_case = case(&fixture.cluster, "active-voice", "changed source");
    assert_ne!(changed_case.case_id(), fixture.cases[0].case_id());

    let changed_suite = GenerationSuiteManifestV1::new(digest("changed protocol"), &fixture.cases)
        .expect("changed suite");
    assert_ne!(
        changed_suite.suite_manifest_id(),
        fixture.suite.suite_manifest_id()
    );

    let changed_repetition =
        GenerationRepetitionRecordV1::new(&fixture.suite, 1, digest("repetition policy"))
            .expect("changed repetition");
    assert_ne!(
        changed_repetition.repetition_id(),
        fixture.repetition.repetition_id()
    );

    let changed_plan = GenerationQualificationPlanV1::new(
        &fixture.suite,
        &fixture.repetitions,
        &fixture.systems,
        &fixture.attempts,
        GenerationQualificationPlanV1Input {
            limits: limits(4),
            selection_policy_digest: digest("changed selection policy"),
            failure_policy_digest: digest("failure policy"),
        },
    )
    .expect("changed plan");
    assert_ne!(
        changed_plan.qualification_plan_id(),
        fixture.plan.qualification_plan_id()
    );
}

#[test]
fn source_zero_size_is_rejected() {
    let fixture = fixture();
    let source = digest("source");
    let result = GenerationCaseManifestV1::new(
        &fixture.cluster,
        GenerationCaseManifestV1Input {
            case_key: "zero-source".to_owned(),
            source_artifact_id: ArtifactId::from_digest(source.clone()),
            source_digest: source,
            source_byte_count: 0,
            case_contract_digest: Digest::sha256(b"contract"),
            language_digest: Digest::sha256(b"language"),
            mode_digest: Digest::sha256(b"mode"),
            format_digest: Digest::sha256(b"format"),
        },
    );
    assert_eq!(
        result,
        Err(GenerationQualificationContractError::InvalidSourceBinding)
    );
}
