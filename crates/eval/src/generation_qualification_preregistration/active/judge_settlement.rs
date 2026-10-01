//! Active-owned durable managed judge cohort settlement.

use super::{
    ActiveGenerationQualificationOperation,
    attempt_ledger::ActiveGenerationQualificationAttemptLedgerClosure,
};
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, GenerationQualificationPreregistrationRepository,
    check_gate, prepared::PreparedGenerationQualificationValidationView,
};
use crate::{
    VerifiedCandidateJudgeJoin, local_ollama_managed_preflight::JudgeSettlementView,
    verified_candidate_batch_set::CandidateBatchSetScope,
};
use rewrite_model::{
    CandidateGenerationReceiptSetV1Relations, CandidateSelectionPolicyV1,
    GenerationQualificationPhaseStatusV1,
};
use rewrite_model_store::{
    CandidateJudgeExecutionV1WriteDisposition, StoredCandidateJudgeExecutionV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

/// Content-free failure of a compiler-derived judge settlement.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ActiveGenerationQualificationJudgeSettlementError {
    /// The original absolute deadline was reached.
    #[error("active judge settlement deadline was reached")]
    DeadlineExceeded,
    /// Cancellation was observed before the original deadline.
    #[error("active judge settlement was cancelled")]
    Cancelled,
    /// The deadline and independent final validation both failed.
    #[error("active judge settlement deadline and finalization failed")]
    DeadlineAndFinalization,
    /// Cancellation and independent final validation both failed.
    #[error("active judge settlement cancellation and finalization failed")]
    CancelledAndFinalization,
    /// Candidate settlement or its Passed target ledger is incomplete.
    #[error("active judge settlement is not ready")]
    NotReady,
    /// System, repetition, material, plan, ledger, or process-local scope differs.
    #[error("active judge settlement scope does not match")]
    OperationScope,
    /// The retained live judge closure failed independent validation.
    #[error("active judge settlement join authority failed")]
    JoinAuthority,
    /// Prepared authority, storage, or canonical readback failed.
    #[error("active judge settlement publication failed")]
    Publication,
    /// Independent mandatory final validation failed after the primary succeeded.
    #[error("active judge settlement finalization failed")]
    MandatoryFinalization,
    /// The primary and independent mandatory final validation both failed.
    #[error("active judge settlement and finalization failed")]
    PrimaryAndFinalization,
}

type SettlementResult = Result<
    (
        StoredCandidateJudgeExecutionV1,
        CandidateJudgeExecutionV1WriteDisposition,
    ),
    ActiveGenerationQualificationJudgeSettlementError,
>;

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Persists the exact managed judge cohort after successful Active execution.
    ///
    /// Settlement follows preregistered repetition order. Already settled live
    /// joins may replay their immutable cohort. Both candidates retain this
    /// process-local subject and exact Prepared scope. Deterministic parent rows
    /// must already cold-read exactly. All records returned here are inert.
    /// Independent uncancelled join and Prepared finalizers always run after a
    /// settlement attempt, followed by deadline-first cancellation sampling.
    ///
    /// # Errors
    ///
    /// Returns content-free readiness, scope, authority, storage or finalization
    /// failures. Substantive failures permanently terminalize the Active owner.
    pub fn persist_candidate_judge_execution(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        join: &mut VerifiedCandidateJudgeJoin<'_, '_, '_, '_>,
        cancellation: &CancellationToken,
    ) -> SettlementResult {
        use ActiveGenerationQualificationJudgeSettlementError as Error;
        if self.terminal
            || self.started_candidate_attempt.is_some()
            || self.pending_completed_candidate.is_some()
        {
            return Err(Error::NotReady);
        }
        let Some(ledger) = self.attempt_ledger.as_ref() else {
            return Err(Error::NotReady);
        };
        let subject_matches = join.matches_active_subject(&self.subject)
            && ledger.matches_active_subject(&self.subject);
        let foundation = self.prepared.plan_foundation().clone();
        let context = JudgeSettlementContext {
            ledger,
            subject: &self.subject,
            subject_matches,
            selection_policy: foundation.candidate_selection_policy(),
            candidate_count: self.next_candidate_attempt,
            executed_count: self.next_judge_repetition,
            executed_joins: &self.executed_judge_joins,
            settled_count: self.next_judge_settlement,
            suite: foundation.suite(),
            cases: foundation.cases(),
            contracts: foundation.deterministic_case_contracts(),
        };
        let mut settled_ordinal = None;
        let primary = self.prepared.with_validated_view(cancellation, |prepared| {
            join.with_settlement_view(cancellation, |view| {
                let ordinal = context.validate(&prepared, &view, cancellation)?;
                let result = repository.persist_judge_execution(view.input, prepared.deadline, cancellation).map_err(map_preparation_error)?;
                settled_ordinal = Some(ordinal);
                Ok(result)
            }).map_err(|()| Error::JoinAuthority)?
        }).map_err(|error| match error {
            super::super::prepared::PreparedGenerationQualificationValidationError::Callback(error) => error,
            _ => Error::Publication,
        });
        let join_failed = join.revalidate(&CancellationToken::new()).is_err();
        let prepared_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let result = finish_settlement(
            primary,
            join_failed || prepared_failed,
            check_gate(self.prepared.operation_deadline(), cancellation)
                .map_err(map_preparation_error),
        );
        if result.is_ok() {
            if settled_ordinal == Some(self.next_judge_settlement) {
                self.next_judge_settlement += 1;
            }
        } else if !matches!(result, Err(Error::NotReady)) {
            self.terminal = true;
        }
        result
    }
}

