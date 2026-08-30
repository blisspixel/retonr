use rewrite_model::{
    CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1, GenerationSystemRecordV1Relations,
    RuntimeBuildIdentity, RuntimeBuildMode,
};

use super::{
    ManagedJudgePrecursorCompilationError, ManagedJudgePrecursorCompilationInput,
    ManagedJudgePrecursorRelationship,
};

pub(super) fn validate(
    input: &ManagedJudgePrecursorCompilationInput<'_, '_, '_, '_>,
) -> Result<(), ManagedJudgePrecursorCompilationError> {
    validate_portable(input)?;
    if !runtime_matches(input) {
        return Err(relationship(ManagedJudgePrecursorRelationship::Runtime));
    }
    if input.prepared_isolation.policy_digest()
        != *input.expected_runtime_state.isolation_policy_digest()
    {
        return Err(relationship(ManagedJudgePrecursorRelationship::Isolation));
    }
    if !model_matches(input) {
        return Err(relationship(ManagedJudgePrecursorRelationship::Model));
    }
    if !policy_matches(input) {
        return Err(relationship(
            ManagedJudgePrecursorRelationship::GenerationPolicy,
        ));
    }
    Ok(())
}

fn validate_portable(
    input: &ManagedJudgePrecursorCompilationInput<'_, '_, '_, '_>,
) -> Result<(), ManagedJudgePrecursorCompilationError> {
    if input.judge_plan.judge_generation_system_id() != input.judge_system.generation_system_id()
        || input.judge_plan.prompt_contract_digest() != input.judge_system.prompt_digest()
        || input.judge_plan.output_schema_digest() != input.judge_system.output_schema_digest()
    {
        return Err(relationship(ManagedJudgePrecursorRelationship::JudgePlan));
    }
    let schedule = CandidateJudgeScheduleV1::new(
        input.judge_plan,
        input.judge_schedule.candidate_receipt_pair_set_id(),
    )
    .map_err(ManagedJudgePrecursorCompilationError::PortableContract)?;
    if &schedule != input.judge_schedule {
        return Err(relationship(
            ManagedJudgePrecursorRelationship::JudgeSchedule,
        ));
    }
    let requests = CandidateJudgeRequestAggregateV1::new(
        input.judge_plan,
        input.judge_schedule,
        input
            .request_aggregate
            .structured_request_binding_ids()
            .to_vec(),
    )
    .map_err(ManagedJudgePrecursorCompilationError::PortableContract)?;
    if &requests != input.request_aggregate {
        return Err(relationship(
            ManagedJudgePrecursorRelationship::RequestAggregate,
        ));
    }
    Ok(())
}

fn runtime_matches(input: &ManagedJudgePrecursorCompilationInput<'_, '_, '_, '_>) -> bool {
    let package_id = input.runtime_manifest.runtime_package_manifest_id();
    let Ok(expected_build) = RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        input.runtime_manifest,
    ) else {
        return false;
    };
    expected_build == *input.runtime_build
        && input
            .runtime_package
            .evidence()
            .runtime_package_manifest_id()
            == &package_id
        && input.runtime_build.package_manifest_digest() == package_id.digest()
        && input.expected_runtime_state.runtime_build_id()
            == &input.runtime_build.runtime_build_id()
        && input.admitted_runtime.runtime_package_manifest_id() == &package_id
        && input.generation_path.runtime_package_manifest_id() == &package_id
        && input.frozen_components.runtime_package_manifest_id() == &package_id
        && input.generation_path.matches_runtime(
            input.admitted_runtime,
            input.runtime_manifest,
            input.generation_path.runtime_version(),
            input.frozen_components.frozen_set_id(),
        )
        && input.admitted_runtime.runtime_admission_join_id()
            == *input.judge_system.runtime_admission_join_id()
        && input.generation_path.managed_generation_path_id()
            == *input.judge_system.managed_generation_path_id()
        && input.frozen_components.frozen_external_component_set_id()
            == *input.judge_system.frozen_external_component_set_id()
        && input.judge_system.runtime_package_manifest_id() == &package_id
        && input.judge_system.runtime_build_id() == &input.runtime_build.runtime_build_id()
        && input.judge_system.effective_runtime_state_id()
            == &input.expected_runtime_state.effective_runtime_state_id()
}

fn model_matches(input: &ManagedJudgePrecursorCompilationInput<'_, '_, '_, '_>) -> bool {
    let model_set = input.launch_plan.model_artifact_set_manifest();
    let model_package = input.launch_plan.model_package_manifest();
    let system_relations = GenerationSystemRecordV1Relations {
        runtime_package_manifest: input.runtime_manifest,
        runtime_build: input.runtime_build,
        effective_runtime_state: input.expected_runtime_state,
        model_artifact_set: model_set,
        model_package_manifest: model_package,
        effective_package_evidence_v2: input.characterized_package.evidence(),
    };
    input.judge_system.model_artifact_id() == input.launch_plan.model_target().artifact_id()
        && input.judge_system.static_model_binding_digest() == input.static_model.binding_digest()
        && input.characterized_package.foundation_id() == input.launch_plan.foundation_id()
        && input.characterized_package.license_control_id()
            == input.launch_plan.model_license_control_id()
        && input
            .static_model
            .validate_against(&input.launch_plan)
            .is_ok()
        && input
            .characterized_package
            .evidence()
            .validate_against(model_set, input.runtime_build, input.expected_runtime_state)
            .is_ok()
        && input
            .judge_system
            .validate_against(system_relations)
            .is_ok()
}

fn policy_matches(input: &ManagedJudgePrecursorCompilationInput<'_, '_, '_, '_>) -> bool {
    let policy = input.generation_policy;
    let bindings = policy.bindings();
    let system = input.judge_system;
    policy.permission() == crate::GenerationSystemPolicyPermission::ConstructGenerationSystem
        && policy.purpose() == crate::GenerationSystemPolicyPurpose::ManagedJudgeGeneration
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

const fn relationship(
    value: ManagedJudgePrecursorRelationship,
) -> ManagedJudgePrecursorCompilationError {
    ManagedJudgePrecursorCompilationError::Relationship(value)
}
