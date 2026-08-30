use rewrite_model::{
    ArtifactId, ArtifactSetId, ComputeBackend, EffectiveRuntimeState, EffectiveRuntimeStateInput,
    ExecutionPlacement, ModelPackageManifestId, RuntimeBuildId, RuntimeBuildIdentity,
};
use rewrite_types::Digest;

use super::{
    MANAGED_OLLAMA_PROVIDER_SNAPSHOT_CONTRACT, ManagedOllamaEffectivePackageSubject,
    ManagedOllamaEffectiveRuntimeState, ManagedOllamaManagedJudgeSubject,
};
use crate::effective_runtime_state_observation::{
    EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION, EffectiveRuntimeStateObservationError,
};

const LAUNCH_POLICY_DOMAIN: &[u8] = b"ollama/v0.32.15/live-launch-policy/v1";
const EFFECTIVE_CONFIGURATION_DOMAIN: &[u8] =
    b"ollama/v0.32.15/complete-effective-configuration/v1";
const LIVE_JOIN_DOMAIN: &[u8] = b"ollama/v0.32.15/live-effective-runtime-state-join/v1";
const PORTABLE_LOADED_COMPONENTS_DOMAIN: &[u8] =
    b"ollama/v0.32.15/portable-observed-loaded-components/v1";

#[derive(Clone)]
pub(super) struct RuntimeBindings {
    pub(super) runtime_build_id: RuntimeBuildId,
    pub(super) provider_runtime_build_id: RuntimeBuildId,
    pub(super) wire_runtime_build_id: RuntimeBuildId,
    pub(super) platform_runtime_build_id: RuntimeBuildId,
    pub(super) runtime_package_digest: Digest,
    pub(super) runtime_installation_generation: u64,
    pub(super) lease_runtime_package_digest: Digest,
    pub(super) runtime_entrypoint_digest: Digest,
    pub(super) packaged_dependencies_digest: Digest,
    pub(super) provider_runtime_package_digest: Digest,
    pub(super) admitted_runtime_package_digest: Digest,
    pub(super) path_runtime_package_digest: Digest,
    pub(super) admitted_frozen_component_set_digest: Digest,
    pub(super) path_frozen_component_set_digest: Digest,
    pub(super) admitted_runtime_id: Digest,
    pub(super) path_admitted_runtime_id: Digest,
    pub(super) generation_path_id: Digest,
    pub(super) admitted_plain_launch_digest: Digest,
    pub(super) retained_plain_launch_digest: Digest,
}

#[derive(Clone)]
pub(super) struct ModelBindings {
    pub(super) input_artifact_set_id: ArtifactSetId,
    pub(super) provider_artifact_set_id: ArtifactSetId,
    pub(super) input_model_package_id: ModelPackageManifestId,
    pub(super) provider_model_package_id: ModelPackageManifestId,
    pub(super) input_installation_generation: u64,
    pub(super) provider_installation_generation: u64,
    pub(super) input_model_artifact_id: ArtifactId,
    pub(super) provider_model_artifact_id: ArtifactId,
    pub(super) wire_model_artifact_id: ArtifactId,
    pub(super) cpu_model_artifact_id: ArtifactId,
    pub(super) worker_model_artifact_id: ArtifactId,
    pub(super) mapping_model_artifact_id: ArtifactId,
    pub(super) model_target_artifact_id: ArtifactId,
    pub(super) input_model_target_digest: Digest,
    pub(super) retained_model_target_digest: Digest,
}

