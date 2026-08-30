use std::{error::Error as StdError, fmt};

use rewrite_model::EffectivePackageEvidenceV2Error;
use thiserror::Error;

use crate::{ManagedOllamaCloseError, ManagedOllamaModelPackageError, PackageAttestationError};

/// Failure to derive a pending effective-package plan.
#[derive(Debug, Error)]
pub enum GenerationEffectivePackageDerivationError {
    /// Work was cancelled before the pending record was complete.
    #[error("effective-package derivation was cancelled")]
    Cancelled,
    /// A typed capability, manifest, installation, policy, or state was substituted.
    #[error("effective-package production relationship is invalid")]
    RelationshipMismatch,
    /// Typed effective-package v2 construction rejected the derived member set.
    #[error("effective-package v2 construction failed")]
    Evidence(#[source] EffectivePackageEvidenceV2Error),
}

/// Derivation failure plus any independently observed close or revalidation failure.
#[derive(Debug)]
pub struct GenerationEffectivePackagePlanError {
    pub(super) derivation: GenerationEffectivePackageDerivationError,
    pub(super) release: Option<GenerationEffectivePackageReleaseError>,
}

impl GenerationEffectivePackagePlanError {
    /// Returns the primary derivation failure.
    #[must_use]
    pub const fn derivation(&self) -> &GenerationEffectivePackageDerivationError {
        &self.derivation
    }

    /// Returns secondary close and revalidation failures, when any occurred.
    #[must_use]
    pub const fn release(&self) -> Option<&GenerationEffectivePackageReleaseError> {
        self.release.as_ref()
    }
}

impl fmt::Display for GenerationEffectivePackagePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.release.is_some() {
            write!(
                formatter,
                "effective-package derivation failed and release checks also failed"
            )
        } else {
            write!(formatter, "effective-package derivation failed")
        }
    }
}

impl StdError for GenerationEffectivePackagePlanError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.derivation)
    }
}

/// Failure to derive a trustworthy managed-cleanup duration.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationEffectivePackageCleanupTimingError {
    /// The cleanup endpoint preceded its start on the monotonic clock.
    #[error("managed cleanup monotonic checkpoints were reversed")]
    ReversedMonotonicCheckpoints,
}

/// Complete failure set from cleanup-gated evidence release.
#[derive(Debug)]
pub struct GenerationEffectivePackageReleaseError {
    pub(super) cleanup: Option<Box<ManagedOllamaCloseError>>,
    pub(super) timing: Option<GenerationEffectivePackageCleanupTimingError>,
    pub(super) model: Option<Box<ManagedOllamaModelPackageError>>,
    pub(super) runtime: Option<Box<PackageAttestationError>>,
    pub(super) evidence: Option<Box<EffectivePackageEvidenceV2Error>>,
}

impl GenerationEffectivePackageReleaseError {
    /// Returns the managed isolation or its built-in package-close failure.
    #[must_use]
    pub fn cleanup(&self) -> Option<&ManagedOllamaCloseError> {
        self.cleanup.as_deref()
    }

    /// Returns the managed-cleanup clock failure, when observed.
    #[must_use]
    pub const fn timing(&self) -> Option<GenerationEffectivePackageCleanupTimingError> {
        self.timing
    }

    /// Returns the independent specialized model-foundation revalidation failure.
    #[must_use]
    pub fn model(&self) -> Option<&ManagedOllamaModelPackageError> {
        self.model.as_deref()
    }

    /// Returns the independent runtime-package revalidation failure.
    #[must_use]
    pub fn runtime(&self) -> Option<&PackageAttestationError> {
        self.runtime.as_deref()
    }

    /// Returns final owned-record validation failure, when one occurred.
    #[must_use]
    pub fn evidence(&self) -> Option<&EffectivePackageEvidenceV2Error> {
        self.evidence.as_deref()
    }
}

impl fmt::Display for GenerationEffectivePackageReleaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "effective-package cleanup-gated release failed")
    }
}

impl StdError for GenerationEffectivePackageReleaseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.cleanup
            .as_deref()
            .map(|error| error as &(dyn StdError + 'static))
            .or_else(|| {
                self.timing
                    .as_ref()
                    .map(|error| error as &(dyn StdError + 'static))
            })
            .or_else(|| {
                self.model
                    .as_deref()
                    .map(|error| error as &(dyn StdError + 'static))
            })
            .or_else(|| {
                self.runtime
                    .as_deref()
                    .map(|error| error as &(dyn StdError + 'static))
            })
            .or_else(|| {
                self.evidence
                    .as_deref()
                    .map(|error| error as &(dyn StdError + 'static))
            })
    }
}
