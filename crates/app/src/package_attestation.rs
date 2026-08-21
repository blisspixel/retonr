use rewrite_model::{ModelPackageManifest, RuntimePackageManifest, RuntimePackageMemberRole};
use rewrite_types::CancellationToken;

use crate::{ArtifactSetInstallationKey, RuntimeArtifactSetLease};

mod contract;
#[cfg(test)]
mod tests;
mod verification;

pub use contract::{
    ModelPackageAttestationEvidence, PACKAGE_ATTESTATION_SCHEMA_VERSION, PackageAttestationError,
    PackageAttestationScope, RuntimePackageAttestationEvidence, RuntimePackageLeaseLimits,
};
use verification::{
    RetainedCodeMember, VerificationObserver, attest_runtime_package, clone_retained_entrypoint,
    clone_retained_native_members,
};

/// Builds point-in-time, static package evidence over a retained managed-set lease.
///
/// This service does not launch a process, observe native loads, or grant runtime
/// qualification. It only joins canonical package meaning to exact managed bytes.
#[derive(Clone, Copy, Debug, Default)]
pub struct PackageAttestationService;

impl PackageAttestationService {
    /// Validates one runtime package and retains every packaged executable-code handle.
    ///
    /// The supplied artifact-set lease is consumed so its managed root and shared
    /// lifecycle locks remain pinned for the complete returned lease lifetime.
    /// Entrypoint, native-dependency, and helper-executable members are reopened,
    /// bounded, hashed, identity-checked, and retained. The returned lease remains
    /// static byte evidence only because this API does not launch by retained handle.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] when limits are invalid, the semantic
    /// manifest does not exactly cover the leased byte set, cancellation is
    /// observed, or managed storage changes or conflicts.
    pub fn attest_runtime(
        artifact_set: RuntimeArtifactSetLease,
        package: &RuntimePackageManifest,
        limits: RuntimePackageLeaseLimits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimePackageLease, PackageAttestationError> {
        let mut observer = VerificationObserver::none();
        Self::attest_runtime_with_observer(
            artifact_set,
            package,
            limits,
            cancellation,
            &mut observer,
        )
    }

    fn attest_runtime_with_observer(
        artifact_set: RuntimeArtifactSetLease,
        package: &RuntimePackageManifest,
        limits: RuntimePackageLeaseLimits,
        cancellation: &CancellationToken,
        observer: &mut VerificationObserver<'_>,
    ) -> Result<RuntimePackageLease, PackageAttestationError> {
        let retained =
            attest_runtime_package(&artifact_set, package, limits, cancellation, observer)?;
        let code_byte_size = retained.iter().try_fold(0u64, |total, member| {
            total
                .checked_add(member.byte_size)
                .ok_or(PackageAttestationError::InvalidLimits)
        })?;
        let code_member_count =
            u32::try_from(retained.len()).map_err(|_| PackageAttestationError::InvalidLimits)?;
        let evidence = RuntimePackageAttestationEvidence::new(
            artifact_set.manifest().artifact_set_id(),
            package.runtime_package_manifest_id(),
            package.entrypoint().artifact_id().clone(),
            code_member_count,
            code_byte_size,
        );
        Ok(RuntimePackageLease {
            artifact_set,
            retained,
            evidence,
            limits,
        })
    }

    /// Validates one model package against an exact retained managed byte set.
    ///
    /// The returned lease pins the verified managed root and lifecycle locks. It
    /// grants no claim that a runtime loaded or used any model member.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] when the model manifest does not exactly
    /// cover the leased byte set, cancellation is observed, or storage changed.
    pub fn attest_model(
        artifact_set: RuntimeArtifactSetLease,
        package: &ModelPackageManifest,
        cancellation: &CancellationToken,
    ) -> Result<ModelPackageLease, PackageAttestationError> {
        ensure_not_cancelled(cancellation)?;
        package
            .validate_against(artifact_set.manifest())
            .map_err(PackageAttestationError::ModelRelationship)?;
        artifact_set
            .revalidate(cancellation)
            .map_err(PackageAttestationError::from_set_lease)?;
        let evidence = ModelPackageAttestationEvidence::new(
            artifact_set.manifest().artifact_set_id(),
            package.model_package_manifest_id(),
            u32::try_from(package.members().len())
                .map_err(|_| PackageAttestationError::InvalidLimits)?,
            artifact_set.manifest().total_byte_size(),
        );
        Ok(ModelPackageLease {
            artifact_set,
            evidence,
        })
    }
}

/// Retained point-in-time runtime-package byte lease.
///
/// All packaged executable-code handles and the exact artifact-set lifecycle
/// lease remain live until this value is dropped. This is not launch authority.
pub struct RuntimePackageLease {
    artifact_set: RuntimeArtifactSetLease,
    retained: Vec<RetainedCodeMember>,
    evidence: RuntimePackageAttestationEvidence,
    limits: RuntimePackageLeaseLimits,
}

impl RuntimePackageLease {
    /// Returns redacted, typed static package evidence.
    #[must_use]
    pub const fn evidence(&self) -> &RuntimePackageAttestationEvidence {
        &self.evidence
    }

