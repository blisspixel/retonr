//! Consuming owner for one traffic-eligible qualification operation.

use std::fmt;

use rewrite_app::CandidateGenerationEvidenceRepository;
use rewrite_model::{
    CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptV1,
    GenerationQualificationOperationPolicyV1, GenerationQualificationRequestProjectionV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    GenerationQualificationPreparationDisposition, GenerationQualificationPreparationError,
    GenerationQualificationPreregistrationRepository, PreparedGenerationQualificationOperation,
    prepared::{
        PreparedGenerationQualificationMandatoryFinalizationError,
        PreparedGenerationQualificationValidationError,
    },
};
use crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject;
use crate::local_ollama_managed_preflight::GenerationQualificationLiveLifecycle;

mod admission;
mod attempt_ledger;
mod candidate;
mod candidate_closeout;
mod candidate_settlement;
mod deterministic;
mod interruption;
mod judge;
mod judge_settlement;
pub use judge_settlement::ActiveGenerationQualificationJudgeSettlementError;
mod receipt_set;
pub use deterministic::ActiveGenerationQualificationDeterministicSettlementError;

pub use receipt_set::ActiveGenerationQualificationReceiptSetError;

pub use attempt_ledger::{
    ActiveGenerationQualificationAttemptLedgerClosure,
    ActiveGenerationQualificationAttemptLedgerError,
    ActiveGenerationQualificationAttemptLedgerErrorKind,
};
pub use candidate::{
    ActiveGenerationQualificationCandidateRunError,
    ActiveGenerationQualificationCandidateRunErrorKind,
    ActiveGenerationQualificationCandidateRunInput,
    ActiveGenerationQualificationCandidateRunOutcome,
};
pub use candidate_closeout::{
    ActiveGenerationQualificationCandidateCloseoutError,
    ActiveGenerationQualificationCandidateCloseoutErrorKind,
    ActiveGenerationQualificationCandidateCloseoutInput,
};
pub use candidate_settlement::{
    ActiveGenerationQualificationCandidateSettlementError,
    ActiveGenerationQualificationCandidateSettlementErrorKind,
};
pub use interruption::{
    ActiveGenerationQualificationOperationInterruption,
    ActiveGenerationQualificationOperationInterruptionError,
    ActiveGenerationQualificationOperationInterruptionErrorKind,
};
pub use judge::{
    ActiveGenerationQualificationJudgeRunError, ActiveGenerationQualificationJudgeRunErrorKind,
    ActiveGenerationQualificationJudgeRunInput,
};

/// Content-redacted failure while consuming Prepared into the active owner.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationActivationError {
    /// The original absolute operation deadline was reached.
    #[error("generation qualification activation deadline was reached")]
    DeadlineExceeded,
    /// Cancellation was observed before the original deadline.
    #[error("generation qualification activation was cancelled")]
    Cancelled,
    /// Fresh validation of the retained Prepared authority failed.
    #[error("generation qualification prepared authority does not match")]
    PreparedAuthority,
    /// The exact platform assessment rejected qualification traffic.
    #[error("generation qualification platform assessment rejected traffic")]
    PlatformRejected,
    /// The exact model-license assessment rejected qualification traffic.
    #[error("generation qualification license assessment rejected traffic")]
    LicenseRejected,
    /// Fresh mandatory finalization validation failed.
    #[error("generation qualification mandatory finalization validation failed")]
    MandatoryFinalization,
    /// Independent activation validation stages failed differently.
    #[error("generation qualification activation validation failures were aggregated")]
    ValidationAggregation,
    /// A precursor checkpoint exists and activation cannot continue.
    #[error("generation qualification candidate attempt is checkpoint-only")]
    CandidateCheckpointOnly,
    /// A failed candidate attempt exists and activation cannot continue.
    #[error("generation qualification candidate attempt failed")]
    CandidateTerminalFailed,
    /// A completed candidate attempt exists and activation cannot continue.
    #[error("generation qualification candidate attempt is already completed")]
    CandidateTerminalCompleted,
    /// A bundle directory exists without matching terminal metadata.
    #[error("generation qualification candidate evidence publication is orphaned")]
    CandidatePublicationOrphan,
    /// Completed metadata does not match a bundle directory on this evidence root.
    #[error("generation qualification candidate evidence storage does not match")]
    CandidateEvidenceStorageMismatch,
    /// Indexed metadata and the evidence root do not form one legal class.
    #[error("generation qualification candidate commit is ambiguous")]
    CandidateAmbiguousCommit,
    /// Evidence staging contains an unexpected direct child.
    #[error("generation qualification candidate evidence staging is unexpected")]
    CandidateUnexpectedStaging,
    /// Candidate metadata or evidence layout is not trustworthy.
    #[error("generation qualification candidate evidence is corrupt")]
    CandidateEvidenceCorrupt,
    /// Candidate inspection exceeded a fixed bound.
    #[error("generation qualification candidate inspection bound was exceeded")]
    CandidateInspectionBoundExceeded,
    /// Candidate metadata or the evidence root could not be read.
    #[error("generation qualification candidate inspection is unavailable")]
    CandidateInspectionUnavailable,
}

