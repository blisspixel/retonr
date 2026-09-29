use rewrite_model::{
    CandidateJudgeJoinRecordV1, CandidateJudgeJoinRecordV1Relations,
    CandidateJudgeTriageReportRelationshipV1, ManagedLocalJudgeReceiptRecordV1,
    ManagedLocalJudgeReceiptRecordV1Input, ManagedLocalJudgeReceiptRecordV1Relations,
};

use super::aggregates::AggregateStage;
use super::plan::PlanStage;
use super::{ExecutionFacts, Expected, map_contract, require_same};
use crate::StoreResult;
use crate::store::candidate_judge_execution::context::Loaded;
use crate::store::candidate_judge_execution::derive;

pub(super) struct ClosureStage {
    pub(super) triage: CandidateJudgeTriageReportRelationshipV1,
    pub(super) receipt: ManagedLocalJudgeReceiptRecordV1,
    pub(super) join: CandidateJudgeJoinRecordV1,
}

pub(super) fn build(
    loaded: &Loaded,
    facts: &ExecutionFacts<'_>,
    planned: &PlanStage,
    aggregated: &AggregateStage,
    expected: Option<&Expected<'_>>,
) -> StoreResult<ClosureStage> {
    let span = derive::ordinal_span(planned.schedule.entry_count())?;
    let triage =
        CandidateJudgeTriageReportRelationshipV1::new(facts.triage_report).map_err(map_contract)?;
    let receipt_relations = ManagedLocalJudgeReceiptRecordV1Relations {
        plan: &planned.plan,
        schedule: &planned.schedule,
        judge_system: loaded.judge.generation_system(),
        request_aggregate: &aggregated.requests,
        response_aggregate: &aggregated.responses,
        observation_batch: &aggregated.observations,
    };
    let receipt = require_same(
        ManagedLocalJudgeReceiptRecordV1::new(
            receipt_relations,
            receipt_input(facts, span.first, span.last),
        )
        .map_err(map_contract)?,
        expected.map(|value| value.receipt),
    )?;
    let join = require_same(
        CandidateJudgeJoinRecordV1::new(CandidateJudgeJoinRecordV1Relations {
            plan: &planned.plan,
            candidate_a_receipt_set: facts.candidate_a_receipt_set,
            candidate_b_receipt_set: facts.candidate_b_receipt_set,
            deterministic_evaluation: &planned.deterministic,
            schedule: &planned.schedule,
            request_aggregate: &aggregated.requests,
            response_aggregate: &aggregated.responses,
            observation_batch: &aggregated.observations,
            managed_receipt: &receipt,
            judge_system: loaded.judge.generation_system(),
            triage_report: &triage,
        })
        .map_err(map_contract)?,
        expected.map(|value| value.join),
    )?;
    Ok(ClosureStage {
        triage,
        receipt,
        join,
    })
}

fn receipt_input(
    facts: &ExecutionFacts<'_>,
    first_response_ordinal: u64,
    last_response_ordinal: u64,
) -> ManagedLocalJudgeReceiptRecordV1Input {
    let mut input = facts.managed_receipt_input.clone();
    input.first_response_ordinal = first_response_ordinal;
    input.last_response_ordinal = last_response_ordinal;
    input
}
