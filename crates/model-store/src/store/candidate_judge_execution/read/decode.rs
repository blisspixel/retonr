use rewrite_model::{
    CandidateJudgeJoinRecordV1, CandidateJudgeJoinRecordV1Relations,
    CandidateJudgeObservationBatchV1, CandidateJudgePlanV1, CandidateJudgePlanV1Relations,
    CandidateJudgeRequestAggregateV1, CandidateJudgeResponseAggregateV1, CandidateJudgeScheduleV1,
    ManagedLocalJudgeReceiptRecordV1, ManagedLocalJudgeReceiptRecordV1Relations,
};

use super::super::codec::{CohortRecords, ExecutionFacts};
use super::super::context::Loaded;
use super::Present;
use crate::{StoreError, StoreResult};

pub(super) fn cohort(
    present: &Present,
    reconstructed: &CohortRecords,
    loaded: &Loaded,
    facts: &ExecutionFacts<'_>,
) -> StoreResult<CohortRecords> {
    let plan = corrupt(CandidateJudgePlanV1::from_json_bytes(
        &present.plan.bytes,
        plan_relations(loaded)?,
    ))?;
    let schedule = corrupt(CandidateJudgeScheduleV1::from_json_bytes(
        &present.schedule.bytes,
        &plan,
        &reconstructed.pair_set_id,
    ))?;
    let requests = corrupt(CandidateJudgeRequestAggregateV1::from_json_bytes(
        &present.request.bytes,
        &plan,
        &schedule,
    ))?;
    let responses = corrupt(CandidateJudgeResponseAggregateV1::from_json_bytes(
        &present.response.bytes,
        &plan,
        &schedule,
        &requests,
    ))?;
    let observations = corrupt(CandidateJudgeObservationBatchV1::from_json_bytes(
        &present.batch.bytes,
        &plan,
        &schedule,
        &requests,
    ))?;
    let receipt = corrupt(ManagedLocalJudgeReceiptRecordV1::from_json_bytes(
        &present.receipt.bytes,
        ManagedLocalJudgeReceiptRecordV1Relations {
            plan: &plan,
            schedule: &schedule,
            judge_system: loaded.judge.generation_system(),
            request_aggregate: &requests,
            response_aggregate: &responses,
            observation_batch: &observations,
        },
    ))?;
    let join = corrupt(CandidateJudgeJoinRecordV1::from_json_bytes(
        &present.join.bytes,
        CandidateJudgeJoinRecordV1Relations {
            plan: &plan,
            candidate_a_receipt_set: facts.candidate_a_receipt_set,
            candidate_b_receipt_set: facts.candidate_b_receipt_set,
            deterministic_evaluation: &reconstructed.deterministic,
            schedule: &schedule,
            request_aggregate: &requests,
            response_aggregate: &responses,
            observation_batch: &observations,
            managed_receipt: &receipt,
            judge_system: loaded.judge.generation_system(),
            triage_report: &reconstructed.triage,
        },
    ))?;
    Ok(CohortRecords {
        plan,
        pair_set_id: reconstructed.pair_set_id.clone(),
        deterministic: reconstructed.deterministic.clone(),
        schedule,
        requests,
        responses,
        observations,
        triage: reconstructed.triage.clone(),
        receipt,
        join,
    })
}

fn plan_relations(loaded: &Loaded) -> StoreResult<CandidateJudgePlanV1Relations<'_>> {
    let repetition = loaded
        .foundation
        .repetitions()
        .get(loaded.repetition_index)
        .ok_or(StoreError::CorruptRecord)?;
    let candidate_a = loaded
        .foundation
        .generation_systems()
        .get(loaded.candidate_a_index)
        .ok_or(StoreError::CorruptRecord)?;
    let candidate_b = loaded
        .foundation
        .generation_systems()
        .get(loaded.candidate_b_index)
        .ok_or(StoreError::CorruptRecord)?;
    Ok(CandidateJudgePlanV1Relations {
        qualification_plan: loaded.foundation.plan(),
        suite: loaded.foundation.suite(),
        repetition,
        selection_policy: loaded.foundation.candidate_selection_policy(),
        planned_attempts: loaded.foundation.planned_attempts(),
        candidate_a_system: candidate_a,
        candidate_b_system: candidate_b,
        judge_system: loaded.judge.generation_system(),
    })
}

fn corrupt<T>(
    result: Result<T, rewrite_model::GenerationQualificationContractError>,
) -> StoreResult<T> {
    result.map_err(|_| StoreError::CorruptRecord)
}
