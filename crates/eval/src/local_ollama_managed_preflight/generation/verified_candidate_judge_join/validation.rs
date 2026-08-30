use rewrite_model::{
    CandidateJudgeJoinRecordV1, CandidateJudgeObservationBatchV1,
    CandidateJudgeResponseAggregateV1, GenerationSystemRecordV1, ManagedLocalJudgeReceiptRecordV1,
};
use rewrite_types::CancellationToken;

use super::{CompiledCandidateJudgeTriage, VerifiedCandidateJudgeJoinView};
use crate::CandidateJudgeRunnerHandoff;

pub(in crate::local_ollama_managed_preflight::generation) fn validate_join_view(
    view: &VerifiedCandidateJudgeJoinView<'_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), VerifiedCandidateJudgeJoinRelationshipError> {
    validate_join_parts(
        view.eval,
        view.judge_system,
        view.response_aggregate,
        view.observation_batch,
        view.managed_receipt,
        view.triage,
        view.record,
        cancellation,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the full retained and stored join closure remains explicit"
)]
pub(super) fn validate_join_parts(
    eval: &CandidateJudgeRunnerHandoff<'_>,
    judge_system: &GenerationSystemRecordV1,
    response_aggregate: &CandidateJudgeResponseAggregateV1,
    observation_batch: &CandidateJudgeObservationBatchV1,
    managed_receipt: &ManagedLocalJudgeReceiptRecordV1,
    stored_triage: &CompiledCandidateJudgeTriage,
    stored_record: &CandidateJudgeJoinRecordV1,
    cancellation: &CancellationToken,
) -> Result<(), VerifiedCandidateJudgeJoinRelationshipError> {
    let recompiled_triage = eval
        .compile_compatibility_triage(observation_batch, cancellation)
        .map_err(|_error| VerifiedCandidateJudgeJoinRelationshipError)?;
    if recompiled_triage.report() != stored_triage.report()
        || recompiled_triage.canonical_json() != stored_triage.canonical_json()
        || recompiled_triage.relationship() != stored_triage.relationship()
    {
        return Err(VerifiedCandidateJudgeJoinRelationshipError);
    }
    let recompiled_record = eval
        .compile_join_record(
            judge_system,
            response_aggregate,
            observation_batch,
            managed_receipt,
            &recompiled_triage,
            cancellation,
        )
        .map_err(|_error| VerifiedCandidateJudgeJoinRelationshipError)?;
    if &recompiled_record != stored_record {
        return Err(VerifiedCandidateJudgeJoinRelationshipError);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub(in crate::local_ollama_managed_preflight::generation) struct VerifiedCandidateJudgeJoinRelationshipError;
