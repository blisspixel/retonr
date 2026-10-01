//! Active-owned publication of the exact policy-derived resource cohort.

use super::{ActiveGenerationQualificationOperation, judge_settlement::JudgeSettlementContext};
use crate::VerifiedGenerationQualificationResourcePhase;
use crate::generation_qualification_preregistration::{
    GenerationQualificationPreparationError, GenerationQualificationPreregistrationRepository,
    check_gate, prepared::PreparedGenerationQualificationValidationError,
};
use rewrite_model::GenerationResourceEvidenceManifestV1;

use rewrite_types::CancellationToken;
use thiserror::Error;

mod publication;

/// Content-free failure of complete live resource settlement.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ActiveGenerationQualificationResourceSettlementError {
    /// The original absolute deadline was reached.
    #[error("active resource settlement deadline was reached")]
    DeadlineExceeded,
    /// Cancellation was observed before the original deadline.
    #[error("active resource settlement was cancelled")]
    Cancelled,
    /// Deadline and mandatory finalization both failed.
    #[error("active resource settlement deadline and finalization failed")]
    DeadlineAndFinalization,
    /// Cancellation and mandatory finalization both failed.
    #[error("active resource settlement cancellation and finalization failed")]
    CancelledAndFinalization,
    /// Candidate or judge execution and settlement are incomplete.
    #[error("active resource settlement is not ready")]
    NotReady,
    /// The exact Prepared scope, consumed joins, or process-local subject differs.
    #[error("active resource settlement scope does not match")]
    OperationScope,
    /// Retained complete Passed join authority failed validation.
    #[error("active resource settlement live authority failed")]
    JoinAuthority,
    /// Durable publication or canonical parent readback failed.
    #[error("active resource settlement publication failed")]
    Publication,
    /// Independent mandatory finalization failed after primary success.
    #[error("active resource settlement finalization failed")]
    MandatoryFinalization,
    /// Primary and independent mandatory finalization both failed.
    #[error("active resource settlement and finalization failed")]
    PrimaryAndFinalization,
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Atomically publishes target resource results and their derived phase manifest.
    ///
    /// Every target and baseline retains this Active subject and exact Prepared
    /// scope. Each retained join must already have been successfully executed and
    /// durably settled in global repetition order. The complete repeatability phase
    /// must already be durable. Full parents are freshly reconstructed atomically. Replay still requires the same
    /// live authority. Returned records and dispositions remain inert; committed
    /// rows from a later failed publication grant no qualification authority.
    /// Independent uncancelled complete-join and Prepared finalizers always run
    /// after an attempted publication, followed by the original deadline gate.
    ///
    /// # Errors
    ///
    /// Returns content-free readiness, scope, authority, publication, or finalizer
    /// errors. Substantive failures permanently terminalize this Active owner.
    pub fn persist_generation_resource_phase(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        phase: &mut VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>,
        cancellation: &CancellationToken,
    ) -> Result<
        (
            GenerationResourceEvidenceManifestV1,
            rewrite_model_store::GenerationResourcePhaseV1Disposition,
        ),
        ActiveGenerationQualificationResourceSettlementError,
    > {
        use ActiveGenerationQualificationResourceSettlementError as Error;
        if self.terminal
            || self.started_candidate_attempt.is_some()
            || self.pending_completed_candidate.is_some()
        {
            return Err(Error::NotReady);
        }
        let Some(ledger) = self.attempt_ledger.as_ref() else {
            return Err(Error::NotReady);
        };
        let foundation = self.prepared.plan_foundation().clone();
        let context = JudgeSettlementContext {
            ledger,
            subject: &self.subject,
            subject_matches: ledger.matches_active_subject(&self.subject),
            selection_policy: foundation.candidate_selection_policy(),
            candidate_count: self.next_candidate_attempt,
            executed_count: self.next_judge_repetition,
            executed_joins: &self.executed_judge_joins,
            settled_count: self.next_judge_settlement,
            suite: foundation.suite(),
            cases: foundation.cases(),
            contracts: foundation.deterministic_case_contracts(),
        };
        let primary = self
            .prepared
            .with_validated_view(cancellation, |prepared| {
                phase
                    .with_settlement_joins(
                        cancellation,
                        |closure, repeatability_manifest, results, manifest, joins| {
                            publication::publish(
                                &context,
                                &prepared,
                                closure,
                                publication::ResourceClosure {
                                    repeatability_manifest,
                                    resource_results: results,
                                    manifest,
                                },
                                joins,
                                repository,
                                cancellation,
                            )
                        },
                    )
                    .map_err(|()| Error::JoinAuthority)?
            })
            .map_err(|error| match error {
                PreparedGenerationQualificationValidationError::Callback(error) => error,
                _ => Error::Publication,
            });
        let phase_failed = phase
            .revalidate_for_mandatory_settlement_finalization()
            .is_err();
        let prepared_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let result = finish(
            primary,
            phase_failed || prepared_failed,
            check_gate(self.prepared.operation_deadline(), cancellation)
                .map_err(map_preparation_error),
        );
        if result.is_err() && !matches!(result, Err(Error::NotReady)) {
            self.terminal = true;
        }
        result
    }
}

fn map_preparation_error(
    error: GenerationQualificationPreparationError,
) -> ActiveGenerationQualificationResourceSettlementError {
    use ActiveGenerationQualificationResourceSettlementError as Error;
    match error {
        GenerationQualificationPreparationError::DeadlineExceeded => Error::DeadlineExceeded,
        GenerationQualificationPreparationError::Cancelled => Error::Cancelled,
        _ => Error::Publication,
    }
}

fn finish<T>(
    primary: Result<T, ActiveGenerationQualificationResourceSettlementError>,
    finalization_failed: bool,
    terminal_gate: Result<(), ActiveGenerationQualificationResourceSettlementError>,
) -> Result<T, ActiveGenerationQualificationResourceSettlementError> {
    use ActiveGenerationQualificationResourceSettlementError as Error;
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
