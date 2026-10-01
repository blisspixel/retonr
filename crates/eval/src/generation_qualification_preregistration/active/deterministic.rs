//! Active-owned deterministic settlement over current target and baseline batches.

use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateGenerationReceiptSetV1Relations,
    CandidateSelectionPolicyV1, GenerationQualificationPhaseStatusV1,
};
use rewrite_model_store::WriteDisposition;
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    ActiveGenerationQualificationOperation,
    attempt_ledger::ActiveGenerationQualificationAttemptLedgerClosure,
};
use crate::{
    CandidateDeterministicCompilerError, VerifiedCandidateBatchSet, VerifiedGenerationCaseMaterial,
    compile_candidate_deterministic_evaluation,
    generation_qualification_preregistration::{
        GenerationQualificationPreparationError, GenerationQualificationPreregistrationRepository,
        check_gate,
        prepared::{
            PreparedGenerationQualificationValidationError,
            PreparedGenerationQualificationValidationView,
        },
    },
    verified_candidate_batch_set::CandidateBatchSetScope,
};

/// Content-free failure of a compiler-derived deterministic settlement.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ActiveGenerationQualificationDeterministicSettlementError {
    /// The original absolute deadline was reached.
    #[error("active deterministic settlement deadline was reached")]
    DeadlineExceeded,
    /// Cancellation was observed before the original deadline.
    #[error("active deterministic settlement was cancelled")]
    Cancelled,
    /// The deadline and independent final validation both failed.
    #[error("active deterministic settlement deadline and finalization failed")]
    DeadlineAndFinalization,
    /// Cancellation and independent final validation both failed.
    #[error("active deterministic settlement cancellation and finalization failed")]
    CancelledAndFinalization,
    /// Candidate settlement or its Passed target ledger is incomplete.
    #[error("active deterministic settlement is not ready")]
    NotReady,
    /// System, repetition, material, plan, ledger, or process-local scope differs.
    #[error("active deterministic settlement scope does not match")]
    OperationScope,
    /// A current candidate batch failed authority validation.
    #[error("active deterministic settlement batch authority failed")]
    BatchAuthority,
    /// The exact deterministic compiler could not produce a trustworthy result.
    #[error("active deterministic settlement compilation failed")]
    Compilation,
    /// Prepared authority, storage, or canonical readback failed.
    #[error("active deterministic settlement publication failed")]
    Publication,
    /// Independent mandatory final validation failed after the primary succeeded.
    #[error("active deterministic settlement finalization failed")]
    MandatoryFinalization,
    /// The primary and independent mandatory final validation both failed.
    #[error("active deterministic settlement and finalization failed")]
    PrimaryAndFinalization,
}

type SettlementResult = Result<
    (CandidateDeterministicEvaluationRecordV1, WriteDisposition),
    ActiveGenerationQualificationDeterministicSettlementError,
