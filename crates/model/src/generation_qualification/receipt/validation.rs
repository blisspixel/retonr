use super::CandidateGenerationReceiptV1Relations;
use crate::generation_qualification::{
    CandidateGenerationEvidenceBundlePublicationModeV1,
    CandidateGenerationEvidenceBundleReadbackStatusV1,
    CandidateGenerationPackageRevalidationStatusV1, CandidateGenerationProcessCleanupStatusV1,
    GenerationQualificationContractError,
};

pub(super) fn validate_receipt_relations(
    relations: CandidateGenerationReceiptV1Relations<'_>,
) -> Result<(), GenerationQualificationContractError> {
    let planned = relations.planned_attempt;
    let precursor = relations.precursor;
    let system = relations.generation_system;
    if relations.qualification_plan.suite_manifest_id() != relations.suite.suite_manifest_id()
        || planned.suite_manifest_id() != relations.suite.suite_manifest_id()
        || planned.case_id() != relations.case.case_id()
        || planned.cluster_id() != relations.cluster.cluster_id()
        || relations.case.cluster_id() != relations.cluster.cluster_id()
        || planned.repetition_id() != relations.repetition.repetition_id()
        || relations.repetition.suite_manifest_id() != relations.suite.suite_manifest_id()
        || planned.generation_system_id() != system.generation_system_id()
        || precursor.qualification_plan_id() != relations.qualification_plan.qualification_plan_id()
        || precursor.planned_attempt_id() != planned.planned_attempt_id()
    {
        return Err(GenerationQualificationContractError::ReceiptRelationshipMismatch);
    }
    validate_system_bindings(relations)?;
    validate_later_bindings(relations)
}

fn validate_system_bindings(
    relations: CandidateGenerationReceiptV1Relations<'_>,
) -> Result<(), GenerationQualificationContractError> {
    let precursor = relations.precursor;
    let system = relations.generation_system;
    if precursor.runtime_admission_join_id() != system.runtime_admission_join_id()
        || precursor.managed_generation_path_id() != system.managed_generation_path_id()
        || precursor.frozen_external_component_set_id() != system.frozen_external_component_set_id()
        || precursor.runtime_package_manifest_id() != system.runtime_package_manifest_id()
        || precursor.runtime_build_id() != system.runtime_build_id()
        || precursor.expected_effective_runtime_state_id() != system.effective_runtime_state_id()
        || precursor.model_artifact_set_id() != system.model_artifact_set_id()
        || precursor.model_package_manifest_id() != system.model_package_manifest_id()
        || precursor.model_artifact_id() != system.model_artifact_id()
        || precursor.effective_package_evidence_v2_id() != system.effective_package_evidence_v2_id()
        || precursor.static_model_binding_digest() != system.static_model_binding_digest()
    {
        return Err(GenerationQualificationContractError::ReceiptRelationshipMismatch);
    }
    Ok(())
}

fn validate_later_bindings(
    relations: CandidateGenerationReceiptV1Relations<'_>,
) -> Result<(), GenerationQualificationContractError> {
    let managed = relations.managed_evidence;
    let cleanup = relations.cleanup;
    let bundle = relations.bundle;
    let readback = relations.readback;
    if managed.precursor_id() != relations.precursor.precursor_id()
        || managed.generation_request_binding_id()
            != relations.planned_attempt.generation_request_binding_id()
        || managed.structured_request_binding_id()
            != relations.precursor.structured_request_binding_id()
        || managed.effective_runtime_state_id()
            != relations.precursor.expected_effective_runtime_state_id()
        || managed.effective_package_evidence_v2_id()
            != relations.precursor.effective_package_evidence_v2_id()
        || cleanup.precursor_id() != relations.precursor.precursor_id()
        || cleanup.managed_evidence_id() != managed.managed_evidence_v2_id()
        || bundle.precursor_id() != relations.precursor.precursor_id()
        || bundle.managed_evidence_id() != managed.managed_evidence_v2_id()
        || bundle.response_id() != managed.response_id()
        || bundle.cleanup_id() != cleanup.cleanup_id()
        || readback.bundle_id() != bundle.evidence_bundle_id()
        || readback.entry_count() != bundle.entry_count()
        || readback.aggregate_byte_count() != bundle.aggregate_byte_count()
        || readback.status() != CandidateGenerationEvidenceBundleReadbackStatusV1::Verified
        || readback.publication_mode()
            != CandidateGenerationEvidenceBundlePublicationModeV1::CreateNewNoReplace
    {
        return Err(GenerationQualificationContractError::ReceiptRelationshipMismatch);
    }
    if cleanup.process_cleanup_status() != CandidateGenerationProcessCleanupStatusV1::Succeeded
        || cleanup.runtime_package_revalidation_status()
            != CandidateGenerationPackageRevalidationStatusV1::Verified
        || cleanup.model_package_revalidation_status()
            != CandidateGenerationPackageRevalidationStatusV1::Verified
        || !cleanup.failure_categories().is_empty()
    {
        return Err(GenerationQualificationContractError::InvalidReceiptPostconditions);
    }
    if bundle.candidate_artifacts().len()
        != usize::from(
            relations
                .planned_attempt
                .output_ceilings()
                .candidate_count(),
        )
        || bundle
            .candidate_artifacts()
            .iter()
            .enumerate()
            .any(|(ordinal, candidate)| usize::from(candidate.ordinal()) != ordinal)
    {
        return Err(GenerationQualificationContractError::ReceiptRelationshipMismatch);
    }
    Ok(())
}
