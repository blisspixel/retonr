use super::fixture::{judge_fixture, judge_plan_input, managed_input, observation_batch};
use super::*;

#[test]
fn repeated_transport_identity_remains_exactly_schedule_indexed() {
    let fixture = judge_fixture();
    let repeated = fixture.response_ids[0].clone();
    let response_ids = vec![repeated.clone(), repeated.clone()];
    let responses = CandidateJudgeResponseAggregateV1::new(
        &fixture.judge_plan,
        &fixture.schedule,
        &fixture.requests,
        response_ids.clone(),
    )
    .expect("schedule-indexed repeated response");
    assert_eq!(responses.retained_session_response_ids(), response_ids);
    assert_eq!(responses.responses()[0].schedule_index(), 0);
    assert_eq!(responses.responses()[1].schedule_index(), 1);
    assert_ne!(
        responses.responses()[0].candidate_judge_response_id(),
        responses.responses()[1].candidate_judge_response_id()
    );
    assert_ne!(
        responses.responses()[0].structured_request_binding_id(),
        responses.responses()[1].structured_request_binding_id()
    );
    assert_eq!(
        responses.responses()[0]
            .candidate_judge_response_id()
            .digest()
            .as_str(),
        "d98c75383331c9f6edb96c547dbbd99f4bcbfcbec5ba31b9c3a82caea48d7ea8"
    );
    assert_eq!(
        responses.responses()[1]
            .candidate_judge_response_id()
            .digest()
            .as_str(),
        "917c2fc3f673fabd840912be9756a9a234a8990231ee0a603805d7e8ffce38f0"
    );
    for response in responses.responses() {
        let debug = format!("{response:?}");
        assert!(debug.contains("CandidateJudgeResponseV1"));
        assert!(!debug.contains(repeated.digest().as_str()));
    }

    let observations = observation_batch(
        &fixture.judge_plan,
        &fixture.schedule,
        &fixture.requests,
        &response_ids,
    );
    let receipt = ManagedLocalJudgeReceiptRecordV1::new(
        ManagedLocalJudgeReceiptRecordV1Relations {
            response_aggregate: &responses,
            observation_batch: &observations,
            ..fixture.receipt_relations()
        },
        managed_input(),
    )
    .expect("receipt accepts exact schedule-indexed repeated transport identity");
    assert_eq!(receipt.attempt_count(), 2);
}

#[test]
fn foreign_plan_schedule_request_and_response_associations_are_rejected() {
    let fixture = judge_fixture();
    let foreign_repetition = GenerationRepetitionRecordV1::new(
        &fixture.pair.suite,
        1,
        digest("foreign judge repetition"),
    )
    .expect("foreign repetition");
    let mut relations = fixture.plan_relations();
    relations.repetition = &foreign_repetition;
    assert_eq!(
        CandidateJudgePlanV1::new(
            relations,
            judge_plan_input(&fixture.judge_system, fixture.pair.case.case_id()),
        ),
        Err(GenerationQualificationContractError::CandidateJudgePlanRelationshipMismatch)
    );

    let same_schedule = CandidateJudgeScheduleV1::new(
        &fixture.judge_plan,
        fixture.deterministic.candidate_receipt_pair_set_id(),
    )
    .expect("same deterministic schedule");
    assert_eq!(same_schedule, fixture.schedule);
    let foreign_plan = CandidateJudgePlanV1::new(
        fixture.plan_relations(),
        CandidateJudgePlanV1Input {
            presentation_seed: 43,
            ..judge_plan_input(&fixture.judge_system, fixture.pair.case.case_id())
        },
    )
    .expect("foreign judge plan");
    let foreign_schedule = CandidateJudgeScheduleV1::new(
        &foreign_plan,
        fixture.deterministic.candidate_receipt_pair_set_id(),
    )
    .expect("foreign schedule");
    let foreign_requests = CandidateJudgeRequestAggregateV1::new(
        &foreign_plan,
        &foreign_schedule,
        fixture.requests.structured_request_binding_ids().to_vec(),
    )
    .expect("foreign requests");
    let foreign_responses = CandidateJudgeResponseAggregateV1::new(
        &foreign_plan,
        &foreign_schedule,
        &foreign_requests,
        fixture.response_ids.clone(),
    )
    .expect("foreign responses");
    assert_eq!(
        CandidateJudgeObservationV1::new(
            &fixture.judge_plan,
            &fixture.schedule,
            &foreign_requests,
            0,
            fixture.response_ids[0].clone(),
            CandidateJudgeChoiceV1::First,
            vec!["meaning".into()],
        ),
        Err(GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch)
    );
    let relations = ManagedLocalJudgeReceiptRecordV1Relations {
        response_aggregate: &foreign_responses,
        ..fixture.receipt_relations()
    };
    assert_eq!(
        ManagedLocalJudgeReceiptRecordV1::new(relations, managed_input()),
        Err(GenerationQualificationContractError::ManagedLocalJudgeReceiptRelationshipMismatch)
    );
}

