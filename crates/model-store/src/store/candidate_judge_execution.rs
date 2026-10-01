//! Atomic schema 12 judge-execution cohort for one pre-output judge plan.
//!
//! The stored cohort is inert. It grants no live, qualification, or activation
//! authority, and this module does not write a qualification record.

use std::{error::Error, fmt};

use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationRecordV1Input,
    CandidateGenerationReceiptSetV1, CandidateJudgeChoiceV1, CandidateJudgeJoinRecordV1,
    CandidateJudgeObservationBatchV1, CandidateJudgePlanV1, CandidateJudgePlanV1Input,
    CandidateJudgeRequestAggregateV1, CandidateJudgeResponseAggregateV1, CandidateJudgeScheduleV1,
    GenerationQualificationPlanId, GenerationRepetitionId, GenerationSystemId,
    ManagedLocalJudgeReceiptRecordV1, ManagedLocalJudgeReceiptRecordV1Input,
    OllamaRetainedSessionResponseId, StructuredCompletionRequestBindingId,
};
use rusqlite::TransactionBehavior;

use super::{ArtifactStateStore, WriteDisposition};
use crate::{StoreError, StoreResult};

mod codec;
mod context;
mod derive;
mod read;
mod rows;
mod text;
mod write;

pub(crate) fn confirm_candidate_judge_execution_snapshot(
    connection: &rusqlite::Connection,
    expected: &StoredCandidateJudgeExecutionV1,
) -> StoreResult<()> {
    read::confirm_snapshot(connection, expected)
}

/// One schedule-ordered observation fact supplied by the caller.
#[derive(Clone, Copy)]
pub struct CandidateJudgeObservationFactV1<'a> {
    /// Retained-session response identity for this schedule index.
    pub retained_session_response_id: &'a OllamaRetainedSessionResponseId,
    /// Normalized judge choice.
    pub choice: CandidateJudgeChoiceV1,
    /// Strictly ascending cited rubric clause identifiers.
    pub cited_rubric_clause_ids: &'a [String],
}

/// Borrowed judge-execution records and the independent facts that rederive them.
#[derive(Clone, Copy)]
pub struct CandidateJudgeExecutionV1Input<'a> {
    /// Qualification plan that already stores candidates A and B.
    pub qualification_plan_id: &'a GenerationQualificationPlanId,
    /// Repetition selected from that plan foundation.
    pub repetition_id: &'a GenerationRepetitionId,
    /// Candidate A generation system.
    pub candidate_a_generation_system_id: &'a GenerationSystemId,
    /// Candidate B generation system.
    pub candidate_b_generation_system_id: &'a GenerationSystemId,
    /// Judge generation system stored outside the plan-system set.
    pub judge_generation_system_id: &'a GenerationSystemId,
    /// Exact judge plan to store.
    pub plan: &'a CandidateJudgePlanV1,
    /// Independently retained judge-plan facts.
    pub plan_input: &'a CandidateJudgePlanV1Input,
    /// Complete candidate A receipt set. It has no table in this cohort.
    pub candidate_a_receipt_set: &'a CandidateGenerationReceiptSetV1,
    /// Complete candidate B receipt set. It has no table in this cohort.
    pub candidate_b_receipt_set: &'a CandidateGenerationReceiptSetV1,
    /// Exact deterministic evaluation to rederive. It has no table in this cohort.
    pub deterministic_evaluation: &'a CandidateDeterministicEvaluationRecordV1,
    /// Independently retained deterministic report relationship.
    pub deterministic_input: CandidateDeterministicEvaluationRecordV1Input<'a>,
    /// Structured request identities in schedule order.
    pub request_binding_ids: &'a [StructuredCompletionRequestBindingId],
    /// Retained-session response identities in schedule order.
    pub retained_session_response_ids: &'a [OllamaRetainedSessionResponseId],
    /// Observation facts in schedule order.
    pub observation_facts: &'a [CandidateJudgeObservationFactV1<'a>],
    /// Exact judge schedule to store.
    pub schedule: &'a CandidateJudgeScheduleV1,
    /// Exact request aggregate to store.
    pub request_aggregate: &'a CandidateJudgeRequestAggregateV1,
    /// Exact response aggregate to store.
    pub response_aggregate: &'a CandidateJudgeResponseAggregateV1,
    /// Exact observation batch to store.
    pub observation_batch: &'a CandidateJudgeObservationBatchV1,
    /// Exact managed receipt to store.
    pub managed_receipt: &'a ManagedLocalJudgeReceiptRecordV1,
    /// Independently retained managed-receipt facts.
    pub managed_receipt_input: &'a ManagedLocalJudgeReceiptRecordV1Input,
    /// Triage-report bytes framed only. The report is not parsed.
    pub triage_report: &'a [u8],
    /// Exact candidate-judge join to store.
    pub join: &'a CandidateJudgeJoinRecordV1,
}

