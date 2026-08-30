use super::super::deterministic_evaluation::fixture::{
    PairFixture, input, pair_fixture, report_relationship,
};
use super::super::*;

pub(in crate::generation_qualification::tests) struct JudgeFixture {
    pub(in crate::generation_qualification::tests) pair: PairFixture,
    pub(in crate::generation_qualification::tests) judge_system: GenerationSystemRecordV1,
    pub(in crate::generation_qualification::tests) deterministic:
        CandidateDeterministicEvaluationRecordV1,
    pub(in crate::generation_qualification::tests) judge_plan: CandidateJudgePlanV1,
    pub(in crate::generation_qualification::tests) schedule: CandidateJudgeScheduleV1,
    pub(in crate::generation_qualification::tests) requests: CandidateJudgeRequestAggregateV1,
    pub(in crate::generation_qualification::tests) responses: CandidateJudgeResponseAggregateV1,
    pub(in crate::generation_qualification::tests) observations: CandidateJudgeObservationBatchV1,
    pub(in crate::generation_qualification::tests) managed_receipt:
        ManagedLocalJudgeReceiptRecordV1,
    pub(in crate::generation_qualification::tests) join: CandidateJudgeJoinRecordV1,
    pub(in crate::generation_qualification::tests) response_ids:
        Vec<OllamaRetainedSessionResponseId>,
    triage_report: CandidateJudgeTriageReportRelationshipV1,
}

impl JudgeFixture {
    pub(super) fn plan_relations(&self) -> CandidateJudgePlanV1Relations<'_> {
        CandidateJudgePlanV1Relations {
            qualification_plan: &self.pair.plan,
            suite: &self.pair.suite,
            repetition: &self.pair.repetition,
            selection_policy: &self.pair.policy,
            planned_attempts: &self.pair.planned,
            candidate_a_system: &self.pair.systems[0],
            candidate_b_system: &self.pair.systems[1],
            judge_system: &self.judge_system,
        }
    }

    pub(super) fn receipt_relations(&self) -> ManagedLocalJudgeReceiptRecordV1Relations<'_> {
        ManagedLocalJudgeReceiptRecordV1Relations {
            plan: &self.judge_plan,
            schedule: &self.schedule,
            judge_system: &self.judge_system,
            request_aggregate: &self.requests,
            response_aggregate: &self.responses,
            observation_batch: &self.observations,
        }
    }

    pub(super) fn join_relations(&self) -> CandidateJudgeJoinRecordV1Relations<'_> {
        CandidateJudgeJoinRecordV1Relations {
            plan: &self.judge_plan,
            candidate_a_receipt_set: &self.pair.candidate_a,
            candidate_b_receipt_set: &self.pair.candidate_b,
            deterministic_evaluation: &self.deterministic,
            schedule: &self.schedule,
            request_aggregate: &self.requests,
            response_aggregate: &self.responses,
            observation_batch: &self.observations,
            managed_receipt: &self.managed_receipt,
            judge_system: &self.judge_system,
            triage_report: &self.triage_report,
        }
    }
}

pub(in crate::generation_qualification::tests) fn judge_fixture() -> JudgeFixture {
    judge_fixture_from_pair(pair_fixture("judge failure policy"))
}

