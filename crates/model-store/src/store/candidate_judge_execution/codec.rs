use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationRecordV1Input,
    CandidateGenerationReceiptSetV1, CandidateJudgeJoinRecordV1, CandidateJudgeObservationBatchV1,
    CandidateJudgePlanV1, CandidateJudgePlanV1Input, CandidateJudgeRequestAggregateV1,
    CandidateJudgeResponseAggregateV1, CandidateJudgeScheduleV1,
    CandidateJudgeTriageReportRelationshipV1, CandidateReceiptPairSetId,
    GenerationQualificationContractError, GenerationQualificationPlanId, GenerationRepetitionId,
    GenerationSystemId, MAX_CANDIDATE_JUDGE_JOIN_JSON_BYTES,
    MAX_CANDIDATE_JUDGE_OBSERVATION_BATCH_JSON_BYTES, MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES,
    MAX_CANDIDATE_JUDGE_REQUEST_AGGREGATE_JSON_BYTES,
    MAX_CANDIDATE_JUDGE_RESPONSE_AGGREGATE_JSON_BYTES, MAX_CANDIDATE_JUDGE_SCHEDULE_JSON_BYTES,
    MAX_MANAGED_LOCAL_JUDGE_RECEIPT_JSON_BYTES, ManagedLocalJudgeReceiptRecordV1,
    ManagedLocalJudgeReceiptRecordV1Input, OllamaRetainedSessionResponseId,
    StructuredCompletionRequestBindingId,
};
use rusqlite::Connection;
use serde::Serialize;

use super::context;
use super::derive::{self, Closed};
use super::{
    CandidateJudgeExecutionV1Input, CandidateJudgeExecutionV1ReadInput,
    CandidateJudgeObservationFactV1, StoredCandidateJudgeExecutionV1,
};
use crate::{StoreError, StoreResult};

mod aggregates;
mod closure;
mod plan;

pub(super) struct ExecutionFacts<'a> {
    pub(super) qualification_plan_id: &'a GenerationQualificationPlanId,
    pub(super) repetition_id: &'a GenerationRepetitionId,
    pub(super) candidate_a_generation_system_id: &'a GenerationSystemId,
    pub(super) candidate_b_generation_system_id: &'a GenerationSystemId,
    pub(super) judge_generation_system_id: &'a GenerationSystemId,
    pub(super) plan_input: &'a CandidateJudgePlanV1Input,
    pub(super) candidate_a_receipt_set: &'a CandidateGenerationReceiptSetV1,
    pub(super) candidate_b_receipt_set: &'a CandidateGenerationReceiptSetV1,
    pub(super) deterministic_input: CandidateDeterministicEvaluationRecordV1Input<'a>,
    pub(super) request_binding_ids: &'a [StructuredCompletionRequestBindingId],
    pub(super) retained_session_response_ids: &'a [OllamaRetainedSessionResponseId],
    pub(super) observation_facts: &'a [CandidateJudgeObservationFactV1<'a>],
    pub(super) managed_receipt_input: &'a ManagedLocalJudgeReceiptRecordV1Input,
    pub(super) triage_report: &'a [u8],
}

pub(super) struct Expected<'a> {
    pub(super) plan: &'a CandidateJudgePlanV1,
    pub(super) deterministic: &'a CandidateDeterministicEvaluationRecordV1,
    pub(super) schedule: &'a CandidateJudgeScheduleV1,
    pub(super) requests: &'a CandidateJudgeRequestAggregateV1,
    pub(super) responses: &'a CandidateJudgeResponseAggregateV1,
    pub(super) observations: &'a CandidateJudgeObservationBatchV1,
    pub(super) receipt: &'a ManagedLocalJudgeReceiptRecordV1,
    pub(super) join: &'a CandidateJudgeJoinRecordV1,
}

