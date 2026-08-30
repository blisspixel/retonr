//! Exact mapping from observed closeout boundaries to portable failure facts.

use std::{error::Error, time::Instant};

use rewrite_app::{
    CandidateGenerationEvidenceBundleError, CandidateGenerationEvidenceRepositoryError,
    CandidateGenerationReceiptCompilationError,
};
use rewrite_model::{
    CandidateGenerationAttemptFailureCategoryV1, CandidateGenerationAttemptFailurePhaseV1,
    GenerationQualificationOperationTerminalStatusV1,
    GenerationQualificationPhaseInterruptionReasonV1, PlannedCandidateAttemptId,
};
use rewrite_types::CancellationToken;

use super::error::ActiveGenerationQualificationCandidateCloseoutErrorKind;
use crate::generation_qualification_preregistration::active::interruption::CandidateCloseoutInterruptionFacts;
use crate::generation_qualification_preregistration::prepared::PreparedGenerationQualificationValidationError;
use crate::{
    ActiveGenerationQualificationCandidateSettlementError, VerifiedCandidateBatchError,
    generation_qualification_preregistration::active::map_preparation_error,
};

pub(super) struct CloseoutPrimaryFailure {
    pub(super) kind: ActiveGenerationQualificationCandidateCloseoutErrorKind,
    pub(super) phase: CandidateGenerationAttemptFailurePhaseV1,
    pub(super) category: CandidateGenerationAttemptFailureCategoryV1,
    pub(super) source: Option<Box<dyn Error + 'static>>,
    observed_at: Instant,
}

impl CloseoutPrimaryFailure {
    pub(super) fn operation_scope() -> Self {
        Self {
            kind: ActiveGenerationQualificationCandidateCloseoutErrorKind::OperationScope,
            phase: CandidateGenerationAttemptFailurePhaseV1::BundlePublication,
            category: CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
            source: None,
            observed_at: Instant::now(),
        }
    }

    pub(super) fn active_authority(error: impl Error + 'static, final_check: bool) -> Self {
        Self::with_source(
            ActiveGenerationQualificationCandidateCloseoutErrorKind::ActiveAuthority,
            if final_check {
                CandidateGenerationAttemptFailurePhaseV1::ReceiptCompilation
            } else {
                CandidateGenerationAttemptFailurePhaseV1::BundlePublication
            },
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch,
            error,
        )
    }

    fn with_source(
        kind: ActiveGenerationQualificationCandidateCloseoutErrorKind,
        phase: CandidateGenerationAttemptFailurePhaseV1,
        category: CandidateGenerationAttemptFailureCategoryV1,
        source: impl Error + 'static,
    ) -> Self {
        Self {
            kind,
            phase,
            category,
            source: Some(Box::new(source)),
            observed_at: Instant::now(),
        }
    }

    pub(super) fn interruption_facts(
        &self,
        planned_attempt_id: PlannedCandidateAttemptId,
    ) -> CandidateCloseoutInterruptionFacts {
        let (reason, terminal_status) = match self.category {
            CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded => (
                GenerationQualificationPhaseInterruptionReasonV1::DeadlineExceeded,
                GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded,
            ),
            CandidateGenerationAttemptFailureCategoryV1::Cancelled => (
                GenerationQualificationPhaseInterruptionReasonV1::Cancelled,
                GenerationQualificationOperationTerminalStatusV1::Cancelled,
            ),
            _ if self.kind
                == ActiveGenerationQualificationCandidateCloseoutErrorKind::ActiveAuthority =>
            {
                (
                    GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift,
                    GenerationQualificationOperationTerminalStatusV1::Failed,
                )
            }
            _ => (
                GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed,
                GenerationQualificationOperationTerminalStatusV1::Failed,
            ),
        };
        CandidateCloseoutInterruptionFacts {
            planned_attempt_id,
            reason,
            terminal_status,
            observed_at: self.observed_at,
        }
    }
}

pub(super) fn publication_failure(error: impl Error + 'static) -> CloseoutPrimaryFailure {
    CloseoutPrimaryFailure::with_source(
        ActiveGenerationQualificationCandidateCloseoutErrorKind::BundlePublication,
        CandidateGenerationAttemptFailurePhaseV1::BundlePublication,
        CandidateGenerationAttemptFailureCategoryV1::PublicationFailed,
        error,
    )
}

pub(super) fn repository_failure(
    error: CandidateGenerationEvidenceRepositoryError,
) -> CloseoutPrimaryFailure {
    if matches!(
        error,
        CandidateGenerationEvidenceRepositoryError::EvidenceBundle(
            CandidateGenerationEvidenceBundleError::PublishedButReadbackFailed { .. }
        )
    ) {
        CloseoutPrimaryFailure::with_source(
            ActiveGenerationQualificationCandidateCloseoutErrorKind::BundleReadback,
            CandidateGenerationAttemptFailurePhaseV1::BundleReadback,
            CandidateGenerationAttemptFailureCategoryV1::ReadbackFailed,
            error,
        )
    } else {
        publication_failure(error)
    }
}

pub(super) fn receipt_failure(
    error: CandidateGenerationReceiptCompilationError,
) -> CloseoutPrimaryFailure {
    if matches!(error, CandidateGenerationReceiptCompilationError::Bundle(_)) {
        CloseoutPrimaryFailure::with_source(
            ActiveGenerationQualificationCandidateCloseoutErrorKind::BundleReadback,
            CandidateGenerationAttemptFailurePhaseV1::BundleReadback,
            CandidateGenerationAttemptFailureCategoryV1::ReadbackFailed,
            error,
        )
    } else {
        CloseoutPrimaryFailure::with_source(
            ActiveGenerationQualificationCandidateCloseoutErrorKind::ReceiptCompilation,
            CandidateGenerationAttemptFailurePhaseV1::ReceiptCompilation,
            CandidateGenerationAttemptFailureCategoryV1::ReceiptInvalid,
            error,
        )
    }
}

