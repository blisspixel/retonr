use std::fmt;

use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::{
    ModelLicenseControlError, ModelLicenseControlId, ModelLicensePermission,
    ModelPackageFoundationId, VerifiedApprovedModelLicenseControl,
    VerifiedManagedOllamaModelPackageLease,
};
use rewrite_model::{ArtifactSetManifest, ModelPackageManifest};

use super::super::{
    ManagedOllamaInputEvidence, ManagedOllamaModelTarget, VerifiedManagedOllamaInputPlan,
};
use super::{ManagedOllamaLaunchError, PackageAttestationService};

/// Failure from the exact live model foundation and license-authority checkpoint.
#[derive(Debug, Error)]
pub enum ManagedOllamaModelAuthorityError {
    /// The consumed authority's exact pointer, permission, and complete
    /// specialized foundation check failed.
    #[error("managed Ollama model-license authority revalidation failed")]
    Revalidation(#[source] ModelLicenseControlError),
}

/// Single app-owned authority required by the executable managed Ollama launch.
///
/// Construction consumes one verified input plan and one verified local-generation
/// license authority. Both must retain the exact selected live specialized lease by
/// pointer identity. Equal portable identities or installation generations cannot
/// substitute for that capability relationship.
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedOllamaLaunchPlan;
///
/// fn clone_plan(value: &VerifiedManagedOllamaLaunchPlan<'_>) {
///     let _forged: VerifiedManagedOllamaLaunchPlan<'_> = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_app::VerifiedManagedOllamaLaunchPlan;
///
/// fn serialize_plan(value: &VerifiedManagedOllamaLaunchPlan<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("launch authority must not serialize");
/// }
/// ```
///
/// The weaker generic input plan is deliberately not part of the public API:
///
/// ```compile_fail
/// use rewrite_app::ManagedOllamaInputPlan;
/// ```
pub struct VerifiedManagedOllamaLaunchPlan<'lease> {
    package: &'lease VerifiedManagedOllamaModelPackageLease,
    input: VerifiedManagedOllamaInputPlan<'lease>,
    license: VerifiedApprovedModelLicenseControl<'lease>,
    license_control_id: ModelLicenseControlId,
}

impl<'lease> VerifiedManagedOllamaLaunchPlan<'lease> {
    /// Returns the stable verified model-foundation identity.
    #[must_use]
    pub const fn foundation_id(&self) -> &ModelPackageFoundationId {
        self.package.foundation_id()
    }

    /// Returns the independently approved portable license-control identity.
    #[must_use]
    pub const fn model_license_control_id(&self) -> &ModelLicenseControlId {
        &self.license_control_id
    }

    /// Returns redacted model-package and private-input binding evidence.
    #[must_use]
    pub const fn input_evidence(&self) -> &ManagedOllamaInputEvidence {
        self.input.evidence()
    }

    /// Returns the exact target-visible GGUF identity and path binding.
    #[must_use]
    pub const fn model_target(&self) -> &ManagedOllamaModelTarget {
        self.input.model_target()
    }

    /// Returns the exact retained GGUF byte count without exposing its handle.
    #[must_use]
    pub const fn model_byte_size(&self) -> u64 {
        self.input.retained_model_weight().byte_size()
    }

    /// Tests whether this launch authority retains the exact selected model lease.
    ///
    /// Equal portable package records cannot substitute for the live lease
    /// retained by this noncloneable launch authority.
    #[must_use]
    #[doc(hidden)]
    pub fn binds_exact_model_package(
        &self,
        selected: &VerifiedManagedOllamaModelPackageLease,
    ) -> bool {
        std::ptr::eq(self.package, selected)
    }

