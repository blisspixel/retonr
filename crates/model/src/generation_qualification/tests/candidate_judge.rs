use super::*;

mod adversarial;
pub(super) mod fixture;
mod response_identity;
mod vectors;

use fixture::*;

#[test]
fn plan_schedule_and_helper_identities_are_stable() {
    let fixture = judge_fixture();
    assert_eq!(fixture.judge_plan.schema_version(), 1);
    assert_eq!(fixture.judge_plan.cases().len(), 1);
    assert_eq!(
        fixture.judge_plan.cases()[0].rubric_clause_ids(),
        &["meaning"]
    );
    assert_eq!(fixture.schedule.entry_count(), 2);
    assert_ne!(
        fixture.schedule.entries()[0].presentation(),
        fixture.schedule.entries()[1].presentation()
    );
    assert_eq!(fixture.schedule.entries()[0].attempt_ordinal(), 0);
    assert_eq!(fixture.schedule.entries()[1].attempt_ordinal(), 0);
    assert_eq!(
        fixture
            .judge_plan
            .candidate_judge_plan_id()
            .digest()
            .as_str(),
        "de90cea2518fd3a05c9395d73683ac3026229a36393d5385f4bb5cddf886b6cf"
    );
    assert_eq!(
        fixture
            .schedule
            .candidate_judge_schedule_id()
            .digest()
            .as_str(),
        "69a89192b87bb095150af5c6894d284e8563c6427b4b3ebfa587e4c05b0f4135"
    );
    assert_eq!(
        fixture.schedule.entries()[0].seed(),
        13_050_059_956_814_819_005_u64
    );
}

