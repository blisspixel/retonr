use rewrite_model::{
    CandidateJudgeChoiceV1, CandidateJudgeObservationBatchV1, CandidateJudgeObservationV1,
    CandidateJudgeResponseAggregateV1, GenerationSystemRecordV1, ManagedLocalJudgeReceiptRecordV1,
    ManagedLocalJudgeReceiptRecordV1Input, ManagedLocalJudgeReceiptRecordV1Relations,
    ManagedOllamaEffectiveRuntimeStateJoinId, OllamaRetainedSessionResponseId,
};
use rewrite_types::{CancellationToken, Digest};

use super::super::CandidateJudgeJoinPrimaryError;
use crate::candidate_judge_preparation::CandidateJudgeJoinCompilationError;
use crate::candidate_judge_preparation::tests::{prepare, ready_config};
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::local_ollama_managed_preflight::generation::managed_schedule_runner::ManagedJudgeScheduleAuthorityFailures;
use crate::verified_candidate_batch_set::tests::offline_judge_system;
use crate::{
    CandidateJudgePreparationOutcome, CandidateJudgeRunnerHandoff,
    local_judge_prompt_contract_digest,
};

pub(super) fn handoff<'store>(
    fixture: &'store Fixture,
    suffix: &str,
) -> CandidateJudgeRunnerHandoff<'store> {
    let prepared = prepare(
        fixture,
        suffix,
        ready_config(vec![0]),
        &CancellationToken::new(),
    );
    let CandidateJudgePreparationOutcome::Ready(ready) = prepared.result.expect("ready") else {
        panic!("passed deterministic record must prepare")
    };
    ready
        .into_runner_handoff(&CancellationToken::new())
        .expect("runner handoff")
}

pub(in crate::local_ollama_managed_preflight::generation) fn portable_outputs(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
) -> (
    CandidateJudgeResponseAggregateV1,
    CandidateJudgeObservationBatchV1,
) {
    let response_ids = (0..handoff.judge_schedule().entries().len())
        .map(|index| {
            OllamaRetainedSessionResponseId::from_derived_digest(Digest::sha256(
                format!("verified join response {index}").as_bytes(),
            ))
        })
        .collect::<Vec<_>>();
    let responses = CandidateJudgeResponseAggregateV1::new(
        handoff.judge_plan(),
        handoff.judge_schedule(),
        handoff.request_aggregate(),
        response_ids.clone(),
    )
    .expect("response aggregate");
    let observations = observation_batch(handoff, &responses, CandidateJudgeChoiceV1::Tie);
    (responses, observations)
}

pub(in crate::local_ollama_managed_preflight::generation) fn observation_batch(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
    responses: &CandidateJudgeResponseAggregateV1,
    choice: CandidateJudgeChoiceV1,
) -> CandidateJudgeObservationBatchV1 {
    let observations = responses
        .responses()
        .iter()
        .enumerate()
        .map(|(index, response)| {
            let entry = &handoff.judge_schedule().entries()[index];
            let planned = handoff
                .judge_plan()
                .cases()
                .iter()
                .find(|case| case.case_id() == entry.case_id())
                .expect("planned case");
            CandidateJudgeObservationV1::new(
                handoff.judge_plan(),
                handoff.judge_schedule(),
                handoff.request_aggregate(),
                index,
                response.retained_session_response_id().clone(),
                choice,
                planned.rubric_clause_ids().to_vec(),
            )
            .expect("observation")
        })
        .collect();
    CandidateJudgeObservationBatchV1::new(
        handoff.judge_plan(),
        handoff.judge_schedule(),
        handoff.request_aggregate(),
        observations,
    )
    .expect("observation batch")
}

pub(in crate::local_ollama_managed_preflight::generation) fn managed_receipt(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
    responses: &CandidateJudgeResponseAggregateV1,
    observations: &CandidateJudgeObservationBatchV1,
) -> ManagedLocalJudgeReceiptRecordV1 {
    ManagedLocalJudgeReceiptRecordV1::new(
        ManagedLocalJudgeReceiptRecordV1Relations {
            plan: handoff.judge_plan(),
            schedule: handoff.judge_schedule(),
            judge_system: handoff.judge_system(),
            request_aggregate: handoff.request_aggregate(),
            response_aggregate: responses,
            observation_batch: observations,
        },
        managed_input(handoff),
    )
    .expect("managed receipt")
}

