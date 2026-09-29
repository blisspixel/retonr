use rewrite_model::GenerationSystemId;
use rusqlite::Connection;

use super::codec::ExecutionFacts;
use crate::store::generation_qualification_plan_foundation::{
    self, GenerationQualificationPlanFoundationV1,
};
use crate::store::generation_system_foundation::{self, GenerationSystemFoundationV1};
use crate::{StoreError, StoreResult};

pub(super) struct Loaded {
    pub(super) foundation: GenerationQualificationPlanFoundationV1,
    pub(super) judge: GenerationSystemFoundationV1,
    pub(super) repetition_index: usize,
    pub(super) candidate_a_index: usize,
    pub(super) candidate_b_index: usize,
}

pub(super) fn load(connection: &Connection, facts: &ExecutionFacts<'_>) -> StoreResult<Loaded> {
    let foundation = generation_qualification_plan_foundation::load_foundation(
        connection,
        facts.qualification_plan_id,
    )?
    .ok_or(StoreError::MissingRecord)?;
    let judge = generation_system_foundation::load_foundation(
        connection,
        facts.judge_generation_system_id,
    )?
    .ok_or(StoreError::MissingRecord)?;
    if judge.generation_system().generation_system_id() != facts.judge_generation_system_id {
        return Err(StoreError::CorruptRecord);
    }
    let repetition_index = foundation
        .repetitions()
        .iter()
        .position(|value| value.repetition_id() == facts.repetition_id)
        .ok_or(StoreError::CorruptRecord)?;
    let first_system_index = system_index(
        foundation.generation_systems(),
        facts.candidate_a_generation_system_id,
    )?;
    let second_system_index = system_index(
        foundation.generation_systems(),
        facts.candidate_b_generation_system_id,
    )?;
    if first_system_index == second_system_index {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Loaded {
        foundation,
        judge,
        repetition_index,
        candidate_a_index: first_system_index,
        candidate_b_index: second_system_index,
    })
}

fn system_index(
    systems: &[rewrite_model::GenerationSystemRecordV1],
    id: &GenerationSystemId,
) -> StoreResult<usize> {
    systems
        .iter()
        .position(|value| value.generation_system_id() == id)
        .ok_or(StoreError::CorruptRecord)
}