#[test]
fn complete_records_round_trip_and_final_claims_stay_false() {
    let fixture = judge_fixture();
    for (bytes, kind) in [
        (
            serde_json::to_vec(&fixture.judge_plan).expect("plan JSON"),
            "plan",
        ),
        (
            serde_json::to_vec(&fixture.schedule).expect("schedule JSON"),
            "schedule",
        ),
    ] {
        match kind {
            "plan" => assert_eq!(
                CandidateJudgePlanV1::from_json_bytes(&bytes, fixture.plan_relations())
                    .expect("plan reload"),
                fixture.judge_plan
            ),
            "schedule" => assert_eq!(
                CandidateJudgeScheduleV1::from_json_bytes(
                    &bytes,
                    &fixture.judge_plan,
                    fixture.deterministic.candidate_receipt_pair_set_id(),
                )
                .expect("schedule reload"),
                fixture.schedule
            ),
            _ => unreachable!(),
        }
    }
    assert_eq!(
        CandidateJudgeRequestAggregateV1::from_json_bytes(
            &serde_json::to_vec(&fixture.requests).expect("request JSON"),
            &fixture.judge_plan,
            &fixture.schedule,
        )
        .expect("request reload"),
        fixture.requests
    );
    assert_eq!(
        CandidateJudgeResponseAggregateV1::from_json_bytes(
            &serde_json::to_vec(&fixture.responses).expect("response JSON"),
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        )
        .expect("response reload"),
        fixture.responses
    );
    assert_eq!(
        CandidateJudgeObservationBatchV1::from_json_bytes(
            &serde_json::to_vec(&fixture.observations).expect("observation JSON"),
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
        )
        .expect("observation reload"),
        fixture.observations
    );
    assert_eq!(
        ManagedLocalJudgeReceiptRecordV1::from_json_bytes(
            &serde_json::to_vec(&fixture.managed_receipt).expect("receipt JSON"),
            fixture.receipt_relations(),
        )
        .expect("receipt reload"),
        fixture.managed_receipt
    );
    let join = CandidateJudgeJoinRecordV1::from_json_bytes(
        &serde_json::to_vec(&fixture.join).expect("join JSON"),
        fixture.join_relations(),
    )
    .expect("join reload");
    assert_eq!(join, fixture.join);
    assert_eq!(
        join.evidence_class(),
        CandidateJudgeEvidenceClassV1::ManagedLocalJudgeTriage
    );
    assert!(!join.candidate_semantics_proven());
    assert!(!join.judge_correctness_proven());
    assert!(!join.qualified());
    assert_eq!(
        fixture
            .managed_receipt
            .managed_local_judge_receipt_id()
            .digest()
            .as_str(),
        "62f23b97bef04fc63dd32a73849d945ad283aa3215070e97c305f8021fa36c1b"
    );
    assert_eq!(
        join.candidate_judge_join_id().digest().as_str(),
        "ac20a9fe339a8744927df88d0a5e7f8374e3f4b893370fe5ac3475d62ccff50c"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the accessor matrix proves the complete portable projection is available"
)]
fn accessors_expose_the_complete_content_free_closure() {
    let fixture = judge_fixture();
    let plan = &fixture.judge_plan;
    std::hint::black_box(plan.schema_version());
    std::hint::black_box(plan.qualification_plan_id());
    std::hint::black_box(plan.suite_manifest_id());
    std::hint::black_box(plan.repetition_id());
    std::hint::black_box(plan.selection_policy_id());
    std::hint::black_box(plan.candidate_a_generation_system_id());
    std::hint::black_box(plan.candidate_b_generation_system_id());
    std::hint::black_box(plan.judge_generation_system_id());
    std::hint::black_box(plan.case_material_set_digest());
    std::hint::black_box(plan.rubric_digest());
    std::hint::black_box(plan.order_policy());
    std::hint::black_box(plan.presentation_seed());
    std::hint::black_box(plan.attempts_per_order());
    std::hint::black_box(plan.prompt_contract_digest());
    std::hint::black_box(plan.output_schema_digest());
    let limits = plan.limits();
    std::hint::black_box(limits.maximum_judge_cases());
    std::hint::black_box(limits.maximum_source_bytes());
    std::hint::black_box(limits.maximum_candidate_bytes());
    std::hint::black_box(limits.maximum_complete_input_bytes());
    std::hint::black_box(limits.maximum_context_tokens());
    std::hint::black_box(limits.maximum_output_tokens());
    std::hint::black_box(limits.maximum_response_bytes());
    std::hint::black_box(limits.maximum_elapsed_milliseconds());
    std::hint::black_box(fixture.schedule.schema_version());
    std::hint::black_box(fixture.schedule.candidate_judge_plan_id());
    std::hint::black_box(fixture.schedule.candidate_receipt_pair_set_id());
    std::hint::black_box(fixture.schedule.presentation_seed());
    std::hint::black_box(fixture.schedule.entries()[0].case_id());
    std::hint::black_box(fixture.schedule.entries()[0].presentation());
    std::hint::black_box(fixture.schedule.entries()[0].attempt_ordinal());
    std::hint::black_box(fixture.schedule.entries()[0].seed());
    std::hint::black_box(fixture.requests.schema_version());
    std::hint::black_box(fixture.requests.structured_request_binding_ids());
    std::hint::black_box(fixture.responses.schema_version());
    std::hint::black_box(fixture.responses.candidate_judge_request_aggregate_id());
    std::hint::black_box(fixture.responses.retained_session_response_ids());
    for response in fixture.responses.responses() {
        std::hint::black_box(response.schema_version());
        std::hint::black_box(response.candidate_judge_plan_id());
        std::hint::black_box(response.candidate_judge_schedule_id());
        std::hint::black_box(response.schedule_index());
        std::hint::black_box(response.structured_request_binding_id());
        std::hint::black_box(response.retained_session_response_id());
        std::hint::black_box(response.candidate_judge_response_id());
    }
    let observation = &fixture.observations.observations()[0];
    std::hint::black_box(observation.case_id());
    std::hint::black_box(observation.presentation());
    std::hint::black_box(observation.attempt_ordinal());
    std::hint::black_box(observation.retained_session_response_id());
    std::hint::black_box(observation.candidate_judge_response());
    std::hint::black_box(observation.choice());
    std::hint::black_box(observation.cited_rubric_clause_ids());
    std::hint::black_box(observation.observation_id());
    std::hint::black_box(fixture.observations.candidate_judge_plan_id());
    std::hint::black_box(fixture.observations.candidate_judge_schedule_id());
    std::hint::black_box(fixture.observations.candidate_judge_request_aggregate_id());

    let receipt = &fixture.managed_receipt;
    std::hint::black_box(receipt.schema_version());
    std::hint::black_box(receipt.managed_preflight_digest());
    std::hint::black_box(receipt.retained_session_preflight_digest());
    std::hint::black_box(receipt.residency_receipt_aggregate_digest());
    std::hint::black_box(receipt.process_observation_aggregate_digest());
    std::hint::black_box(receipt.native_load_observation_aggregate_digest());
    std::hint::black_box(receipt.connection_observation_aggregate_digest());
    std::hint::black_box(receipt.effective_runtime_state_observation_aggregate_digest());
    std::hint::black_box(receipt.first_response_ordinal());
    std::hint::black_box(receipt.last_response_ordinal());
    std::hint::black_box(receipt.judge_effective_runtime_state_join_id());
    std::hint::black_box(receipt.cleanup_disposition());
    std::hint::black_box(receipt.runtime_package_revalidation_status());
    std::hint::black_box(receipt.model_package_revalidation_status());
    std::hint::black_box(receipt.evidence_class());

    let join = &fixture.join;
    std::hint::black_box(join.schema_version());
    std::hint::black_box(join.candidate_judge_plan_id());
    std::hint::black_box(join.candidate_receipt_pair_set_id());
    std::hint::black_box(join.candidate_a_receipt_set_id());
    std::hint::black_box(join.candidate_b_receipt_set_id());
    std::hint::black_box(join.candidate_a_generation_system_id());
    std::hint::black_box(join.candidate_b_generation_system_id());
    std::hint::black_box(join.judge_generation_system_id());
    std::hint::black_box(join.deterministic_evaluation_id());
    std::hint::black_box(join.case_material_set_digest());
    std::hint::black_box(join.suite_pair_digest());
    std::hint::black_box(join.judge_schedule_id());
    std::hint::black_box(join.managed_local_judge_receipt_id());
    std::hint::black_box(join.judge_request_aggregate_id());
    std::hint::black_box(join.judge_response_aggregate_id());
    std::hint::black_box(join.judge_observation_batch_id());
    std::hint::black_box(join.attempt_count());
    std::hint::black_box(join.judge_runtime_admission_join_id());
    std::hint::black_box(join.judge_managed_generation_path_id());
    std::hint::black_box(join.judge_frozen_external_component_set_id());
    std::hint::black_box(join.judge_runtime_package_manifest_id());
    std::hint::black_box(join.judge_runtime_build_id());
    std::hint::black_box(join.judge_effective_runtime_state_id());
    std::hint::black_box(join.judge_effective_runtime_state_join_id());
    std::hint::black_box(join.judge_model_package_manifest_id());
    std::hint::black_box(join.judge_model_artifact_id());
    std::hint::black_box(join.judge_effective_package_evidence_v2_id());
    std::hint::black_box(join.judge_runtime_installation_generation());
    std::hint::black_box(join.judge_model_installation_generation());
    std::hint::black_box(join.triage_report_digest());

    for redacted in [
        format!("{plan:?}"),
        format!("{:?}", fixture.schedule),
        format!("{:?}", fixture.requests),
        format!("{:?}", fixture.responses),
        format!("{:?}", fixture.observations),
        format!("{:?}", fixture.managed_receipt),
        format!("{join:?}"),
    ] {
        assert!(!redacted.contains("judge managed preflight"));
    }
}

