use std::{
    fmt,
    time::{Duration, Instant},
};

use rewrite_model::{
    ArtifactSetId, ArtifactSetManifest, EffectivePackageEvidenceV2, ModelPackageManifestId,
    RuntimeBuildIdentity, RuntimePackageManifest, RuntimePackageManifestId,
};
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::CancellationToken;

use crate::effective_runtime_state_observation::{
    ManagedOllamaEffectiveRuntimeState, ManagedOllamaResourceAttemptSubject,
};
use crate::{
    ManagedOllamaIsolationLease, ModelLicenseControlId, ModelPackageFoundationId,
    RuntimePackageLease, VerifiedAdmittedRuntime, VerifiedManagedGenerationPath,
    VerifiedManagedOllamaModelPackageLease,
};

use super::derivation::{ProductionInputs, derive};

mod error;
mod timing;

pub use error::{
    GenerationEffectivePackageCleanupTimingError, GenerationEffectivePackageDerivationError,
    GenerationEffectivePackagePlanError, GenerationEffectivePackageReleaseError,
};

use timing::run_timed_release_and_validation_steps;

/// A noncloneable pending effective-package release bound to live retained leases.
///
/// The effective-package record is deliberately inaccessible until [`Self::release`]
/// closes the contained managed isolation lease and independently revalidates both
/// retained packages.
///
/// ```compile_fail
/// use rewrite_app::VerifiedGenerationEffectivePackagePlan;
///
/// fn clone_plan(value: &VerifiedGenerationEffectivePackagePlan<'_, '_>) {
///     let _copy: VerifiedGenerationEffectivePackagePlan<'_, '_> = value.clone();
/// }
/// ```
pub struct VerifiedGenerationEffectivePackagePlan<'model, 'runtime> {
    managed_ollama: ManagedOllamaIsolationLease<'model>,
    model_package: &'model VerifiedManagedOllamaModelPackageLease,
    runtime_package: &'runtime mut RuntimePackageLease,
    artifact_set: ArtifactSetManifest,
    runtime_build: RuntimeBuildIdentity,
    effective_state: ManagedOllamaEffectiveRuntimeState,
    evidence: EffectivePackageEvidenceV2,
    foundation_id: ModelPackageFoundationId,
    license_control_id: ModelLicenseControlId,
}

impl<'model, 'runtime> VerifiedGenerationEffectivePackagePlan<'model, 'runtime> {
    /// Derives a pending record from one exact completed live generation tuple.
    ///
    /// This consumes the still-live isolation lease after final observations exist.
    /// It validates retained typed relationships without repeating large package
    /// hashing passes. Failure still closes the lease and runs both independent
    /// final package revalidations.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationEffectivePackagePlanError`] for cancellation, a
    /// substituted capability or manifest, invalid member derivation, or secondary
    /// cleanup or final package-revalidation failures.
    #[expect(
        clippy::too_many_arguments,
        reason = "every independently verified production input remains explicit"
    )]
    pub fn prepare(
        runtime_manifest: &RuntimePackageManifest,
        runtime_package: &'runtime mut RuntimePackageLease,
        model_package: &'model VerifiedManagedOllamaModelPackageLease,
        runtime_build: &RuntimeBuildIdentity,
        effective_state: ManagedOllamaEffectiveRuntimeState,
        admitted_runtime: &VerifiedAdmittedRuntime,
        generation_path: &VerifiedManagedGenerationPath,
        frozen_components: &VerifiedFrozenExternalNativeComponentSet,
        prepared_isolation: &PreparedIsolation,
        managed_ollama: ManagedOllamaIsolationLease<'model>,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationEffectivePackagePlanError> {
        let license_control_id = managed_ollama.model_license_control_id().clone();
        let result = derive(&ProductionInputs {
            runtime_manifest,
            runtime_package,
            model_package,
            runtime_build,
            effective_state: &effective_state,
            admitted_runtime,
            generation_path,
            frozen_components,
            prepared_isolation,
            managed_ollama: &managed_ollama,
            cancellation,
        });
        match result {
            Ok(derived) => Ok(Self {
                managed_ollama,
                model_package,
                runtime_package,
                artifact_set: derived.artifact_set,
                runtime_build: runtime_build.clone(),
                effective_state,
                evidence: derived.evidence,
                foundation_id: model_package.foundation_id().clone(),
                license_control_id,
            }),
            Err(derivation) => {
                let release = finish_release(managed_ollama, model_package, runtime_package);
                Err(GenerationEffectivePackagePlanError {
                    derivation,
                    release,
                })
            }
        }
    }

    /// Closes the exact managed bracket and releases its inert v2 evidence.
    ///
    /// Cleanup runs first. Specialized model-foundation and runtime-package
    /// revalidation then run independently with fresh cancellation tokens even if
    /// cleanup failed. Evidence is released only when all operations and a final
    /// owned-record validation pass.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationEffectivePackageReleaseError`] retaining every cleanup,
    /// package-revalidation, or typed validation failure.
    pub fn release(
        self,
    ) -> Result<ReleasedGenerationEffectivePackageV2, GenerationEffectivePackageReleaseError> {
        let Self {
            managed_ollama,
            model_package,
            runtime_package,
            artifact_set,
            runtime_build,
            effective_state,
            evidence,
            foundation_id,
            license_control_id,
        } = self;
        let runtime_artifact_set_id = runtime_package.evidence().artifact_set_id().clone();
        let runtime_package_manifest_id = runtime_package
            .evidence()
            .runtime_package_manifest_id()
            .clone();
        let model_artifact_set_id = model_package.artifact_set_id().clone();
        let model_package_manifest_id = model_package.model_package_manifest_id().clone();
        let (cleanup, cleanup_elapsed, cleanup_completed_at, model, runtime, evidence_error) =
            run_timed_release_and_validation_steps(
                || managed_ollama.close(&CancellationToken::new()),
                || model_package.revalidate(&CancellationToken::new()),
                || runtime_package.revalidate(&CancellationToken::new()),
                || {
                    evidence.validate_against(
                        &artifact_set,
                        &runtime_build,
                        effective_state.state(),
                    )
                },
                Instant::now,
            );
        let cleanup_elapsed = match cleanup_elapsed {
            Ok(elapsed)
                if cleanup.is_none()
                    && model.is_none()
                    && runtime.is_none()
                    && evidence_error.is_none() =>
            {
                elapsed
            }
            result => {
                return Err(GenerationEffectivePackageReleaseError {
                    cleanup: cleanup.map(Box::new),
                    timing: result.err(),
                    model: model.map(Box::new),
                    runtime: runtime.map(Box::new),
                    evidence: evidence_error.map(Box::new),
                });
            }
        };
        let resource_attempt_subject = effective_state.into_resource_attempt_subject();
        Ok(ReleasedGenerationEffectivePackageV2 {
            evidence,
            foundation_id,
            license_control_id,
            runtime_artifact_set_id,
            runtime_package_manifest_id,
            model_artifact_set_id,
            model_package_manifest_id,
            cleanup_elapsed,
            cleanup_completed_at,
            resource_attempt_subject,
        })
    }
}