pub(super) fn receipt_with_runtime_generation(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
    responses: &CandidateJudgeResponseAggregateV1,
    observations: &CandidateJudgeObservationBatchV1,
    receipt: &ManagedLocalJudgeReceiptRecordV1,
    runtime_generation: u64,
) -> ManagedLocalJudgeReceiptRecordV1 {
    ManagedLocalJudgeReceiptRecordV1::new(
        ManagedLocalJudgeReceiptRecordV1Relations {
            plan: handoff.judge_plan(),
            schedule: handoff.judge_schedule(),
            judge_system: handoff.judge_system(),
            request_aggregate: handoff.request_aggregate(),
            response_aggregate: responses,
            observation_batch: observations,
        },
        ManagedLocalJudgeReceiptRecordV1Input {
            judge_runtime_installation_generation: runtime_generation,
            judge_model_installation_generation: receipt.judge_model_installation_generation(),
            managed_preflight_digest: receipt.managed_preflight_digest().clone(),
            retained_session_preflight_digest: receipt.retained_session_preflight_digest().clone(),
            residency_receipt_aggregate_digest: receipt
                .residency_receipt_aggregate_digest()
                .clone(),
            process_observation_aggregate_digest: receipt
                .process_observation_aggregate_digest()
                .clone(),
            native_load_observation_aggregate_digest: receipt
                .native_load_observation_aggregate_digest()
                .clone(),
            connection_observation_aggregate_digest: receipt
                .connection_observation_aggregate_digest()
                .clone(),
            effective_runtime_state_observation_aggregate_digest: receipt
                .effective_runtime_state_observation_aggregate_digest()
                .clone(),
            judge_effective_runtime_state_join_id: receipt
                .judge_effective_runtime_state_join_id()
                .clone(),
            first_response_ordinal: receipt.first_response_ordinal(),
            last_response_ordinal: receipt.last_response_ordinal(),
        },
    )
    .expect("valid substituted receipt")
}

pub(super) fn foreign_judge_system() -> GenerationSystemRecordV1 {
    offline_judge_system(
        "foreign join judge",
        local_judge_prompt_contract_digest(),
        rewrite_inference::local_judge_attempt_output_contract().schema_digest,
    )
}

pub(super) fn failures() -> ManagedJudgeScheduleAuthorityFailures {
    ManagedJudgeScheduleAuthorityFailures::cancelled_for_test()
}

pub(super) fn primary() -> CandidateJudgeJoinPrimaryError {
    CandidateJudgeJoinPrimaryError::Join(CandidateJudgeJoinCompilationError::Cancelled)
}

pub(in crate::local_ollama_managed_preflight::generation) fn managed_input(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
) -> ManagedLocalJudgeReceiptRecordV1Input {
    let count = u64::from(handoff.judge_schedule().entry_count());
    ManagedLocalJudgeReceiptRecordV1Input {
        judge_runtime_installation_generation: 7,
        judge_model_installation_generation: 11,
        managed_preflight_digest: Digest::sha256(b"join managed preflight"),
        retained_session_preflight_digest: Digest::sha256(b"join retained preflight"),
        residency_receipt_aggregate_digest: Digest::sha256(b"join residency aggregate"),
        process_observation_aggregate_digest: Digest::sha256(b"join process aggregate"),
        native_load_observation_aggregate_digest: Digest::sha256(b"join native aggregate"),
        connection_observation_aggregate_digest: Digest::sha256(b"join connection aggregate"),
        effective_runtime_state_observation_aggregate_digest: Digest::sha256(
            b"join effective state aggregate",
        ),
        judge_effective_runtime_state_join_id:
            ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(Digest::sha256(
                b"join effective state",
            )),
        first_response_ordinal: 8,
        last_response_ordinal: 7 + count * 9,
    }
}
