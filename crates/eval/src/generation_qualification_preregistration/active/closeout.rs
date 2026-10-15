//! Consuming terminal qualification closeout over complete Passed resources.

use super::{
    ActiveGenerationQualificationOperation,
    judge_settlement::JudgeSettlementContext,
    resource_settlement::{self, publication},
};
use crate::{
    VerifiedGenerationQualificationResourcePhase,
    generation_qualification_preregistration::{
        GenerationQualificationPreparationError, GenerationQualificationPreregistrationRepository,
        check_gate, prepared::PreparedGenerationQualificationValidationError,
    },
};
use rewrite_model::{
    GenerationHumanAdjudicationEvidenceManifestV1, GenerationQualificationOperationReceiptV1,
    GenerationQualificationPhaseStatusV1, GenerationQualificationRecordV1,
};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

pub mod evidence;
#[cfg(test)]
pub(crate) mod tests;

/// Content-free failure of active qualification closeout.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ActiveGenerationQualificationCloseoutError {
    /// The original absolute deadline was reached.
    #[error("active qualification closeout deadline was reached")]
    DeadlineExceeded,
    /// Cancellation was observed before the original deadline.
    #[error("active qualification closeout was cancelled")]
    Cancelled,
    /// Deadline and mandatory finalization both failed.
    #[error("active qualification closeout deadline and finalization failed")]
    DeadlineAndFinalization,
    /// Cancellation and mandatory finalization both failed.
    #[error("active qualification closeout cancellation and finalization failed")]
    CancelledAndFinalization,
    /// Candidate or judge execution and settlement are incomplete.
    #[error("active qualification closeout is not ready")]
    NotReady,
    /// The exact Prepared scope, consumed joins, or process-local subject differs.
    #[error("active qualification closeout scope does not match")]
    OperationScope,
    /// Retained complete Passed join authority failed validation.
    #[error("active qualification closeout live authority failed")]
    JoinAuthority,
    /// Durable publication or canonical parent readback failed.
    #[error("active qualification closeout publication failed")]
    Publication,
    /// Independent mandatory finalization failed after primary success.
    #[error("active qualification closeout finalization failed")]
    MandatoryFinalization,
    /// Primary and independent mandatory finalization both failed.
    #[error("active qualification closeout and finalization failed")]
    PrimaryAndFinalization,
}

impl From<resource_settlement::ActiveGenerationQualificationResourceSettlementError>
    for ActiveGenerationQualificationCloseoutError
{
    fn from(
        error: resource_settlement::ActiveGenerationQualificationResourceSettlementError,
    ) -> Self {
        match error {
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::DeadlineExceeded => Self::DeadlineExceeded,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::Cancelled => Self::Cancelled,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::DeadlineAndFinalization => Self::DeadlineAndFinalization,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::CancelledAndFinalization => Self::CancelledAndFinalization,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::NotReady => Self::NotReady,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::OperationScope => Self::OperationScope,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::JoinAuthority => Self::JoinAuthority,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::Publication => Self::Publication,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::MandatoryFinalization => Self::MandatoryFinalization,
            resource_settlement::ActiveGenerationQualificationResourceSettlementError::PrimaryAndFinalization => Self::PrimaryAndFinalization,
        }
    }
}

impl From<ActiveGenerationQualificationCloseoutError>
    for resource_settlement::ActiveGenerationQualificationResourceSettlementError
{
    fn from(error: ActiveGenerationQualificationCloseoutError) -> Self {
        match error {
            ActiveGenerationQualificationCloseoutError::DeadlineExceeded => Self::DeadlineExceeded,
            ActiveGenerationQualificationCloseoutError::Cancelled => Self::Cancelled,
            ActiveGenerationQualificationCloseoutError::DeadlineAndFinalization => {
                Self::DeadlineAndFinalization
            }
            ActiveGenerationQualificationCloseoutError::CancelledAndFinalization => {
                Self::CancelledAndFinalization
            }
            ActiveGenerationQualificationCloseoutError::NotReady => Self::NotReady,
            ActiveGenerationQualificationCloseoutError::OperationScope => Self::OperationScope,
            ActiveGenerationQualificationCloseoutError::JoinAuthority => Self::JoinAuthority,
            ActiveGenerationQualificationCloseoutError::Publication => Self::Publication,
            ActiveGenerationQualificationCloseoutError::MandatoryFinalization => {
                Self::MandatoryFinalization
            }
            ActiveGenerationQualificationCloseoutError::PrimaryAndFinalization => {
                Self::PrimaryAndFinalization
            }
        }
    }
}