/// Noncloneable, nonserializable owner of one traffic-eligible operation.
///
/// This state consumes Prepared once and retains its exact preregistration,
/// platform, license, request-stream, deadline, and process-local lifecycle
/// authorities. It does not itself claim qualification or production approval.
/// Candidate and judge traffic are released only through exact sequence-checked
/// methods on this owner.
///
/// ```compile_fail
/// use rewrite_eval::ActiveGenerationQualificationOperation;
/// fn cannot_clone(value: &ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>) {
///     let _forged = (*value).clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::ActiveGenerationQualificationOperation;
/// fn require_serialize<T: serde::Serialize>(_: &T) {}
/// fn cannot_serialize(value: &ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>) {
///     require_serialize(value);
/// }
/// ```
pub struct ActiveGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease> {
    prepared: PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
    subject: ActiveGenerationQualificationSubject,
    lifecycle: GenerationQualificationLiveLifecycle,
    next_candidate_attempt: usize,
    started_candidate_attempt: Option<attempt_ledger::ActiveCandidateAttemptScope>,
    pending_completed_candidate: Option<attempt_ledger::PendingActiveCandidateAttempt>,
    target_attempt_records: Vec<CandidateGenerationAttemptRecordV1>,
    target_attempt_receipts: Vec<CandidateGenerationReceiptV1>,
    attempt_ledger: Option<ActiveGenerationQualificationAttemptLedgerClosure>,
    operation_interruption: Option<ActiveGenerationQualificationOperationInterruption>,
    next_judge_repetition: usize,
    next_judge_settlement: usize,
    executed_judge_joins: Vec<rewrite_model::CandidateJudgeJoinId>,
    terminal: bool,
}

impl<'records, 'store, 'platform, 'proof, 'lease>
    PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>
{
    /// Consumes a traffic-eligible Prepared authority into its sole active owner.
    ///
    /// Normal validation observes the original deadline and caller cancellation.
    /// Independent mandatory validation then uses fresh uncancelled authority and
    /// checks every retained finalizer even when normal validation failed.
    /// A read-only candidate admission check then runs. Activation continues only
    /// when every planned attempt is not started, staging is empty, and this plan
    /// has no bundle directory. The check does not retry, repair, promote, delete,
    /// or fabricate evidence, and it does not insert the traffic precursor.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for expiry, cancellation, rejected traffic,
    /// retained-authority drift, independently observed finalization failure, or any
    /// candidate attempt that is not pristine.
    pub fn activate(
        mut self,
        repository: &GenerationQualificationPreregistrationRepository,
        evidence: &CandidateGenerationEvidenceRepository,
        cancellation: &CancellationToken,
    ) -> Result<
        ActiveGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
        GenerationQualificationActivationError,
    > {
        validate_activation(&mut self, cancellation)?;
        admission::reconcile(&self, repository, evidence, cancellation)?;
        Ok(ActiveGenerationQualificationOperation {
            prepared: self,
            subject: ActiveGenerationQualificationSubject::new(),
            lifecycle: GenerationQualificationLiveLifecycle::new(),
            next_candidate_attempt: 0,
            started_candidate_attempt: None,
            pending_completed_candidate: None,
            target_attempt_records: Vec::new(),
            target_attempt_receipts: Vec::new(),
            attempt_ledger: None,
            operation_interruption: None,
            next_judge_repetition: 0,
            next_judge_settlement: 0,
            executed_judge_joins: Vec::new(),
            terminal: false,
        })
    }
}