#[derive(Clone)]
pub(super) struct BracketBindings {
    pub(super) request_digest: Digest,
    pub(super) receipt_request_digest: Digest,
    pub(super) provider_request_digest: Digest,
    pub(super) wire_request_digest: Digest,
    pub(super) receipt_response_digest: Digest,
    pub(super) provider_response_digest: Digest,
    pub(super) wire_response_digest: Digest,
    pub(super) receipt_complete_binding_digest: Digest,
    pub(super) receipt_preflight_digest: Digest,
    pub(super) receipt_first_response_ordinal: u64,
    pub(super) receipt_last_response_ordinal: u64,
    pub(super) receipt_first_residency_ordinal: u64,
    pub(super) receipt_last_residency_ordinal: u64,
    pub(super) request_context_tokens: u32,
    pub(super) receipt_context_tokens: u32,
    pub(super) wire_context_tokens: u32,
    pub(super) cpu_context_tokens: u32,
    pub(super) input_layout_digest: Digest,
    pub(super) isolation_layout_digest: Digest,
    pub(super) input_member_count: u32,
    pub(super) isolation_member_count: u32,
    pub(super) input_total_bytes: u64,
    pub(super) isolation_total_bytes: u64,
    pub(super) retained_input_bound_launch_digest: Digest,
    pub(super) observed_input_bound_launch_digest: Digest,
    pub(super) input_mapping_digest: Digest,
}

#[derive(Clone)]
pub(super) struct ExecutionBindings {
    pub(super) initial_isolation_digest: Digest,
    pub(super) final_isolation_digest: Digest,
    pub(super) cpu_isolation_digest: Digest,
    pub(super) initial_worker_digest: Digest,
    pub(super) final_worker_digest: Digest,
    pub(super) cpu_worker_digest: Digest,
    pub(super) reviewed_worker_artifact_id: ArtifactId,
    pub(super) initial_worker_artifact_id: ArtifactId,
    pub(super) final_worker_artifact_id: ArtifactId,
    pub(super) initial_worker_runtime_package_digest: Digest,
    pub(super) final_worker_runtime_package_digest: Digest,
    pub(super) worker_portable_configuration_digest: Digest,
    pub(super) final_worker_portable_configuration_digest: Digest,
    pub(super) cpu_portable_configuration_digest: Digest,
    pub(super) worker_portable_closure_digest: Digest,
    pub(super) platform_portable_closure_digest: Digest,
    pub(super) cpu_portable_closure_digest: Digest,
    pub(super) server_portable_component_set_digest: Digest,
    pub(super) server_native_load_observation_digest: Digest,
    pub(super) server_process_evidence_digest: Digest,
    pub(super) server_native_process_evidence_digest: Digest,
    pub(super) server_entrypoint_digest: Digest,
    pub(super) server_process_profile_valid: bool,
    pub(super) server_native_load_runtime_package_digest: Digest,
    pub(super) server_native_component_count: u32,
    pub(super) worker_native_load_digest: Digest,
    pub(super) platform_native_load_digest: Digest,
    pub(super) cpu_native_load_digest: Digest,
    pub(super) model_mapping_digest: Digest,
    pub(super) cpu_model_mapping_digest: Digest,
    pub(super) residency_digest: Digest,
    pub(super) cpu_residency_digest: Digest,
    pub(super) wire_configuration_digest: Digest,
    pub(super) platform_digest: Digest,
    pub(super) platform_kernel_observation_digest: Digest,
    pub(super) execution_class_digest: Digest,
    pub(super) isolation_policy_digest: Digest,
    pub(super) worker_profile_valid: bool,
    pub(super) worker_native_component_count: u32,
    pub(super) model_mapping_region_count: u32,
    pub(super) runtime_reported_accelerator_bytes: u64,
    pub(super) isolation_canaries_valid: bool,
    pub(super) compute_backend: ComputeBackend,
    pub(super) placement: ExecutionPlacement,
}

#[derive(Clone)]
pub(super) struct LiveEffectiveStateFacts {
    pub(super) runtime: RuntimeBindings,
    pub(super) model: ModelBindings,
    pub(super) bracket: BracketBindings,
    pub(super) execution: ExecutionBindings,
    pub(super) leaf_schema_versions: [u32; 4],
    pub(super) provider_snapshot_digest: Digest,
    pub(super) provider_observation_binding_digest: Digest,
    pub(super) cpu_observation_binding_digest: Digest,
}