#[test]
fn response_association_substitution_fails_closed() {
    let fixture = judge_fixture();
    let repeated = fixture.response_ids[0].clone();
    let response_ids = vec![repeated.clone(), repeated];
    let responses = CandidateJudgeResponseAggregateV1::new(
        &fixture.judge_plan,
        &fixture.schedule,
        &fixture.requests,
        response_ids.clone(),
    )
    .expect("schedule-indexed repeated response");
    let observations = observation_batch(
        &fixture.judge_plan,
        &fixture.schedule,
        &fixture.requests,
        &response_ids,
    );
    let bytes = serde_json::to_vec(&responses).expect("response JSON");

    let mut value = response_value(&bytes);
    value["responses"]
        .as_array_mut()
        .expect("response array")
        .swap(0, 1);
    assert_response_rejected(&fixture, &value);

    let mut value = response_value(&bytes);
    value["responses"][0]["structured_request_binding_id"] = serde_json::Value::String(
        fixture.requests.structured_request_binding_ids()[1]
            .digest()
            .as_str()
            .to_owned(),
    );
    assert_response_rejected(&fixture, &value);

    for field in ["candidate_judge_plan_id", "candidate_judge_schedule_id"] {
        let mut value = response_value(&bytes);
        value["responses"][0][field] =
            serde_json::Value::String(digest("forged response association").as_str().to_owned());
        assert_response_rejected(&fixture, &value);
    }
    for index in [1, 99] {
        let mut value = response_value(&bytes);
        value["responses"][0]["schedule_index"] = index.into();
        assert_response_rejected(&fixture, &value);
    }

    let mut value = response_value(&bytes);
    value["candidate_judge_request_aggregate_id"] =
        serde_json::Value::String(digest("forged request aggregate").as_str().to_owned());
    assert_response_rejected(&fixture, &value);

    let mut value = response_value(&bytes);
    value["responses"][0]["schema_version"] = 2.into();
    assert_eq!(
        decode_responses(&fixture, &value),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );

    let observation_bytes = serde_json::to_vec(&observations).expect("observation JSON");
    let mut value: serde_json::Value =
        serde_json::from_slice(&observation_bytes).expect("observation value");
    value["observations"][0]["candidate_judge_response"]["schedule_index"] = 1.into();
    let forged = serde_json::to_vec(&value).expect("forged observation JSON");
    assert_eq!(
        CandidateJudgeObservationBatchV1::from_json_bytes(
            &forged,
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        ),
        Err(GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch)
    );

    let mut value: serde_json::Value =
        serde_json::from_slice(&observation_bytes).expect("observation value");
    value["observations"][0]["candidate_judge_response"]["schema_version"] = 2.into();
    let forged = serde_json::to_vec(&value).expect("forged observation JSON");
    assert_eq!(
        CandidateJudgeObservationBatchV1::from_json_bytes(
            &forged,
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        ),
        Err(GenerationQualificationContractError::UnsupportedSchema(2))
    );
}

fn response_value(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).expect("response value")
}

fn assert_response_rejected(fixture: &super::fixture::JudgeFixture, value: &serde_json::Value) {
    assert_eq!(
        decode_responses(fixture, value),
        Err(GenerationQualificationContractError::CandidateJudgeAggregateRelationshipMismatch)
    );
}

fn decode_responses(
    fixture: &super::fixture::JudgeFixture,
    value: &serde_json::Value,
) -> Result<CandidateJudgeResponseAggregateV1, GenerationQualificationContractError> {
    CandidateJudgeResponseAggregateV1::from_json_bytes(
        &serde_json::to_vec(&value).expect("forged response JSON"),
        &fixture.judge_plan,
        &fixture.schedule,
        &fixture.requests,
    )
}
