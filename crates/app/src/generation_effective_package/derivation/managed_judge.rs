use rewrite_model::{
    EffectivePackageEvidenceMode, EffectivePackageEvidenceV2, EffectiveRuntimeState,
    RuntimeBuildIdentity, RuntimePackageManifest,
};
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;
use rewrite_runtime_isolation::PreparedIsolation;
use rewrite_types::CancellationToken;

use crate::{
    ModelLicenseControlId, VerifiedAdmittedRuntime, VerifiedManagedGenerationPath,
    VerifiedManagedOllamaModelPackageLease,
};

use super::{
    DerivedEvidence, acquisition_digest, completeness_digest, derive_member_evidence,
    isolation_exclusion_digest, license_digest, runtime_closure_digest, transformation,
};
use crate::generation_effective_package::{
    GenerationEffectivePackageDerivationError, MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_ID,
    MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_VERSION,
};

pub(crate) struct ManagedJudgeBatchInputs<'a, 'model> {
    pub(crate) runtime_manifest: &'a RuntimePackageManifest,
    pub(crate) model_package: &'model VerifiedManagedOllamaModelPackageLease,
    pub(crate) runtime_build: &'a RuntimeBuildIdentity,
    pub(crate) effective_state: &'a EffectiveRuntimeState,
    pub(crate) admitted_runtime: &'a VerifiedAdmittedRuntime,
    pub(crate) generation_path: &'a VerifiedManagedGenerationPath,
    pub(crate) frozen_components: &'a VerifiedFrozenExternalNativeComponentSet,
    pub(crate) prepared_isolation: &'a PreparedIsolation,
    pub(crate) license_control_id: &'a ModelLicenseControlId,
}

pub(crate) fn derive_managed_judge_batch(
    input: &ManagedJudgeBatchInputs<'_, '_>,
    cancellation: &CancellationToken,
) -> Result<DerivedEvidence, GenerationEffectivePackageDerivationError> {
    ensure_active(cancellation)?;
    let view = input.model_package.private_view();
    let artifact_set = view.artifact_set_manifest().clone();
    let model_manifest = view.model_package_manifest();
    let foundation = view.foundation_evidence();
    let member_evidence = derive_member_evidence(
        model_manifest,
        foundation.provenance_manifest().relative_path(),
    )?;
    let evidence = EffectivePackageEvidenceV2::new(
        &artifact_set,
        input.runtime_build,
        input.effective_state,
        rewrite_model::EffectivePackageEvidenceV2Input {
            evidence_mode: EffectivePackageEvidenceMode::ManagedImmutablePackage,
            evidence_contract_id: MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_ID.to_owned(),
            evidence_contract_schema_version: MANAGED_OLLAMA_EFFECTIVE_PACKAGE_V2_CONTRACT_VERSION,
            member_evidence,
            artifact_set_completeness_evidence_digest: completeness_digest(input.model_package),
            acquisition_evidence_digest: acquisition_digest(input.model_package),
            license_review_evidence_digest: license_digest(input.license_control_id),
            transformation: transformation(model_manifest.transformation()),
            runtime_load_closure_evidence_digest: runtime_closure_digest(
                input.runtime_manifest,
                input.generation_path,
                input.frozen_components,
                input.effective_state.loaded_components_digest(),
            ),
            exclusion_isolation_evidence_digest: isolation_exclusion_digest(
                input.runtime_manifest,
                input.prepared_isolation,
                input.admitted_runtime,
            ),
        },
    )
    .map_err(GenerationEffectivePackageDerivationError::Evidence)?;
    ensure_active(cancellation)?;
    Ok(DerivedEvidence {
        artifact_set,
        evidence,
    })
}

fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), GenerationEffectivePackageDerivationError> {
    if cancellation.is_cancelled() {
        Err(GenerationEffectivePackageDerivationError::Cancelled)
    } else {
        Ok(())
    }
}