/// Independent facts required to cold-read one judge-execution cohort.
#[derive(Clone, Copy)]
pub struct CandidateJudgeExecutionV1ReadInput<'a> {
    /// Qualification plan that already stores candidates A and B.
    pub qualification_plan_id: &'a GenerationQualificationPlanId,
    /// Repetition selected from that plan foundation.
    pub repetition_id: &'a GenerationRepetitionId,
    /// Candidate A generation system.
    pub candidate_a_generation_system_id: &'a GenerationSystemId,
    /// Candidate B generation system.
    pub candidate_b_generation_system_id: &'a GenerationSystemId,
    /// Judge generation system stored outside the plan-system set.
    pub judge_generation_system_id: &'a GenerationSystemId,
    /// Independently retained judge-plan facts.
    pub plan_input: &'a CandidateJudgePlanV1Input,
    /// Complete candidate A receipt set. It has no table in this cohort.
    pub candidate_a_receipt_set: &'a CandidateGenerationReceiptSetV1,
    /// Complete candidate B receipt set. It has no table in this cohort.
    pub candidate_b_receipt_set: &'a CandidateGenerationReceiptSetV1,
    /// Independently retained deterministic report relationship.
    pub deterministic_input: CandidateDeterministicEvaluationRecordV1Input<'a>,
    /// Structured request identities in schedule order.
    pub request_binding_ids: &'a [StructuredCompletionRequestBindingId],
    /// Retained-session response identities in schedule order.
    pub retained_session_response_ids: &'a [OllamaRetainedSessionResponseId],
    /// Observation facts in schedule order.
    pub observation_facts: &'a [CandidateJudgeObservationFactV1<'a>],
    /// Independently retained managed-receipt facts.
    pub managed_receipt_input: &'a ManagedLocalJudgeReceiptRecordV1Input,
    /// Triage-report bytes framed only. The report is not parsed.
    pub triage_report: &'a [u8],
}

/// Owned judge-execution cohort reconstructed from durable rows.
///
/// The value is inert. It grants no live, qualification, or activation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredCandidateJudgeExecutionV1 {
    plan: CandidateJudgePlanV1,
    schedule: CandidateJudgeScheduleV1,
    request_aggregate: CandidateJudgeRequestAggregateV1,
    response_aggregate: CandidateJudgeResponseAggregateV1,
    observation_batch: CandidateJudgeObservationBatchV1,
    managed_receipt: ManagedLocalJudgeReceiptRecordV1,
    join: CandidateJudgeJoinRecordV1,
}

impl StoredCandidateJudgeExecutionV1 {
    /// Returns the exact judge plan.
    #[must_use]
    pub const fn plan(&self) -> &CandidateJudgePlanV1 {
        &self.plan
    }

    /// Returns the exact judge schedule.
    #[must_use]
    pub const fn schedule(&self) -> &CandidateJudgeScheduleV1 {
        &self.schedule
    }

    /// Returns the exact request aggregate.
    #[must_use]
    pub const fn request_aggregate(&self) -> &CandidateJudgeRequestAggregateV1 {
        &self.request_aggregate
    }

    /// Returns the exact response aggregate.
    #[must_use]
    pub const fn response_aggregate(&self) -> &CandidateJudgeResponseAggregateV1 {
        &self.response_aggregate
    }

    /// Returns the exact observation batch.
    #[must_use]
    pub const fn observation_batch(&self) -> &CandidateJudgeObservationBatchV1 {
        &self.observation_batch
    }

    /// Returns the exact managed local-judge receipt.
    #[must_use]
    pub const fn managed_receipt(&self) -> &ManagedLocalJudgeReceiptRecordV1 {
        &self.managed_receipt
    }

    /// Returns the exact candidate-judge join.
    #[must_use]
    pub const fn join(&self) -> &CandidateJudgeJoinRecordV1 {
        &self.join
    }

    fn from_cohort(records: codec::CohortRecords) -> Self {
        Self {
            plan: records.plan,
            schedule: records.schedule,
            request_aggregate: records.requests,
            response_aggregate: records.responses,
            observation_batch: records.observations,
            managed_receipt: records.receipt,
            join: records.join,
        }
    }
}