impl ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Returns the exact operation-policy repository readback.
    #[must_use]
    pub const fn operation_policy(&self) -> &GenerationQualificationOperationPolicyV1 {
        self.prepared.operation_policy()
    }

    /// Returns the exact request-projection repository readback.
    #[must_use]
    pub const fn request_projection(&self) -> &GenerationQualificationRequestProjectionV1 {
        self.prepared.request_projection()
    }

    /// Reports whether this owner ever acquired a live managed authority.
    #[must_use]
    pub fn ever_acquired_live_authority(&self) -> bool {
        self.lifecycle.ever_acquired()
    }

    /// Returns the process-local peak live-authority count for this owner.
    #[must_use]
    pub fn peak_live_authorities(&self) -> u8 {
        self.lifecycle.peak_live_authorities()
    }

    /// Returns the number of exact plan-order candidate attempts durably settled.
    #[must_use]
    pub const fn settled_candidate_attempts(&self) -> usize {
        self.next_candidate_attempt
    }

    /// Returns the number of attempts checkpointed, including one unresolved attempt.
    #[must_use]
    pub const fn executed_candidate_attempts(&self) -> usize {
        if self.started_candidate_attempt.is_some() || self.pending_completed_candidate.is_some() {
            self.next_candidate_attempt + 1
        } else {
            self.next_candidate_attempt
        }
    }

    /// Reports whether one checkpointed attempt has an unresolved execution outcome.
    #[must_use]
    pub const fn has_started_candidate_attempt(&self) -> bool {
        self.started_candidate_attempt.is_some()
    }

    /// Reports whether one completed attempt still needs durable batch settlement.
    #[must_use]
    pub const fn has_pending_completed_candidate(&self) -> bool {
        self.pending_completed_candidate.is_some()
    }

    /// Returns the sealed target ledger closure when evidence collection finished.
    #[must_use]
    pub const fn attempt_ledger(
        &self,
    ) -> Option<&ActiveGenerationQualificationAttemptLedgerClosure> {
        match self.attempt_ledger.as_ref() {
            Some(ledger) => Some(ledger),
            None => match self.operation_interruption.as_ref() {
                Some(interruption) => Some(interruption.attempt_ledger()),
                None => None,
            },
        }
    }

    /// Returns the exact operation interruption closure after terminal failure.
    #[must_use]
    pub const fn operation_interruption(
        &self,
    ) -> Option<&ActiveGenerationQualificationOperationInterruption> {
        self.operation_interruption.as_ref()
    }

    /// Returns the number of successful strict judge repetitions consumed in order.
    #[must_use]
    pub const fn completed_judge_repetitions(&self) -> usize {
        self.next_judge_repetition
    }

    /// Reports whether a failed strict run permanently terminalized this owner.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }

    /// Freshly validates the retained active authority and every mandatory finalizer.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for expiry, cancellation, traffic-disposition
    /// drift, any changed retained relationship, or mandatory finalization failure.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), GenerationQualificationActivationError> {
        validate_activation(&mut self.prepared, cancellation)
    }
}

impl fmt::Debug for ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationOperation")
            .field(
                "operation_policy_id",
                self.prepared.operation_policy().operation_policy_id(),
            )
            .field(
                "request_projection_id",
                self.prepared.request_projection().request_projection_id(),
            )
            .field(
                "ever_acquired_live_authority",
                &self.ever_acquired_live_authority(),
            )
            .field("peak_live_authorities", &self.peak_live_authorities())
            .field(
                "settled_candidate_attempts",
                &self.settled_candidate_attempts(),
            )
            .field(
                "executed_candidate_attempts",
                &self.executed_candidate_attempts(),
            )
            .field(
                "pending_completed_candidate",
                &self.has_pending_completed_candidate(),
            )
            .field(
                "started_candidate_attempt",
                &self.has_started_candidate_attempt(),
            )
            .field("attempt_ledger_sealed", &self.attempt_ledger().is_some())
            .field(
                "operation_interruption_available",
                &self.operation_interruption.is_some(),
            )
            .field(
                "completed_judge_repetitions",
                &self.completed_judge_repetitions(),
            )
            .field("terminal", &self.is_terminal())
            .finish_non_exhaustive()
    }
}

