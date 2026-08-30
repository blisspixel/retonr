//! Offline preregistration authority for one generation qualification operation.

mod active;
mod draft;
mod prepared;
mod pretraffic_terminalization;
mod projected;
mod repository;

pub use active::{
    ActiveGenerationQualificationAttemptLedgerClosure,
    ActiveGenerationQualificationAttemptLedgerError,
    ActiveGenerationQualificationAttemptLedgerErrorKind,
    ActiveGenerationQualificationCandidateCloseoutError,
    ActiveGenerationQualificationCandidateCloseoutErrorKind,
    ActiveGenerationQualificationCandidateCloseoutInput,
    ActiveGenerationQualificationCandidateRunError,
    ActiveGenerationQualificationCandidateRunErrorKind,
    ActiveGenerationQualificationCandidateRunInput,
    ActiveGenerationQualificationCandidateRunOutcome,
    ActiveGenerationQualificationCandidateSettlementError,
    ActiveGenerationQualificationCandidateSettlementErrorKind,
    ActiveGenerationQualificationJudgeRunError, ActiveGenerationQualificationJudgeRunErrorKind,
    ActiveGenerationQualificationJudgeRunInput, ActiveGenerationQualificationOperation,
    ActiveGenerationQualificationOperationInterruption,
    ActiveGenerationQualificationOperationInterruptionError,
    ActiveGenerationQualificationOperationInterruptionErrorKind,
    GenerationQualificationActivationError,
};
pub use draft::GenerationQualificationOperationDraft;
pub use prepared::{
    GenerationQualificationPreparationDisposition, PreparedGenerationQualificationOperation,
};
pub use pretraffic_terminalization::{
    FinalizedPhasePolicyRefusedPretrafficGenerationQualification,
    FinalizedRejectedPretrafficGenerationQualification,
    GenerationQualificationPhasePolicyRefusalError,
    GenerationQualificationPretrafficTerminalizationError,
};
pub use projected::ProjectedGenerationQualificationOperation;
pub use repository::{
    GenerationQualificationPreregistrationOpenError,
    GenerationQualificationPreregistrationRepository,
};

use std::time::Instant;

use rewrite_app::{
    GenerationQualificationLicenseAssessmentError, GenerationQualificationPlatformAssessmentError,
    GenerationQualificationRequestProjectionError,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

/// Content-redacted failure while preparing one qualification operation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationPreparationError {
    /// The original absolute operation deadline was reached.
    #[error("generation qualification preparation deadline was reached")]
    DeadlineExceeded,
    /// The original absolute operation deadline could not be represented.
    #[error("generation qualification preparation deadline is unavailable")]
    DeadlineUnavailable,
    /// Cancellation was observed at a mandatory checkpoint.
    #[error("generation qualification preparation was cancelled")]
    Cancelled,
    /// The operation policy or its exact portable closure did not match.
    #[error("generation qualification operation policy does not match")]
    OperationPolicyMismatch,
    /// The request stream, projection, or exact projection closure did not match.
    #[error("generation qualification request projection does not match")]
    RequestProjectionMismatch,
    /// A canonical preregistration record could not be encoded.
    #[error("generation qualification preregistration encoding failed")]
    RepositoryEncoding,
    /// A canonical preregistration record exceeded its fixed bound.
    #[error("generation qualification preregistration limit was exceeded")]
    RepositoryLimit,
    /// The exact current durable preregistration store was unavailable.
    #[error("generation qualification preregistration repository is unavailable")]
    RepositoryUnavailable,
    /// An immutable preregistration identity already named different bytes.
    #[error("generation qualification preregistration record conflicts")]
    RepositoryConflict,
    /// Atomic readback did not reproduce both exact canonical records.
    #[error("generation qualification preregistration readback does not match")]
    ReadbackMismatch,
    /// The app-owned platform assessment authority did not match or revalidate.
    #[error("generation qualification platform assessment does not match")]
    PlatformAssessmentMismatch,
    /// The app-owned license assessment authority did not match or revalidate.
    #[error("generation qualification license assessment does not match")]
    LicenseAssessmentMismatch,
}

fn check_gate(
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPreparationError> {
    if Instant::now() >= deadline {
        Err(GenerationQualificationPreparationError::DeadlineExceeded)
    } else if cancellation.is_cancelled() {
        Err(GenerationQualificationPreparationError::Cancelled)
    } else {
        Ok(())
    }
}

fn map_projection_error(
    error: GenerationQualificationRequestProjectionError,
) -> GenerationQualificationPreparationError {
    match error {
        GenerationQualificationRequestProjectionError::Cancelled => {
            GenerationQualificationPreparationError::Cancelled
        }
        GenerationQualificationRequestProjectionError::DeadlineExceeded => {
            GenerationQualificationPreparationError::DeadlineExceeded
        }
        GenerationQualificationRequestProjectionError::DeadlineUnavailable => {
            GenerationQualificationPreparationError::DeadlineUnavailable
        }
        _ => GenerationQualificationPreparationError::RequestProjectionMismatch,
    }
}

fn map_platform_error(
    error: GenerationQualificationPlatformAssessmentError,
) -> GenerationQualificationPreparationError {
    if matches!(
        error,
        GenerationQualificationPlatformAssessmentError::Cancelled
    ) {
        GenerationQualificationPreparationError::Cancelled
    } else {
        GenerationQualificationPreparationError::PlatformAssessmentMismatch
    }
}

fn map_license_error(
    error: &GenerationQualificationLicenseAssessmentError,
) -> GenerationQualificationPreparationError {
    if matches!(
        error,
        GenerationQualificationLicenseAssessmentError::Cancelled
    ) {
        GenerationQualificationPreparationError::Cancelled
    } else {
        GenerationQualificationPreparationError::LicenseAssessmentMismatch
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn original_deadline_precedes_cancellation() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            check_gate(Instant::now(), &cancellation),
            Err(GenerationQualificationPreparationError::DeadlineExceeded)
        );
    }

    #[test]
    fn active_deadline_observes_cancellation() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert_eq!(
            check_gate(Instant::now() + Duration::from_secs(1), &cancellation),
            Err(GenerationQualificationPreparationError::Cancelled)
        );
    }
}

#[cfg(test)]
#[path = "generation_qualification_preregistration/state_machine_tests.rs"]
mod state_machine_tests;