impl fmt::Debug for VerifiedGenerationEffectivePackagePlan<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedGenerationEffectivePackagePlan")
            .field("foundation_id", &self.foundation_id)
            .field("license_control_id", &self.license_control_id)
            .field(
                "effective_package_id",
                &self.evidence.effective_package_evidence_v2_id(),
            )
            .finish_non_exhaustive()
    }
}

/// Cleanup-gated effective-package v2 evidence for one completed generation tuple.
///
/// This wrapper is noncloneable and nonserializable. Its contained model-layer
/// record remains inert and makes no qualification or model-use claim.
pub struct ReleasedGenerationEffectivePackageV2 {
    evidence: EffectivePackageEvidenceV2,
    foundation_id: ModelPackageFoundationId,
    license_control_id: ModelLicenseControlId,
    runtime_artifact_set_id: ArtifactSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    cleanup_elapsed: Duration,
    cleanup_completed_at: Instant,
    resource_attempt_subject: Option<ManagedOllamaResourceAttemptSubject>,
}

impl ReleasedGenerationEffectivePackageV2 {
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn exact_candidate_precursor_test_fixture(
        evidence: EffectivePackageEvidenceV2,
        foundation_id: ModelPackageFoundationId,
        license_control_id: ModelLicenseControlId,
        runtime_artifact_set_id: ArtifactSetId,
        runtime_package_manifest_id: RuntimePackageManifestId,
        model_artifact_set_id: ArtifactSetId,
        model_package_manifest_id: ModelPackageManifestId,
    ) -> Self {
        Self {
            evidence,
            foundation_id,
            license_control_id,
            runtime_artifact_set_id,
            runtime_package_manifest_id,
            model_artifact_set_id,
            model_package_manifest_id,
            cleanup_elapsed: Duration::ZERO,
            cleanup_completed_at: Instant::now(),
            resource_attempt_subject: None,
        }
    }

    #[cfg(all(test, feature = "test-support"))]
    pub(crate) fn retain_resource_attempt_test_subject(
        &mut self,
        completion: &rewrite_ollama::OllamaResidentResourceObservedCompletion,
        worker_evidence: &rewrite_runtime_attestor::ManagedGenerationWorkerEvidence,
        runtime_package: &RuntimePackageLease,
        model_package: &VerifiedManagedOllamaModelPackageLease,
    ) {
        self.resource_attempt_subject =
            Some(ManagedOllamaResourceAttemptSubject::exact_test_fixture(
                completion,
                worker_evidence,
                runtime_package,
                model_package,
            ));
    }

    pub(crate) fn binds_resource_attempt_completion(
        &self,
        completion: &rewrite_ollama::OllamaResidentResourceObservedCompletion,
    ) -> bool {
        self.resource_attempt_subject
            .as_ref()
            .is_some_and(|subject| subject.binds_completion(completion))
    }

