use std::fmt;

use rewrite_model::{
    CandidateJudgeJoinRecordV1, CandidateJudgeJoinRecordV1Relations,
    CandidateJudgeObservationBatchV1, CandidateJudgeResponseAggregateV1,
    CandidateJudgeTriageReportRelationshipV1, GenerationQualificationContractError,
    GenerationSystemRecordV1, ManagedLocalJudgeReceiptRecordV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{CandidateJudgeRunnerHandoff, CompiledCandidateJudgeTriage};
use crate::{CandidateJudgePreparationSide, VerifiedCandidateBatchSetError};

/// Exact relationship rejected while closing the portable candidate-judge join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateJudgeJoinCompilationRelationship {
    /// The app-retained judge system differed from the prepared eval authority.
    JudgeSystem,
    /// Canonical triage bytes and their framed digest differed.
    TriageReport,
}

/// Content-redacted failure from exact candidate-judge join compilation.
#[derive(Error)]
pub(crate) enum CandidateJudgeJoinCompilationError {
    /// Cooperative cancellation was observed before the record could be released.
    #[error("candidate judge join compilation was cancelled")]
    Cancelled,
    /// One retained candidate authority failed while its portable receipt was read.
    #[error("candidate judge join candidate authority failed for {side:?}")]
    CandidateBatchSet {
        /// Identity-significant candidate side.
        side: CandidateJudgePreparationSide,
        /// Typed authority failure, omitted from debug output.
        #[source]
        source: VerifiedCandidateBatchSetError,
    },
    /// One exact retained relationship differed.
    #[error("candidate judge join relationship is invalid: {0:?}")]
    Relationship(CandidateJudgeJoinCompilationRelationship),
    /// The portable join contract rejected the retained closure.
    #[error("candidate judge join portable contract failed")]
    Portable(#[source] GenerationQualificationContractError),
}

impl fmt::Debug for CandidateJudgeJoinCompilationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CandidateJudgeJoinCompilationError");
        match self {
            Self::Cancelled => debug.field("kind", &"cancelled"),
            Self::CandidateBatchSet { side, .. } => debug
                .field("kind", &"candidate_batch_set")
                .field("side", side),
            Self::Relationship(relationship) => debug
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::Portable(_) => debug.field("kind", &"portable"),
        };
        debug.finish_non_exhaustive()
    }
}

impl CandidateJudgeRunnerHandoff<'_> {
    /// Reloads the two held portable candidate receipt sets and closes the inert
    /// join record without widening access to either live candidate authority.
    pub(crate) fn compile_join_record(
        &self,
        judge_system: &GenerationSystemRecordV1,
        response_aggregate: &CandidateJudgeResponseAggregateV1,
        observation_batch: &CandidateJudgeObservationBatchV1,
        managed_receipt: &ManagedLocalJudgeReceiptRecordV1,
        triage: &CompiledCandidateJudgeTriage,
        cancellation: &CancellationToken,
    ) -> Result<CandidateJudgeJoinRecordV1, CandidateJudgeJoinCompilationError> {
        self.compile_join_record_from_triage_parts(
            judge_system,
            response_aggregate,
            observation_batch,
            managed_receipt,
            triage.canonical_json(),
            triage.relationship(),
            cancellation,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the test seam preserves every independent portable join input"
    )]
    fn compile_join_record_from_triage_parts(
        &self,
        judge_system: &GenerationSystemRecordV1,
        response_aggregate: &CandidateJudgeResponseAggregateV1,
        observation_batch: &CandidateJudgeObservationBatchV1,
        managed_receipt: &ManagedLocalJudgeReceiptRecordV1,
        triage_json: &[u8],
        triage_report: &CandidateJudgeTriageReportRelationshipV1,
        cancellation: &CancellationToken,
    ) -> Result<CandidateJudgeJoinRecordV1, CandidateJudgeJoinCompilationError> {
        ensure_active(cancellation)?;
        let candidate_a = self
            .candidate_a
            .receipt_set(cancellation)
            .map_err(
                |source| CandidateJudgeJoinCompilationError::CandidateBatchSet {
                    side: CandidateJudgePreparationSide::CandidateA,
                    source,
                },
            )?;
        let candidate_b = self
            .candidate_b
            .receipt_set(cancellation)
            .map_err(
                |source| CandidateJudgeJoinCompilationError::CandidateBatchSet {
                    side: CandidateJudgePreparationSide::CandidateB,
                    source,
                },
            )?;
        if judge_system != &self.judge_system {
            return Err(CandidateJudgeJoinCompilationError::Relationship(
                CandidateJudgeJoinCompilationRelationship::JudgeSystem,
            ));
        }
        let rederived_triage = CandidateJudgeTriageReportRelationshipV1::new(triage_json)
            .map_err(CandidateJudgeJoinCompilationError::Portable)?;
        if &rederived_triage != triage_report {
            return Err(CandidateJudgeJoinCompilationError::Relationship(
                CandidateJudgeJoinCompilationRelationship::TriageReport,
            ));
        }
        let record = CandidateJudgeJoinRecordV1::new(CandidateJudgeJoinRecordV1Relations {
            plan: &self.judge_plan,
            candidate_a_receipt_set: candidate_a,
            candidate_b_receipt_set: candidate_b,
            deterministic_evaluation: &self.deterministic_evaluation,
            schedule: &self.judge_schedule,
            request_aggregate: &self.request_aggregate,
            response_aggregate,
            observation_batch,
            managed_receipt,
            judge_system,
            triage_report,
        })
        .map_err(CandidateJudgeJoinCompilationError::Portable)?;
        ensure_active(cancellation)?;
        Ok(record)
    }

    #[cfg(test)]
    #[expect(
        clippy::too_many_arguments,
        reason = "the regression seam mutates only inert triage framing"
    )]
    pub(crate) fn compile_join_record_with_triage_for_test(
        &self,
        judge_system: &GenerationSystemRecordV1,
        response_aggregate: &CandidateJudgeResponseAggregateV1,
        observation_batch: &CandidateJudgeObservationBatchV1,
        managed_receipt: &ManagedLocalJudgeReceiptRecordV1,
        triage_json: &[u8],
        triage_report: &CandidateJudgeTriageReportRelationshipV1,
        cancellation: &CancellationToken,
    ) -> Result<CandidateJudgeJoinRecordV1, CandidateJudgeJoinCompilationError> {
        self.compile_join_record_from_triage_parts(
            judge_system,
            response_aggregate,
            observation_batch,
            managed_receipt,
            triage_json,
            triage_report,
            cancellation,
        )
    }
}

fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), CandidateJudgeJoinCompilationError> {
    if cancellation.is_cancelled() {
        Err(CandidateJudgeJoinCompilationError::Cancelled)
    } else {
        Ok(())
    }
}