    /// Returns the exact managed-set installation protected by this live lease.
    #[must_use]
    pub const fn installation_key(&self) -> &ArtifactSetInstallationKey {
        self.artifact_set.key()
    }

    /// Rehashes retained handles and rechecks their current canonical names.
    ///
    /// Callers must revalidate immediately before any later launch or native-load
    /// observation. This method still does not grant launch or load authority.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] on cancellation, byte drift, object
    /// replacement, tree drift, or an unsafe managed-storage boundary.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), PackageAttestationError> {
        verification::revalidate_retained_package(
            &self.artifact_set,
            &mut self.retained,
            self.limits,
            cancellation,
        )
    }

    /// Clones the exact retained entrypoint file object for a handle-based launch.
    ///
    /// The complete package is revalidated before the clone is returned. The caller
    /// must keep this package lease alive for the complete launched-process lifetime
    /// and pass the file only to a launch boundary that never reopens a pathname.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] on cancellation, package drift, an
    /// ambiguous entrypoint, or a native handle-clone failure.
    pub fn clone_entrypoint_for_launch(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<std::fs::File, PackageAttestationError> {
        self.revalidate(cancellation)?;
        clone_retained_entrypoint(&self.retained)
    }

    /// Clones every retained packaged-code object for native-load observation.
    ///
    /// The complete package is revalidated first. The returned capabilities retain
    /// no filesystem paths and are ordered exactly like the package's code members.
    /// Keep this package lease alive until observation completes.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] on cancellation, package drift, or a
    /// native handle-clone failure.
    pub fn clone_members_for_native_observation(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<Vec<rewrite_runtime_attestor::RetainedNativePackageMember>, PackageAttestationError>
    {
        self.revalidate(cancellation)?;
        clone_retained_native_members(&self.retained)
    }
}

impl std::fmt::Debug for RuntimePackageLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimePackageLease")
            .field("evidence", &self.evidence)
            .field("installation_key", self.artifact_set.key())
            .finish_non_exhaustive()
    }
}

/// Retained point-in-time model-package byte lease.
///
/// The managed artifact-set root and lifecycle locks remain pinned. This lease
/// does not claim that a runtime loaded or used the package.
pub struct ModelPackageLease {
    artifact_set: RuntimeArtifactSetLease,
    evidence: ModelPackageAttestationEvidence,
}

impl ModelPackageLease {
    /// Returns redacted, typed static model-package evidence.
    #[must_use]
    pub const fn evidence(&self) -> &ModelPackageAttestationEvidence {
        &self.evidence
    }

    /// Returns the exact managed-set installation protected by this live lease.
    #[must_use]
    pub const fn installation_key(&self) -> &ArtifactSetInstallationKey {
        self.artifact_set.key()
    }

    /// Revalidates every managed model-package byte and the exact tree boundary.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] on cancellation or managed-byte drift.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), PackageAttestationError> {
        self.artifact_set
            .revalidate(cancellation)
            .map_err(PackageAttestationError::from_set_lease)
    }
}

impl std::fmt::Debug for ModelPackageLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModelPackageLease")
            .field("evidence", &self.evidence)
            .field("installation_key", self.artifact_set.key())
            .finish_non_exhaustive()
    }
}

fn ensure_not_cancelled(cancellation: &CancellationToken) -> Result<(), PackageAttestationError> {
    if cancellation.is_cancelled() {
        Err(PackageAttestationError::Cancelled)
    } else {
        Ok(())
    }
}

fn is_packaged_code(roles: &[RuntimePackageMemberRole]) -> bool {
    roles.iter().any(|role| {
        matches!(
            role,
            RuntimePackageMemberRole::Entrypoint
                | RuntimePackageMemberRole::NativeDependency
                | RuntimePackageMemberRole::HelperExecutable
        )
    })
}