#[test]
fn empty_citations_and_response_substitution_are_rejected() {
    let fixture = judge_fixture();
    assert_eq!(
        CandidateJudgeObservationV1::new(
            &fixture.judge_plan,
            &fixture.schedule,
            &fixture.requests,
            0,
            fixture.response_ids[0].clone(),
            CandidateJudgeChoiceV1::First,
            Vec::new(),
        ),
        Err(GenerationQualificationContractError::CandidateJudgeObservationRelationshipMismatch)
    );
    let substituted = observation_batch(
        &fixture.judge_plan,
        &fixture.schedule,
        &fixture.requests,
        &[
            fixture.response_ids[1].clone(),
            fixture.response_ids[0].clone(),
        ],
    );
    let relations = ManagedLocalJudgeReceiptRecordV1Relations {
        observation_batch: &substituted,
        ..fixture.receipt_relations()
    };
    assert_eq!(
        ManagedLocalJudgeReceiptRecordV1::new(relations, managed_input()),
        Err(GenerationQualificationContractError::ManagedLocalJudgeReceiptRelationshipMismatch)
    );
    for (first, last) in [(0, 17), (9, 26), (7, 25), (8, 24), (8, 26)] {
        let mut invalid_span = managed_input();
        invalid_span.first_response_ordinal = first;
        invalid_span.last_response_ordinal = last;
        assert_eq!(
            ManagedLocalJudgeReceiptRecordV1::new(fixture.receipt_relations(), invalid_span),
            Err(GenerationQualificationContractError::ManagedLocalJudgeReceiptRelationshipMismatch),
            "response ordinal span {first}..={last} must be rejected"
        );
    }
}

