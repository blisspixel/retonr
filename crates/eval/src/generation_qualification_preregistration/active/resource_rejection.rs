//! Consuming policy-directed resource rejection without positive human authority.

use super::{
    ActiveGenerationQualificationOperation,
    judge_settlement::JudgeSettlementContext,
    resource_settlement::{
        ActiveGenerationQualificationResourceSettlementError as Error, finish,
        map_preparation_error, publication,
    },
};
use crate::{
    VerifiedGenerationQualificationResourcePhase,
    generation_qualification_preregistration::{
        GenerationQualificationPreregistrationRepository, check_gate,
        prepared::PreparedGenerationQualificationValidationError,
    },
};
use rewrite_model::{
    GenerationQualificationOperationReceiptV1, GenerationQualificationPhaseStatusV1,
    GenerationQualificationRecordV1,
};
use rewrite_types::CancellationToken;

mod evidence;
#[cfg(test)]
pub(super) mod tests;

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Consumes this operation and failed resource authority into an inert rejection.
    ///
    /// This policy-directed negative Completed receipt skips human review. It grants
    /// no verified qualification, activation, selection, or generation authority.
    /// Exact same-subject live joins and durable canonical parents are revalidated.
    /// Independent uncancelled resource/join and Prepared finalizers run before
    /// receipt derivation and again after the attempt, including on primary failure.
    /// The original absolute deadline has final precedence over cancellation.
    /// A later cancellation or finalizer failure may leave a committed inert
    /// Rejected record while returning an error. Such rows grant no capability.
    ///
    /// # Errors
    /// Refuses Passed resources, incomplete execution, foreign or changed scope,
    /// corrupt or incomplete durable parents, cancellation, deadline or finalization
    /// failure. This owner and the resource authority are consumed on every path.
    pub fn reject_failed_generation_resources(
        mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        mut phase: VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>,
        cancellation: &CancellationToken,
    ) -> Result<
        (
            GenerationQualificationOperationReceiptV1,
            GenerationQualificationRecordV1,
            rewrite_model_store::WriteDisposition,
        ),
        Error,
    > {
        let initial_phase_failed = phase
            .revalidate_for_mandatory_settlement_finalization()
            .is_err();
        let initial_prepared_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let primary = if initial_phase_failed || initial_prepared_failed {
            Err(Error::MandatoryFinalization)
        } else {
            self.publish_rejection(repository, &mut phase, cancellation)
        };
        let phase_failed = phase
            .revalidate_for_mandatory_settlement_finalization()
            .is_err();
        let prepared_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let terminal_gate = check_gate(self.prepared.operation_deadline(), cancellation)
            .map_err(map_preparation_error);
        let initial_failed = initial_phase_failed || initial_prepared_failed;
        let final_failed = phase_failed || prepared_failed;
        // A single initial mandatory failure is not two independent failures.
        // Deadline/cancellation still retain precedence and the cleanup evidence.
        if initial_failed && !final_failed && terminal_gate.is_ok() {
            return Err(Error::MandatoryFinalization);
        }
        finish(primary, initial_failed || final_failed, terminal_gate)
    }

    fn publish_rejection(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        phase: &mut VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>,
        cancellation: &CancellationToken,
    ) -> Result<
        (
            GenerationQualificationOperationReceiptV1,
            GenerationQualificationRecordV1,
            rewrite_model_store::WriteDisposition,
        ),
        Error,
    > {
        if self.terminal
            || self.started_candidate_attempt.is_some()
            || self.pending_completed_candidate.is_some()
        {
            return Err(Error::NotReady);
        }
        let ledger = self.attempt_ledger.as_ref().ok_or(Error::NotReady)?;
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
        let peak = u32::from(self.peak_live_authorities());
        self.prepared
            .with_validated_view(cancellation, |prepared| {
                phase
                    .with_settlement_joins(
                        cancellation,
                        |closure, repeatability_manifest, results, manifest, joins| {
                            if manifest.status() != GenerationQualificationPhaseStatusV1::Failed {
                                return Err(Error::OperationScope);
                            }
                            publication::with_input(
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
                                |repository, input| {
                                    evidence::publish(
                                        &context,
                                        &prepared,
                                        repository,
                                        input,
                                        peak,
                                        cancellation,
                                    )
                                },
                            )
                        },
                    )
                    .map_err(|()| Error::JoinAuthority)?
            })
            .map_err(|error| match error {
                PreparedGenerationQualificationValidationError::Callback(error) => error,
                PreparedGenerationQualificationValidationError::Initial(error)
                | PreparedGenerationQualificationValidationError::Final(error) => {
                    map_preparation_error(error)
                }
                PreparedGenerationQualificationValidationError::InitialAndFinal { .. }
                | PreparedGenerationQualificationValidationError::CallbackAndFinal { .. } => {
                    Error::PrimaryAndFinalization
                }
            })
    }
}
