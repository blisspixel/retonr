use rewrite_runtime_isolation::RetainedIsolationLease;
use rewrite_types::CancellationToken;

use crate::{
    RuntimePackageLease, VerifiedApprovedModelLicenseControl,
    VerifiedManagedOllamaModelPackageLease,
};

use super::authority::revalidate_exact_model_authority;
use super::{ManagedOllamaLaunchError, ManagedOllamaPostAcquisitionFailure};

mod kernel;
pub(super) use kernel::post_acquisition_failure;

pub(super) fn finalize_post_acquisition_binding_failure(
    isolation: RetainedIsolationLease,
    model_package: &VerifiedManagedOllamaModelPackageLease,
    license: &VerifiedApprovedModelLicenseControl<'_>,
    runtime_package: &mut RuntimePackageLease,
) -> ManagedOllamaLaunchError {
    finalize_post_acquisition(
        ManagedOllamaPostAcquisitionFailure::InputBindingMismatch,
        isolation,
        model_package,
        license,
        runtime_package,
    )
}

pub(super) fn finalize_post_acquisition_cancellation(
    isolation: RetainedIsolationLease,
    model_package: &VerifiedManagedOllamaModelPackageLease,
    license: &VerifiedApprovedModelLicenseControl<'_>,
    runtime_package: &mut RuntimePackageLease,
) -> ManagedOllamaLaunchError {
    finalize_post_acquisition(
        ManagedOllamaPostAcquisitionFailure::Cancelled,
        isolation,
        model_package,
        license,
        runtime_package,
    )
}

pub(super) fn finalize_post_acquisition_deadline(
    isolation: RetainedIsolationLease,
    model_package: &VerifiedManagedOllamaModelPackageLease,
    license: &VerifiedApprovedModelLicenseControl<'_>,
    runtime_package: &mut RuntimePackageLease,
) -> ManagedOllamaLaunchError {
    finalize_post_acquisition(
        ManagedOllamaPostAcquisitionFailure::DeadlineExceeded,
        isolation,
        model_package,
        license,
        runtime_package,
    )
}

fn finalize_post_acquisition(
    primary: ManagedOllamaPostAcquisitionFailure,
    isolation: RetainedIsolationLease,
    model_package: &VerifiedManagedOllamaModelPackageLease,
    license: &VerifiedApprovedModelLicenseControl<'_>,
    runtime_package: &mut RuntimePackageLease,
) -> ManagedOllamaLaunchError {
    kernel::finalize_post_acquisition_with(
        primary,
        || isolation.close(&CancellationToken::new()),
        || revalidate_exact_model_authority(model_package, license, &CancellationToken::new()),
        || runtime_package.revalidate(&CancellationToken::new()),
    )
}
