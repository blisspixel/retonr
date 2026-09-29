use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationRecordV1Input,
    CandidateDeterministicReportRelationshipV1, CandidateDeterministicReportSummaryV1,
    CandidateDeterministicTransformationCoverageV1, CandidateGenerationReceiptSetV1,
    CandidateJudgeCaseV1, CandidateJudgeChoiceV1, CandidateJudgeJoinRecordV1,
    CandidateJudgeJoinRecordV1Relations, CandidateJudgeLimitsV1, CandidateJudgeObservationBatchV1,
    CandidateJudgeObservationV1, CandidateJudgeOrderPolicyV1, CandidateJudgePlanV1,
    CandidateJudgePlanV1Input, CandidateJudgePlanV1Relations, CandidateJudgeRequestAggregateV1,
    CandidateJudgeResponseAggregateV1, CandidateJudgeScheduleV1,
    CandidateJudgeTriageReportRelationshipV1, CandidateReceiptPairSetId, GenerationSystemRecordV1,
    ManagedLocalJudgeReceiptRecordV1, ManagedLocalJudgeReceiptRecordV1Input,
    ManagedLocalJudgeReceiptRecordV1Relations, ManagedOllamaEffectiveRuntimeStateJoinId,
    OllamaRetainedSessionResponseId, StructuredCompletionRequestBindingId,
};
use rewrite_types::Digest;

use super::super::{
    CandidateJudgeExecutionV1Input, CandidateJudgeExecutionV1ReadInput,
    CandidateJudgeObservationFactV1,
};
use crate::ArtifactStateStore;
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};
use crate::store::generation_system_foundation::GenerationSystemFoundationV1Input;

#[path = "support/receipts.rs"]
mod receipts;

pub(super) struct Cohort {
    pub(super) fixture: Fixture,
    pub(super) judge: GenerationSystemRecordV1,
    pub(super) receipt_a: CandidateGenerationReceiptSetV1,
    pub(super) receipt_b: CandidateGenerationReceiptSetV1,
    pub(super) case_material: Digest,
    pub(super) suite_pair: Digest,
    pub(super) reports: CandidateDeterministicReportRelationshipV1,
    pub(super) deterministic: CandidateDeterministicEvaluationRecordV1,
    pub(super) plan_input: CandidateJudgePlanV1Input,
    pub(super) plan: CandidateJudgePlanV1,
    pub(super) schedule: CandidateJudgeScheduleV1,
    pub(super) request_ids: Vec<StructuredCompletionRequestBindingId>,
    pub(super) response_ids: Vec<OllamaRetainedSessionResponseId>,
    pub(super) clauses: Vec<String>,
    pub(super) requests: CandidateJudgeRequestAggregateV1,
    pub(super) responses: CandidateJudgeResponseAggregateV1,
    pub(super) observations: CandidateJudgeObservationBatchV1,
    pub(super) receipt_input: ManagedLocalJudgeReceiptRecordV1Input,
    pub(super) receipt: ManagedLocalJudgeReceiptRecordV1,
    pub(super) triage: Vec<u8>,
    pub(super) join: CandidateJudgeJoinRecordV1,
}

pub(super) struct Session {
    pub(super) _directory: tempfile::TempDir,
    pub(super) store: ArtifactStateStore,
    pub(super) cohort: Cohort,
}