/// Per-record outcome of one atomic judge-execution write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateJudgeExecutionV1WriteDisposition {
    /// Judge-plan write outcome.
    pub plan: WriteDisposition,
    /// Judge-schedule write outcome.
    pub schedule: WriteDisposition,
    /// Request-aggregate write outcome.
    pub request_aggregate: WriteDisposition,
    /// Response-aggregate write outcome.
    pub response_aggregate: WriteDisposition,
    /// Observation-batch write outcome.
    pub observation_batch: WriteDisposition,
    /// Managed-receipt write outcome.
    pub managed_receipt: WriteDisposition,
    /// Candidate-judge join write outcome.
    pub join: WriteDisposition,
}

/// Failure of one atomic judge-execution transaction.
pub enum CandidateJudgeExecutionV1TransactionError<E> {
    /// Durable storage or typed relationship validation failed.
    Store(StoreError),
    /// A cancellation or deadline gate rejected the operation.
    Gate(E),
}

impl<E> fmt::Debug for CandidateJudgeExecutionV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "CandidateJudgeExecutionV1TransactionError::Store",
            Self::Gate(_) => "CandidateJudgeExecutionV1TransactionError::Gate",
        })
    }
}

impl<E> fmt::Display for CandidateJudgeExecutionV1TransactionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Store(_) => "candidate judge execution storage failed",
            Self::Gate(_) => "candidate judge execution gate rejected the commit",
        })
    }
}

impl<E> Error for CandidateJudgeExecutionV1TransactionError<E> {}

impl ArtifactStateStore {
    /// Atomically stores and cold-revalidates one schema 12 judge-execution cohort.
    ///
    /// The plan foundation and judge system are cold-read on the open immediate
    /// transaction before any cohort row is inserted. Gates run before lock
    /// acquisition, after acquisition, and before commit. The stored value grants
    /// no live, qualification, or activation authority.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a rejected gate, missing dependency, corrupt
    /// cohort, immutable conflict, invalid judge closure, or commit failure. No
    /// qualification record is written.
    pub fn transact_candidate_judge_execution_v1<E>(
        &mut self,
        input: CandidateJudgeExecutionV1Input<'_>,
        mut gate: impl FnMut() -> Result<(), E>,
    ) -> Result<
        CandidateJudgeExecutionV1WriteDisposition,
        CandidateJudgeExecutionV1TransactionError<E>,
    > {
        gate().map_err(CandidateJudgeExecutionV1TransactionError::Gate)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(StoreError::from)
            .map_err(CandidateJudgeExecutionV1TransactionError::Store)?;
        gate().map_err(CandidateJudgeExecutionV1TransactionError::Gate)?;
        let prepared = codec::prepare(&transaction, &input)
            .map_err(CandidateJudgeExecutionV1TransactionError::Store)?;
        let disposition = write::insert_cohort(&transaction, &prepared)
            .map_err(CandidateJudgeExecutionV1TransactionError::Store)?;
        let stored = read::load(&transaction, &prepared.read)
            .map_err(CandidateJudgeExecutionV1TransactionError::Store)?;
        match stored {
            Some(value) if codec::matches_prepared(&value, &prepared) => {}
            Some(_) => {
                return Err(CandidateJudgeExecutionV1TransactionError::Store(
                    StoreError::ImmutableConflict,
                ));
            }
            None => {
                return Err(CandidateJudgeExecutionV1TransactionError::Store(
                    StoreError::CorruptRecord,
                ));
            }
        }
        gate().map_err(CandidateJudgeExecutionV1TransactionError::Gate)?;
        transaction
            .commit()
            .map_err(StoreError::from)
            .map_err(CandidateJudgeExecutionV1TransactionError::Store)?;
        Ok(disposition)
    }

    /// Cold-reads one judge-execution cohort.
    ///
    /// `Ok(None)` means the foundations loaded, the cohort rederived, and all
    /// seven tables are empty for that judge plan. A present row whose chain does
    /// not rederive is [`StoreError::CorruptRecord`]. The read does not modify
    /// durable rows.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when a foundation is missing, stored bytes or
    /// indexed columns disagree, or the read modifies the database.
    pub fn candidate_judge_execution_v1(
        &self,
        input: CandidateJudgeExecutionV1ReadInput<'_>,
    ) -> StoreResult<Option<StoredCandidateJudgeExecutionV1>> {
        let before = self.connection.total_changes();
        let transaction = self.connection.unchecked_transaction()?;
        let stored = read::load(&transaction, &input)?;
        transaction.commit()?;
        if self.connection.total_changes() == before {
            Ok(stored)
        } else {
            Err(StoreError::CorruptRecord)
        }
    }
}

#[cfg(test)]
#[path = "candidate_judge_execution/tests.rs"]
mod tests;
