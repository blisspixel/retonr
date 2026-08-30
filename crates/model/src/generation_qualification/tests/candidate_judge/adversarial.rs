use super::fixture::judge_fixture;
use super::*;

fn mutate(bytes: &[u8], key: &str, value: serde_json::Value) -> Vec<u8> {
    let mut json: serde_json::Value = serde_json::from_slice(bytes).expect("record JSON");
    json.as_object_mut()
        .expect("record object")
        .insert(key.into(), value);
    serde_json::to_vec(&json).expect("mutated JSON")
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one matrix applies the same strict schema checks to every decoder"
)]
fn every_decoder_rejects_future_schema_and_unknown_fields() {
    let fixture = judge_fixture();
    let plan = serde_json::to_vec(&fixture.judge_plan).expect("plan JSON");
    assert_eq!(
        CandidateJudgePlanV1::from_json_bytes(
            &mutate(&plan, "schema_version", 2.into()),
            fixture.plan_relations(),
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    assert_eq!(
        CandidateJudgePlanV1::from_json_bytes(
            &mutate(&plan, "unknown", true.into()),
            fixture.plan_relations(),
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );

    let schedule = serde_json::to_vec(&fixture.schedule).expect("schedule JSON");
    assert_eq!(
        CandidateJudgeScheduleV1::from_json_bytes(
            &mutate(&schedule, "schema_version", 2.into()),
            &fixture.judge_plan,
            fixture.deterministic.candidate_receipt_pair_set_id(),
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    assert_eq!(
        CandidateJudgeScheduleV1::from_json_bytes(
            &mutate(&schedule, "unknown", true.into()),
            &fixture.judge_plan,
            fixture.deterministic.candidate_receipt_pair_set_id(),
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );

    let requests = serde_json::to_vec(&fixture.requests).expect("requests JSON");
    assert_eq!(
        CandidateJudgeRequestAggregateV1::from_json_bytes(
            &mutate(&requests, "schema_version", 2.into()),
            &fixture.judge_plan,
            &fixture.schedule,
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    assert_eq!(
        CandidateJudgeRequestAggregateV1::from_json_bytes(
            &mutate(&requests, "unknown", true.into()),
            &fixture.judge_plan,
            &fixture.schedule,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );

    let responses = serde_json::to_vec(&fixture.responses).expect("responses JSON");
    assert_eq!(
        CandidateJudgeResponseAggregateV1::from_json_bytes(
            &mutate(&responses, "schema_version", 2.into()),
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    assert_eq!(
        CandidateJudgeResponseAggregateV1::from_json_bytes(
            &mutate(&responses, "unknown", true.into()),
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );

    let observations = serde_json::to_vec(&fixture.observations).expect("observations JSON");
    assert_eq!(
        CandidateJudgeObservationBatchV1::from_json_bytes(
            &mutate(&observations, "schema_version", 2.into()),
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    assert_eq!(
        CandidateJudgeObservationBatchV1::from_json_bytes(
            &mutate(&observations, "unknown", true.into()),
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );

    let receipt = serde_json::to_vec(&fixture.managed_receipt).expect("receipt JSON");
    assert_eq!(
        ManagedLocalJudgeReceiptRecordV1::from_json_bytes(
            &mutate(&receipt, "schema_version", 2.into()),
            fixture.receipt_relations(),
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    assert_eq!(
        ManagedLocalJudgeReceiptRecordV1::from_json_bytes(
            &mutate(&receipt, "unknown", true.into()),
            fixture.receipt_relations(),
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );

    let join = serde_json::to_vec(&fixture.join).expect("join JSON");
    assert_eq!(
        CandidateJudgeJoinRecordV1::from_json_bytes(
            &mutate(&join, "schema_version", 2.into()),
            fixture.join_relations(),
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
    assert_eq!(
        CandidateJudgeJoinRecordV1::from_json_bytes(
            &mutate(&join, "unknown", true.into()),
            fixture.join_relations(),
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
}

#[test]
fn every_decoder_checks_its_byte_ceiling_before_json_parsing() {
    let fixture = judge_fixture();
    assert_eq!(
        CandidateJudgePlanV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES + 1],
            fixture.plan_relations(),
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        CandidateJudgeScheduleV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_JUDGE_SCHEDULE_JSON_BYTES + 1],
            &fixture.judge_plan,
            fixture.deterministic.candidate_receipt_pair_set_id(),
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        CandidateJudgeRequestAggregateV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_JUDGE_REQUEST_AGGREGATE_JSON_BYTES + 1],
            &fixture.judge_plan,
            &fixture.schedule,
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        CandidateJudgeResponseAggregateV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_JUDGE_RESPONSE_AGGREGATE_JSON_BYTES + 1],
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        CandidateJudgeObservationBatchV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_JUDGE_OBSERVATION_BATCH_JSON_BYTES + 1],
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
    assert_eq!(
        ManagedLocalJudgeReceiptRecordV1::from_json_bytes(
            &vec![b' '; MAX_MANAGED_LOCAL_JUDGE_RECEIPT_JSON_BYTES + 1],
            fixture.receipt_relations(),
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
}