fn validate_activation(
    prepared: &mut PreparedGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationActivationError> {
    let normal = prepared
        .with_validated_view(cancellation, |view| {
            ensure_traffic_eligible(view.disposition)
        })
        .map_err(map_normal_validation_error);
    let mandatory = prepared
        .with_mandatory_finalization_validated_view(|view| {
            ensure_traffic_eligible(view.disposition)
        })
        .map_err(|error| map_mandatory_validation_error(&error));
    combine_validation(normal, mandatory)
}

const fn ensure_traffic_eligible(
    disposition: GenerationQualificationPreparationDisposition,
) -> Result<(), GenerationQualificationActivationError> {
    match disposition {
        GenerationQualificationPreparationDisposition::TrafficEligible => Ok(()),
        GenerationQualificationPreparationDisposition::PlatformRejected => {
            Err(GenerationQualificationActivationError::PlatformRejected)
        }
        GenerationQualificationPreparationDisposition::LicenseRejected => {
            Err(GenerationQualificationActivationError::LicenseRejected)
        }
    }
}

fn map_normal_validation_error(
    error: PreparedGenerationQualificationValidationError<GenerationQualificationActivationError>,
) -> GenerationQualificationActivationError {
    match error {
        PreparedGenerationQualificationValidationError::Initial(error)
        | PreparedGenerationQualificationValidationError::Final(error) => {
            map_preparation_error(error)
        }
        PreparedGenerationQualificationValidationError::Callback(error) => error,
        PreparedGenerationQualificationValidationError::InitialAndFinal {
            initial,
            final_validation,
        } => combine_distinct_errors(
            map_preparation_error(initial),
            map_preparation_error(final_validation),
        ),
        PreparedGenerationQualificationValidationError::CallbackAndFinal {
            callback,
            final_validation,
        } => combine_distinct_errors(callback, map_preparation_error(final_validation)),
    }
}

fn map_mandatory_validation_error(
    error: &PreparedGenerationQualificationMandatoryFinalizationError<
        GenerationQualificationActivationError,
    >,
) -> GenerationQualificationActivationError {
    match error {
        PreparedGenerationQualificationMandatoryFinalizationError::Callback(error) => *error,
        PreparedGenerationQualificationMandatoryFinalizationError::Initial(_)
        | PreparedGenerationQualificationMandatoryFinalizationError::Final(_)
        | PreparedGenerationQualificationMandatoryFinalizationError::InitialAndFinal { .. } => {
            GenerationQualificationActivationError::MandatoryFinalization
        }
        PreparedGenerationQualificationMandatoryFinalizationError::CallbackAndFinal {
            callback,
            final_validation: _,
        } => combine_distinct_errors(
            *callback,
            GenerationQualificationActivationError::MandatoryFinalization,
        ),
    }
}

const fn map_preparation_error(
    error: GenerationQualificationPreparationError,
) -> GenerationQualificationActivationError {
    match error {
        GenerationQualificationPreparationError::DeadlineExceeded => {
            GenerationQualificationActivationError::DeadlineExceeded
        }
        GenerationQualificationPreparationError::Cancelled => {
            GenerationQualificationActivationError::Cancelled
        }
        _ => GenerationQualificationActivationError::PreparedAuthority,
    }
}

fn combine_validation(
    normal: Result<(), GenerationQualificationActivationError>,
    mandatory: Result<(), GenerationQualificationActivationError>,
) -> Result<(), GenerationQualificationActivationError> {
    match (normal, mandatory) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(normal), Err(mandatory)) => Err(combine_distinct_errors(normal, mandatory)),
    }
}

fn combine_distinct_errors(
    first: GenerationQualificationActivationError,
    second: GenerationQualificationActivationError,
) -> GenerationQualificationActivationError {
    if first == second {
        first
    } else {
        GenerationQualificationActivationError::ValidationAggregation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_traffic_eligible_disposition_can_activate() {
        assert_eq!(
            ensure_traffic_eligible(GenerationQualificationPreparationDisposition::TrafficEligible),
            Ok(())
        );
        assert_eq!(
            ensure_traffic_eligible(
                GenerationQualificationPreparationDisposition::PlatformRejected
            ),
            Err(GenerationQualificationActivationError::PlatformRejected)
        );
        assert_eq!(
            ensure_traffic_eligible(GenerationQualificationPreparationDisposition::LicenseRejected),
            Err(GenerationQualificationActivationError::LicenseRejected)
        );
    }

    #[test]
    fn equal_failures_retain_their_specific_category() {
        assert_eq!(
            combine_distinct_errors(
                GenerationQualificationActivationError::Cancelled,
                GenerationQualificationActivationError::Cancelled,
            ),
            GenerationQualificationActivationError::Cancelled
        );
        assert_eq!(
            combine_distinct_errors(
                GenerationQualificationActivationError::Cancelled,
                GenerationQualificationActivationError::MandatoryFinalization,
            ),
            GenerationQualificationActivationError::ValidationAggregation
        );
    }
}