pub(super) fn batch_failure(error: VerifiedCandidateBatchError) -> CloseoutPrimaryFailure {
    let readback = matches!(
        error,
        VerifiedCandidateBatchError::ReceiptCompilation(
            CandidateGenerationReceiptCompilationError::Bundle(_)
        )
    );
    if readback {
        CloseoutPrimaryFailure::with_source(
            ActiveGenerationQualificationCandidateCloseoutErrorKind::BundleReadback,
            CandidateGenerationAttemptFailurePhaseV1::BundleReadback,
            CandidateGenerationAttemptFailureCategoryV1::ReadbackFailed,
            error,
        )
    } else {
        CloseoutPrimaryFailure::with_source(
            ActiveGenerationQualificationCandidateCloseoutErrorKind::ReceiptCompilation,
            CandidateGenerationAttemptFailurePhaseV1::ReceiptCompilation,
            CandidateGenerationAttemptFailureCategoryV1::ReceiptInvalid,
            error,
        )
    }
}

pub(super) fn settlement_failure(
    error: ActiveGenerationQualificationCandidateSettlementError,
) -> CloseoutPrimaryFailure {
    CloseoutPrimaryFailure::with_source(
        ActiveGenerationQualificationCandidateCloseoutErrorKind::ReceiptCompilation,
        CandidateGenerationAttemptFailurePhaseV1::ReceiptCompilation,
        CandidateGenerationAttemptFailureCategoryV1::ReceiptInvalid,
        error,
    )
}

pub(super) const fn settlement_finalization_failed(
    error: &ActiveGenerationQualificationCandidateSettlementError,
) -> bool {
    matches!(
        error,
        ActiveGenerationQualificationCandidateSettlementError::MandatoryFinalization
            | ActiveGenerationQualificationCandidateSettlementError::PrimaryAndFinalization { .. }
    )
}

pub(super) fn map_prepared_closeout_error(
    error: PreparedGenerationQualificationValidationError<CloseoutPrimaryFailure>,
) -> CloseoutPrimaryFailure {
    match error {
        PreparedGenerationQualificationValidationError::Initial(error)
        | PreparedGenerationQualificationValidationError::InitialAndFinal {
            initial: error, ..
        } => CloseoutPrimaryFailure::active_authority(map_preparation_error(error), false),
        PreparedGenerationQualificationValidationError::Final(error) => {
            CloseoutPrimaryFailure::active_authority(map_preparation_error(error), true)
        }
        PreparedGenerationQualificationValidationError::Callback(error)
        | PreparedGenerationQualificationValidationError::CallbackAndFinal {
            callback: error,
            ..
        } => error,
    }
}

pub(super) fn apply_terminal_precedence(
    mut failure: CloseoutPrimaryFailure,
    deadline: Instant,
    cancellation: &CancellationToken,
) -> CloseoutPrimaryFailure {
    failure.observed_at = Instant::now();
    if failure.observed_at >= deadline {
        failure.category = CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded;
    } else if cancellation.is_cancelled() {
        failure.category = CandidateGenerationAttemptFailureCategoryV1::Cancelled;
    }
    failure
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn deadline_precedes_cancellation_without_changing_the_observed_stage() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let failure = apply_terminal_precedence(
            CloseoutPrimaryFailure::operation_scope(),
            Instant::now(),
            &cancellation,
        );
        assert_eq!(
            failure.phase,
            CandidateGenerationAttemptFailurePhaseV1::BundlePublication
        );
        assert_eq!(
            failure.category,
            CandidateGenerationAttemptFailureCategoryV1::DeadlineExceeded
        );

        let failure = apply_terminal_precedence(
            CloseoutPrimaryFailure::operation_scope(),
            Instant::now() + Duration::from_mins(1),
            &cancellation,
        );
        assert_eq!(
            failure.category,
            CandidateGenerationAttemptFailureCategoryV1::Cancelled
        );
    }

    #[test]
    fn repository_postcommit_failure_is_readback_not_publication() {
        let failure =
            repository_failure(CandidateGenerationEvidenceRepositoryError::EvidenceBundle(
                CandidateGenerationEvidenceBundleError::PublishedButReadbackFailed {
                    source: Box::new(CandidateGenerationEvidenceBundleError::TreeMismatch),
                },
            ));
        assert_eq!(
            failure.kind,
            ActiveGenerationQualificationCandidateCloseoutErrorKind::BundleReadback
        );
        assert_eq!(
            failure.phase,
            CandidateGenerationAttemptFailurePhaseV1::BundleReadback
        );
        assert_eq!(
            failure.category,
            CandidateGenerationAttemptFailureCategoryV1::ReadbackFailed
        );
    }

    #[test]
    fn settlement_finalization_failure_is_not_erased_by_closeout_mapping() {
        assert!(settlement_finalization_failed(
            &ActiveGenerationQualificationCandidateSettlementError::MandatoryFinalization
        ));
        assert!(settlement_finalization_failed(
            &ActiveGenerationQualificationCandidateSettlementError::PrimaryAndFinalization {
                primary: Box::new(
                    ActiveGenerationQualificationCandidateSettlementError::OperationScope,
                ),
            }
        ));
        assert!(!settlement_finalization_failed(
            &ActiveGenerationQualificationCandidateSettlementError::OperationScope
        ));
    }
}
