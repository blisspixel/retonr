use std::fmt;

use rewrite_runtime_isolation::IsolationError;
use thiserror::Error;

use crate::PackageAttestationError;

use super::ManagedOllamaModelAuthorityError;

/// Failure while constructing or reobserving the closed managed Ollama launch.
#[derive(Error)]
pub enum ManagedOllamaLaunchError {
    /// Runtime, package lease, model lease, input plan, or admission did not bind.
    #[error("managed Ollama launch relationship is invalid")]
    RelationshipMismatch,
    /// The admitted startup capability authorizes another plain launch profile.
    #[error("managed Ollama launch is not the launch authorized by runtime admission")]
    UnauthorizedLaunch,
    /// Retained runtime or model-package bytes failed revalidation.
    #[error("managed Ollama retained package verification failed")]
    Package(#[source] PackageAttestationError),
    /// The exact model foundation or consumed license authority failed revalidation.
    #[error("managed Ollama model authority verification failed")]
    ModelAuthority(#[source] ManagedOllamaModelAuthorityError),
    /// Isolation rejected the launch, private input tree, or later observation.
    #[error("managed Ollama isolation launch failed")]
    Isolation(#[source] IsolationError),
    /// Post-launch input evidence did not bind to the retained plan and launch.
    #[error("managed Ollama private input evidence does not match its launch")]
    IsolationInputMismatch,
    /// A live post-acquisition check failed and every mandatory finalizer ran.
    #[error("managed Ollama post-acquisition validation failed")]
    PostAcquisitionFailure {
        /// Exact live check that failed before authority release.
        primary: ManagedOllamaPostAcquisitionFailure,
        /// Independently retained cleanup and package-authority failures.
        finalization: ManagedOllamaLaunchFinalizationFailures,
    },
    /// A reentrant caller attempted to use the retained isolation capability.
    #[error("managed Ollama isolation capability is already in use")]
    CapabilityBusy,
}

/// Exact live check that failed after managed process acquisition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManagedOllamaPostAcquisitionFailure {
    /// Target-visible private inputs did not bind to the retained launch plan.
    InputBindingMismatch,
    /// The caller operation was cancelled before the live capability could be released.
    Cancelled,
    /// The original absolute operation deadline expired after process acquisition.
    DeadlineExceeded,
}

/// Exhaustive finalization failures after a live post-acquisition failure.
pub struct ManagedOllamaLaunchFinalizationFailures {
    cleanup: Option<Box<IsolationError>>,
    model: Option<Box<ManagedOllamaModelAuthorityError>>,
    runtime: Option<Box<PackageAttestationError>>,
}

impl ManagedOllamaLaunchFinalizationFailures {
    pub(super) const fn new(
        cleanup: Option<Box<IsolationError>>,
        model: Option<Box<ManagedOllamaModelAuthorityError>>,
        runtime: Option<Box<PackageAttestationError>>,
    ) -> Self {
        Self {
            cleanup,
            model,
            runtime,
        }
    }

    /// Returns the independently observed isolation-cleanup failure.
    #[must_use]
    pub fn cleanup(&self) -> Option<&IsolationError> {
        self.cleanup.as_deref()
    }

    /// Returns the independently observed exact model and license failure.
    #[must_use]
    pub fn model(&self) -> Option<&ManagedOllamaModelAuthorityError> {
        self.model.as_deref()
    }

    /// Returns the independently observed runtime-package failure.
    #[must_use]
    pub fn runtime(&self) -> Option<&PackageAttestationError> {
        self.runtime.as_deref()
    }

    /// Returns whether any mandatory finalizer failed.
    #[must_use]
    pub const fn has_failures(&self) -> bool {
        self.cleanup.is_some() || self.model.is_some() || self.runtime.is_some()
    }
}

impl fmt::Debug for ManagedOllamaLaunchFinalizationFailures {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedOllamaLaunchFinalizationFailures")
            .field("cleanup_failed", &self.cleanup.is_some())
            .field("model_failed", &self.model.is_some())
            .field("runtime_failed", &self.runtime.is_some())
            .finish()
    }
}

impl fmt::Debug for ManagedOllamaLaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("ManagedOllamaLaunchError");
        match self {
            Self::RelationshipMismatch => value.field("kind", &"relationship_mismatch"),
            Self::UnauthorizedLaunch => value.field("kind", &"unauthorized_launch"),
            Self::Package(_) => value.field("kind", &"package"),
            Self::ModelAuthority(_) => value.field("kind", &"model_authority"),
            Self::Isolation(_) => value.field("kind", &"isolation"),
            Self::IsolationInputMismatch => value.field("kind", &"isolation_input_mismatch"),
            Self::PostAcquisitionFailure {
                primary,
                finalization,
            } => value
                .field("kind", &"post_acquisition_failure")
                .field("primary", primary)
                .field("finalization", finalization),
            Self::CapabilityBusy => value.field("kind", &"capability_busy"),
        };
        value.finish_non_exhaustive()
    }
}

/// Failure while closing the complete managed Ollama bracket.
#[derive(Debug, Error)]
pub enum ManagedOllamaCloseError {
    /// The managed process tree could not be terminated and reaped.
    #[error("managed Ollama isolation cleanup failed")]
    Isolation(#[source] IsolationError),
    /// Final model-foundation or license-authority revalidation failed after cleanup.
    #[error("managed Ollama final model authority verification failed")]
    ModelAuthority(#[source] ManagedOllamaModelAuthorityError),
    /// Cleanup and final model-authority revalidation both failed.
    #[error("managed Ollama isolation cleanup and final model authority verification failed")]
    IsolationAndModelAuthority {
        /// Process-tree cleanup failure.
        isolation: IsolationError,
        /// Independent model-foundation and license-authority failure.
        authority: ManagedOllamaModelAuthorityError,
    },
}
