use std::marker::PhantomData;

use rewrite_model::{ModelPackageManifest, RuntimePackageManifest, RuntimePackageMemberRole};
use rewrite_types::{CancellationToken, Digest};

use crate::{ArtifactSetInstallationKey, RuntimeArtifactSetLease};

mod contract;
mod managed_ollama_model;
mod ollama_input;
mod subjects;
#[cfg(test)]
mod tests;
mod verification;

pub use contract::{
    ModelPackageAttestationEvidence, ModelPackageLeaseLimits, PACKAGE_ATTESTATION_SCHEMA_VERSION,
    PackageAttestationError, PackageAttestationScope, RuntimePackageAttestationEvidence,
    RuntimePackageLeaseLimits,
};
pub use managed_ollama_model::{
    ManagedOllamaModelPackageError, ModelPackageFoundationId,
    VerifiedManagedOllamaModelPackageLease,
};
#[cfg(test)]
pub(crate) use ollama_input::ManagedOllamaInputPlan;
pub use ollama_input::{
    MANAGED_OLLAMA_INPUT_SCHEMA_VERSION, MANAGED_OLLAMA_V0_32_15_ENDPOINT, ManagedOllamaCloseError,
    ManagedOllamaInputError, ManagedOllamaInputEvidence, ManagedOllamaIsolationLease,
    ManagedOllamaLaunchError, ManagedOllamaLaunchFinalizationFailures,
    ManagedOllamaModelAuthorityError, ManagedOllamaModelTarget,
    ManagedOllamaPostAcquisitionFailure, VerifiedManagedOllamaInputPlan,
    VerifiedManagedOllamaLaunchPlan, managed_ollama_v0_32_15_launch_spec,
};
pub(crate) use subjects::ManagedOllamaSubjectBinding;
pub(crate) use subjects::{
    ManagedOllamaLiveSubjectToken, ModelPackageIdentityToken, RuntimePackageIdentityToken,
};
use verification::{
    RetainedCodeMember, RetainedModelMember, VerificationObserver, attest_model_package,
    attest_runtime_package, clone_retained_entrypoint, clone_retained_model_members,
    clone_retained_native_members, recheck_retained_model_package_identities,
    retain_model_package_for_full_verification, revalidate_retained_model_members,
    revalidate_verified_model_members_once,
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
            artifact_set.manifest().total_byte_size(),
        );
        Ok(RuntimePackageLease {
            artifact_set,
            retained,
            evidence,
            limits,
            identity: RuntimePackageIdentityToken::new(),
        })
    }

    /// Validates one model package against an exact retained managed byte set.
    ///
    /// The returned lease pins the verified managed root and lifecycle locks. It
    /// applies [`ModelPackageLeaseLimits::default`] and grants no claim that a
    /// runtime loaded or used any model member.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] when default limits are exceeded, the
    /// model manifest does not exactly cover the leased byte set, cancellation is
    /// observed, or storage changed.
    pub fn attest_model(
        artifact_set: RuntimeArtifactSetLease,
        package: &ModelPackageManifest,
        cancellation: &CancellationToken,
    ) -> Result<ModelPackageLease, PackageAttestationError> {
        Self::attest_model_with_limits(
            artifact_set,
            package,
            ModelPackageLeaseLimits::default(),
            cancellation,
        )
    }

    /// Verifies and retains every exact model-package member under caller ceilings.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] for invalid or exceeded limits,
    /// cancellation, package mismatch, managed-tree drift, member-byte drift, or
    /// an unsafe retained object.
    pub fn attest_model_with_limits(
        artifact_set: RuntimeArtifactSetLease,
        package: &ModelPackageManifest,
        limits: ModelPackageLeaseLimits,
        cancellation: &CancellationToken,
    ) -> Result<ModelPackageLease, PackageAttestationError> {
        let mut observer = VerificationObserver::none();
        Self::attest_model_with_limits_and_observer(
            artifact_set,
            package,
            limits,
            cancellation,
            &mut observer,
        )
    }

    #[cfg(test)]
    fn attest_model_with_observer(
        artifact_set: RuntimeArtifactSetLease,
        package: &ModelPackageManifest,
        cancellation: &CancellationToken,
        observer: &mut VerificationObserver<'_>,
    ) -> Result<ModelPackageLease, PackageAttestationError> {
        Self::attest_model_with_limits_and_observer(
            artifact_set,
            package,
            ModelPackageLeaseLimits::default(),
            cancellation,
            observer,
        )
    }

    fn attest_model_with_limits_and_observer(
        artifact_set: RuntimeArtifactSetLease,
        package: &ModelPackageManifest,
        limits: ModelPackageLeaseLimits,
        cancellation: &CancellationToken,
        observer: &mut VerificationObserver<'_>,
    ) -> Result<ModelPackageLease, PackageAttestationError> {
        let retained =
            attest_model_package(&artifact_set, package, limits, cancellation, observer)?;
        let evidence = ModelPackageAttestationEvidence::new(
            artifact_set.manifest().artifact_set_id(),
            package.model_package_manifest_id(),
            u32::try_from(package.members().len())
                .map_err(|_| PackageAttestationError::InvalidModelLimits)?,
            artifact_set.manifest().total_byte_size(),
        );
        Ok(ModelPackageLease {
            artifact_set,
            retained,
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
    identity: RuntimePackageIdentityToken,
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

    pub(crate) fn identity_token(&self) -> RuntimePackageIdentityToken {
        self.identity.clone()
    }

    pub(crate) fn binds_identity_token(&self, token: &RuntimePackageIdentityToken) -> bool {
        self.identity.ptr_eq(token)
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
        clone_retained_entrypoint(&self.retained, cancellation)
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
    retained: Vec<RetainedModelMember>,
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
        revalidate_retained_model_members(&self.artifact_set, &self.retained, cancellation)
    }

    /// Clones every retained model-package object into an opaque isolation source.
    ///
    /// The complete model package is revalidated before any handle is cloned. The
    /// returned capability contains no host path and cannot outlive this lease. It
    /// exposes no raw constructor or file handle; a later application-owned adapter
    /// may translate it into a platform isolation capability without reopening a
    /// pathname.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] on cancellation, package drift, object
    /// replacement, canonical-name drift, or a native handle-clone failure.
    pub fn clone_members_for_isolation(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<RetainedModelPackageIsolationSource<'_>, PackageAttestationError> {
        self.revalidate(cancellation)?;
        let members = clone_retained_model_members(&self.retained)?;
        verification::revalidate_detached_model_members(&members, cancellation)?;
        let binding_digest = isolation_source_binding_digest(
            self.evidence.artifact_set_id(),
            self.evidence.model_package_manifest_id(),
            self.installation_key().installation_generation(),
        );
        Ok(RetainedModelPackageIsolationSource {
            artifact_set_id: self.evidence.artifact_set_id().clone(),
            model_package_manifest_id: self.evidence.model_package_manifest_id().clone(),
            installation_generation: self.installation_key().installation_generation(),
            byte_size: self.evidence.byte_size(),
            binding_digest,
            members,
            _lease: PhantomData,
        })
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

/// Opaque retained model-package objects prepared for a managed isolation boundary.
///
/// This capability owns clones of exact verified file objects but deliberately
/// exposes neither host paths nor raw file handles. It grants no load, execution,
/// model-use, or qualification claim and cannot outlive the originating lease.
/// A future child adapter inside this module may consume the private member vector
/// and transfer its handles directly into the isolation crate. That internal seam
/// must remain consuming and must not add a public handle or host-path accessor.
pub struct RetainedModelPackageIsolationSource<'lease> {
    artifact_set_id: rewrite_model::ArtifactSetId,
    model_package_manifest_id: rewrite_model::ModelPackageManifestId,
    installation_generation: u64,
    byte_size: u64,
    binding_digest: Digest,
    members: Vec<RetainedModelMember>,
    _lease: PhantomData<&'lease ModelPackageLease>,
}

impl RetainedModelPackageIsolationSource<'_> {
    /// Returns the exact managed byte-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &rewrite_model::ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact semantic model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &rewrite_model::ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the exact positive local installation generation.
    #[must_use]
    pub const fn installation_generation(&self) -> u64 {
        self.installation_generation
    }

    /// Returns the number of retained model-package objects.
    #[must_use]
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Returns the checked total retained model-package bytes.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns a content-free binding of package identities and installation generation.
    #[must_use]
    pub const fn binding_digest(&self) -> &Digest {
        &self.binding_digest
    }

    /// Rehashes every cloned object without consulting or exposing a host path.
    ///
    /// This verifies only the retained isolation-source objects. The originating
    /// lease must be revalidated separately to recheck the canonical managed tree.
    ///
    /// # Errors
    ///
    /// Returns [`PackageAttestationError`] on cancellation, byte drift, object
    /// replacement, or a retained-handle operation failure.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), PackageAttestationError> {
        verification::revalidate_detached_model_members(&self.members, cancellation)
    }
}

impl std::fmt::Debug for RetainedModelPackageIsolationSource<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedModelPackageIsolationSource")
            .field("artifact_set_id", &self.artifact_set_id)
            .field("model_package_manifest_id", &self.model_package_manifest_id)
            .field("installation_generation", &self.installation_generation)
            .field("binding_digest", &self.binding_digest)
            .field("member_count", &self.members.len())
            .finish_non_exhaustive()
    }
}

fn isolation_source_binding_digest(
    artifact_set_id: &rewrite_model::ArtifactSetId,
    model_package_manifest_id: &rewrite_model::ModelPackageManifestId,
    installation_generation: u64,
) -> Digest {
    let mut material = b"retonr:retained-model-package-isolation-source:v1\0".to_vec();
    material.extend_from_slice(artifact_set_id.digest().as_str().as_bytes());
    material.extend_from_slice(model_package_manifest_id.digest().as_str().as_bytes());
    material.extend_from_slice(&installation_generation.to_be_bytes());
    Digest::sha256(&material)
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
                | RuntimePackageMemberRole::WorkerExecutable
        )
    })
}