pub(super) struct CohortRecords {
    pub(super) plan: CandidateJudgePlanV1,
    pub(super) pair_set_id: CandidateReceiptPairSetId,
    pub(super) deterministic: CandidateDeterministicEvaluationRecordV1,
    pub(super) schedule: CandidateJudgeScheduleV1,
    pub(super) requests: CandidateJudgeRequestAggregateV1,
    pub(super) responses: CandidateJudgeResponseAggregateV1,
    pub(super) observations: CandidateJudgeObservationBatchV1,
    pub(super) triage: CandidateJudgeTriageReportRelationshipV1,
    pub(super) receipt: ManagedLocalJudgeReceiptRecordV1,
    pub(super) join: CandidateJudgeJoinRecordV1,
}

pub(super) struct EncodedCohort {
    pub(super) plan: Vec<u8>,
    pub(super) schedule: Vec<u8>,
    pub(super) requests: Vec<u8>,
    pub(super) responses: Vec<u8>,
    pub(super) observations: Vec<u8>,
    pub(super) receipt: Vec<u8>,
    pub(super) join: Vec<u8>,
}

pub(super) struct PreparedCohort<'a> {
    pub(super) records: CohortRecords,
    pub(super) encoded: EncodedCohort,
    pub(super) closed: Closed,
    pub(super) read: CandidateJudgeExecutionV1ReadInput<'a>,
}

pub(super) fn prepare<'a>(
    connection: &Connection,
    input: &'a CandidateJudgeExecutionV1Input<'a>,
) -> StoreResult<PreparedCohort<'a>> {
    let facts = facts_from_write(input);
    let expected = Expected {
        plan: input.plan,
        deterministic: input.deterministic_evaluation,
        schedule: input.schedule,
        requests: input.request_aggregate,
        responses: input.response_aggregate,
        observations: input.observation_batch,
        receipt: input.managed_receipt,
        join: input.join,
    };
    let (records, _loaded) = reconstruct(connection, &facts, Some(&expected))?;
    let encoded = encode(&records)?;
    let closed = derive::close(&records)?;
    Ok(PreparedCohort {
        records,
        encoded,
        closed,
        read: read_input(input),
    })
}

pub(super) fn reconstruct(
    connection: &Connection,
    facts: &ExecutionFacts<'_>,
    expected: Option<&Expected<'_>>,
) -> StoreResult<(CohortRecords, context::Loaded)> {
    let loaded = context::load(connection, facts)?;
    let planned = plan::build(&loaded, facts, expected)?;
    let aggregated = aggregates::build(&planned, facts, expected)?;
    let closed = closure::build(&loaded, facts, &planned, &aggregated, expected)?;
    Ok((
        CohortRecords {
            plan: planned.plan,
            pair_set_id: planned.pair_set_id,
            deterministic: planned.deterministic,
            schedule: planned.schedule,
            requests: aggregated.requests,
            responses: aggregated.responses,
            observations: aggregated.observations,
            triage: closed.triage,
            receipt: closed.receipt,
            join: closed.join,
        },
        loaded,
    ))
}

pub(super) fn matches_prepared(
    stored: &StoredCandidateJudgeExecutionV1,
    prepared: &PreparedCohort<'_>,
) -> bool {
    stored.plan() == &prepared.records.plan
        && stored.schedule() == &prepared.records.schedule
        && stored.request_aggregate() == &prepared.records.requests
        && stored.response_aggregate() == &prepared.records.responses
        && stored.observation_batch() == &prepared.records.observations
        && stored.managed_receipt() == &prepared.records.receipt
        && stored.join() == &prepared.records.join
}