>;

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Compiles and durably settles one exact target/baseline deterministic result.
    ///
    /// Both batch sets must retain this Active subject and the exact Prepared plan,
    /// system, repetition, selection policy and complete planned-attempt closure.
    /// The target is independently reconstructed from its Passed sealed ledger.
    /// Baseline authority comes from the complete live batch set over those exact
    /// Prepared relations; Active does not retain a separate baseline ledger.
    ///
    /// Passed and Failed compiler results are both valid inert evidence. Receipt
    /// sets commit separately before schema 15 settlement. A subsequent failure can
    /// leave inert parent rows but cannot return a settled result. All three live
    /// evidence finalizers and Prepared finalization run independently without
    /// cancellation, then the original deadline and cancellation are sampled again.
    /// No stored record grants qualification, activation, launch, or live use.
    ///
    /// # Errors
    ///
    /// Returns content-free scope, authority, compiler, publication or finalization
    /// failures. Substantive failure permanently terminalizes this Active owner.
    pub fn persist_candidate_deterministic_evaluation(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        target: &VerifiedCandidateBatchSet,
        baseline: &VerifiedCandidateBatchSet,
        material: &VerifiedGenerationCaseMaterial<'_>,
        cancellation: &CancellationToken,
    ) -> SettlementResult {
        use ActiveGenerationQualificationDeterministicSettlementError as Error;
        if self.terminal
            || self.started_candidate_attempt.is_some()
            || self.pending_completed_candidate.is_some()
        {
            return Err(Error::NotReady);
        }
        let Some(ledger) = self.attempt_ledger.as_ref() else {
            return Err(Error::NotReady);
        };
        let foundation = self.prepared.plan_foundation();
        let context = DeterministicSettlementContext {
            ledger,
            target,
            baseline,
            material,
            selection_policy: foundation.candidate_selection_policy().clone(),
            candidate_count: self.next_candidate_attempt,
            subject_matches: ledger.matches_active_subject(&self.subject)
                && target.matches_active_subject(&self.subject)
                && baseline.matches_active_subject(&self.subject),
            material_matches: material.suite() == foundation.suite()
                && material.cases() == foundation.cases()
                && material.contracts() == foundation.deterministic_case_contracts(),
        };
        let primary = self
            .prepared
            .with_validated_view(cancellation, |view| {
                context.settle(&view, repository, cancellation)
            })
            .map_err(|error| {
                map_validation_error(error, self.prepared.operation_deadline(), cancellation)
            });
        // Each authority is sampled even when another independent finalizer fails.
        let target_failed = target.revalidate(&CancellationToken::new()).is_err();
        let baseline_failed = baseline.revalidate(&CancellationToken::new()).is_err();
        let material_failed = material.revalidate(&CancellationToken::new()).is_err();
        let prepared_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let result = finish_settlement(
            primary,
            target_failed || baseline_failed || material_failed || prepared_failed,
            check_gate(self.prepared.operation_deadline(), cancellation)
                .map_err(map_preparation_error),
        );
        if result.is_err() && !matches!(result, Err(Error::NotReady)) {
            self.terminal = true;
        }
        result
    }
}

struct DeterministicSettlementContext<'a, 'store> {
    ledger: &'a ActiveGenerationQualificationAttemptLedgerClosure,
    target: &'a VerifiedCandidateBatchSet,
    baseline: &'a VerifiedCandidateBatchSet,
    material: &'a VerifiedGenerationCaseMaterial<'store>,
    selection_policy: CandidateSelectionPolicyV1,
    candidate_count: usize,
    subject_matches: bool,
    material_matches: bool,
}