pub(super) fn build_effective_state(
    build: &RuntimeBuildIdentity,
    facts: &LiveEffectiveStateFacts,
) -> Result<ManagedOllamaEffectiveRuntimeState, EffectiveRuntimeStateObservationError> {
    validate(facts)?;
    if build.runtime_build_id() != facts.runtime.runtime_build_id
        || build.package_manifest_digest() != &facts.runtime.runtime_package_digest
        || build.entrypoint_digest() != &facts.runtime.runtime_entrypoint_digest
        || build.packaged_dependencies_digest() != &facts.runtime.packaged_dependencies_digest
    {
        return Err(EffectiveRuntimeStateObservationError::RelationshipMismatch);
    }
    let launch_policy_digest = digest(
        LAUNCH_POLICY_DOMAIN,
        &[&facts.runtime.retained_plain_launch_digest],
    );
    let effective_configuration_digest = digest(
        EFFECTIVE_CONFIGURATION_DOMAIN,
        &[
            &facts.execution.wire_configuration_digest,
            &facts.runtime.retained_plain_launch_digest,
            &facts.bracket.input_mapping_digest,
            &facts.model.retained_model_target_digest,
            &facts.execution.isolation_policy_digest,
            facts.model.input_model_package_id.digest(),
            facts.model.input_model_artifact_id.digest(),
        ],
    );
    let loaded_components_digest = digest(
        PORTABLE_LOADED_COMPONENTS_DOMAIN,
        &[
            &facts.execution.server_portable_component_set_digest,
            &facts.execution.worker_portable_closure_digest,
        ],
    );
    let state = EffectiveRuntimeState::new(
        build,
        EffectiveRuntimeStateInput {
            provider_snapshot_contract: MANAGED_OLLAMA_PROVIDER_SNAPSHOT_CONTRACT.to_owned(),
            provider_snapshot_schema_version: EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION,
            provider_snapshot_digest: facts.provider_snapshot_digest.clone(),
            launch_policy_digest,
            loaded_components_digest,
            effective_configuration_digest,
            platform_digest: facts.execution.platform_digest.clone(),
            execution_class_digest: facts.execution.execution_class_digest.clone(),
            isolation_policy_digest: facts.execution.isolation_policy_digest.clone(),
            effective_context_tokens: facts.bracket.receipt_context_tokens,
            compute_backend: facts.execution.compute_backend,
            placement: facts.execution.placement,
        },
    )
    .map_err(EffectiveRuntimeStateObservationError::InvalidEffectiveState)?;
    let state_id = state.effective_runtime_state_id();
    let binding_digest = digest(
        LIVE_JOIN_DOMAIN,
        &[
            state_id.digest(),
            &facts.provider_snapshot_digest,
            &facts.provider_observation_binding_digest,
            &facts.execution.wire_configuration_digest,
            &facts.execution.platform_digest,
            &facts.execution.platform_kernel_observation_digest,
            &facts.execution.execution_class_digest,
            &facts.cpu_observation_binding_digest,
            &facts.runtime.admitted_runtime_id,
            &facts.runtime.generation_path_id,
            &facts.runtime.admitted_frozen_component_set_digest,
            &facts.bracket.request_digest,
            &facts.bracket.receipt_response_digest,
            &facts.execution.residency_digest,
            &facts.execution.initial_isolation_digest,
            &facts.execution.server_process_evidence_digest,
            &facts.execution.initial_worker_digest,
            &facts.execution.server_native_load_observation_digest,
            &facts.execution.worker_native_load_digest,
            &facts.execution.model_mapping_digest,
            &number_digest(facts.model.input_installation_generation),
            &number_digest(facts.runtime.runtime_installation_generation),
            &facts.bracket.retained_input_bound_launch_digest,
            &facts.bracket.input_layout_digest,
        ],
    );
    Ok(ManagedOllamaEffectiveRuntimeState {
        state,
        binding_digest,
        managed_judge_subject: managed_judge_subject(facts),
        effective_package_subject: effective_package_subject(facts),
    })
}

