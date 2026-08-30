use rewrite_model::GenerationResourceObservationProfileV1;

use super::VerifiedGenerationQualificationResourceAttemptObservation;

pub(super) fn assert_complete_getter_surface(
    observed: &VerifiedGenerationQualificationResourceAttemptObservation,
) {
    assert_eq!(observed.resource_policy_digest(), &observed.policy_digest);
    assert_eq!(
        observed.operation_policy_id(),
        &observed.operation_policy_id
    );
    assert_eq!(
        observed.qualification_plan_id(),
        &observed.qualification_plan_id
    );
    assert_eq!(observed.suite_manifest_id(), &observed.suite_manifest_id);
    assert_eq!(
        observed.target_generation_system_id(),
        &observed.target_generation_system_id
    );
    assert_eq!(
        observed.request_binding_digest(),
        &observed.request_binding_digest
    );
    assert_eq!(observed.response_id(), &observed.response_id);
    assert_eq!(observed.response_ordinal(), observed.response_ordinal);
    assert_eq!(
        observed.receipt_binding_digest(),
        &observed.receipt_binding_digest
    );
    assert_eq!(
        observed.worker_evidence_digest(),
        &observed.worker_evidence_digest
    );
    assert_eq!(
        observed.worker_observation_digest(),
        &observed.worker_observation_digest
    );
    assert_eq!(
        observed.effective_package_id(),
        &observed.effective_package_id
    );
    assert_eq!(
        observed.runtime_package_manifest_id(),
        &observed.runtime_package_manifest_id
    );
    assert_eq!(
        observed.model_package_manifest_id(),
        &observed.model_package_manifest_id
    );
    assert_eq!(
        observed.observation_profile(),
        GenerationResourceObservationProfileV1::ManagedOllamaV0_32_15LinuxV1
    );
    assert_eq!(
        observed.total_duration_nanoseconds(),
        observed.values.total_duration_nanoseconds
    );
    assert_eq!(
        observed.load_duration_nanoseconds(),
        observed.values.load_duration_nanoseconds
    );
    assert_eq!(
        observed.prompt_evaluation_duration_nanoseconds(),
        observed.values.prompt_evaluation_duration_nanoseconds
    );
    assert_eq!(
        observed.evaluation_duration_nanoseconds(),
        observed.values.evaluation_duration_nanoseconds
    );
    assert_eq!(
        observed.attempt_elapsed_nanoseconds(),
        observed.values.attempt_elapsed_nanoseconds
    );
    assert_eq!(
        observed.first_response_elapsed_nanoseconds(),
        observed.values.first_response_elapsed_nanoseconds
    );
    assert_eq!(
        observed.cleanup_elapsed_nanoseconds(),
        observed.values.cleanup_elapsed_nanoseconds
    );
}
