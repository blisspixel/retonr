use rewrite_app::effective_runtime_state_observation::{
    EffectiveRuntimeStateObservationError, ManagedOllamaEffectiveRuntimeState,
};
use rewrite_app::{ManagedOllamaCloseError, ManagedOllamaLaunchError, PackageAttestationError};
use rewrite_inference::CandidateOutputError;
use rewrite_inference::{InferenceError, StructuredCompletionResponse};
use rewrite_model::EffectiveRuntimeState;
use rewrite_ollama::{
    OllamaResidentResourceObservedCompletion, OllamaResidentSessionExecutionReceipt,
};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerError, ManagedGenerationWorkerEvidence,
    ManagedGenerationWorkerResourceObservation,
};
use thiserror::Error;

use crate::{
    LocalOllamaManagedBuildBinding, LocalOllamaManagedPreflightError,
    LocalOllamaManagedPreflightReport,
};

use super::{LocalOllamaManagedGenerationEvidence, ManagedOllamaGenerationBracketObservationV1};

/// Failure from one retained managed-generation operation.
#[derive(Debug, Error)]
pub enum LocalOllamaManagedGenerationError {
    /// Managed package, launch, preflight, observation, or report validation failed.
    #[error("managed Ollama generation prerequisite failed: {0}")]
    Managed(#[from] LocalOllamaManagedPreflightError),
    /// The closed app-owned launch or private input join failed.
    #[error("closed managed Ollama launch failed: {0}")]
    Launch(#[from] ManagedOllamaLaunchError),
    /// The separately retained generation-worker observation failed.
    #[error("managed Ollama generation worker observation failed: {0}")]
    Worker(#[from] ManagedGenerationWorkerError),
    /// The admitted runtime, reviewed worker path, or frozen native set does not bind.
    #[error("managed Ollama generation authority binding is invalid")]
    InvalidGenerationAuthority,
    /// The retained Ollama session failed closed before completing its exact sequence.
    #[error("managed Ollama retained generation session failed: {0}")]
    Session(#[source] InferenceError),
    /// Process cleanup or final model-package revalidation failed.
    #[error("managed Ollama generation close failed: {0}")]
    Cleanup(#[source] ManagedOllamaCloseError),
    /// The primary operation and independent cleanup both failed.
    #[error("managed Ollama generation cleanup failed with {cleanup} after {operation}")]
    CleanupAfterFailure {
        /// Original operation failure retained without weakening cleanup reporting.
        #[source]
        operation: Box<LocalOllamaManagedGenerationError>,
        /// Independent cleanup or final model-package revalidation failure.
        cleanup: ManagedOllamaCloseError,
    },
    /// Final runtime-package revalidation failed after operation and cleanup passed.
    #[error("runtime package revalidation failed after managed Ollama cleanup: {0}")]
    RuntimePackageAfterCleanup(#[source] Box<PackageAttestationError>),
    /// Operation and final runtime-package revalidation both failed.
    #[error("runtime package revalidation failed with {runtime_package} after {operation}")]
    RuntimePackageAfterOperationFailure {
        /// Original operation failure.
        operation: Box<LocalOllamaManagedGenerationError>,
        /// Independent final runtime-package revalidation failure.
        runtime_package: Box<PackageAttestationError>,
    },
    /// Cleanup and final runtime-package revalidation both failed.
    #[error(
        "runtime package revalidation failed with {runtime_package} after cleanup failed with {cleanup}"
    )]
    RuntimePackageAfterCleanupFailure {
        /// Independent cleanup failure.
        cleanup: Box<ManagedOllamaCloseError>,
        /// Independent final runtime-package revalidation failure.
        runtime_package: Box<PackageAttestationError>,
    },
    /// Operation, cleanup, and final runtime-package revalidation all failed.
    #[error(
        "runtime package revalidation failed with {runtime_package} after cleanup failed with {cleanup} and operation failed with {operation}"
    )]
    RuntimePackageAfterOperationAndCleanupFailure {
        /// Original operation failure.
        operation: Box<LocalOllamaManagedGenerationError>,
        /// Independent cleanup failure.
        cleanup: Box<ManagedOllamaCloseError>,
        /// Independent final runtime-package revalidation failure.
        runtime_package: Box<PackageAttestationError>,
    },
    /// Live leaf observation or exact effective-state cross-validation failed.
    #[error("managed Ollama effective runtime-state observation failed: {0}")]
    EffectiveState(#[from] EffectiveRuntimeStateObservationError),
    /// Live records do not match the exact app-verified attempt precursor.
    #[error("managed Ollama candidate-attempt relationship is invalid")]
    InvalidCandidateAttemptRelationship,
    /// The bounded structured response is not the exact declared candidate envelope.
    #[error("managed Ollama candidate response validation failed: {0}")]
    ResponseValidation(#[source] CandidateOutputError),
}

/// One content response plus redacted evidence from its closed managed bracket.
#[derive(Debug)]
pub struct LocalOllamaManagedGenerationOutcome {
    pub(super) response: StructuredCompletionResponse,
    pub(super) residency_receipt: OllamaResidentSessionExecutionReceipt,
    pub(super) managed_preflight: LocalOllamaManagedPreflightReport,
    pub(super) managed_build: LocalOllamaManagedBuildBinding,
    pub(super) evidence: LocalOllamaManagedGenerationEvidence,
    pub(super) bracket_observation: ManagedOllamaGenerationBracketObservationV1,
    pub(super) effective_runtime_state: EffectiveRuntimeState,
}

impl LocalOllamaManagedGenerationOutcome {
    /// Returns the bounded untrusted structured response.
    #[must_use]
    pub const fn response(&self) -> &StructuredCompletionResponse {
        &self.response
    }

    /// Returns the content-free runtime-reported residency receipt.
    #[must_use]
    pub const fn residency_receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        &self.residency_receipt
    }

    /// Returns the completed inert managed preflight report.
    #[must_use]
    pub const fn managed_preflight(&self) -> &LocalOllamaManagedPreflightReport {
        &self.managed_preflight
    }

    /// Returns the package-declared runtime-build binding.
    #[must_use]
    pub const fn managed_build(&self) -> &LocalOllamaManagedBuildBinding {
        &self.managed_build
    }

    /// Returns the original schema-1 retained-generation evidence.
    #[must_use]
    pub const fn evidence(&self) -> &LocalOllamaManagedGenerationEvidence {
        &self.evidence
    }

    /// Returns the richer effective-state-bound completed-bracket observation.
    #[must_use]
    pub const fn bracket_observation(&self) -> &ManagedOllamaGenerationBracketObservationV1 {
        &self.bracket_observation
    }

    /// Returns the inert app-observed effective runtime state.
    #[must_use]
    pub const fn effective_runtime_state(&self) -> &EffectiveRuntimeState {
        &self.effective_runtime_state
    }

    /// Splits the result into its response, legacy evidence, bracket observation,
    /// and inert effective state.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        StructuredCompletionResponse,
        OllamaResidentSessionExecutionReceipt,
        LocalOllamaManagedPreflightReport,
        LocalOllamaManagedBuildBinding,
        LocalOllamaManagedGenerationEvidence,
        ManagedOllamaGenerationBracketObservationV1,
        EffectiveRuntimeState,
    ) {
        (
            self.response,
            self.residency_receipt,
            self.managed_preflight,
            self.managed_build,
            self.evidence,
            self.bracket_observation,
            self.effective_runtime_state,
        )
    }
}

pub(super) struct PendingLocalOllamaManagedGenerationOutcome {
    pub(super) completion: PendingManagedGenerationCompletion,
    pub(super) managed_preflight: LocalOllamaManagedPreflightReport,
    pub(super) managed_build: LocalOllamaManagedBuildBinding,
    pub(super) evidence: LocalOllamaManagedGenerationEvidence,
    pub(super) bracket_observation: ManagedOllamaGenerationBracketObservationV1,
    pub(super) effective_runtime_state: ManagedOllamaEffectiveRuntimeState,
}

pub(super) enum PendingManagedGenerationCompletion {
    Compatibility {
        response: Box<StructuredCompletionResponse>,
        residency_receipt: Box<OllamaResidentSessionExecutionReceipt>,
    },
    ResourceObserved(Box<PendingResourceObservedGenerationCompletion>),
}

pub(super) struct PendingResourceObservedGenerationCompletion {
    pub(super) completion: Box<OllamaResidentResourceObservedCompletion>,
    pub(super) worker_observation: ManagedGenerationWorkerResourceObservation,
    pub(super) worker_evidence: ManagedGenerationWorkerEvidence,
}

impl PendingManagedGenerationCompletion {
    pub(super) const fn response(&self) -> &StructuredCompletionResponse {
        match self {
            Self::Compatibility { response, .. } => response,
            Self::ResourceObserved(resource) => resource.completion.response(),
        }
    }

    pub(super) const fn residency_receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        match self {
            Self::Compatibility {
                residency_receipt, ..
            } => residency_receipt,
            Self::ResourceObserved(resource) => resource.completion.resident_execution_receipt(),
        }
    }
}

impl PendingLocalOllamaManagedGenerationOutcome {
    pub(super) const fn response(&self) -> &StructuredCompletionResponse {
        self.completion.response()
    }