pub(super) struct JudgeSettlementContext<'a> {
    pub(super) ledger: &'a ActiveGenerationQualificationAttemptLedgerClosure,
    pub(super) subject:
        &'a crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject,
    pub(super) subject_matches: bool,
    pub(super) selection_policy: &'a CandidateSelectionPolicyV1,
    pub(super) candidate_count: usize,
    pub(super) executed_count: usize,
    pub(super) executed_joins: &'a [rewrite_model::CandidateJudgeJoinId],
    pub(super) settled_count: usize,
    pub(super) suite: &'a rewrite_model::GenerationSuiteManifestV1,
    pub(super) cases: &'a [rewrite_model::GenerationCaseManifestV1],
    pub(super) contracts: &'a [crate::GenerationDeterministicCaseContractV1],
}

impl JudgeSettlementContext<'_> {
    pub(super) fn validate(
        &self,
        view: &PreparedGenerationQualificationValidationView<'_>,
        evidence: &JudgeSettlementView<'_, '_>,
        cancellation: &CancellationToken,
    ) -> Result<usize, ActiveGenerationQualificationJudgeSettlementError> {
        use ActiveGenerationQualificationJudgeSettlementError as Error;
        let relations = view.operation_policy_relations;
        if self.candidate_count != relations.planned_attempts.len()
            || self.ledger.manifest().status() != GenerationQualificationPhaseStatusV1::Passed
        {
            return Err(Error::NotReady);
        }
        if !self.subject_matches
            || !evidence.target.matches_active_subject(self.subject)
            || !evidence.baseline.matches_active_subject(self.subject)
            || evidence.material.suite() != self.suite
            || evidence.material.cases() != self.cases
            || evidence.material.contracts() != self.contracts
        {
            return Err(Error::OperationScope);
        }
        let ordinal = relations
            .repetitions
            .iter()
            .position(|row| row.repetition_id() == evidence.input.repetition_id)
            .ok_or(Error::OperationScope)?;
        if ordinal >= self.executed_count || ordinal > self.settled_count {
            return Err(Error::NotReady);
        }
        if self.executed_joins.get(ordinal) != Some(evidence.input.join.candidate_judge_join_id()) {
            return Err(Error::OperationScope);
        }
        let repetition = &relations.repetitions[ordinal];
        let scope = CandidateBatchSetScope {
            plan: relations.plan,
            suite: relations.suite,
            repetition,
            system: relations.target_system.generation_system,
            selection_policy: self.selection_policy,
            planned_attempts: relations.planned_attempts,
        };
        evidence
            .target
            .validate_prepared_scope(scope, cancellation)
            .map_err(|_| Error::OperationScope)?;
        evidence
            .baseline
            .validate_prepared_scope(
                CandidateBatchSetScope {
                    system: relations.baseline_system.generation_system,
                    ..scope
                },
                cancellation,
            )
            .map_err(|_| Error::OperationScope)?;
        let reconstructed = super::receipt_set::rederive_from_ledger(
            CandidateGenerationReceiptSetV1Relations {
                qualification_plan: relations.plan,
                suite: relations.suite,
                repetition,
                generation_system: relations.target_system.generation_system,
                selection_policy: self.selection_policy,
                planned_attempts: relations.planned_attempts,
                attempt_records: &[],
                receipts: &[],
            },
            self.ledger.target_attempt_records(),
            self.ledger.target_attempt_receipts(),
        )
        .map_err(|_| Error::OperationScope)?;
        if &reconstructed != evidence.input.candidate_a_receipt_set {
            return Err(Error::OperationScope);
        }
        Ok(ordinal)
    }
}

fn map_preparation_error(
    error: GenerationQualificationPreparationError,
) -> ActiveGenerationQualificationJudgeSettlementError {
    use ActiveGenerationQualificationJudgeSettlementError as Error;
    match error {
        GenerationQualificationPreparationError::DeadlineExceeded => Error::DeadlineExceeded,
        GenerationQualificationPreparationError::Cancelled => Error::Cancelled,
        _ => Error::Publication,
    }
}

fn finish_settlement<T>(
    primary: Result<T, ActiveGenerationQualificationJudgeSettlementError>,
    finalization_failed: bool,
    terminal_gate: Result<(), ActiveGenerationQualificationJudgeSettlementError>,
) -> Result<T, ActiveGenerationQualificationJudgeSettlementError> {
    use ActiveGenerationQualificationJudgeSettlementError as Error;
    match (terminal_gate, primary, finalization_failed) {
        (Err(Error::DeadlineExceeded), _, true) => Err(Error::DeadlineAndFinalization),
        (Err(Error::Cancelled), _, true) => Err(Error::CancelledAndFinalization),
        (Err(error), _, _) => Err(error),
        (Ok(()), Ok(_), true) => Err(Error::MandatoryFinalization),
        (Ok(()), Err(_), true) => Err(Error::PrimaryAndFinalization),
        (Ok(()), result, false) => result,
    }
}

#[cfg(test)]
mod tests;