pub(super) fn session(seed: u64) -> Session {
    let directory = tempfile::tempdir().expect("temp directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let cohort = cohort(seed);
    support::persist_plan_foundation(&mut store, &cohort.fixture);
    store
        .transact_generation_system_foundation_v1(
            GenerationSystemFoundationV1Input {
                generation_system: &cohort.judge,
                relations: cohort.fixture.system.relations(),
            },
            |_| Ok::<_, ()>(()),
        )
        .expect("judge system");
    Session {
        _directory: directory,
        store,
        cohort,
    }
}

pub(super) fn cohort(seed: u64) -> Cohort {
    let fixture = support::fixture();
    let judge =
        GenerationSystemRecordV1::new(fixture.system.relations(), fixture.system.input("judge"))
            .expect("judge");
    let receipt_a = receipts::receipt_set(&fixture, 0);
    let receipt_b = receipts::receipt_set(&fixture, 1);
    let case_material = digest("judge case material");
    let suite_pair = digest("judge suite pair");
    let reports = reports(true);
    let deterministic = CandidateDeterministicEvaluationRecordV1::new(
        &receipt_a,
        &receipt_b,
        deterministic_input(&case_material, &suite_pair, &reports),
    )
    .expect("deterministic evaluation");
    let plan_input = plan_input(&judge, &fixture, &case_material, seed);
    let plan = CandidateJudgePlanV1::new(
        plan_relations(&fixture, &judge),
        owned_plan_input(&plan_input),
    )
    .expect("judge plan");
    let pair = CandidateReceiptPairSetId::from_receipt_sets(&receipt_a, &receipt_b).expect("pair");
    let schedule = CandidateJudgeScheduleV1::new(&plan, &pair).expect("schedule");
    let request_ids = binding_ids();
    let response_ids = response_ids();
    let requests = CandidateJudgeRequestAggregateV1::new(&plan, &schedule, request_ids.clone())
        .expect("requests");
    let responses =
        CandidateJudgeResponseAggregateV1::new(&plan, &schedule, &requests, response_ids.clone())
            .expect("responses");
    let clauses = vec!["meaning".to_owned()];
    let observations = observation_batch(&plan, &schedule, &requests, &response_ids, &clauses);
    let stored_receipt_input = receipt_input(1, 2);
    let receipt = ManagedLocalJudgeReceiptRecordV1::new(
        receipt_relations(
            &plan,
            &schedule,
            &judge,
            &requests,
            &responses,
            &observations,
        ),
        receipt_input(8, 25),
    )
    .expect("managed receipt");
    let triage = br#"{"schema_version":1,"status":"triage"}"#.to_vec();
    let triage_report =
        CandidateJudgeTriageReportRelationshipV1::new(&triage).expect("triage report");
    let join = CandidateJudgeJoinRecordV1::new(CandidateJudgeJoinRecordV1Relations {
        plan: &plan,
        candidate_a_receipt_set: &receipt_a,
        candidate_b_receipt_set: &receipt_b,
        deterministic_evaluation: &deterministic,
        schedule: &schedule,
        request_aggregate: &requests,
        response_aggregate: &responses,
        observation_batch: &observations,
        managed_receipt: &receipt,
        judge_system: &judge,
        triage_report: &triage_report,
    })
    .expect("join");
    Cohort {
        fixture,
        judge,
        receipt_a,
        receipt_b,
        case_material,
        suite_pair,
        reports,
        deterministic,
        plan_input,
        plan,
        schedule,
        request_ids,
        response_ids,
        clauses,
        requests,
        responses,
        observations,
        receipt_input: stored_receipt_input,
        receipt,
        triage,
        join,
    }
}

pub(super) fn facts(cohort: &Cohort) -> Vec<CandidateJudgeObservationFactV1<'_>> {
    cohort
        .response_ids
        .iter()
        .map(
            |retained_session_response_id| CandidateJudgeObservationFactV1 {
                retained_session_response_id,
                choice: CandidateJudgeChoiceV1::First,
                cited_rubric_clause_ids: &cohort.clauses,
            },
        )
        .collect()
}

pub(super) fn write_input<'a>(
    cohort: &'a Cohort,
    facts: &'a [CandidateJudgeObservationFactV1<'a>],
) -> CandidateJudgeExecutionV1Input<'a> {
    CandidateJudgeExecutionV1Input {
        qualification_plan_id: cohort.fixture.plan.qualification_plan_id(),
        repetition_id: cohort.fixture.repetitions[0].repetition_id(),
        candidate_a_generation_system_id: cohort.fixture.systems[0].generation_system_id(),
        candidate_b_generation_system_id: cohort.fixture.systems[1].generation_system_id(),
        judge_generation_system_id: cohort.judge.generation_system_id(),
        plan: &cohort.plan,
        plan_input: &cohort.plan_input,
        candidate_a_receipt_set: &cohort.receipt_a,
        candidate_b_receipt_set: &cohort.receipt_b,
        deterministic_evaluation: &cohort.deterministic,
        deterministic_input: deterministic_input(
            &cohort.case_material,
            &cohort.suite_pair,
            &cohort.reports,
        ),
        request_binding_ids: &cohort.request_ids,
        retained_session_response_ids: &cohort.response_ids,
        observation_facts: facts,
        schedule: &cohort.schedule,
        request_aggregate: &cohort.requests,
        response_aggregate: &cohort.responses,
        observation_batch: &cohort.observations,
        managed_receipt: &cohort.receipt,
        managed_receipt_input: &cohort.receipt_input,
        triage_report: &cohort.triage,
        join: &cohort.join,
    }
}

pub(super) fn read_input<'a>(
    cohort: &'a Cohort,
    facts: &'a [CandidateJudgeObservationFactV1<'a>],
) -> CandidateJudgeExecutionV1ReadInput<'a> {
    let write = write_input(cohort, facts);
    CandidateJudgeExecutionV1ReadInput {
        qualification_plan_id: write.qualification_plan_id,
        repetition_id: write.repetition_id,
        candidate_a_generation_system_id: write.candidate_a_generation_system_id,
        candidate_b_generation_system_id: write.candidate_b_generation_system_id,
        judge_generation_system_id: write.judge_generation_system_id,
        plan_input: write.plan_input,
        candidate_a_receipt_set: write.candidate_a_receipt_set,
        candidate_b_receipt_set: write.candidate_b_receipt_set,
        deterministic_input: write.deterministic_input,
        request_binding_ids: write.request_binding_ids,
        retained_session_response_ids: write.retained_session_response_ids,
        observation_facts: write.observation_facts,
        managed_receipt_input: write.managed_receipt_input,
        triage_report: write.triage_report,
    }
}