#[test]
fn join_rejects_foreign_plan_repetition_and_selection_with_the_same_systems() {
    let fixture = judge_fixture();
    let foreign_qualification = GenerationQualificationPlanV1::new(
        &fixture.pair.suite,
        std::slice::from_ref(&fixture.pair.repetition),
        &fixture.pair.systems,
        &fixture.pair.planned,
        GenerationQualificationPlanV1Input {
            limits: limits(2),
            selection_policy_digest: fixture.pair.policy.selection_policy_id().digest().clone(),
            failure_policy_digest: digest("foreign failure policy"),
        },
    )
    .expect("foreign qualification");
    let foreign_plan = CandidateJudgePlanV1::new(
        CandidateJudgePlanV1Relations {
            qualification_plan: &foreign_qualification,
            ..fixture.plan_relations()
        },
        judge_plan_input(&fixture.judge_system, fixture.pair.case.case_id()),
    )
    .expect("foreign judge plan");
    assert_eq!(
        CandidateJudgeJoinRecordV1::new(CandidateJudgeJoinRecordV1Relations {
            plan: &foreign_plan,
            ..fixture.join_relations()
        }),
        Err(GenerationQualificationContractError::CandidateJudgeJoinRelationshipMismatch)
    );

    let foreign_repetition = GenerationRepetitionRecordV1::new(
        &fixture.pair.suite,
        1,
        digest("foreign joined repetition"),
    )
    .expect("foreign repetition");
    let foreign_selection =
        CandidateSelectionPolicyV1::new(&fixture.pair.suite, &[0]).expect("foreign selection");
    let mut foreign_attempts = fixture.pair.planned.clone();
    foreign_attempts.extend(
        fixture
            .pair
            .systems
            .iter()
            .enumerate()
            .map(|(index, system)| {
                attempt_record(
                    &fixture.pair.suite,
                    &fixture.pair.case,
                    &fixture.pair.cluster,
                    &foreign_repetition,
                    system,
                    index + fixture.pair.planned.len(),
                )
            }),
    );
    let foreign_qualification = GenerationQualificationPlanV1::new(
        &fixture.pair.suite,
        &[fixture.pair.repetition.clone(), foreign_repetition.clone()],
        &fixture.pair.systems,
        &foreign_attempts,
        GenerationQualificationPlanV1Input {
            limits: limits(4),
            selection_policy_digest: foreign_selection.selection_policy_id().digest().clone(),
            failure_policy_digest: digest("foreign joined failure policy"),
        },
    )
    .expect("foreign qualification closure");
    let foreign_plan = CandidateJudgePlanV1::new(
        CandidateJudgePlanV1Relations {
            qualification_plan: &foreign_qualification,
            suite: &fixture.pair.suite,
            repetition: &foreign_repetition,
            selection_policy: &foreign_selection,
            planned_attempts: &foreign_attempts,
            candidate_a_system: &fixture.pair.systems[0],
            candidate_b_system: &fixture.pair.systems[1],
            judge_system: &fixture.judge_system,
        },
        judge_plan_input(&fixture.judge_system, fixture.pair.case.case_id()),
    )
    .expect("foreign judge plan closure");
    assert_eq!(
        CandidateJudgeJoinRecordV1::new(CandidateJudgeJoinRecordV1Relations {
            plan: &foreign_plan,
            ..fixture.join_relations()
        }),
        Err(GenerationQualificationContractError::CandidateJudgeJoinRelationshipMismatch)
    );
}

