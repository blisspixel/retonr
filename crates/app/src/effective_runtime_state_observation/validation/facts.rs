use rewrite_runtime_attestor::ManagedGenerationWorkerProfile;
use rewrite_types::Digest;

use super::{
    CommonFacts, CpuExecutionSources, CpuFacts, MANAGED_OLLAMA_INPUT_SCHEMA_VERSION,
    ProviderSources, valid_runtime_profile,
};

pub(super) fn common_facts(sources: &ProviderSources<'_>) -> CommonFacts {
    let execution = sources.receipt.execution();
    CommonFacts {
        runtime_profile_valid: valid_runtime_profile(sources.runtime_build),
        model_input_schema_valid: sources.model_input.schema_version()
            == MANAGED_OLLAMA_INPUT_SCHEMA_VERSION,
        model_installation_generation: sources.model_input.installation_generation(),
        input_member_count: sources.model_input.member_count(),
        input_total_bytes: sources.model_input.total_bytes(),
        input_model_digest: sources.model_input.model_artifact_id().digest().clone(),
        request_model_digest: sources.request.artifact_id.digest().clone(),
        binding_model_digest: sources.model.artifact_id().digest().clone(),
        input_reference_digest: sources.model_input.reference_digest().clone(),
        binding_reference_digest: Digest::sha256(sources.model.reference().as_bytes()),
        receipt_reference_digest: sources.receipt.runtime_reference_digest().clone(),
        binding_inventory_digest: sources.model.inventory_digest().clone(),
        receipt_inventory_digest: sources.receipt.inventory_digest().clone(),
        request_valid: sources.request.validate().is_ok()
            && sources.request.artifact_digest == *sources.model.artifact_digest(),
        request_binding_digest: sources.request.binding_digest(),
        receipt_request_digest: execution.request_digest().clone(),
        request_context_tokens: sources.request.context_token_limit,
        receipt_context_tokens: sources.receipt.context_tokens(),
        receipt_first_response: execution.first_response_ordinal(),
        receipt_last_response: execution.last_response_ordinal(),
        receipt_first_residency: sources.receipt.first_residency_ordinal(),
        receipt_last_residency: sources.receipt.last_residency_ordinal(),
        residency_claims_valid: sources.receipt.runtime_reported_residency_proven()
            && !sources.receipt.application_handler_proven()
            && !sources.receipt.model_use_proven()
            && !sources.receipt.resident_page_identity_proven()
            && !sources.receipt.effective_runtime_identity_proven()
            && !sources.receipt.qualified(),
    }
}

pub(super) fn cpu_facts(sources: &CpuExecutionSources<'_>) -> CpuFacts {
    let runtime_inputs = sources.initial_isolation.runtime_inputs();
    CpuFacts {
        input_model_digest: sources.model_input.model_artifact_id().digest().clone(),
        request_model_digest: sources.request.artifact_id.digest().clone(),
        worker_model_digest: sources.initial_worker.model_artifact_id().digest().clone(),
        mapping_model_digest: sources.model_mapping.model_artifact_id().digest().clone(),
        input_layout_digest: sources.model_input.input_layout_digest().clone(),
        isolation_layout_digest: runtime_inputs.layout_digest().clone(),
        input_member_count: sources.model_input.member_count(),
        isolation_member_count: runtime_inputs.member_count(),
        input_total_bytes: sources.model_input.total_bytes(),
        isolation_total_bytes: runtime_inputs.total_bytes(),
        stable_isolation: sources.initial_isolation == sources.final_isolation,
        isolation_canaries_valid: sources
            .initial_isolation
            .preparation()
            .all_canaries_passed()
            && sources
                .initial_isolation
                .device_boundary()
                .all_visibility_canaries_passed()
            && !runtime_inputs.is_empty()
            && runtime_inputs.input_mount().device() != 0
            && runtime_inputs.input_mount().inode() != 0,
        stable_worker: sources.initial_worker == sources.final_worker,
        worker_profile_valid: sources.initial_worker.profile()
            == ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
        worker_native_components: sources.worker_native_load.component_count(),
        model_mapping_regions: sources.model_mapping.mapping_region_count(),
        request_binding_digest: sources.request.binding_digest(),
        receipt_request_digest: sources.receipt.execution().request_digest().clone(),
        request_context_tokens: sources.request.context_token_limit,
        receipt_context_tokens: sources.receipt.context_tokens(),
        runtime_reported_accelerator_bytes: sources.receipt.accelerator_bytes(),
        residency_claims_valid: sources.receipt.runtime_reported_residency_proven()
            && !sources.receipt.application_handler_proven()
            && !sources.receipt.model_use_proven()
            && !sources.receipt.resident_page_identity_proven()
            && !sources.receipt.effective_runtime_identity_proven()
            && !sources.receipt.qualified(),
    }
}