pub(super) fn require_same<T: PartialEq>(reconstructed: T, caller: Option<&T>) -> StoreResult<T> {
    if caller.is_none_or(|value| value == &reconstructed) {
        Ok(reconstructed)
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

pub(super) fn map_contract(error: GenerationQualificationContractError) -> StoreError {
    match error {
        GenerationQualificationContractError::NonCanonicalEncoding => StoreError::CorruptRecord,
        GenerationQualificationContractError::EncodedRecordTooLarge
        | GenerationQualificationContractError::CanonicalEncodingTooLarge => {
            StoreError::RecordTooLarge
        }
        other => StoreError::InvalidGenerationQualificationPlan(other),
    }
}

fn facts_from_write<'a>(input: &'a CandidateJudgeExecutionV1Input<'a>) -> ExecutionFacts<'a> {
    ExecutionFacts {
        qualification_plan_id: input.qualification_plan_id,
        repetition_id: input.repetition_id,
        candidate_a_generation_system_id: input.candidate_a_generation_system_id,
        candidate_b_generation_system_id: input.candidate_b_generation_system_id,
        judge_generation_system_id: input.judge_generation_system_id,
        plan_input: input.plan_input,
        candidate_a_receipt_set: input.candidate_a_receipt_set,
        candidate_b_receipt_set: input.candidate_b_receipt_set,
        deterministic_input: input.deterministic_input,
        request_binding_ids: input.request_binding_ids,
        retained_session_response_ids: input.retained_session_response_ids,
        observation_facts: input.observation_facts,
        managed_receipt_input: input.managed_receipt_input,
        triage_report: input.triage_report,
    }
}

pub(super) fn facts_from_read<'a>(
    input: &'a CandidateJudgeExecutionV1ReadInput<'a>,
) -> ExecutionFacts<'a> {
    ExecutionFacts {
        qualification_plan_id: input.qualification_plan_id,
        repetition_id: input.repetition_id,
        candidate_a_generation_system_id: input.candidate_a_generation_system_id,
        candidate_b_generation_system_id: input.candidate_b_generation_system_id,
        judge_generation_system_id: input.judge_generation_system_id,
        plan_input: input.plan_input,
        candidate_a_receipt_set: input.candidate_a_receipt_set,
        candidate_b_receipt_set: input.candidate_b_receipt_set,
        deterministic_input: input.deterministic_input,
        request_binding_ids: input.request_binding_ids,
        retained_session_response_ids: input.retained_session_response_ids,
        observation_facts: input.observation_facts,
        managed_receipt_input: input.managed_receipt_input,
        triage_report: input.triage_report,
    }
}

fn read_input<'a>(
    input: &'a CandidateJudgeExecutionV1Input<'a>,
) -> CandidateJudgeExecutionV1ReadInput<'a> {
    CandidateJudgeExecutionV1ReadInput {
        qualification_plan_id: input.qualification_plan_id,
        repetition_id: input.repetition_id,
        candidate_a_generation_system_id: input.candidate_a_generation_system_id,
        candidate_b_generation_system_id: input.candidate_b_generation_system_id,
        judge_generation_system_id: input.judge_generation_system_id,
        plan_input: input.plan_input,
        candidate_a_receipt_set: input.candidate_a_receipt_set,
        candidate_b_receipt_set: input.candidate_b_receipt_set,
        deterministic_input: input.deterministic_input,
        request_binding_ids: input.request_binding_ids,
        retained_session_response_ids: input.retained_session_response_ids,
        observation_facts: input.observation_facts,
        managed_receipt_input: input.managed_receipt_input,
        triage_report: input.triage_report,
    }
}

fn encode(records: &CohortRecords) -> StoreResult<EncodedCohort> {
    Ok(EncodedCohort {
        plan: encode_record(&records.plan, MAX_CANDIDATE_JUDGE_PLAN_JSON_BYTES)?,
        schedule: encode_record(&records.schedule, MAX_CANDIDATE_JUDGE_SCHEDULE_JSON_BYTES)?,
        requests: encode_record(
            &records.requests,
            MAX_CANDIDATE_JUDGE_REQUEST_AGGREGATE_JSON_BYTES,
        )?,
        responses: encode_record(
            &records.responses,
            MAX_CANDIDATE_JUDGE_RESPONSE_AGGREGATE_JSON_BYTES,
        )?,
        observations: encode_record(
            &records.observations,
            MAX_CANDIDATE_JUDGE_OBSERVATION_BATCH_JSON_BYTES,
        )?,
        receipt: encode_record(&records.receipt, MAX_MANAGED_LOCAL_JUDGE_RECEIPT_JSON_BYTES)?,
        join: encode_record(&records.join, MAX_CANDIDATE_JUDGE_JOIN_JSON_BYTES)?,
    })
}

fn encode_record<T: Serialize>(value: &T, maximum: usize) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.is_empty() {
        Err(StoreError::CorruptRecord)
    } else if bytes.len() > maximum {
        Err(StoreError::RecordTooLarge)
    } else {
        Ok(bytes)
    }
}