pub(in crate::generation_qualification::tests) fn judge_fixture_from_pair(
    pair: PairFixture,
) -> JudgeFixture {
    let support = super::super::super::generation_system::test_support::fixture(false);
    let judge_system =
        GenerationSystemRecordV1::new(support.relations(), support.input()).expect("judge system");
    let reports = report_relationship(1);
    let case_material = digest("judge case material");
    let suite_pair = digest("judge suite pair");
    let deterministic = CandidateDeterministicEvaluationRecordV1::new(
        &pair.candidate_a,
        &pair.candidate_b,
        input(&case_material, &suite_pair, &reports),
    )
    .expect("passed deterministic evaluation");
    let relations = CandidateJudgePlanV1Relations {
        qualification_plan: &pair.plan,
        suite: &pair.suite,
        repetition: &pair.repetition,
        selection_policy: &pair.policy,
        planned_attempts: &pair.planned,
        candidate_a_system: &pair.systems[0],
        candidate_b_system: &pair.systems[1],
        judge_system: &judge_system,
    };
    let judge_plan = CandidateJudgePlanV1::new(
        relations,
        judge_plan_input(&judge_system, pair.case.case_id()),
    )
    .expect("judge plan");
    let schedule =
        CandidateJudgeScheduleV1::new(&judge_plan, deterministic.candidate_receipt_pair_set_id())
            .expect("judge schedule");
    let request_ids = (0..schedule.entries().len())
        .map(|index| {
            StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                "judge request {index}"
            )))
        })
        .collect();
    let requests = CandidateJudgeRequestAggregateV1::new(&judge_plan, &schedule, request_ids)
        .expect("request aggregate");
    let response_ids = (0..schedule.entries().len())
        .map(|index| {
            OllamaRetainedSessionResponseId::from_derived_digest(digest(&format!(
                "judge response {index}"
            )))
        })
        .collect::<Vec<_>>();
    let responses = CandidateJudgeResponseAggregateV1::new(
        &judge_plan,
        &schedule,
        &requests,
        response_ids.clone(),
    )
    .expect("response aggregate");
    let observations = observation_batch(&judge_plan, &schedule, &requests, &response_ids);
    let receipt_relations = ManagedLocalJudgeReceiptRecordV1Relations {
        plan: &judge_plan,
        schedule: &schedule,
        judge_system: &judge_system,
        request_aggregate: &requests,
        response_aggregate: &responses,
        observation_batch: &observations,
    };
    let managed_receipt = ManagedLocalJudgeReceiptRecordV1::new(receipt_relations, managed_input())
        .expect("managed receipt");
    let triage_report =
        CandidateJudgeTriageReportRelationshipV1::new(br#"{"schema_version":1,"status":"triage"}"#)
            .expect("triage report");
    let join = CandidateJudgeJoinRecordV1::new(CandidateJudgeJoinRecordV1Relations {
        plan: &judge_plan,
        candidate_a_receipt_set: &pair.candidate_a,
        candidate_b_receipt_set: &pair.candidate_b,
        deterministic_evaluation: &deterministic,
        schedule: &schedule,
        request_aggregate: &requests,
        response_aggregate: &responses,
        observation_batch: &observations,
        managed_receipt: &managed_receipt,
        judge_system: &judge_system,
        triage_report: &triage_report,
    })
    .expect("candidate judge join");
    JudgeFixture {
        pair,
        judge_system,
        deterministic,
        judge_plan,
        schedule,
        requests,
        responses,
        observations,
        managed_receipt,
        join,
        response_ids,
        triage_report,
    }
}

pub(super) fn judge_plan_input(
    judge: &GenerationSystemRecordV1,
    case_id: &GenerationCaseId,
) -> CandidateJudgePlanV1Input {
    CandidateJudgePlanV1Input {
        case_material_set_digest: digest("judge case material"),
        rubric_digest: digest("judge rubric"),
        cases: vec![
            CandidateJudgeCaseV1::new(case_id.clone(), vec!["meaning".into()]).expect("judge case"),
        ],
        order_policy: CandidateJudgeOrderPolicyV1::BothOrders,
        presentation_seed: 42,
        attempts_per_order: 1,
        limits: CandidateJudgeLimitsV1::new(1, 1_024, 1_024, 4_096, 4_096, 256, 4_096, 10_000)
            .expect("judge limits"),
        prompt_contract_digest: judge.prompt_digest().clone(),
        output_schema_digest: judge.output_schema_digest().clone(),
    }
}

pub(super) fn observation_batch(
    plan: &CandidateJudgePlanV1,
    schedule: &CandidateJudgeScheduleV1,
    requests: &CandidateJudgeRequestAggregateV1,
    response_ids: &[OllamaRetainedSessionResponseId],
) -> CandidateJudgeObservationBatchV1 {
    let observations = response_ids
        .iter()
        .enumerate()
        .map(|(index, response_id)| {
            CandidateJudgeObservationV1::new(
                plan,
                schedule,
                requests,
                index,
                response_id.clone(),
                CandidateJudgeChoiceV1::First,
                vec!["meaning".into()],
            )
            .expect("judge observation")
        })
        .collect();
    CandidateJudgeObservationBatchV1::new(plan, schedule, requests, observations)
        .expect("observation batch")
}

pub(super) fn managed_input() -> ManagedLocalJudgeReceiptRecordV1Input {
    ManagedLocalJudgeReceiptRecordV1Input {
        judge_runtime_installation_generation: 7,
        judge_model_installation_generation: 11,
        managed_preflight_digest: digest("judge managed preflight"),
        retained_session_preflight_digest: digest("judge retained preflight"),
        residency_receipt_aggregate_digest: digest("judge residency aggregate"),
        process_observation_aggregate_digest: digest("judge process aggregate"),
        native_load_observation_aggregate_digest: digest("judge native aggregate"),
        connection_observation_aggregate_digest: digest("judge connection aggregate"),
        effective_runtime_state_observation_aggregate_digest: digest("judge state aggregate"),
        judge_effective_runtime_state_join_id:
            ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest("judge live join")),
        first_response_ordinal: 8,
        last_response_ordinal: 25,
    }
}