pub(super) fn reports(passed: bool) -> CandidateDeterministicReportRelationshipV1 {
    let coverage = CandidateDeterministicTransformationCoverageV1::new(0, 0).expect("coverage");
    let summary = |passed_count| {
        CandidateDeterministicReportSummaryV1::new(1, passed_count, coverage).expect("summary")
    };
    CandidateDeterministicReportRelationshipV1::new(
        br#"{"schema_version":1,"passed":1}"#,
        br#"{"schema_version":1,"passed":1}"#,
        summary(1),
        summary(u32::from(passed)),
    )
    .expect("reports")
}

fn plan_relations<'a>(
    fixture: &'a Fixture,
    judge: &'a GenerationSystemRecordV1,
) -> CandidateJudgePlanV1Relations<'a> {
    CandidateJudgePlanV1Relations {
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
        repetition: &fixture.repetitions[0],
        selection_policy: &fixture.selection_policy,
        planned_attempts: &fixture.attempts,
        candidate_a_system: &fixture.systems[0],
        candidate_b_system: &fixture.systems[1],
        judge_system: judge,
    }
}

fn plan_input(
    judge: &GenerationSystemRecordV1,
    fixture: &Fixture,
    case_material: &Digest,
    seed: u64,
) -> CandidateJudgePlanV1Input {
    CandidateJudgePlanV1Input {
        case_material_set_digest: case_material.clone(),
        rubric_digest: digest("judge rubric"),
        cases: vec![
            CandidateJudgeCaseV1::new(
                fixture.cases[0].case_id().clone(),
                vec!["meaning".to_owned()],
            )
            .expect("judge case"),
        ],
        order_policy: CandidateJudgeOrderPolicyV1::BothOrders,
        presentation_seed: seed,
        attempts_per_order: 1,
        limits: CandidateJudgeLimitsV1::new(1, 1_024, 1_024, 4_096, 4_096, 256, 4_096, 10_000)
            .expect("limits"),
        prompt_contract_digest: judge.prompt_digest().clone(),
        output_schema_digest: judge.output_schema_digest().clone(),
    }
}

fn owned_plan_input(input: &CandidateJudgePlanV1Input) -> CandidateJudgePlanV1Input {
    CandidateJudgePlanV1Input {
        case_material_set_digest: input.case_material_set_digest.clone(),
        rubric_digest: input.rubric_digest.clone(),
        cases: input.cases.clone(),
        order_policy: input.order_policy,
        presentation_seed: input.presentation_seed,
        attempts_per_order: input.attempts_per_order,
        limits: input.limits,
        prompt_contract_digest: input.prompt_contract_digest.clone(),
        output_schema_digest: input.output_schema_digest.clone(),
    }
}

fn binding_ids() -> Vec<StructuredCompletionRequestBindingId> {
    (0..2)
        .map(|index| {
            StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                "judge request {index}"
            )))
        })
        .collect()
}

fn response_ids() -> Vec<OllamaRetainedSessionResponseId> {
    (0..2)
        .map(|index| {
            OllamaRetainedSessionResponseId::from_derived_digest(digest(&format!(
                "judge response {index}"
            )))
        })
        .collect()
}

fn observation_batch(
    plan: &CandidateJudgePlanV1,
    schedule: &CandidateJudgeScheduleV1,
    requests: &CandidateJudgeRequestAggregateV1,
    response_ids: &[OllamaRetainedSessionResponseId],
    clauses: &[String],
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
                clauses.to_vec(),
            )
            .expect("observation")
        })
        .collect();
    CandidateJudgeObservationBatchV1::new(plan, schedule, requests, observations).expect("batch")
}

fn receipt_relations<'a>(
    plan: &'a CandidateJudgePlanV1,
    schedule: &'a CandidateJudgeScheduleV1,
    judge: &'a GenerationSystemRecordV1,
    requests: &'a CandidateJudgeRequestAggregateV1,
    responses: &'a CandidateJudgeResponseAggregateV1,
    observations: &'a CandidateJudgeObservationBatchV1,
) -> ManagedLocalJudgeReceiptRecordV1Relations<'a> {
    ManagedLocalJudgeReceiptRecordV1Relations {
        plan,
        schedule,
        judge_system: judge,
        request_aggregate: requests,
        response_aggregate: responses,
        observation_batch: observations,
    }
}

pub(super) fn receipt_input(
    first_response_ordinal: u64,
    last_response_ordinal: u64,
) -> ManagedLocalJudgeReceiptRecordV1Input {
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
        first_response_ordinal,
        last_response_ordinal,
    }
}

fn deterministic_input<'a>(
    case_material: &'a Digest,
    suite_pair: &'a Digest,
    reports: &'a CandidateDeterministicReportRelationshipV1,
) -> CandidateDeterministicEvaluationRecordV1Input<'a> {
    CandidateDeterministicEvaluationRecordV1Input {
        case_material_set_digest: case_material,
        suite_pair_digest: suite_pair,
        report_relationship: reports,
    }
}

pub(super) fn digest(value: &str) -> Digest {
    Digest::sha256(value.as_bytes())
}
