use rewrite_model::{
    CandidateJudgeObservationBatchV1, CandidateJudgeObservationV1,
    CandidateJudgeRequestAggregateV1, CandidateJudgeResponseAggregateV1,
};

use super::plan::PlanStage;
use super::{ExecutionFacts, Expected, map_contract, require_same};
use crate::{StoreError, StoreResult};

pub(super) struct AggregateStage {
    pub(super) requests: CandidateJudgeRequestAggregateV1,
    pub(super) responses: CandidateJudgeResponseAggregateV1,
    pub(super) observations: CandidateJudgeObservationBatchV1,
}

pub(super) fn build(
    planned: &PlanStage,
    facts: &ExecutionFacts<'_>,
    expected: Option<&Expected<'_>>,
) -> StoreResult<AggregateStage> {
    let requests = require_same(
        CandidateJudgeRequestAggregateV1::new(
            &planned.plan,
            &planned.schedule,
            facts.request_binding_ids.to_vec(),
        )
        .map_err(map_contract)?,
        expected.map(|value| value.requests),
    )?;
    let responses = require_same(
        CandidateJudgeResponseAggregateV1::new(
            &planned.plan,
            &planned.schedule,
            &requests,
            facts.retained_session_response_ids.to_vec(),
        )
        .map_err(map_contract)?,
        expected.map(|value| value.responses),
    )?;
    let observations = observation_batch(planned, &requests, facts)?;
    let observations = require_same(observations, expected.map(|value| value.observations))?;
    Ok(AggregateStage {
        requests,
        responses,
        observations,
    })
}

fn observation_batch(
    planned: &PlanStage,
    requests: &CandidateJudgeRequestAggregateV1,
    facts: &ExecutionFacts<'_>,
) -> StoreResult<CandidateJudgeObservationBatchV1> {
    if facts.observation_facts.len() != planned.schedule.entries().len() {
        return Err(StoreError::CorruptRecord);
    }
    let mut observations = Vec::with_capacity(facts.observation_facts.len());
    for (index, fact) in facts.observation_facts.iter().enumerate() {
        observations.push(
            CandidateJudgeObservationV1::new(
                &planned.plan,
                &planned.schedule,
                requests,
                index,
                fact.retained_session_response_id.clone(),
                fact.choice,
                fact.cited_rubric_clause_ids.to_vec(),
            )
            .map_err(map_contract)?,
        );
    }
    CandidateJudgeObservationBatchV1::new(&planned.plan, &planned.schedule, requests, observations)
        .map_err(map_contract)
}
