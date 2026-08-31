//! Fresh validation reserved for mandatory operation finalization.

use std::{error::Error, fmt};

use rewrite_app::{
    GenerationQualificationLicenseAssessmentError, GenerationQualificationPlatformAssessmentError,
    GenerationQualificationRequestProjectionError,
};
use rewrite_types::CancellationToken;

use super::{
    GenerationQualificationPreparationError, PreparedGenerationQualificationOperation,
    PreparedGenerationQualificationValidationView, disposition, license_relations,
    platform_relations, validate_readback_closure,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MandatoryFinalizer {
    RequestProjection,
    CanonicalReadback,
    PlatformAuthority,
    LicenseAuthority,
}

enum MandatoryFinalizerFailure {
    RequestProjection(GenerationQualificationRequestProjectionError),
    CanonicalReadback(GenerationQualificationPreparationError),
    PlatformAuthority(GenerationQualificationPlatformAssessmentError),
    LicenseAuthority(GenerationQualificationLicenseAssessmentError),
}

/// Content-redacted report retaining every exact mandatory-finalizer failure.
pub(crate) struct PreparedGenerationQualificationMandatoryFinalizationFailures {
    request_projection: Option<GenerationQualificationRequestProjectionError>,
    canonical_readback: Option<GenerationQualificationPreparationError>,
    platform_authority: Option<GenerationQualificationPlatformAssessmentError>,
    license_authority: Option<GenerationQualificationLicenseAssessmentError>,
}

impl PreparedGenerationQualificationMandatoryFinalizationFailures {
    fn is_empty(&self) -> bool {
        self.request_projection.is_none()
            && self.canonical_readback.is_none()
            && self.platform_authority.is_none()
            && self.license_authority.is_none()
    }

    /// Reports whether request-projection reconstruction failed.
    pub(crate) const fn request_projection_failed(&self) -> bool {
        self.request_projection.is_some()
    }

    /// Reports whether canonical readback closure validation failed.
    pub(crate) const fn canonical_readback_failed(&self) -> bool {
        self.canonical_readback.is_some()
    }

    /// Reports whether platform-authority validation failed.
    pub(crate) const fn platform_authority_failed(&self) -> bool {
        self.platform_authority.is_some()
    }

    /// Reports whether license-authority validation failed.
    pub(crate) const fn license_authority_failed(&self) -> bool {
        self.license_authority.is_some()
    }
}

impl fmt::Debug for PreparedGenerationQualificationMandatoryFinalizationFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedGenerationQualificationMandatoryFinalizationFailures")
            .field(
                "request_projection_failed",
                &self.request_projection_failed(),
            )
            .field(
                "canonical_readback_failed",
                &self.canonical_readback_failed(),
            )
            .field(
                "platform_authority_failed",
                &self.platform_authority_failed(),
            )
            .field("license_authority_failed", &self.license_authority_failed())
            .finish()
    }
}

impl fmt::Display for PreparedGenerationQualificationMandatoryFinalizationFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("mandatory generation qualification finalization validation failed")
    }
}

impl Error for PreparedGenerationQualificationMandatoryFinalizationFailures {}

/// Failure bracketing use of the exact Prepared finalization view.
pub(crate) enum PreparedGenerationQualificationMandatoryFinalizationError<E> {
    Initial(Box<PreparedGenerationQualificationMandatoryFinalizationFailures>),
    Callback(E),
    Final(Box<PreparedGenerationQualificationMandatoryFinalizationFailures>),
    InitialAndFinal {
        initial: Box<PreparedGenerationQualificationMandatoryFinalizationFailures>,
        final_validation: Box<PreparedGenerationQualificationMandatoryFinalizationFailures>,
    },
    CallbackAndFinal {
        callback: E,
        final_validation: Box<PreparedGenerationQualificationMandatoryFinalizationFailures>,
    },
}

impl<E> fmt::Debug for PreparedGenerationQualificationMandatoryFinalizationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Initial(error) => formatter.debug_tuple("Initial").field(error).finish(),
            Self::Callback(_) => formatter.write_str("Callback(<redacted>)"),
            Self::Final(error) => formatter.debug_tuple("Final").field(error).finish(),
            Self::InitialAndFinal {
                initial,
                final_validation,
            } => formatter
                .debug_struct("InitialAndFinal")
                .field("initial", initial)
                .field("final_validation", final_validation)
                .finish(),
            Self::CallbackAndFinal {
                callback: _,
                final_validation,
            } => formatter
                .debug_struct("CallbackAndFinal")
                .field("callback", &"<redacted>")
                .field("final_validation", final_validation)
                .finish(),
        }
    }
}

