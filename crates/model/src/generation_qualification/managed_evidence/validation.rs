use super::ManagedOllamaCandidateGenerationEvidenceV2Relations;
use crate::generation_qualification::GenerationQualificationContractError;

pub(super) fn validate_relations(
    relations: ManagedOllamaCandidateGenerationEvidenceV2Relations<'_>,
) -> Result<(), GenerationQualificationContractError> {
    let precursor = relations.precursor;
    let planned = relations.planned_attempt;
    let system = relations.generation_system;
    let package = relations.effective_package_evidence_v2;

    let selected_records_match = precursor.planned_attempt_id() == planned.planned_attempt_id()
        && planned.generation_system_id() == system.generation_system_id();
    let precursor_system_matches = precursor.runtime_admission_join_id()
        == system.runtime_admission_join_id()
        && precursor.managed_generation_path_id() == system.managed_generation_path_id()
        && precursor.frozen_external_component_set_id()
            == system.frozen_external_component_set_id()
        && precursor.runtime_package_manifest_id() == system.runtime_package_manifest_id()
        && precursor.runtime_build_id() == system.runtime_build_id()
        && precursor.expected_effective_runtime_state_id() == system.effective_runtime_state_id()
        && precursor.model_artifact_set_id() == system.model_artifact_set_id()
        && precursor.model_package_manifest_id() == system.model_package_manifest_id()
        && precursor.model_artifact_id() == system.model_artifact_id()
        && precursor.effective_package_evidence_v2_id()
            == system.effective_package_evidence_v2_id()
        && precursor.static_model_binding_digest() == system.static_model_binding_digest();
    let package_matches = package.effective_package_evidence_v2_id()
        == *precursor.effective_package_evidence_v2_id()
        && package.artifact_set_id() == precursor.model_artifact_set_id()
        && package.runtime_build_id() == precursor.runtime_build_id()
        && package.effective_runtime_state_id() == precursor.expected_effective_runtime_state_id();
    if selected_records_match && precursor_system_matches && package_matches {
        Ok(())
    } else {
        Err(GenerationQualificationContractError::ManagedEvidenceRelationshipMismatch)
    }
}