    pub(crate) fn binds_resource_attempt_worker(
        &self,
        observation: &rewrite_runtime_attestor::ManagedGenerationWorkerResourceObservation,
        evidence: &rewrite_runtime_attestor::ManagedGenerationWorkerEvidence,
    ) -> bool {
        self.resource_attempt_subject
            .as_ref()
            .is_some_and(|subject| subject.binds_worker(observation, evidence))
    }

    pub(crate) fn binds_resource_attempt_packages(
        &self,
        runtime_package: &RuntimePackageLease,
        model_package: &VerifiedManagedOllamaModelPackageLease,
    ) -> bool {
        self.resource_attempt_subject
            .as_ref()
            .is_some_and(|subject| {
                subject.binds_runtime_package(runtime_package)
                    && subject.binds_model_package(model_package)
            })
    }

    pub(crate) fn resource_attempt_session_subject(
        &self,
    ) -> Option<rewrite_ollama::OllamaRetainedSessionSubjectToken> {
        self.resource_attempt_subject
            .as_ref()
            .map(ManagedOllamaResourceAttemptSubject::retained_session_subject)
    }

    /// Returns the cleanup-gated inert model-layer record.
    #[must_use]
    pub const fn evidence(&self) -> &EffectivePackageEvidenceV2 {
        &self.evidence
    }

    /// Returns the specialized verified model foundation used by the derivation.
    #[must_use]
    pub const fn foundation_id(&self) -> &ModelPackageFoundationId {
        &self.foundation_id
    }

    /// Returns the exact approved control identity consumed by the live launch.
    #[must_use]
    pub const fn license_control_id(&self) -> &ModelLicenseControlId {
        &self.license_control_id
    }

    /// Returns the exact retained runtime artifact-set identity.
    #[must_use]
    pub const fn runtime_artifact_set_id(&self) -> &ArtifactSetId {
        &self.runtime_artifact_set_id
    }

    /// Returns the exact retained runtime-package identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact retained model artifact-set identity.
    #[must_use]
    pub const fn model_artifact_set_id(&self) -> &ArtifactSetId {
        &self.model_artifact_set_id
    }

    /// Returns the exact retained model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the app-measured time spent only closing managed Ollama isolation.
    #[must_use]
    pub const fn cleanup_elapsed(&self) -> Duration {
        self.cleanup_elapsed
    }

    pub(crate) fn managed_attempt_elapsed_since(&self, started: Instant) -> Option<Duration> {
        self.cleanup_completed_at.checked_duration_since(started)
    }

    /// Consumes the release wrapper and returns the inert model-layer evidence.
    #[must_use]
    pub fn into_evidence(self) -> EffectivePackageEvidenceV2 {
        self.evidence
    }
}

impl fmt::Debug for ReleasedGenerationEffectivePackageV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReleasedGenerationEffectivePackageV2")
            .field("foundation_id", &self.foundation_id)
            .field("license_control_id", &self.license_control_id)
            .field(
                "effective_package_id",
                &self.evidence.effective_package_evidence_v2_id(),
            )
            .finish_non_exhaustive()
    }
}

fn finish_release(
    managed_ollama: ManagedOllamaIsolationLease<'_>,
    model_package: &VerifiedManagedOllamaModelPackageLease,
    runtime_package: &mut RuntimePackageLease,
) -> Option<GenerationEffectivePackageReleaseError> {
    let (cleanup, model, runtime) = run_release_steps(
        || managed_ollama.close(&CancellationToken::new()),
        || model_package.revalidate(&CancellationToken::new()),
        || runtime_package.revalidate(&CancellationToken::new()),
    );
    if cleanup.is_none() && model.is_none() && runtime.is_none() {
        None
    } else {
        Some(GenerationEffectivePackageReleaseError {
            cleanup: cleanup.map(Box::new),
            timing: None,
            model: model.map(Box::new),
            runtime: runtime.map(Box::new),
            evidence: None,
        })
    }
}

pub(super) fn run_release_steps<C, M, R>(
    cleanup: impl FnOnce() -> Result<(), C>,
    model: impl FnOnce() -> Result<(), M>,
    runtime: impl FnOnce() -> Result<(), R>,
) -> (Option<C>, Option<M>, Option<R>) {
    let cleanup = cleanup().err();
    let model = model().err();
    let runtime = runtime().err();
    (cleanup, model, runtime)
}

#[cfg(test)]
pub(super) fn run_release_and_validation_steps<C, M, R, E>(
    cleanup: impl FnOnce() -> Result<(), C>,
    model: impl FnOnce() -> Result<(), M>,
    runtime: impl FnOnce() -> Result<(), R>,
    validate: impl FnOnce() -> Result<(), E>,
) -> (Option<C>, Option<M>, Option<R>, Option<E>) {
    let (cleanup, model, runtime) = run_release_steps(cleanup, model, runtime);
    let validation = if cleanup.is_none() && model.is_none() && runtime.is_none() {
        validate().err()
    } else {
        None
    };
    (cleanup, model, runtime, validation)
}

#[cfg(test)]
#[path = "plan/tests.rs"]
mod tests;
