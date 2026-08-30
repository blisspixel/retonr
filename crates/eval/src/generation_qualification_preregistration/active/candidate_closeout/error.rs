//! Content-redacted public candidate-closeout errors.

use std::{error::Error, fmt};

use rewrite_model::CandidateGenerationAttemptRecordV1;

use super::failure::CloseoutPrimaryFailure;
use crate::generation_qualification_preregistration::active::interruption::CandidateCloseoutInterruptionDisposition;

/// Stable content-free category for Active candidate-closeout failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveGenerationQualificationCandidateCloseoutErrorKind {
    /// A prior failure already terminalized the operation.
    OperationTerminated,
    /// No completed candidate is pending.
    NoPendingCandidate,
    /// The supplied closeout authority or exact scope differed.
    OperationScope,
    /// Prepared or Active authority validation failed.
    ActiveAuthority,
    /// Bundle planning or no-replace publication failed.
    BundlePublication,
    /// Post-commit or retained bundle readback failed.
    BundleReadback,
    /// Receipt compilation or final candidate join failed.
    ReceiptCompilation,
    /// Durable terminal metadata could not be committed or confirmed.
    Durability,
    /// Independent mandatory finalization also failed.
    PrimaryAndFinalization,
    /// The exact portable failed-attempt record could not be constructed.
    FailureRecord,
}

/// Content-redacted failure that may retain one exact portable failed record.
pub struct ActiveGenerationQualificationCandidateCloseoutError {
    kind: ActiveGenerationQualificationCandidateCloseoutErrorKind,
    attempt_record: Option<Box<CandidateGenerationAttemptRecordV1>>,
    operation_interruption_available: bool,
    interruption_compilation_failed: bool,
    mandatory_finalization_failed: bool,
    source: Option<Box<dyn Error + 'static>>,
}

impl ActiveGenerationQualificationCandidateCloseoutError {
    /// Returns the stable content-free failure category.
    #[must_use]
    pub const fn kind(&self) -> ActiveGenerationQualificationCandidateCloseoutErrorKind {
        self.kind
    }

    /// Returns the exact failed-attempt record when trustworthy closure succeeded.
    #[must_use]
    pub fn attempt_record(&self) -> Option<&CandidateGenerationAttemptRecordV1> {
        self.attempt_record.as_deref()
    }

    /// Reports whether independent mandatory finalization also failed.
    #[must_use]
    pub const fn mandatory_finalization_failed(&self) -> bool {
        self.mandatory_finalization_failed
    }

    /// Reports whether the Active owner retained an exact operation interruption.
    #[must_use]
    pub const fn operation_interruption_available(&self) -> bool {
        self.operation_interruption_available
    }

    /// Reports whether portable interruption compilation itself failed.
    #[must_use]
    pub const fn interruption_compilation_failed(&self) -> bool {
        self.interruption_compilation_failed
    }

    pub(super) fn simple(kind: ActiveGenerationQualificationCandidateCloseoutErrorKind) -> Self {
        Self {
            kind,
            attempt_record: None,
            operation_interruption_available: false,
            interruption_compilation_failed: false,
            mandatory_finalization_failed: false,
            source: None,
        }
    }

    pub(super) fn closed(
        failure: CloseoutPrimaryFailure,
        record: CandidateGenerationAttemptRecordV1,
        disposition: CandidateCloseoutInterruptionDisposition,
    ) -> Self {
        let mandatory_finalization_failed = disposition.finalization_failed();
        Self {
            kind: if mandatory_finalization_failed {
                ActiveGenerationQualificationCandidateCloseoutErrorKind::PrimaryAndFinalization
            } else {
                failure.kind
            },
            attempt_record: Some(Box::new(record)),
            operation_interruption_available: disposition.available(),
            interruption_compilation_failed: disposition.compilation_failed(),
            mandatory_finalization_failed,
            source: failure.source,
        }
    }

    pub(super) fn record_failure(
        error: impl Error + 'static,
        mandatory_finalization_failed: bool,
    ) -> Self {
        Self {
            kind: ActiveGenerationQualificationCandidateCloseoutErrorKind::FailureRecord,
            attempt_record: None,
            operation_interruption_available: false,
            interruption_compilation_failed: true,
            mandatory_finalization_failed,
            source: Some(Box::new(error)),
        }
    }

    pub(super) fn durability(error: impl Error + 'static) -> Self {
        Self {
            kind: ActiveGenerationQualificationCandidateCloseoutErrorKind::Durability,
            attempt_record: None,
            operation_interruption_available: false,
            interruption_compilation_failed: false,
            mandatory_finalization_failed: false,
            source: Some(Box::new(error)),
        }
    }
}

impl fmt::Display for ActiveGenerationQualificationCandidateCloseoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("active generation qualification candidate closeout failed")
    }
}

impl fmt::Debug for ActiveGenerationQualificationCandidateCloseoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActiveGenerationQualificationCandidateCloseoutError")
            .field("kind", &self.kind)
            .field("attempt_record_available", &self.attempt_record.is_some())
            .field(
                "operation_interruption_available",
                &self.operation_interruption_available,
            )
            .field(
                "interruption_compilation_failed",
                &self.interruption_compilation_failed,
            )
            .field(
                "mandatory_finalization_failed",
                &self.mandatory_finalization_failed,
            )
            .finish_non_exhaustive()
    }
}

impl Error for ActiveGenerationQualificationCandidateCloseoutError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_error_debug_and_display_are_content_redacted() {
        let error = ActiveGenerationQualificationCandidateCloseoutError {
            kind: ActiveGenerationQualificationCandidateCloseoutErrorKind::BundlePublication,
            attempt_record: None,
            operation_interruption_available: false,
            interruption_compilation_failed: false,
            mandatory_finalization_failed: true,
            source: Some(Box::new(std::io::Error::other("sensitive source detail"))),
        };
        assert_eq!(
            error.kind(),
            ActiveGenerationQualificationCandidateCloseoutErrorKind::BundlePublication
        );
        assert!(error.attempt_record().is_none());
        assert!(error.mandatory_finalization_failed());
        assert!(!error.operation_interruption_available());
        assert!(!error.interruption_compilation_failed());
        assert!(!format!("{error:?}").contains("sensitive source detail"));
        assert_eq!(
            error.to_string(),
            "active generation qualification candidate closeout failed"
        );
    }
}
