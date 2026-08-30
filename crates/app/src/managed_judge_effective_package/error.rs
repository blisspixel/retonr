use std::{error::Error as StdError, fmt};

use rewrite_model::EffectivePackageEvidenceV2Error;
use thiserror::Error;

use crate::{
    ManagedOllamaCloseError, ManagedOllamaLaunchError, ManagedOllamaModelPackageError,
    PackageAttestationError,
};

use super::ManagedJudgeEffectivePackageRelationship;

/// Primary failure before a pending release capability was complete.
#[derive(Error)]
pub enum ManagedJudgeEffectivePackageDerivationError {
    /// Work was cancelled during bounded preparation.
    #[error("managed judge effective-package derivation was cancelled")]
    Cancelled,
    /// Initial specialized model or license revalidation failed.
    #[error("managed judge model authority revalidation failed")]
    ModelRevalidation(#[source] Box<ManagedOllamaLaunchError>),
    /// Initial runtime package revalidation failed.
    #[error("managed judge runtime package revalidation failed")]
    RuntimeRevalidation(#[source] Box<PackageAttestationError>),
    /// An exact retained relationship was substituted.
    #[error("managed judge effective-package relationship is invalid: {0:?}")]
    Relationship(ManagedJudgeEffectivePackageRelationship),
    /// Typed V2 evidence construction rejected internally derived fields.
    #[error("managed judge effective-package evidence construction failed")]
    Evidence(#[source] Box<crate::GenerationEffectivePackageDerivationError>),
}

impl fmt::Debug for ManagedJudgeEffectivePackageDerivationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut value = formatter.debug_struct("ManagedJudgeEffectivePackageDerivationError");
        match self {
            Self::Cancelled => value.field("kind", &"cancelled"),
            Self::ModelRevalidation(_) => value.field("kind", &"model_revalidation"),
            Self::RuntimeRevalidation(_) => value.field("kind", &"runtime_revalidation"),
            Self::Relationship(relationship) => value
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::Evidence(_) => value.field("kind", &"evidence"),
        };
        value.finish_non_exhaustive()
    }
}

/// Primary derivation failure plus every mandatory finalization failure.
pub struct ManagedJudgeEffectivePackagePlanError {
    pub(super) primary: ManagedJudgeEffectivePackageDerivationError,
    pub(super) finalization: Option<ManagedJudgeEffectivePackageReleaseError>,
}

impl ManagedJudgeEffectivePackagePlanError {
    /// Returns the primary preparation failure.
    #[must_use]
    pub const fn primary(&self) -> &ManagedJudgeEffectivePackageDerivationError {
        &self.primary
    }
    /// Returns all secondary mandatory finalization failures.
    #[must_use]
    pub const fn finalization(&self) -> Option<&ManagedJudgeEffectivePackageReleaseError> {
        self.finalization.as_ref()
    }
}

impl fmt::Debug for ManagedJudgeEffectivePackagePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgeEffectivePackagePlanError")
            .field("primary", &self.primary)
            .field("has_finalization_failures", &self.finalization.is_some())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ManagedJudgeEffectivePackagePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "managed judge effective-package preparation failed"
        )
    }
}

impl StdError for ManagedJudgeEffectivePackagePlanError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.primary)
    }
}

/// Failure of final evidence or retained schedule-authority validation.
#[derive(Error)]
pub enum ManagedJudgeEffectivePackageFinalValidationError {
    /// Typed V2 evidence no longer validates against its exact inputs.
    #[error("managed judge effective-package evidence validation failed")]
    Evidence(#[source] EffectivePackageEvidenceV2Error),
    /// The retained schedule authority no longer rederived exactly.
    #[error("managed judge observation authority validation failed")]
    Authority(#[source] crate::ManagedJudgeObservationAuthorityError),
    /// Retained portable, evidence, or schedule relationships no longer agree.
    #[error("managed judge released-package relationship validation failed")]
    Relationship,
    /// The candidate evidence selected for release did not equal expected evidence.
    #[error("managed judge effective-package release evidence changed")]
    EvidenceChanged,
}

impl fmt::Debug for ManagedJudgeEffectivePackageFinalValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Evidence(_) => "evidence",
            Self::Authority(_) => "authority",
            Self::Relationship => "relationship",
            Self::EvidenceChanged => "evidence_changed",
        };
        formatter
            .debug_struct("ManagedJudgeEffectivePackageFinalValidationError")
            .field("kind", &kind)
            .finish_non_exhaustive()
    }
}

/// Complete cleanup, model, runtime, and validation failure set.
pub struct ManagedJudgeEffectivePackageReleaseError {
    pub(super) cleanup: Option<Box<ManagedOllamaCloseError>>,
    pub(super) model: Option<Box<ManagedOllamaModelPackageError>>,
    pub(super) runtime: Option<Box<PackageAttestationError>>,
    pub(super) evidence: Option<Box<ManagedJudgeEffectivePackageFinalValidationError>>,
}

impl ManagedJudgeEffectivePackageReleaseError {
    /// Returns the managed-bracket cleanup failure.
    #[must_use]
    pub fn cleanup(&self) -> Option<&ManagedOllamaCloseError> {
        self.cleanup.as_deref()
    }
    /// Returns the independent specialized model-package failure.
    #[must_use]
    pub fn model(&self) -> Option<&ManagedOllamaModelPackageError> {
        self.model.as_deref()
    }
    /// Returns the independent runtime-package failure.
    #[must_use]
    pub fn runtime(&self) -> Option<&PackageAttestationError> {
        self.runtime.as_deref()
    }
    /// Returns final evidence or retained-authority validation failure.
    #[must_use]
    pub fn evidence(&self) -> Option<&ManagedJudgeEffectivePackageFinalValidationError> {
        self.evidence.as_deref()
    }
}

impl fmt::Debug for ManagedJudgeEffectivePackageReleaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedJudgeEffectivePackageReleaseError")
            .field("cleanup_failed", &self.cleanup.is_some())
            .field("model_failed", &self.model.is_some())
            .field("runtime_failed", &self.runtime.is_some())
            .field("evidence_failed", &self.evidence.is_some())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for ManagedJudgeEffectivePackageReleaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "managed judge effective-package release failed")
    }
}

impl StdError for ManagedJudgeEffectivePackageReleaseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.cleanup
            .as_deref()
            .map(|error| error as &(dyn StdError + 'static))
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