    pub(crate) const fn package(&self) -> &'lease VerifiedManagedOllamaModelPackageLease {
        self.package
    }

    pub(super) const fn input(&self) -> &VerifiedManagedOllamaInputPlan<'_> {
        &self.input
    }

    pub(super) const fn license(&self) -> &VerifiedApprovedModelLicenseControl<'_> {
        &self.license
    }

    pub(crate) const fn model_artifact_set_manifest(&self) -> &ArtifactSetManifest {
        self.package.private_view().artifact_set_manifest()
    }

    pub(crate) const fn model_package_manifest(&self) -> &ModelPackageManifest {
        self.package.private_view().model_package_manifest()
    }

    pub(crate) fn model_installation_generation(&self) -> u64 {
        self.package.private_view().installation_generation()
    }

    pub(crate) fn revalidate_for_candidate_precursor(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedOllamaModelAuthorityError> {
        revalidate_exact_model_authority(self.package, &self.license, cancellation)
    }

    pub(crate) fn revalidate_for_managed_judge_precursor(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedOllamaModelAuthorityError> {
        revalidate_exact_model_authority(self.package, &self.license, cancellation)
    }

    #[cfg(test)]
    pub(crate) fn verify_internal_handoff_for_test(
        self,
        selected: &VerifiedManagedOllamaModelPackageLease,
        expected_control_id: &ModelLicenseControlId,
    ) -> bool {
        let getters_match = std::ptr::eq(self.package(), selected)
            && self.input().binds_exact_package_lease(selected)
            && self.license().control_id() == expected_control_id;
        let (package, input, license, control_id) = self.consume();
        getters_match
            && std::ptr::eq(package, selected)
            && input.binds_exact_package_lease(selected)
            && license.control_id() == expected_control_id
            && &control_id == expected_control_id
    }

    #[cfg(test)]
    pub(crate) fn revalidate_for_materialization_test(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), ManagedOllamaLaunchError> {
        revalidate_exact_model_authority(self.package, &self.license, cancellation)
            .map_err(ManagedOllamaLaunchError::ModelAuthority)
    }
}

impl<'lease> VerifiedManagedOllamaLaunchPlan<'lease> {
    pub(super) fn consume(
        self,
    ) -> (
        &'lease VerifiedManagedOllamaModelPackageLease,
        VerifiedManagedOllamaInputPlan<'lease>,
        VerifiedApprovedModelLicenseControl<'lease>,
        ModelLicenseControlId,
    ) {
        (
            self.package,
            self.input,
            self.license,
            self.license_control_id,
        )
    }
}

impl fmt::Debug for VerifiedManagedOllamaLaunchPlan<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedManagedOllamaLaunchPlan")
            .field("foundation_id", self.foundation_id())
            .field("license_control_id", &self.license_control_id)
            .field("input_evidence", self.input.evidence())
            .field("model_target", self.input.model_target())
            .finish_non_exhaustive()
    }
}

impl PackageAttestationService {
    /// Joins exact live model input and approved local-generation license authority.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedOllamaLaunchError`] when either input or license authority
    /// names another live lease, permission, foundation, package, or generation.
    /// This structural join performs no content rehash. The executable launch
    /// performs mandatory complete revalidation immediately before materializing
    /// private input.
    pub fn authorize_managed_ollama_v0_32_15_launch<'lease>(
        selected: &'lease VerifiedManagedOllamaModelPackageLease,
        input: VerifiedManagedOllamaInputPlan<'lease>,
        license: VerifiedApprovedModelLicenseControl<'lease>,
    ) -> Result<VerifiedManagedOllamaLaunchPlan<'lease>, ManagedOllamaLaunchError> {
        validate_exact_join(selected, &input, &license)?;
        let license_control_id = license.control_id().clone();
        Ok(VerifiedManagedOllamaLaunchPlan {
            package: selected,
            input,
            license,
            license_control_id,
        })
    }
}

pub(super) fn revalidate_exact_model_authority<'lease>(
    package: &'lease VerifiedManagedOllamaModelPackageLease,
    license: &VerifiedApprovedModelLicenseControl<'lease>,
    cancellation: &CancellationToken,
) -> Result<(), ManagedOllamaModelAuthorityError> {
    license
        .revalidate_for_use(
            package,
            ModelLicensePermission::LocalGeneration,
            cancellation,
        )
        .map_err(ManagedOllamaModelAuthorityError::Revalidation)
}

fn validate_exact_join(
    selected: &VerifiedManagedOllamaModelPackageLease,
    input: &VerifiedManagedOllamaInputPlan<'_>,
    license: &VerifiedApprovedModelLicenseControl<'_>,
) -> Result<(), ManagedOllamaLaunchError> {
    let evidence = input.evidence();
    let generation = selected.private_view().installation_generation();
    if license.permission() != ModelLicensePermission::LocalGeneration {
        return Err(ManagedOllamaLaunchError::ModelAuthority(
            ManagedOllamaModelAuthorityError::Revalidation(
                ModelLicenseControlError::PermissionMismatch,
            ),
        ));
    }
    if !license.binds_exact_package_lease(selected) {
        return Err(ManagedOllamaLaunchError::ModelAuthority(
            ManagedOllamaModelAuthorityError::Revalidation(ModelLicenseControlError::LeaseMismatch),
        ));
    }
    if !input.binds_exact_package_lease(selected)
        || input.foundation_id() != selected.foundation_id()
        || license.foundation_id() != selected.foundation_id()
        || evidence.artifact_set_id() != selected.artifact_set_id()
        || evidence.model_package_manifest_id() != selected.model_package_manifest_id()
        || evidence.installation_generation() != generation
        || input.model_target().artifact_id() != evidence.model_artifact_id()
        || input.model_target().target_digest() != evidence.model_target_digest()
        || input.retained_model_weight().artifact_id() != evidence.model_artifact_id()
    {
        return Err(ManagedOllamaLaunchError::RelationshipMismatch);
    }
    Ok(())
}
