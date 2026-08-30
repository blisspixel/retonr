use super::GenerationSystemRecordV1Relations;
use crate::generation_qualification::GenerationQualificationContractError;
use crate::{
    ArtifactId, EffectivePackageMemberPurpose, EffectivePackageMemberUseV2, ModelPackageMemberRole,
};

pub(super) fn validate_relation_inputs(
    relations: GenerationSystemRecordV1Relations<'_>,
    selected_model_artifact: &ArtifactId,
) -> Result<(), GenerationQualificationContractError> {
    if relations.runtime_build.package_manifest_digest()
        != relations
            .runtime_package_manifest
            .runtime_package_manifest_id()
            .digest()
    {
        return Err(GenerationQualificationContractError::RuntimePackageMismatch);
    }
    if relations.effective_runtime_state.runtime_build_id()
        != &relations.runtime_build.runtime_build_id()
    {
        return Err(GenerationQualificationContractError::RuntimeBuildMismatch);
    }
    if relations
        .model_package_manifest
        .validate_against(relations.model_artifact_set)
        .is_err()
    {
        return Err(GenerationQualificationContractError::ModelPackageMismatch);
    }
    relations
        .effective_package_evidence_v2
        .validate_against(
            relations.model_artifact_set,
            relations.runtime_build,
            relations.effective_runtime_state,
        )
        .map_err(|_| GenerationQualificationContractError::EffectivePackageMismatch)?;
    if !selected_model_is_effective(relations, selected_model_artifact) {
        return Err(GenerationQualificationContractError::ModelArtifactMismatch);
    }
    Ok(())
}

fn selected_model_is_effective(
    relations: GenerationSystemRecordV1Relations<'_>,
    selected: &ArtifactId,
) -> bool {
    relations
        .model_package_manifest
        .members()
        .iter()
        .filter(|member| {
            member.artifact_id() == selected
                && member.roles().iter().any(|role| {
                    matches!(
                        role,
                        ModelPackageMemberRole::ModelWeights
                            | ModelPackageMemberRole::ModelWeightShard
                    )
                })
        })
        .any(|member| {
            relations
                .effective_package_evidence_v2
                .member_evidence()
                .iter()
                .any(|evidence| {
                    evidence.relative_path() == member.relative_path()
                        && evidence.artifact_id() == selected
                        && effective_model_weight(evidence.member_use())
                })
        })
}

fn effective_model_weight(member_use: &EffectivePackageMemberUseV2) -> bool {
    match member_use {
        EffectivePackageMemberUseV2::Effective { purposes }
        | EffectivePackageMemberUseV2::EffectiveAndEvidence { purposes, .. } => {
            purposes.contains(&EffectivePackageMemberPurpose::ModelWeights)
        }
        EffectivePackageMemberUseV2::EvidenceOnly { .. }
        | EffectivePackageMemberUseV2::Excluded { .. } => false,
    }
}
