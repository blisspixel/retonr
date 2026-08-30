use super::{
    GenerationCaseManifestV1, GenerationClusterRecordV1, GenerationQualificationContractError,
    GenerationQualificationPlanV1, GenerationRepetitionRecordV1, GenerationSuiteManifestV1,
    MAX_GENERATION_CASE_JSON_BYTES, MAX_GENERATION_CLUSTER_JSON_BYTES,
    MAX_GENERATION_PLAN_JSON_BYTES, MAX_GENERATION_REPETITION_JSON_BYTES,
    MAX_GENERATION_SUITE_JSON_BYTES, fixture,
};

fn changed_schema<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut value = serde_json::to_value(value).expect("value");
    value["schema_version"] = serde_json::json!(2);
    serde_json::to_vec(&value).expect("changed value")
}

#[test]
fn all_records_round_trip_only_exact_canonical_json() {
    let fixture = fixture();
    let cluster = serde_json::to_vec(&fixture.cluster).expect("cluster encodes");
    assert_eq!(
        GenerationClusterRecordV1::from_json_bytes(&cluster).expect("cluster decodes"),
        fixture.cluster
    );
    let case = serde_json::to_vec(&fixture.cases[0]).expect("case encodes");
    assert_eq!(
        GenerationCaseManifestV1::from_json_bytes(&case, &fixture.cluster).expect("case decodes"),
        fixture.cases[0]
    );
    let suite = serde_json::to_vec(&fixture.suite).expect("suite encodes");
    assert_eq!(
        GenerationSuiteManifestV1::from_json_bytes(&suite, &fixture.cases).expect("suite decodes"),
        fixture.suite
    );
    let repetition = serde_json::to_vec(&fixture.repetition).expect("repetition encodes");
    assert_eq!(
        GenerationRepetitionRecordV1::from_json_bytes(&repetition, &fixture.suite)
            .expect("repetition decodes"),
        fixture.repetition
    );
    let plan = serde_json::to_vec(&fixture.plan).expect("plan encodes");
    assert_eq!(
        GenerationQualificationPlanV1::from_json_bytes(
            &plan,
            &fixture.suite,
            &fixture.repetitions,
            &fixture.systems,
            &fixture.attempts,
        )
        .expect("plan decodes"),
        fixture.plan
    );
}

#[test]
fn whitespace_and_field_reordering_are_noncanonical() {
    let fixture = fixture();
    let canonical =
        String::from_utf8(serde_json::to_vec(&fixture.cluster).expect("encode")).expect("UTF-8");
    let whitespace = format!(" {canonical}");
    assert_eq!(
        GenerationClusterRecordV1::from_json_bytes(whitespace.as_bytes()),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let reordered = format!(
        "{{\"cluster_key\":\"editorial-core\",\"schema_version\":1,\"cluster_policy_digest\":\"{}\"}}",
        fixture.cluster.cluster_policy_digest()
    );
    assert_eq!(
        GenerationClusterRecordV1::from_json_bytes(reordered.as_bytes()),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
}

#[test]
fn malformed_unknown_duplicate_missing_and_trailing_json_fail_closed() {
    let fixture = fixture();
    let digest = fixture.cluster.cluster_policy_digest();
    let invalid = [
        "not json".to_owned(),
        format!(
            "{{\"schema_version\":1,\"cluster_key\":\"editorial-core\",\"cluster_policy_digest\":\"{digest}\",\"unknown\":true}}"
        ),
        format!(
            "{{\"schema_version\":1,\"schema_version\":1,\"cluster_key\":\"editorial-core\",\"cluster_policy_digest\":\"{digest}\"}}"
        ),
        "{\"schema_version\":1,\"cluster_key\":\"editorial-core\"}".to_owned(),
        format!(
            "{{\"schema_version\":1,\"cluster_key\":\"editorial-core\",\"cluster_policy_digest\":\"{digest}\"}}true"
        ),
        format!(
            "{{\"schema_version\":1,\"cluster_key\":\"editorial-core\",\"cluster_policy_digest\":\"{}\"}}",
            "A".repeat(64)
        ),
    ];
    for bytes in invalid {
        assert_eq!(
            GenerationClusterRecordV1::from_json_bytes(bytes.as_bytes()),
            Err(GenerationQualificationContractError::InvalidEncoding)
        );
    }
}

#[test]
fn unsupported_schema_is_distinct_for_every_record() {
    let fixture = fixture();
    assert_eq!(
        GenerationClusterRecordV1::from_json_bytes(&changed_schema(&fixture.cluster))
            .expect_err("future cluster schema must fail"),
        GenerationQualificationContractError::UnsupportedSchema(2)
    );
    assert_eq!(
        GenerationCaseManifestV1::from_json_bytes(
            &changed_schema(&fixture.cases[0]),
            &fixture.cluster,
        )
        .expect_err("future case schema must fail"),
        GenerationQualificationContractError::UnsupportedSchema(2)
    );
    assert_eq!(
        GenerationSuiteManifestV1::from_json_bytes(
            &changed_schema(&fixture.suite),
            &fixture.cases,
        )
        .expect_err("future suite schema must fail"),
        GenerationQualificationContractError::UnsupportedSchema(2)
    );
    assert_eq!(
        GenerationRepetitionRecordV1::from_json_bytes(
            &changed_schema(&fixture.repetition),
            &fixture.suite,
        )
        .expect_err("future repetition schema must fail"),
        GenerationQualificationContractError::UnsupportedSchema(2)
    );
    assert_eq!(
        GenerationQualificationPlanV1::from_json_bytes(
            &changed_schema(&fixture.plan),
            &fixture.suite,
            &fixture.repetitions,
            &fixture.systems,
            &fixture.attempts,
        )
        .expect_err("future plan schema must fail"),
        GenerationQualificationContractError::UnsupportedSchema(2)
    );
}

#[test]
fn every_decoder_checks_size_before_parsing() {
    let fixture = fixture();
    assert_eq!(
        GenerationClusterRecordV1::from_json_bytes(&vec![
            b' ';
            MAX_GENERATION_CLUSTER_JSON_BYTES + 1
        ]),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationCaseManifestV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_CASE_JSON_BYTES + 1],
            &fixture.cluster
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationSuiteManifestV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_SUITE_JSON_BYTES + 1],
            &fixture.cases
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationRepetitionRecordV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_REPETITION_JSON_BYTES + 1],
            &fixture.suite
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        GenerationQualificationPlanV1::from_json_bytes(
            &vec![b' '; MAX_GENERATION_PLAN_JSON_BYTES + 1],
            &fixture.suite,
            &fixture.repetitions,
            &fixture.systems,
            &fixture.attempts,
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
}

#[test]
fn nested_plan_fields_and_retry_policy_are_strict() {
    let fixture = fixture();
    let mut value = serde_json::to_value(&fixture.plan).expect("plan value");
    value["limits"]["unknown"] = serde_json::json!(1);
    assert_eq!(
        GenerationQualificationPlanV1::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &fixture.suite,
            &fixture.repetitions,
            &fixture.systems,
            &fixture.attempts,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    let mut value = serde_json::to_value(&fixture.plan).expect("plan value");
    value["retry_policy"] = serde_json::json!("retry_once");
    assert_eq!(
        GenerationQualificationPlanV1::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &fixture.suite,
            &fixture.repetitions,
            &fixture.systems,
            &fixture.attempts,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
}