impl PreparedGenerationQualificationOperation<'_, '_, '_, '_, '_> {
    /// Runs every mandatory finalizer with fresh cancellation authority.
    pub(crate) fn revalidate_for_mandatory_finalization(
        &mut self,
    ) -> Result<(), PreparedGenerationQualificationMandatoryFinalizationFailures> {
        let expected_disposition = self.disposition;
        let failures = run_all_finalizers(|finalizer, cancellation| match finalizer {
            MandatoryFinalizer::RequestProjection => self
                .stream
                .revalidate_for_mandatory_finalization()
                .map_err(MandatoryFinalizerFailure::RequestProjection),
            MandatoryFinalizer::CanonicalReadback => {
                let result =
                    validate_readback_closure(&self.readbacks, &self.stream).and_then(|()| {
                        if disposition(&self.platform, &self.license) == expected_disposition {
                            Ok(())
                        } else {
                            Err(GenerationQualificationPreparationError::ReadbackMismatch)
                        }
                    });
                result.map_err(MandatoryFinalizerFailure::CanonicalReadback)
            }
            MandatoryFinalizer::PlatformAuthority => self
                .platform
                .revalidate(
                    platform_relations(&self.readbacks, &self.stream),
                    cancellation,
                )
                .map_err(MandatoryFinalizerFailure::PlatformAuthority),
            MandatoryFinalizer::LicenseAuthority => self
                .license
                .revalidate_portable(
                    license_relations(
                        &self.readbacks,
                        &self.stream,
                        &self.license_assessment_policy,
                        &self.production_approval_policy,
                    ),
                    cancellation,
                )
                .map_err(MandatoryFinalizerFailure::LicenseAuthority),
        });
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures)
        }
    }

    /// Brackets access to the exact Prepared view with independent finalization checks.
    pub(crate) fn with_mandatory_finalization_validated_view<T, E>(
        &mut self,
        use_view: impl for<'view> FnOnce(
            PreparedGenerationQualificationValidationView<'view>,
        ) -> Result<T, E>,
    ) -> Result<T, PreparedGenerationQualificationMandatoryFinalizationError<E>> {
        let initial = self.revalidate_for_mandatory_finalization();
        let callback = if initial.is_ok() {
            Some(use_view(self.validation_view()))
        } else {
            None
        };
        let final_validation = self.revalidate_for_mandatory_finalization();
        combine_validation_results(initial, callback, final_validation)
    }

    #[cfg(test)]
    pub(crate) fn expire_deadline_for_test(&mut self) {
        self.deadline = std::time::Instant::now();
    }
}

fn run_all_finalizers(
    mut run: impl FnMut(MandatoryFinalizer, &CancellationToken) -> Result<(), MandatoryFinalizerFailure>,
) -> PreparedGenerationQualificationMandatoryFinalizationFailures {
    let mut failures = PreparedGenerationQualificationMandatoryFinalizationFailures {
        request_projection: None,
        canonical_readback: None,
        platform_authority: None,
        license_authority: None,
    };
    for finalizer in [
        MandatoryFinalizer::RequestProjection,
        MandatoryFinalizer::CanonicalReadback,
        MandatoryFinalizer::PlatformAuthority,
        MandatoryFinalizer::LicenseAuthority,
    ] {
        let cancellation = CancellationToken::new();
        if let Err(failure) = run(finalizer, &cancellation) {
            match failure {
                MandatoryFinalizerFailure::RequestProjection(error) => {
                    failures.request_projection = Some(error);
                }
                MandatoryFinalizerFailure::CanonicalReadback(error) => {
                    failures.canonical_readback = Some(error);
                }
                MandatoryFinalizerFailure::PlatformAuthority(error) => {
                    failures.platform_authority = Some(error);
                }
                MandatoryFinalizerFailure::LicenseAuthority(error) => {
                    failures.license_authority = Some(error);
                }
            }
        }
    }
    failures
}

fn combine_validation_results<T, E>(
    initial: Result<(), PreparedGenerationQualificationMandatoryFinalizationFailures>,
    callback: Option<Result<T, E>>,
    final_validation: Result<(), PreparedGenerationQualificationMandatoryFinalizationFailures>,
) -> Result<T, PreparedGenerationQualificationMandatoryFinalizationError<E>> {
    match (initial, callback, final_validation) {
        (Err(initial), None, Err(final_validation)) => Err(
            PreparedGenerationQualificationMandatoryFinalizationError::InitialAndFinal {
                initial: Box::new(initial),
                final_validation: Box::new(final_validation),
            },
        ),
        (Err(initial), None, Ok(())) => Err(
            PreparedGenerationQualificationMandatoryFinalizationError::Initial(Box::new(initial)),
        ),
        (Ok(()), Some(Err(callback)), Err(final_validation)) => Err(
            PreparedGenerationQualificationMandatoryFinalizationError::CallbackAndFinal {
                callback,
                final_validation: Box::new(final_validation),
            },
        ),
        (Ok(()), Some(Err(callback)), Ok(())) => {
            Err(PreparedGenerationQualificationMandatoryFinalizationError::Callback(callback))
        }
        (Ok(()), Some(Ok(_)), Err(final_validation)) => Err(
            PreparedGenerationQualificationMandatoryFinalizationError::Final(Box::new(
                final_validation,
            )),
        ),
        (Ok(()), Some(Ok(value)), Ok(())) => Ok(value),
        (Err(_), Some(_), _) | (Ok(()), None, _) => {
            unreachable!("callback presence is determined by initial validation")
        }
    }
}

#[cfg(test)]
mod tests;
