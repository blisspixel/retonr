use rewrite_inference::{
    CandidateOutputPolicy, StructuredCompletionRequest, candidate_output_contract,
};
use rewrite_model::{
    ArtifactSetManifest, GenerationSystemRecordV1Relations, ModelPackageManifest,
    RuntimeBuildIdentity, RuntimeBuildMode, RuntimePackageManifestId,
};

use super::{CandidateAttemptPrecursorCompilationError, CandidateAttemptPrecursorCompilationInput};

pub(super) fn output_policy(
    ceilings: rewrite_model::CandidateOutputCeilingsV1,
) -> Result<CandidateOutputPolicy, CandidateAttemptPrecursorCompilationError> {
    let policy = CandidateOutputPolicy::new(
        ceilings.candidate_count(),
        ceilings.maximum_candidate_bytes(),
        ceilings.maximum_aggregate_candidate_bytes(),
    )
    .map_err(|_error| CandidateAttemptPrecursorCompilationError::RelationshipMismatch)?;
    if policy.maximum_envelope_bytes() != ceilings.maximum_envelope_bytes() {
        return Err(CandidateAttemptPrecursorCompilationError::RelationshipMismatch);
    }
    Ok(policy)
}

pub(super) fn validate(
    input: &CandidateAttemptPrecursorCompilationInput<'_, '_, '_, '_>,
    structured: &StructuredCompletionRequest,
) -> Result<(), CandidateAttemptPrecursorCompilationError> {
    let model_set = input.launch_plan.model_artifact_set_manifest();
    let model_package = input.launch_plan.model_package_manifest();
    let runtime_package_id = input.runtime_manifest.runtime_package_manifest_id();
    let expected_build = RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        input.runtime_manifest,
    )
    .map_err(|_error| CandidateAttemptPrecursorCompilationError::RelationshipMismatch)?;

    if runtime_matches(input, &runtime_package_id, &expected_build)
        && model_records_match(input, model_set, model_package)
        && policy_matches(input)
        && request_matches(input, structured)
    {
        Ok(())
    } else {
        Err(CandidateAttemptPrecursorCompilationError::RelationshipMismatch)
    }
}

fn runtime_matches(
    input: &CandidateAttemptPrecursorCompilationInput<'_, '_, '_, '_>,
    package_id: &RuntimePackageManifestId,
    expected_build: &RuntimeBuildIdentity,
) -> bool {
    expected_build == input.runtime_build
        && input
            .runtime_package
            .evidence()
            .runtime_package_manifest_id()
            == package_id
        && input.runtime_build.package_manifest_digest() == package_id.digest()
        && input.expected_runtime_state.runtime_build_id()
            == &input.runtime_build.runtime_build_id()
        && input.admitted_runtime.runtime_package_manifest_id() == package_id
        && input.generation_path.runtime_package_manifest_id() == package_id
        && input.frozen_components.runtime_package_manifest_id() == package_id
        && input.generation_path.matches_runtime(
            input.admitted_runtime,
            input.runtime_manifest,
            input.generation_path.runtime_version(),
            input.frozen_components.frozen_set_id(),
        )
}

fn model_records_match(
    input: &CandidateAttemptPrecursorCompilationInput<'_, '_, '_, '_>,
    model_set: &ArtifactSetManifest,
    model_package: &ModelPackageManifest,
) -> bool {
    let system = input.generation_system;
    let characterized = input.characterized_package;
    let relations = GenerationSystemRecordV1Relations {
        runtime_package_manifest: input.runtime_manifest,
        runtime_build: input.runtime_build,
        effective_runtime_state: input.expected_runtime_state,
        model_artifact_set: model_set,
        model_package_manifest: model_package,
        effective_package_evidence_v2: characterized.evidence(),
    };
    input.admitted_runtime.runtime_admission_join_id() == *system.runtime_admission_join_id()
        && input.generation_path.managed_generation_path_id()
            == *system.managed_generation_path_id()
        && input.frozen_components.frozen_external_component_set_id()
            == *system.frozen_external_component_set_id()
        && system.model_artifact_id() == input.launch_plan.model_target().artifact_id()
        && system.static_model_binding_digest() == input.static_model.binding_digest()
        && characterized.foundation_id() == input.launch_plan.foundation_id()
        && characterized.license_control_id() == input.launch_plan.model_license_control_id()
        && input
            .static_model
            .validate_against(&input.launch_plan)
            .is_ok()
        && characterized
            .evidence()
            .validate_against(model_set, input.runtime_build, input.expected_runtime_state)
            .is_ok()
        && system.validate_against(relations).is_ok()
}

fn policy_matches(input: &CandidateAttemptPrecursorCompilationInput<'_, '_, '_, '_>) -> bool {
    let policy = input.generation_policy;
    let bindings = policy.bindings();
    let system = input.generation_system;
    policy.permission() == crate::GenerationSystemPolicyPermission::ConstructGenerationSystem
        && policy.purpose() == crate::GenerationSystemPolicyPurpose::ManagedCandidateGeneration
        && bindings.strategy_digest() == system.strategy_digest()
        && bindings.planner_digest() == system.planner_digest()
        && bindings.validator_digest() == system.validator_digest()
        && bindings.adapter_digest() == system.adapter_digest()
        && bindings.prompt_digest() == system.prompt_digest()
        && bindings.output_schema_digest() == system.output_schema_digest()
        && bindings.request_policy_digest() == system.request_policy_digest()
        && bindings.language_digest() == system.language_digest()
        && bindings.mode_digest() == system.mode_digest()
        && bindings.format_digest() == system.format_digest()
        && bindings.operating_system_digest() == system.operating_system_digest()
        && bindings.architecture_digest() == system.architecture_digest()
        && bindings.execution_class_digest() == system.execution_class_digest()
        && bindings.hardware_envelope_digest() == system.hardware_envelope_digest()
}

fn request_matches(
    input: &CandidateAttemptPrecursorCompilationInput<'_, '_, '_, '_>,
    structured: &StructuredCompletionRequest,
) -> bool {
    let request = &input.generation_request;
    let attempt = input.planned_attempt;
    let system = input.generation_system;
    let ceilings = input.output_ceilings;
    let limits = input.qualification_plan.limits();
    let input_bytes = u64::try_from(request.input.len()).unwrap_or(u64::MAX);
    request.artifact_id == *system.model_artifact_id()
        && request.artifact_id == *input.launch_plan.model_target().artifact_id()
        && request.artifact_digest == *request.artifact_id.digest()
        && request.sampling.seed == Some(attempt.declared_seed())
        && request.generation_request_binding_id() == *attempt.generation_request_binding_id()
        && request.output == candidate_output_contract()
        && request.output.schema_digest == *attempt.candidate_output_contract_digest()
        && request.output.schema_digest == *system.output_schema_digest()
        && request.source_byte_count == attempt.source_byte_count()
        && attempt.source_artifact_id().digest() == attempt.source_digest()
        && request.candidate_count == ceilings.candidate_count()
        && request.candidate_byte_limit == ceilings.maximum_candidate_bytes()
        && structured.output_byte_limit == ceilings.maximum_envelope_bytes()
        && ceilings == attempt.output_ceilings()
        && ceilings.candidate_count() <= limits.maximum_candidates_per_completion()
        && request.source_byte_count <= limits.maximum_retained_input_bytes()
        && input_bytes <= limits.maximum_retained_input_bytes()
}