/// Typed outcome of human adjudication for active operation closeout.
pub enum ActiveGenerationHumanAdjudicationOutcome<'a> {
    /// Human adjudication policy was denied by the source policy.
    PolicyDenied(
        Box<rewrite_app::VerifiedGenerationQualificationHumanAdjudicationPolicyDenied<'a>>,
    ),
    /// Human review was conducted with the given manifest and evidence digests.
    Reviewed {
        /// Exact human adjudication phase manifest.
        human_manifest: &'a GenerationHumanAdjudicationEvidenceManifestV1,
        /// Exact human adjudication evidence digests.
        human_evidence_digests: &'a [Digest],
    },
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Consumes this operation and Passed resource authority into a terminal qualification.
    ///
    /// When human adjudication passed, stores a Qualified record.
    /// When human adjudication failed or was policy-denied, stores a Rejected record.
    /// Exact same-subject live joins and durable canonical parents are revalidated.
    /// Independent uncancelled resource/join and Prepared finalizers run before
    /// receipt derivation and again after the attempt, including on primary failure.
    /// The original absolute deadline has final precedence over cancellation.
    ///
    /// # Errors
    /// Refuses Failed or Skipped resources, incomplete execution, foreign or changed scope,
    /// corrupt or incomplete durable parents, cancellation, deadline or finalization failure.
    /// This owner and the resource authority are consumed on every path.
    pub fn close_generation_qualification(
        mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        phase: &mut VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>,
        human_adjudication: &ActiveGenerationHumanAdjudicationOutcome<'_>,
        cancellation: &CancellationToken,
    ) -> Result<
        (
            GenerationQualificationOperationReceiptV1,
            GenerationQualificationRecordV1,
            rewrite_model_store::WriteDisposition,
        ),
        ActiveGenerationQualificationCloseoutError,
    > {
        let initial_phase_failed = phase
            .revalidate_for_mandatory_settlement_finalization()
            .is_err();
        let initial_prepared_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let primary = if initial_phase_failed || initial_prepared_failed {
            Err(ActiveGenerationQualificationCloseoutError::MandatoryFinalization)
        } else {
            self.publish_closeout(repository, phase, human_adjudication, cancellation)
        };
        let phase_failed = phase
            .revalidate_for_mandatory_settlement_finalization()
            .is_err();
        let prepared_failed = self
            .prepared
            .revalidate_for_mandatory_finalization()
            .is_err();
        let terminal_gate = check_gate(self.prepared.operation_deadline(), cancellation)
            .map_err(map_closeout_preparation_error);
        let initial_failed = initial_phase_failed || initial_prepared_failed;
        let final_failed = phase_failed || prepared_failed;
        if initial_failed && !final_failed && terminal_gate.is_ok() {
            return Err(ActiveGenerationQualificationCloseoutError::MandatoryFinalization);
        }
        finish_closeout(primary, initial_failed || final_failed, terminal_gate)
    }

    fn publish_closeout(
        &mut self,
        repository: &mut GenerationQualificationPreregistrationRepository,
        phase: &mut VerifiedGenerationQualificationResourcePhase<'_, '_, '_, '_>,
        human_adjudication: &ActiveGenerationHumanAdjudicationOutcome<'_>,
        cancellation: &CancellationToken,
    ) -> Result<
        (
            GenerationQualificationOperationReceiptV1,
            GenerationQualificationRecordV1,
            rewrite_model_store::WriteDisposition,
        ),
        ActiveGenerationQualificationCloseoutError,
    > {
        use ActiveGenerationQualificationCloseoutError as Error;
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
                            if manifest.status() != GenerationQualificationPhaseStatusV1::Passed {
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
                                        human_adjudication,
                                        peak,
                                        cancellation,
                                    )
                                    .map_err(Into::into)
                                },
                            )
                            .map_err(Into::into)
                        },
                    )
                    .map_err(|()| Error::JoinAuthority)?
            })
            .map_err(|error| match error {
                PreparedGenerationQualificationValidationError::Callback(error) => error,
                PreparedGenerationQualificationValidationError::Initial(error)
                | PreparedGenerationQualificationValidationError::Final(error) => {
                    map_closeout_preparation_error(error)
                }
                PreparedGenerationQualificationValidationError::InitialAndFinal { .. }
                | PreparedGenerationQualificationValidationError::CallbackAndFinal { .. } => {
                    Error::PrimaryAndFinalization
                }
            })
    }
}

pub(super) fn map_closeout_preparation_error(
    error: GenerationQualificationPreparationError,
) -> ActiveGenerationQualificationCloseoutError {
    use ActiveGenerationQualificationCloseoutError as Error;
    match error {
        GenerationQualificationPreparationError::DeadlineExceeded => Error::DeadlineExceeded,
        GenerationQualificationPreparationError::Cancelled => Error::Cancelled,
        _ => Error::Publication,
    }
}

fn finish_closeout<T>(
    primary: Result<T, ActiveGenerationQualificationCloseoutError>,
    finalization_failed: bool,
    terminal_gate: Result<(), ActiveGenerationQualificationCloseoutError>,
) -> Result<T, ActiveGenerationQualificationCloseoutError> {
    use ActiveGenerationQualificationCloseoutError as Error;
    match (terminal_gate, primary, finalization_failed) {
        (Err(Error::DeadlineExceeded), _, true) => Err(Error::DeadlineAndFinalization),
        (Err(Error::Cancelled), _, true) => Err(Error::CancelledAndFinalization),
        (Err(error), _, _) => Err(error),
        (Ok(()), Ok(_), true) => Err(Error::MandatoryFinalization),
        (Ok(()), Err(_), true) => Err(Error::PrimaryAndFinalization),
        (Ok(()), result, false) => result,
    }
}
