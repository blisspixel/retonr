use rewrite_model::{
    ArtifactSetManifest, EffectivePackageEvidenceV2, EffectiveRuntimeState, RuntimeBuildIdentity,
};
use rewrite_types::CancellationToken;

use crate::{
    ManagedOllamaCloseError, ManagedOllamaIsolationLease, ManagedOllamaModelPackageError,
    PackageAttestationError, RuntimePackageLease, VerifiedManagedJudgeObservationAuthority,
    VerifiedManagedOllamaModelPackageLease,
};

use super::{
    ManagedJudgeEffectivePackageFinalValidationError, ManagedJudgeEffectivePackageReleaseError,
};

pub(super) struct FinalizationFailures {
    cleanup: Option<ManagedOllamaCloseError>,
    model: Option<ManagedOllamaModelPackageError>,
    runtime: Option<PackageAttestationError>,
    evidence: Option<ManagedJudgeEffectivePackageFinalValidationError>,
}

impl FinalizationFailures {
    pub(super) fn into_error(self) -> Option<ManagedJudgeEffectivePackageReleaseError> {
        if self.cleanup.is_none()
            && self.model.is_none()
            && self.runtime.is_none()
            && self.evidence.is_none()
        {
            None
        } else {
            Some(ManagedJudgeEffectivePackageReleaseError {
                cleanup: self.cleanup.map(Box::new),
                model: self.model.map(Box::new),
                runtime: self.runtime.map(Box::new),
                evidence: self.evidence.map(Box::new),
            })
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "each independent final authority and inert validation input remains explicit"
)]
pub(super) fn finalize(
    managed_ollama: ManagedOllamaIsolationLease<'_>,
    model_package: &VerifiedManagedOllamaModelPackageLease,
    runtime_package: &mut RuntimePackageLease,
    observation_authority: &VerifiedManagedJudgeObservationAuthority,
    expected_evidence: &EffectivePackageEvidenceV2,
    released_evidence: Option<&EffectivePackageEvidenceV2>,
    artifact_set: &ArtifactSetManifest,
    runtime_build: &RuntimeBuildIdentity,
    runtime_state: &EffectiveRuntimeState,
) -> FinalizationFailures {
    let cleanup = managed_ollama.close(&CancellationToken::new()).err();
    let model = model_package.revalidate(&CancellationToken::new()).err();
    let runtime = runtime_package.revalidate(&CancellationToken::new()).err();
    let evidence = final_validation(
        observation_authority,
        expected_evidence,
        released_evidence,
        artifact_set,
        runtime_build,
        runtime_state,
    )
    .err();
    FinalizationFailures {
        cleanup,
        model,
        runtime,
        evidence,
    }
}

fn final_validation(
    observation_authority: &VerifiedManagedJudgeObservationAuthority,
    expected_evidence: &EffectivePackageEvidenceV2,
    released_evidence: Option<&EffectivePackageEvidenceV2>,
    artifact_set: &ArtifactSetManifest,
    runtime_build: &RuntimeBuildIdentity,
    runtime_state: &EffectiveRuntimeState,
) -> Result<(), ManagedJudgeEffectivePackageFinalValidationError> {
    observation_authority
        .revalidate_retained_bindings(&CancellationToken::new())
        .map_err(ManagedJudgeEffectivePackageFinalValidationError::Authority)?;
    expected_evidence
        .validate_against(artifact_set, runtime_build, runtime_state)
        .map_err(ManagedJudgeEffectivePackageFinalValidationError::Evidence)?;
    if released_evidence.is_some_and(|evidence| evidence != expected_evidence) {
        return Err(ManagedJudgeEffectivePackageFinalValidationError::EvidenceChanged);
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn run_finalizers<C, M, R, E>(
    cleanup: impl FnOnce() -> Result<(), C>,
    model: impl FnOnce() -> Result<(), M>,
    runtime: impl FnOnce() -> Result<(), R>,
    evidence: impl FnOnce() -> Result<(), E>,
) -> (Option<C>, Option<M>, Option<R>, Option<E>) {
    (
        cleanup().err(),
        model().err(),
        runtime().err(),
        evidence().err(),
    )
}
