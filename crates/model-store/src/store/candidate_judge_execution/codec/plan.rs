use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateJudgePlanV1, CandidateJudgePlanV1Input,
    CandidateJudgePlanV1Relations, CandidateJudgeScheduleV1, CandidateReceiptPairSetId,
};

use super::{ExecutionFacts, Expected, map_contract, require_same};
use crate::store::candidate_judge_execution::context::Loaded;
use crate::{StoreError, StoreResult};

pub(super) struct PlanStage {
    pub(super) plan: CandidateJudgePlanV1,
    pub(super) pair_set_id: CandidateReceiptPairSetId,
    pub(super) deterministic: CandidateDeterministicEvaluationRecordV1,
    pub(super) schedule: CandidateJudgeScheduleV1,
}

pub(super) fn build(
    loaded: &Loaded,
    facts: &ExecutionFacts<'_>,
    expected: Option<&Expected<'_>>,
) -> StoreResult<PlanStage> {
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
    let relations = CandidateJudgePlanV1Relations {
        qualification_plan: loaded.foundation.plan(),
        suite: loaded.foundation.suite(),
        repetition,
        selection_policy: loaded.foundation.candidate_selection_policy(),
        planned_attempts: loaded.foundation.planned_attempts(),
        candidate_a_system: candidate_a,
        candidate_b_system: candidate_b,
        judge_system: loaded.judge.generation_system(),
    };
    let plan = require_same(
        CandidateJudgePlanV1::new(relations, owned_plan_input(facts.plan_input))
            .map_err(map_contract)?,
        expected.map(|value| value.plan),
    )?;
    let pair_set_id = CandidateReceiptPairSetId::from_receipt_sets(
        facts.candidate_a_receipt_set,
        facts.candidate_b_receipt_set,
    )
    .map_err(map_contract)?;
    let deterministic = require_same(
        CandidateDeterministicEvaluationRecordV1::new(
            facts.candidate_a_receipt_set,
            facts.candidate_b_receipt_set,
            facts.deterministic_input,
        )
        .map_err(map_contract)?,
        expected.map(|value| value.deterministic),
    )?;
    if deterministic.candidate_receipt_pair_set_id() != &pair_set_id {
        return Err(StoreError::CorruptRecord);
    }
    let schedule = require_same(
        CandidateJudgeScheduleV1::new(&plan, &pair_set_id).map_err(map_contract)?,
        expected.map(|value| value.schedule),
    )?;
    Ok(PlanStage {
        plan,
        pair_set_id,
        deterministic,
        schedule,
    })
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
