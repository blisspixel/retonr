use std::{cell::Cell, fmt};

use rewrite_app::{
    GenerationEffectivePackageDerivationError, GenerationEffectivePackagePlanError,
    GenerationEffectivePackageReleaseError, GenerationQualificationResourceAttemptObservationError,
    ManagedOllamaCloseError, PackageAttestationError,
};
use rewrite_model::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureCategoryV1,
    CandidateGenerationAttemptFailurePhaseV1, CandidateGenerationAttemptFailureV1Input,
    GenerationQualificationContractError,
};
use rewrite_ollama::OllamaResponseObservationPhase;
use thiserror::Error;

use super::super::LocalOllamaManagedGenerationError;

mod classification;

use classification::{
    cleanup_disposition, generation_failure_category, generation_failure_phase,
    generation_has_embedded_cleanup, release_cleanup_failed,
};

/// Exact primary cause of a failed managed candidate attempt.
///
/// Every contained error is already bounded and redacted by its originating trust
/// boundary. Generated content and request bytes are never retained here.
#[derive(Error)]
pub enum ManagedCandidateAttemptPrimaryFailure {
    /// Managed launch, traffic, response, or observation failed.
    #[error("managed candidate generation operation failed")]
    Generation(#[source] LocalOllamaManagedGenerationError),
    /// Effective-package derivation failed, possibly with secondary release failures.
    #[error("managed candidate effective-package preparation failed")]
    EffectivePackagePlan(#[source] GenerationEffectivePackagePlanError),
    /// Cleanup-gated effective-package release failed.
    #[error("managed candidate effective-package release failed")]
    EffectivePackageRelease(#[source] GenerationEffectivePackageReleaseError),
    /// Released package authority did not match the characterized package.
    #[error("managed candidate characterized package relationship changed")]
    CharacterizedPackageMismatch,
    /// Resource measurement policy, target, or retained observations did not bind.
    #[error("managed candidate resource observation failed")]
    ResourceObservation(#[source] GenerationQualificationResourceAttemptObservationError),
    /// The operation policy did not designate the exact attempted target, plan, and suite.
    #[error("managed candidate resource target relationship changed")]
    ResourceOperationScopeMismatch,
    /// Final managed evidence construction rejected the exact relationship join.
    #[error("managed candidate evidence construction failed")]
    Record(#[source] GenerationQualificationContractError),
}

impl fmt::Debug for ManagedCandidateAttemptPrimaryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ManagedCandidateAttemptPrimaryFailure")
            .field(&match self {
                Self::Generation(_) => "generation",
                Self::EffectivePackagePlan(_) => "effective_package_plan",
                Self::EffectivePackageRelease(_) => "effective_package_release",
                Self::CharacterizedPackageMismatch => "characterized_package_mismatch",
                Self::ResourceObservation(_) => "resource_observation",
                Self::ResourceOperationScopeMismatch => "resource_operation_scope_mismatch",
                Self::Record(_) => "record",
            })
            .finish()
    }
}

/// Borrowed, lossless view of cleanup and final package-revalidation failures.
pub enum ManagedCandidateAttemptCleanupFailures<'a> {
    /// The failure path closed the retained bracket directly.
    RetainedBracket {
        /// Process-tree cleanup and final model-authority failure, if present.
        cleanup: Option<&'a ManagedOllamaCloseError>,
        /// Independent final runtime-package revalidation failure, if present.
        runtime_package: Option<&'a PackageAttestationError>,
    },
    /// A legacy aggregate retained its cleanup causes alongside the operation.
    EmbeddedGeneration(&'a LocalOllamaManagedGenerationError),
    /// Both a legacy aggregate and the outer retained bracket recorded failures.
    RetainedAndEmbeddedGeneration {
        /// Legacy operation aggregate containing its embedded cleanup causes.
        embedded_generation: &'a LocalOllamaManagedGenerationError,
        /// Outer process-tree cleanup and final model-authority failure, if present.
        cleanup: Option<&'a ManagedOllamaCloseError>,
        /// Independent outer runtime-package revalidation failure, if present.
        runtime_package: Option<&'a PackageAttestationError>,
    },
    /// Effective-package release retained all cleanup and revalidation failures.
    EffectivePackageRelease(&'a GenerationEffectivePackageReleaseError),
}

impl fmt::Debug for ManagedCandidateAttemptCleanupFailures<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ManagedCandidateAttemptCleanupFailures")
            .field(&match self {
                Self::RetainedBracket { .. } => "retained_bracket",
                Self::EmbeddedGeneration(_) => "embedded_generation",
                Self::RetainedAndEmbeddedGeneration { .. } => "retained_and_embedded_generation",
                Self::EffectivePackageRelease(_) => "effective_package_release",
            })
            .finish()
    }
}

#[derive(Debug)]
pub(super) struct RetainedBracketCleanupFailures {
    cleanup: Option<Box<ManagedOllamaCloseError>>,
    runtime_package: Option<Box<PackageAttestationError>>,
}

impl RetainedBracketCleanupFailures {
    pub(super) fn new(
        cleanup: Option<ManagedOllamaCloseError>,
        runtime_package: Option<PackageAttestationError>,
    ) -> Option<Self> {
        if cleanup.is_none() && runtime_package.is_none() {
            None
        } else {
            Some(Self {
                cleanup: cleanup.map(Box::new),
                runtime_package: runtime_package.map(Box::new),
            })
        }
    }

    pub(super) fn view(&self) -> ManagedCandidateAttemptCleanupFailures<'_> {
        ManagedCandidateAttemptCleanupFailures::RetainedBracket {
            cleanup: self.cleanup.as_deref(),
            runtime_package: self.runtime_package.as_deref(),
        }
    }
}

/// Failure to construct the mandatory portable failed-attempt record.
#[derive(Error)]
#[error("managed candidate failed-attempt record construction failed")]
pub struct ManagedCandidateAttemptFailureRecordError {
    #[source]
    source: GenerationQualificationContractError,
    primary: ManagedCandidateAttemptPrimaryFailure,
    retained_cleanup: Option<RetainedBracketCleanupFailures>,
}

impl fmt::Debug for ManagedCandidateAttemptFailureRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedCandidateAttemptFailureRecordError")
            .field("contract_error", &self.source)
            .field("primary", &self.primary)
            .field(
                "cleanup_failures_retained",
                &self.cleanup_failures().is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl ManagedCandidateAttemptFailureRecordError {
    pub(super) fn new(
        source: GenerationQualificationContractError,
        primary: ManagedCandidateAttemptPrimaryFailure,
        retained_cleanup: Option<RetainedBracketCleanupFailures>,
    ) -> Self {
        Self {
            source,
            primary,
            retained_cleanup,
        }
    }

    /// Returns the model-contract rejection that prevented an exact failed record.
    #[must_use]
    pub const fn contract_error(&self) -> &GenerationQualificationContractError {
        &self.source
    }

    /// Returns the original primary attempt failure.
    #[must_use]
    pub const fn primary_failure(&self) -> &ManagedCandidateAttemptPrimaryFailure {
        &self.primary
    }

    /// Returns every independently retained cleanup or revalidation failure.
    #[must_use]
    pub fn cleanup_failures(&self) -> Option<ManagedCandidateAttemptCleanupFailures<'_>> {
        cleanup_failures(&self.primary, self.retained_cleanup.as_ref())
    }
}

/// Failure that prevented either completed authority or an exact failed record.
#[derive(Debug, Error)]
pub enum ManagedCandidateAttemptExecutionError {
    /// Construction of the mandatory portable failed-attempt record was rejected.
    #[error(transparent)]
    FailureRecord(Box<ManagedCandidateAttemptFailureRecordError>),
}

impl From<ManagedCandidateAttemptFailureRecordError> for ManagedCandidateAttemptExecutionError {
    fn from(error: ManagedCandidateAttemptFailureRecordError) -> Self {
        Self::FailureRecord(Box::new(error))
    }
}

impl From<Box<ManagedCandidateAttemptFailureRecordError>>
    for ManagedCandidateAttemptExecutionError
{
    fn from(error: Box<ManagedCandidateAttemptFailureRecordError>) -> Self {
        Self::FailureRecord(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ManagedCandidateAttemptFailureFacts {
    pub(super) failure_phase: CandidateGenerationAttemptFailurePhaseV1,
    pub(super) failure_category: CandidateGenerationAttemptFailureCategoryV1,
    pub(super) traffic_observed: bool,
    pub(super) output_observed: bool,
    pub(super) cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1,
}

impl ManagedCandidateAttemptFailureFacts {
    pub(super) const fn input(self) -> CandidateGenerationAttemptFailureV1Input {
        CandidateGenerationAttemptFailureV1Input {
            failure_phase: self.failure_phase,
            failure_category: self.failure_category,
            traffic_observed: self.traffic_observed,
            output_observed: self.output_observed,
            cleanup_disposition: self.cleanup_disposition,
        }
    }

    pub(super) const fn with_cleanup_failed(mut self, failed: bool) -> Self {
        if failed {
            self.cleanup_disposition = CandidateGenerationAttemptCleanupDispositionV1::Failed;
        }
        self
    }
}

pub(crate) struct ManagedCandidateAttemptProgress {
    phase: Cell<CandidateGenerationAttemptFailurePhaseV1>,
    traffic_observed: Cell<bool>,
    output_observed: Cell<bool>,
}

impl ManagedCandidateAttemptProgress {
    pub(crate) const fn new() -> Self {
        Self {
            phase: Cell::new(CandidateGenerationAttemptFailurePhaseV1::PreTrafficRevalidation),
            traffic_observed: Cell::new(false),
            output_observed: Cell::new(false),
        }
    }

    pub(crate) fn set_phase(&self, phase: CandidateGenerationAttemptFailurePhaseV1) {
        self.phase.set(phase);
    }

    pub(crate) fn observe_response(
        &self,
        phase: OllamaResponseObservationPhase,
        preflight_responses: usize,
        generation_response_ordinal: usize,
    ) {
        match phase {
            OllamaResponseObservationPhase::AfterResponse { ordinal }
                if ordinal > preflight_responses =>
            {
                self.traffic_observed.set(true);
                if ordinal == generation_response_ordinal {
                    self.output_observed.set(true);
                    self.phase
                        .set(CandidateGenerationAttemptFailurePhaseV1::ResponseValidation);
                } else if ordinal > generation_response_ordinal {
                    self.phase
                        .set(CandidateGenerationAttemptFailurePhaseV1::FinalObservation);
                }
            }
            OllamaResponseObservationPhase::AfterFailedAttempt {
                completed_responses,
            } if completed_responses >= preflight_responses => {
                self.traffic_observed.set(true);
            }
            OllamaResponseObservationPhase::BeforeResponses
            | OllamaResponseObservationPhase::AfterResponse { .. }
            | OllamaResponseObservationPhase::AfterFailedAttempt { .. } => {}
        }
    }

    pub(super) fn generation_failure_facts(
        &self,
        error: &LocalOllamaManagedGenerationError,
    ) -> ManagedCandidateAttemptFailureFacts {
        let phase = generation_failure_phase(self.phase.get(), error);
        ManagedCandidateAttemptFailureFacts {
            failure_phase: phase,
            failure_category: generation_failure_category(error),
            traffic_observed: self.traffic_observed.get(),
            output_observed: self.output_observed.get(),
            cleanup_disposition: if generation_has_embedded_cleanup(error) {
                CandidateGenerationAttemptCleanupDispositionV1::Failed
            } else {
                CandidateGenerationAttemptCleanupDispositionV1::Succeeded
            },
        }
    }
}

pub(super) fn effective_package_plan_failure_facts(
    error: &GenerationEffectivePackagePlanError,
    progress: &ManagedCandidateAttemptProgress,
) -> ManagedCandidateAttemptFailureFacts {
    let category = match error.derivation() {
        GenerationEffectivePackageDerivationError::Cancelled => {
            CandidateGenerationAttemptFailureCategoryV1::Cancelled
        }
        GenerationEffectivePackageDerivationError::RelationshipMismatch => {
            CandidateGenerationAttemptFailureCategoryV1::RelationshipMismatch
        }
        GenerationEffectivePackageDerivationError::Evidence(_) => {
            CandidateGenerationAttemptFailureCategoryV1::ManagedEvidenceInvalid
        }
    };
    ManagedCandidateAttemptFailureFacts {
        failure_phase: CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation,
        failure_category: category,
        traffic_observed: progress.traffic_observed.get(),
        output_observed: progress.output_observed.get(),
        cleanup_disposition: cleanup_disposition(error.release()),
    }
}

pub(super) fn effective_package_release_failure_facts(
    error: &GenerationEffectivePackageReleaseError,
    progress: &ManagedCandidateAttemptProgress,
) -> ManagedCandidateAttemptFailureFacts {
    let cleanup_failed = release_cleanup_failed(error);
    ManagedCandidateAttemptFailureFacts {
        failure_phase: if cleanup_failed {
            CandidateGenerationAttemptFailurePhaseV1::Cleanup
        } else {
            CandidateGenerationAttemptFailurePhaseV1::ManagedEvidenceCompilation
        },
        failure_category: if cleanup_failed {
            CandidateGenerationAttemptFailureCategoryV1::CleanupFailed
        } else {
            CandidateGenerationAttemptFailureCategoryV1::ManagedEvidenceInvalid
        },
        traffic_observed: progress.traffic_observed.get(),
        output_observed: progress.output_observed.get(),
        cleanup_disposition: if cleanup_failed {
            CandidateGenerationAttemptCleanupDispositionV1::Failed
        } else {
            CandidateGenerationAttemptCleanupDispositionV1::Succeeded
        },
    }
}

pub(super) fn fixed_failure_facts(
    phase: CandidateGenerationAttemptFailurePhaseV1,
    category: CandidateGenerationAttemptFailureCategoryV1,
    progress: &ManagedCandidateAttemptProgress,
) -> ManagedCandidateAttemptFailureFacts {
    ManagedCandidateAttemptFailureFacts {
        failure_phase: phase,
        failure_category: category,
        traffic_observed: progress.traffic_observed.get(),
        output_observed: progress.output_observed.get(),
        cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
    }
}

pub(super) fn cleanup_failures<'a>(
    primary: &'a ManagedCandidateAttemptPrimaryFailure,
    retained: Option<&'a RetainedBracketCleanupFailures>,
) -> Option<ManagedCandidateAttemptCleanupFailures<'a>> {
    let embedded = match primary {
        ManagedCandidateAttemptPrimaryFailure::Generation(error)
            if generation_has_embedded_cleanup(error) =>
        {
            Some(error)
        }
        ManagedCandidateAttemptPrimaryFailure::Generation(_)
        | ManagedCandidateAttemptPrimaryFailure::EffectivePackagePlan(_)
        | ManagedCandidateAttemptPrimaryFailure::EffectivePackageRelease(_)
        | ManagedCandidateAttemptPrimaryFailure::CharacterizedPackageMismatch
        | ManagedCandidateAttemptPrimaryFailure::ResourceObservation(_)
        | ManagedCandidateAttemptPrimaryFailure::ResourceOperationScopeMismatch
        | ManagedCandidateAttemptPrimaryFailure::Record(_) => None,
    };
    match (retained, embedded) {
        (Some(retained), Some(embedded_generation)) => {
            return Some(
                ManagedCandidateAttemptCleanupFailures::RetainedAndEmbeddedGeneration {
                    embedded_generation,
                    cleanup: retained.cleanup.as_deref(),
                    runtime_package: retained.runtime_package.as_deref(),
                },
            );
        }
        (Some(retained), None) => return Some(retained.view()),
        (None, Some(error)) => {
            return Some(ManagedCandidateAttemptCleanupFailures::EmbeddedGeneration(
                error,
            ));
        }
        (None, None) => {}
    }
    let release = match primary {
        ManagedCandidateAttemptPrimaryFailure::EffectivePackagePlan(error) => error.release(),
        ManagedCandidateAttemptPrimaryFailure::EffectivePackageRelease(error) => Some(error),
        ManagedCandidateAttemptPrimaryFailure::Generation(_)
        | ManagedCandidateAttemptPrimaryFailure::CharacterizedPackageMismatch
        | ManagedCandidateAttemptPrimaryFailure::ResourceObservation(_)
        | ManagedCandidateAttemptPrimaryFailure::ResourceOperationScopeMismatch
        | ManagedCandidateAttemptPrimaryFailure::Record(_) => None,
    }?;
    release_cleanup_failed(release)
        .then_some(ManagedCandidateAttemptCleanupFailures::EffectivePackageRelease(release))
}

impl fmt::Display for ManagedCandidateAttemptCleanupFailures<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("managed candidate cleanup or revalidation failed")
    }
}

#[cfg(test)]
mod tests;