impl DeterministicSettlementContext<'_, '_> {
    fn settle(
        &self,
        view: &PreparedGenerationQualificationValidationView<'_>,
        repository: &mut GenerationQualificationPreregistrationRepository,
        cancellation: &CancellationToken,
    ) -> SettlementResult {
        use ActiveGenerationQualificationDeterministicSettlementError as Error;
        if !self.subject_matches || !self.material_matches {
            return Err(Error::OperationScope);
        }
        let relations = view.operation_policy_relations;
        if self.candidate_count != relations.planned_attempts.len()
            || self.ledger.manifest().status() != GenerationQualificationPhaseStatusV1::Passed
        {
            return Err(Error::NotReady);
        }
        let target_receipts = self
            .target
            .receipt_set(cancellation)
            .map_err(|error| map_batch_error(&error))?;
        let baseline_receipts = self
            .baseline
            .receipt_set(cancellation)
            .map_err(|error| map_batch_error(&error))?;
        let repetition = relations
            .repetitions
            .iter()
            .find(|repetition| repetition.repetition_id() == target_receipts.repetition_id())
            .ok_or(Error::OperationScope)?;
        let target_scope = CandidateBatchSetScope {
            plan: relations.plan,
            suite: relations.suite,
            repetition,
            system: relations.target_system.generation_system,
            selection_policy: &self.selection_policy,
            planned_attempts: relations.planned_attempts,
        };
        self.target
            .validate_prepared_scope(target_scope, cancellation)
            .map_err(|error| map_batch_error(&error))?;
        self.baseline
            .validate_prepared_scope(
                CandidateBatchSetScope {
                    system: relations.baseline_system.generation_system,
                    ..target_scope
                },
                cancellation,
            )
            .map_err(|error| map_batch_error(&error))?;
        let expected_target = super::receipt_set::rederive_from_ledger(
            CandidateGenerationReceiptSetV1Relations {
                qualification_plan: relations.plan,
                suite: relations.suite,
                repetition,
                generation_system: relations.target_system.generation_system,
                selection_policy: &self.selection_policy,
                planned_attempts: relations.planned_attempts,
                attempt_records: &[],
                receipts: &[],
            },
            self.ledger.target_attempt_records(),
            self.ledger.target_attempt_receipts(),
        )
        .map_err(|_| Error::OperationScope)?;
        if &expected_target != target_receipts {
            return Err(Error::OperationScope);
        }
        let record = compile_candidate_deterministic_evaluation(
            self.target,
            self.baseline,
            self.material,
            cancellation,
        )
        .map_err(|error| match error {
            CandidateDeterministicCompilerError::Cancelled => Error::Cancelled,
            _ => Error::Compilation,
        })?;
        check_gate(view.deadline, cancellation).map_err(map_preparation_error)?;
        repository
            .persist_receipt_set(target_receipts, view.deadline, cancellation)
            .map_err(map_preparation_error)?;
        repository
            .persist_receipt_set(baseline_receipts, view.deadline, cancellation)
            .map_err(map_preparation_error)?;
        let disposition = repository
            .persist_deterministic_evaluation(&record, view.deadline, cancellation)
            .map_err(map_preparation_error)?;
        Ok((record, disposition))
    }
}

fn map_batch_error(
    error: &crate::VerifiedCandidateBatchSetError,
) -> ActiveGenerationQualificationDeterministicSettlementError {
    use ActiveGenerationQualificationDeterministicSettlementError as Error;
    match error {
        crate::VerifiedCandidateBatchSetError::Cancelled => Error::Cancelled,
        crate::VerifiedCandidateBatchSetError::Relationship(_) => Error::OperationScope,
        _ => Error::BatchAuthority,
    }
}

fn map_preparation_error(
    error: GenerationQualificationPreparationError,
) -> ActiveGenerationQualificationDeterministicSettlementError {
    use ActiveGenerationQualificationDeterministicSettlementError as Error;
    match error {
        GenerationQualificationPreparationError::DeadlineExceeded => Error::DeadlineExceeded,
        GenerationQualificationPreparationError::Cancelled => Error::Cancelled,
        _ => Error::Publication,
    }
}

fn map_validation_error(
    error: PreparedGenerationQualificationValidationError<
        ActiveGenerationQualificationDeterministicSettlementError,
    >,
    deadline: std::time::Instant,
    cancellation: &CancellationToken,
) -> ActiveGenerationQualificationDeterministicSettlementError {
    match error {
        PreparedGenerationQualificationValidationError::Callback(error) => error,
        PreparedGenerationQualificationValidationError::Initial(error)
        | PreparedGenerationQualificationValidationError::Final(error) => {
            map_preparation_error(error)
        }
        _ => check_gate(deadline, cancellation).err().map_or(
            ActiveGenerationQualificationDeterministicSettlementError::Publication,
            map_preparation_error,
        ),
    }
}

fn finish_settlement<T>(
    primary: Result<T, ActiveGenerationQualificationDeterministicSettlementError>,
    finalization_failed: bool,
    terminal_gate: Result<(), ActiveGenerationQualificationDeterministicSettlementError>,
) -> Result<T, ActiveGenerationQualificationDeterministicSettlementError> {
    use ActiveGenerationQualificationDeterministicSettlementError as Error;
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
pub(super) mod tests;
