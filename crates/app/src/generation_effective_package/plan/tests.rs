use std::error::Error as StdError;

use rewrite_model::EffectivePackageEvidenceV2Error;
use rewrite_runtime_isolation::IsolationError;

use crate::{ManagedOllamaCloseError, ManagedOllamaModelPackageError, PackageAttestationError};

use super::*;

fn complete_release_error() -> GenerationEffectivePackageReleaseError {
    GenerationEffectivePackageReleaseError {
        cleanup: Some(Box::new(ManagedOllamaCloseError::Isolation(
            IsolationError::Cancelled,
        ))),
        timing: Some(GenerationEffectivePackageCleanupTimingError::ReversedMonotonicCheckpoints),
        model: Some(Box::new(
            ManagedOllamaModelPackageError::FoundationBindingChanged,
        )),
        runtime: Some(Box::new(PackageAttestationError::InvalidLimits)),
        evidence: Some(Box::new(EffectivePackageEvidenceV2Error::InvalidMetadata)),
    }
}

#[test]
fn release_error_preserves_every_independent_failure() {
    let error = complete_release_error();
    assert!(matches!(
        error.cleanup(),
        Some(ManagedOllamaCloseError::Isolation(
            IsolationError::Cancelled
        ))
    ));
    assert_eq!(
        error.timing(),
        Some(GenerationEffectivePackageCleanupTimingError::ReversedMonotonicCheckpoints)
    );
    assert!(matches!(
        error.model(),
        Some(ManagedOllamaModelPackageError::FoundationBindingChanged)
    ));
    assert!(matches!(
        error.runtime(),
        Some(PackageAttestationError::InvalidLimits)
    ));
    assert_eq!(
        error.evidence(),
        Some(&EffectivePackageEvidenceV2Error::InvalidMetadata)
    );
    assert_eq!(
        error.to_string(),
        "effective-package cleanup-gated release failed"
    );
    assert!(StdError::source(&error).is_some());
}

#[test]
fn plan_error_keeps_primary_derivation_and_secondary_release() {
    let error = GenerationEffectivePackagePlanError {
        derivation: GenerationEffectivePackageDerivationError::Cancelled,
        release: Some(complete_release_error()),
    };
    assert!(matches!(
        error.derivation(),
        GenerationEffectivePackageDerivationError::Cancelled
    ));
    assert!(error.release().is_some());
    assert_eq!(
        error.to_string(),
        "effective-package derivation failed and release checks also failed"
    );
    assert!(StdError::source(&error).is_some());

    let without_release = GenerationEffectivePackagePlanError {
        derivation: GenerationEffectivePackageDerivationError::RelationshipMismatch,
        release: None,
    };
    assert_eq!(
        without_release.to_string(),
        "effective-package derivation failed"
    );
    assert!(without_release.release().is_none());
}
