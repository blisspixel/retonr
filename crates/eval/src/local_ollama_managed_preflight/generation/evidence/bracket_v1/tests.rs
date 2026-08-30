use rewrite_model::{EffectiveRuntimeStateId, RuntimeBuildIdentity, RuntimeBuildMode};
use rewrite_types::Digest;

use super::{
    MANAGED_OLLAMA_GENERATION_BRACKET_OBSERVATION_SCHEMA_VERSION,
    ManagedOllamaGenerationBracketObservationV1, bracket_observation_binding_digest,
};

fn state_id(label: &str) -> EffectiveRuntimeStateId {
    serde_json::from_value(serde_json::Value::String(
        Digest::sha256(label.as_bytes()).as_str().to_owned(),
    ))
    .expect("effective state ID")
}
use crate::{
    local_ollama_managed_preflight::test_support::package_for_version,
    local_ollama_model_binding::{
        LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, tests::exact_binding_fixture,
    },
};

fn fixture_evidence(accelerator_bytes: u64) -> ManagedOllamaGenerationBracketObservationV1 {
    let package = package_for_version(LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION);
    let runtime_build =
        RuntimeBuildIdentity::new_from_package_manifest(RuntimeBuildMode::ManagedProcess, &package)
            .expect("runtime build");
    let (_plan, model, _binding) = exact_binding_fixture();
    let mut evidence = ManagedOllamaGenerationBracketObservationV1 {
        schema_version: MANAGED_OLLAMA_GENERATION_BRACKET_OBSERVATION_SCHEMA_VERSION,
        binding_digest: Digest::sha256(b"pending"),
        managed_preflight_binding_digest: Digest::sha256(b"managed preflight"),
        managed_build_binding_digest: Digest::sha256(b"managed build"),
        runtime_build_id: runtime_build.runtime_build_id(),
        admitted_runtime_id: Digest::sha256(b"admitted runtime"),
        generation_path_id: Digest::sha256(b"generation path"),
        frozen_external_component_set_id: Digest::sha256(b"frozen components"),
        runtime_package_installation_generation: 11,
        model_package_installation_generation: 7,
        effective_runtime_state_id: state_id("effective state"),
        effective_runtime_state_join_digest: Digest::sha256(b"effective state join"),
        static_model_binding_digest: model.binding_digest().clone(),
        model_package_manifest_id: model.model_package_manifest_id.clone(),
        model_artifact_id: model.model_artifact_id.clone(),
        request_binding_digest: Digest::sha256(b"request"),
        response_binding_digest: Digest::sha256(b"response"),
        residency_contract_digest: Digest::sha256(b"residency contract"),
        residency_observation_digest: Digest::sha256(b"residency observation"),
        post_generation_process_evidence_digest: Digest::sha256(b"process"),
        post_generation_native_load_observation_digest: Digest::sha256(b"native load"),
        managed_input_mapping_digest: Digest::sha256(b"input mapping"),
        managed_input_layout_digest: Digest::sha256(b"input layout"),
        input_bound_launch_spec_digest: Digest::sha256(b"input bound launch"),
        generation_worker_evidence_digest: Digest::sha256(b"worker"),
        generation_worker_native_load_observation_digest: Digest::sha256(b"worker native load"),
        generation_worker_model_mapping_observation_digest: Digest::sha256(b"worker model mapping"),
        final_isolation_evidence_digest: Digest::sha256(b"isolation"),
        connection_observation_digest: Digest::sha256(b"connections"),
        connection_observation_count: 17,
        first_generation_response_ordinal: 8,
        last_generation_response_ordinal: 16,
        effective_context_tokens: 2048,
        runtime_reported_accelerator_bytes: accelerator_bytes,
        missing_effective_state_relationships: Vec::new(),
        static_model_package_relationship_verified: true,
        process_retained_through_generation: true,
        runtime_package_lease_retained_through_generation: true,
        model_package_lease_retained_through_generation: true,
        package_leases_revalidated_immediately_after_generation: true,
        package_leases_revalidated_after_final_observation: true,
        private_model_input_reobserved_after_generation: true,
        exact_model_weight_mapping_observed: true,
        all_responses_used_retained_transport: true,
        kernel_attribution_checked_around_every_response: true,
        runtime_reported_residency_proven: true,
        effective_context_capacity_observed: true,
        process_retained_after_return: false,
        model_loaded_proven: true,
        model_used_proven: false,
        application_handler_proven: false,
        effective_runtime_state_proven: true,
        qualified: false,
    };
    evidence.binding_digest =
        bracket_observation_binding_digest(&evidence).expect("binding digest");
    evidence
}

#[test]
fn redacted_contract_binds_positive_and_negative_claims() {
    let evidence = fixture_evidence(0);
    assert_eq!(
        evidence.schema_version(),
        MANAGED_OLLAMA_GENERATION_BRACKET_OBSERVATION_SCHEMA_VERSION
    );
    assert_eq!(evidence.binding_digest(), &evidence.binding_digest);
    assert_eq!(evidence.runtime_build_id(), &evidence.runtime_build_id);
    assert_eq!(
        evidence.admitted_runtime_id(),
        &evidence.admitted_runtime_id
    );
    assert_eq!(evidence.generation_path_id(), &evidence.generation_path_id);
    assert_eq!(
        evidence.frozen_external_component_set_id(),
        &evidence.frozen_external_component_set_id
    );
    assert_eq!(evidence.runtime_package_installation_generation(), 11);
    assert_eq!(evidence.model_package_installation_generation(), 7);
    assert_eq!(
        evidence.effective_runtime_state_id(),
        &evidence.effective_runtime_state_id
    );
    assert_eq!(
        evidence.effective_runtime_state_join_digest(),
        &evidence.effective_runtime_state_join_digest
    );
    assert_eq!(
        evidence.model_package_manifest_id(),
        &evidence.model_package_manifest_id
    );
    assert_eq!(evidence.model_artifact_id(), &evidence.model_artifact_id);
    assert_eq!(
        evidence.response_binding_digest(),
        &evidence.response_binding_digest
    );
    assert_eq!(evidence.effective_context_tokens(), 2048);
    assert!(evidence.missing_effective_state_relationships().is_empty());
    assert!(evidence.process_retained_through_generation());
    assert!(evidence.runtime_package_lease_retained_through_generation());
    assert!(evidence.model_package_lease_retained_through_generation());
    assert!(evidence.package_leases_revalidated_immediately_after_generation());
    assert!(evidence.package_leases_revalidated_after_final_observation());
    assert!(evidence.exact_model_weight_mapping_observed());
    assert!(evidence.model_loaded_proven());
    assert!(evidence.runtime_reported_residency_proven());
    assert!(evidence.effective_context_capacity_observed());
    assert!(!evidence.process_retained_after_return());
    assert!(!evidence.model_used_proven());
    assert!(!evidence.application_handler_proven());
    assert!(evidence.effective_runtime_state_proven());
    assert!(!evidence.qualified());
    assert_ne!(
        evidence.binding_digest(),
        fixture_evidence(1).binding_digest()
    );

    let encoded = serde_json::to_string(&evidence).expect("serialize evidence");
    assert!(encoded.contains("runtime_package_lease_retained_through_generation"));
    assert!(encoded.contains("model_package_lease_retained_through_generation"));
    assert!(!encoded.contains("\"package_lease_retained_through_generation\""));
    assert!(!encoded.contains("bounded fixture"));
    assert!(!encoded.contains("model:exact"));
}

mod compatibility;