#[test]
fn strict_decoders_reject_unknown_trailing_and_oversized_input() {
    let fixture = judge_fixture();
    let canonical = serde_json::to_vec(&fixture.schedule).expect("schedule JSON");
    let mut trailing = canonical.clone();
    trailing.push(b' ');
    assert_eq!(
        CandidateJudgeScheduleV1::from_json_bytes(
            &trailing,
            &fixture.judge_plan,
            fixture.deterministic.candidate_receipt_pair_set_id(),
        ),
        Err(GenerationQualificationContractError::NonCanonicalEncoding)
    );
    let mut value: serde_json::Value = serde_json::from_slice(&canonical).expect("JSON value");
    value
        .as_object_mut()
        .expect("object")
        .insert("unknown".into(), true.into());
    assert_eq!(
        CandidateJudgeScheduleV1::from_json_bytes(
            &serde_json::to_vec(&value).expect("mutated JSON"),
            &fixture.judge_plan,
            fixture.deterministic.candidate_receipt_pair_set_id(),
        ),
        Err(GenerationQualificationContractError::InvalidEncoding)
    );
    assert_eq!(
        CandidateJudgeJoinRecordV1::from_json_bytes(
            &vec![b' '; MAX_CANDIDATE_JUDGE_JOIN_JSON_BYTES + 1],
            fixture.join_relations(),
        ),
        Err(GenerationQualificationContractError::EncodedRecordTooLarge)
    );
}

#[test]
fn bounds_and_triage_framing_are_enforced() {
    assert_eq!(
        MANAGED_LOCAL_JUDGE_EFFECTIVE_RUNTIME_STATE_JOIN_DOMAIN,
        b"retonr:managed-local-judge-effective-runtime-state-join:v1\0"
    );
    assert!(CandidateJudgeLimitsV1::new(0, 1, 1, 1, 1, 1, 256, 1).is_err());
    assert!(CandidateJudgeLimitsV1::new(1, 1, 1, 1, 1, 1, 255, 1).is_err());
    assert!(
        CandidateJudgeCaseV1::new(
            GenerationCaseId(Digest::sha256(b"case")),
            vec!["bad clause".into()],
        )
        .is_err()
    );
    assert_eq!(
        CandidateJudgeTriageReportRelationshipV1::new(&[]),
        Err(GenerationQualificationContractError::InvalidCandidateJudgeTriageReport)
    );
    let triage = CandidateJudgeTriageReportRelationshipV1::new(b"{}").expect("triage framing");
    assert_eq!(
        triage.digest().as_str(),
        "9efbcf0ab17792bf0f14bf701c6bc815e26bcf80e8514a793f42cf1c8e7269f5"
    );
}
