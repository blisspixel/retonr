//! Exact owned inert parent snapshots, independently retained above storage.

use crate::{StoreError, StoreResult, StoredCandidateJudgeExecutionV1};
use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateGenerationReceiptSetV1,
    GenerationRepeatabilityResultRecordV1,
};

/// Exact typed parent records for one preregistered Passed repetition.
///
/// This owned snapshot is inert and is not a live authority constructor. The
/// Active publisher derives it inside its validated same-subject join projection.
#[derive(Clone)]
pub struct GenerationRepeatabilityPhaseParentV1 {
    /// Independently retained exact target receipt set.
    pub target_receipt_set: CandidateGenerationReceiptSetV1,
    /// Independently retained exact baseline receipt set.
    pub baseline_receipt_set: CandidateGenerationReceiptSetV1,
    /// Independently derived exact deterministic compiler record.
    pub deterministic_evaluation: CandidateDeterministicEvaluationRecordV1,
    /// Exact seven-record cohort independently compared with the live join view.
    pub judge_execution: StoredCandidateJudgeExecutionV1,
}

pub(super) fn confirm(
    connection: &rusqlite::Connection,
    result: &GenerationRepeatabilityResultRecordV1,
    expected: &GenerationRepeatabilityPhaseParentV1,
    baseline_system_id: &rewrite_model::GenerationSystemId,
) -> StoreResult<()> {
    let judge = expected.judge_execution.join();
    let plan = expected.judge_execution.plan();
    if result.candidate_generation_receipt_set_id()
        != Some(expected.target_receipt_set.receipt_set_id())
        || result.candidate_deterministic_evaluation_id()
            != Some(
                expected
                    .deterministic_evaluation
                    .deterministic_evaluation_id(),
            )
        || result.candidate_judge_join_id() != Some(judge.candidate_judge_join_id())
        || plan.qualification_plan_id() != result.generation_qualification_plan_id()
        || plan.suite_manifest_id() != result.suite_manifest_id()
        || plan.repetition_id() != result.repetition_id()
        || plan.candidate_a_generation_system_id() != result.generation_system_id()
        || plan.candidate_b_generation_system_id() != baseline_system_id
        || judge.candidate_a_receipt_set_id() != expected.target_receipt_set.receipt_set_id()
        || judge.candidate_b_receipt_set_id() != expected.baseline_receipt_set.receipt_set_id()
        || judge.deterministic_evaluation_id()
            != expected
                .deterministic_evaluation
                .deterministic_evaluation_id()
    {
        return Err(StoreError::CorruptRecord);
    }
    for receipt in [&expected.target_receipt_set, &expected.baseline_receipt_set] {
        if crate::store::candidate_generation_receipt_set::load_candidate_receipt_set(
            connection, receipt,
        )?
        .as_ref()
            != Some(receipt)
        {
            return Err(StoreError::CorruptRecord);
        }
    }
    if crate::store::candidate_deterministic_evaluation::load_candidate_deterministic_evaluation(
        connection,
        &expected.deterministic_evaluation,
    )?
    .as_ref()
        != Some(&expected.deterministic_evaluation)
    {
        return Err(StoreError::CorruptRecord);
    }
    crate::store::candidate_judge_execution::confirm_candidate_judge_execution_snapshot(
        connection,
        &expected.judge_execution,
    )
}