fn effective_package_subject(
    facts: &LiveEffectiveStateFacts,
) -> ManagedOllamaEffectivePackageSubject {
    ManagedOllamaEffectivePackageSubject {
        admitted_runtime_id: facts.runtime.admitted_runtime_id.clone(),
        generation_path_id: facts.runtime.generation_path_id.clone(),
        runtime_installation_generation: facts.runtime.runtime_installation_generation,
        model_artifact_set_id: facts.model.input_artifact_set_id.clone(),
        model_package_manifest_id: facts.model.input_model_package_id.clone(),
        model_installation_generation: facts.model.input_installation_generation,
        input_bound_launch_spec_digest: facts.bracket.retained_input_bound_launch_digest.clone(),
        isolation_policy_digest: facts.execution.isolation_policy_digest.clone(),
        live_subject: None,
        runtime_package_identity: None,
        model_package_identity: None,
        retained_session_subject: None,
    }
}

fn managed_judge_subject(facts: &LiveEffectiveStateFacts) -> ManagedOllamaManagedJudgeSubject {
    ManagedOllamaManagedJudgeSubject {
        server_process: facts.execution.server_process_evidence_digest.clone(),
        server_native_load: facts
            .execution
            .server_native_load_observation_digest
            .clone(),
        initial_worker: facts.execution.initial_worker_digest.clone(),
        final_worker: facts.execution.final_worker_digest.clone(),
        worker_native_load: facts.execution.worker_native_load_digest.clone(),
        model_mapping: facts.execution.model_mapping_digest.clone(),
        request: facts.bracket.request_digest.clone(),
        response: facts.bracket.receipt_response_digest.clone(),
        receipt: facts.bracket.receipt_complete_binding_digest.clone(),
        preflight: facts.bracket.receipt_preflight_digest.clone(),
        first_response_ordinal: facts.bracket.receipt_first_response_ordinal,
        last_response_ordinal: facts.bracket.receipt_last_response_ordinal,
        first_residency_ordinal: facts.bracket.receipt_first_residency_ordinal,
        last_residency_ordinal: facts.bracket.receipt_last_residency_ordinal,
    }
}

