//! Atomic inert judge cohort settlement with exact durable parent readback.

use super::{GenerationQualificationPreregistrationRepository, map_store_error};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, check_gate,
};
use rewrite_model_store::{
    CandidateDeterministicEvaluationV1ReadInput, CandidateGenerationReceiptSetV1ReadInput,
    CandidateJudgeExecutionV1Input, CandidateJudgeExecutionV1ReadInput,
    CandidateJudgeExecutionV1TransactionError, CandidateJudgeExecutionV1WriteDisposition,
    StoredCandidateJudgeExecutionV1,
};
use rewrite_types::CancellationToken;
use std::time::Instant;

impl GenerationQualificationPreregistrationRepository {
    pub(crate) fn persist_judge_execution(
        &mut self,
        input: CandidateJudgeExecutionV1Input<'_>,
        deadline: Instant,
        cancellation: &CancellationToken,
    ) -> Result<
        (
            StoredCandidateJudgeExecutionV1,
            CandidateJudgeExecutionV1WriteDisposition,
        ),
        GenerationQualificationPreparationError,
    > {
        check_gate(deadline, cancellation)?;
        for record in [input.candidate_a_receipt_set, input.candidate_b_receipt_set] {
            let stored = self
                .store
                .candidate_generation_receipt_set_v1(CandidateGenerationReceiptSetV1ReadInput {
                    record,
                })
                .map_err(|error| map_store_error(&error))?;
            if stored.as_ref() != Some(record) {
                return Err(GenerationQualificationPreparationError::ReadbackMismatch);
            }
        }
        let evaluation = self
            .store
            .candidate_deterministic_evaluation_v1(CandidateDeterministicEvaluationV1ReadInput {
                record: input.deterministic_evaluation,
            })
            .map_err(|error| map_store_error(&error))?;
        if evaluation.as_ref() != Some(input.deterministic_evaluation) {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        let disposition = self
            .store
            .transact_candidate_judge_execution_v1(input, || check_gate(deadline, cancellation))
            .map_err(|error| match error {
                CandidateJudgeExecutionV1TransactionError::Store(error) => map_store_error(&error),
                CandidateJudgeExecutionV1TransactionError::Gate(error) => error,
            })?;
        check_gate(deadline, cancellation)?;
        let stored = self
            .store
            .candidate_judge_execution_v1(CandidateJudgeExecutionV1ReadInput {
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
            })
            .map_err(|error| map_store_error(&error))?
            .ok_or(GenerationQualificationPreparationError::ReadbackMismatch)?;
        check_gate(deadline, cancellation)?;
        if stored.plan() != input.plan
            || stored.schedule() != input.schedule
            || stored.request_aggregate() != input.request_aggregate
            || stored.response_aggregate() != input.response_aggregate
            || stored.observation_batch() != input.observation_batch
            || stored.managed_receipt() != input.managed_receipt
            || stored.join() != input.join
        {
            return Err(GenerationQualificationPreparationError::ReadbackMismatch);
        }
        Ok((stored, disposition))
    }
}
