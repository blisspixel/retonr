use rewrite_app::{
    ManagedOllamaCloseError, ManagedOllamaModelAuthorityError, ManagedOllamaModelPackageError,
    ModelLicenseControlError, PackageAttestationError,
};

use super::{LocalOllamaManagedGenerationError, finalize_retained_bracket};

fn operation_error() -> LocalOllamaManagedGenerationError {
    LocalOllamaManagedGenerationError::InvalidGenerationAuthority
}

fn cleanup_error() -> ManagedOllamaCloseError {
    ManagedOllamaCloseError::ModelAuthority(ManagedOllamaModelAuthorityError::Revalidation(
        ModelLicenseControlError::Lease(ManagedOllamaModelPackageError::Package(
            PackageAttestationError::InvalidModelLimits,
        )),
    ))
}

fn runtime_error() -> PackageAttestationError {
    PackageAttestationError::InvalidLimits
}

#[test]
fn outcome_is_released_only_after_cleanup_and_runtime_revalidation_succeed() {
    assert_eq!(
        finalize_retained_bracket::<u8>(Ok(7), Ok(()), Ok(())).expect("released"),
        7
    );
    assert!(matches!(
        finalize_retained_bracket::<()>(Err(operation_error()), Ok(()), Ok(())),
        Err(LocalOllamaManagedGenerationError::InvalidGenerationAuthority)
    ));
    assert!(matches!(
        finalize_retained_bracket::<()>(Ok(()), Err(cleanup_error()), Ok(())),
        Err(LocalOllamaManagedGenerationError::Cleanup(_))
    ));
    assert!(matches!(
        finalize_retained_bracket::<()>(Ok(()), Ok(()), Err(runtime_error())),
        Err(LocalOllamaManagedGenerationError::RuntimePackageAfterCleanup(_))
    ));
}

#[test]
fn every_independent_finalization_failure_is_preserved() {
    assert!(matches!(
        finalize_retained_bracket::<()>(Err(operation_error()), Err(cleanup_error()), Ok(())),
        Err(LocalOllamaManagedGenerationError::CleanupAfterFailure { .. })
    ));
    assert!(matches!(
        finalize_retained_bracket::<()>(Err(operation_error()), Ok(()), Err(runtime_error())),
        Err(LocalOllamaManagedGenerationError::RuntimePackageAfterOperationFailure { .. })
    ));
    assert!(matches!(
        finalize_retained_bracket::<()>(Ok(()), Err(cleanup_error()), Err(runtime_error())),
        Err(LocalOllamaManagedGenerationError::RuntimePackageAfterCleanupFailure { .. })
    ));
    assert!(matches!(
        finalize_retained_bracket::<()>(
            Err(operation_error()),
            Err(cleanup_error()),
            Err(runtime_error())
        ),
        Err(
            LocalOllamaManagedGenerationError::RuntimePackageAfterOperationAndCleanupFailure { .. }
        )
    ));
}