fn validate(facts: &LiveEffectiveStateFacts) -> Result<(), EffectiveRuntimeStateObservationError> {
    let runtime = &facts.runtime;
    let model = &facts.model;
    let bracket = &facts.bracket;
    let execution = &facts.execution;
    let runtime_valid = runtime.runtime_build_id == runtime.provider_runtime_build_id
        && runtime.runtime_build_id == runtime.wire_runtime_build_id
        && runtime.runtime_build_id == runtime.platform_runtime_build_id
        && runtime.runtime_package_digest == runtime.provider_runtime_package_digest
        && runtime.runtime_package_digest == runtime.lease_runtime_package_digest
        && runtime.runtime_package_digest == runtime.admitted_runtime_package_digest
        && runtime.runtime_package_digest == runtime.path_runtime_package_digest
        && runtime.runtime_installation_generation > 0
        && runtime.admitted_frozen_component_set_digest == runtime.path_frozen_component_set_digest
        && runtime.admitted_runtime_id == runtime.path_admitted_runtime_id
        && runtime.admitted_plain_launch_digest == runtime.retained_plain_launch_digest;
    let model_valid = model.input_artifact_set_id == model.provider_artifact_set_id
        && model.input_model_package_id == model.provider_model_package_id
        && model.input_installation_generation > 0
        && model.input_installation_generation == model.provider_installation_generation
        && [
            &model.provider_model_artifact_id,
            &model.wire_model_artifact_id,
            &model.cpu_model_artifact_id,
            &model.worker_model_artifact_id,
            &model.mapping_model_artifact_id,
            &model.model_target_artifact_id,
        ]
        .iter()
        .all(|artifact| *artifact == &model.input_model_artifact_id)
        && model.input_model_target_digest == model.retained_model_target_digest;
    let bracket_valid = bracket.request_digest == bracket.receipt_request_digest
        && bracket.request_digest == bracket.provider_request_digest
        && bracket.request_digest == bracket.wire_request_digest
        && bracket.receipt_response_digest == bracket.provider_response_digest
        && bracket.receipt_response_digest == bracket.wire_response_digest
        && bracket.request_context_tokens > 0
        && bracket.request_context_tokens == bracket.receipt_context_tokens
        && bracket.request_context_tokens == bracket.wire_context_tokens
        && bracket.request_context_tokens == bracket.cpu_context_tokens
        && bracket.input_layout_digest == bracket.isolation_layout_digest
        && bracket.input_member_count > 0
        && bracket.input_member_count == bracket.isolation_member_count
        && bracket.input_total_bytes > 0
        && bracket.input_total_bytes == bracket.isolation_total_bytes
        && bracket.retained_input_bound_launch_digest == bracket.observed_input_bound_launch_digest;
    let execution_valid = execution.initial_isolation_digest == execution.final_isolation_digest
        && execution.initial_isolation_digest == execution.cpu_isolation_digest
        && execution.initial_worker_digest == execution.final_worker_digest
        && execution.initial_worker_digest == execution.cpu_worker_digest
        && execution.reviewed_worker_artifact_id == execution.initial_worker_artifact_id
        && execution.reviewed_worker_artifact_id == execution.final_worker_artifact_id
        && execution.initial_worker_runtime_package_digest == runtime.runtime_package_digest
        && execution.final_worker_runtime_package_digest == runtime.runtime_package_digest
        && execution.worker_portable_configuration_digest
            == execution.final_worker_portable_configuration_digest
        && execution.worker_portable_configuration_digest
            == execution.cpu_portable_configuration_digest
        && execution.worker_portable_closure_digest == execution.platform_portable_closure_digest
        && execution.worker_portable_closure_digest == execution.cpu_portable_closure_digest
        && execution.server_process_evidence_digest
            == execution.server_native_process_evidence_digest
        && execution.server_entrypoint_digest == runtime.runtime_entrypoint_digest
        && execution.server_process_profile_valid
        && execution.server_native_load_runtime_package_digest == runtime.runtime_package_digest
        && execution.server_native_component_count > 0
        && execution.worker_native_load_digest == execution.platform_native_load_digest
        && execution.worker_native_load_digest == execution.cpu_native_load_digest
        && execution.model_mapping_digest == execution.cpu_model_mapping_digest
        && execution.residency_digest == execution.cpu_residency_digest
        && execution.worker_profile_valid
        && execution.worker_native_component_count > 0
        && execution.model_mapping_region_count > 0
        && execution.runtime_reported_accelerator_bytes == 0
        && execution.isolation_canaries_valid
        && execution.compute_backend == ComputeBackend::NativeCpu
        && execution.placement == ExecutionPlacement::CpuOnly;
    let schemas_valid = facts
        .leaf_schema_versions
        .iter()
        .all(|schema| *schema == EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION);
    if !runtime_valid || !model_valid || !bracket_valid || !execution_valid || !schemas_valid {
        return Err(EffectiveRuntimeStateObservationError::RelationshipMismatch);
    }
    Ok(())
}

fn digest(domain: &[u8], values: &[&Digest]) -> Digest {
    let mut bytes = Vec::new();
    push(&mut bytes, domain);
    for value in values {
        push(&mut bytes, value.as_str().as_bytes());
    }
    Digest::sha256(&bytes)
}

fn push(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u64).to_be_bytes());
    target.extend_from_slice(value);
}

fn number_digest(value: u64) -> Digest {
    Digest::sha256(&value.to_be_bytes())
}

#[cfg(test)]
mod tests;