    pub(super) const fn residency_receipt(&self) -> &OllamaResidentSessionExecutionReceipt {
        self.completion.residency_receipt()
    }
}

impl PendingLocalOllamaManagedGenerationOutcome {
    pub(super) fn after_cleanup(self) -> LocalOllamaManagedGenerationOutcome {
        let PendingManagedGenerationCompletion::Compatibility {
            response,
            residency_receipt,
        } = self.completion
        else {
            unreachable!("compatibility runner cannot request resource observation")
        };
        LocalOllamaManagedGenerationOutcome {
            response: *response,
            residency_receipt: *residency_receipt,
            managed_preflight: self.managed_preflight,
            managed_build: self.managed_build,
            evidence: self.evidence,
            bracket_observation: self.bracket_observation,
            effective_runtime_state: self.effective_runtime_state.into_state(),
        }
    }
}

pub(super) fn finalize_retained_bracket<T>(
    operation: Result<T, LocalOllamaManagedGenerationError>,
    cleanup: Result<(), ManagedOllamaCloseError>,
    runtime_revalidation: Result<(), PackageAttestationError>,
) -> Result<T, LocalOllamaManagedGenerationError> {
    match (operation, cleanup, runtime_revalidation) {
        (Err(operation), Err(cleanup), Err(runtime_package)) => Err(
            LocalOllamaManagedGenerationError::RuntimePackageAfterOperationAndCleanupFailure {
                operation: Box::new(operation),
                cleanup: Box::new(cleanup),
                runtime_package: Box::new(runtime_package),
            },
        ),
        (Err(operation), Ok(()), Err(runtime_package)) => Err(
            LocalOllamaManagedGenerationError::RuntimePackageAfterOperationFailure {
                operation: Box::new(operation),
                runtime_package: Box::new(runtime_package),
            },
        ),
        (Ok(_outcome), Err(cleanup), Err(runtime_package)) => Err(
            LocalOllamaManagedGenerationError::RuntimePackageAfterCleanupFailure {
                cleanup: Box::new(cleanup),
                runtime_package: Box::new(runtime_package),
            },
        ),
        (Ok(_outcome), Ok(()), Err(runtime_package)) => Err(
            LocalOllamaManagedGenerationError::RuntimePackageAfterCleanup(Box::new(
                runtime_package,
            )),
        ),
        (Err(operation), Err(cleanup), Ok(())) => {
            Err(LocalOllamaManagedGenerationError::CleanupAfterFailure {
                operation: Box::new(operation),
                cleanup,
            })
        }
        (Ok(_outcome), Err(cleanup), Ok(())) => {
            Err(LocalOllamaManagedGenerationError::Cleanup(cleanup))
        }
        (Ok(outcome), Ok(()), Ok(())) => Ok(outcome),
        (Err(operation), Ok(()), Ok(())) => Err(operation),
    }
}
