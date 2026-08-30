use rewrite_model::{ArtifactId, ModelPackageManifestId, RuntimeBuildId};
use rewrite_types::Digest;

use super::{
    LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION, LocalOllamaManagedGenerationEvidence,
    evidence_binding_digest,
};
use crate::LocalOllamaEffectiveStateMissingRelationship;

fn typed<T: serde::de::DeserializeOwned>(label: &str) -> T {
    serde_json::from_value(serde_json::Value::String(
        Digest::sha256(label.as_bytes()).as_str().to_owned(),
    ))
    .expect("typed digest")
}

fn fixture() -> LocalOllamaManagedGenerationEvidence {
    let mut evidence = LocalOllamaManagedGenerationEvidence {
        schema_version: LOCAL_OLLAMA_MANAGED_GENERATION_EVIDENCE_SCHEMA_VERSION,
        binding_digest: Digest::sha256(b"pending"),
        managed_preflight_binding_digest: Digest::sha256(b"managed preflight"),
        managed_build_binding_digest: Digest::sha256(b"managed build"),
        runtime_build_id: typed::<RuntimeBuildId>("runtime build"),
        static_model_binding_digest: Digest::sha256(b"static model"),
        model_package_manifest_id: typed::<ModelPackageManifestId>("model package"),
        model_artifact_id: typed::<ArtifactId>("model"),
        request_binding_digest: Digest::sha256(b"request"),
        response_binding_digest: Digest::sha256(b"response"),
        residency_contract_digest: Digest::sha256(b"residency contract"),
        residency_observation_digest: Digest::sha256(b"residency observation"),
        post_generation_process_evidence_digest: Digest::sha256(b"process"),
        post_generation_native_load_observation_digest: Digest::sha256(b"native load"),
        final_isolation_evidence_digest: Digest::sha256(b"isolation"),
        connection_observation_digest: Digest::sha256(b"connections"),
        connection_observation_count: 17,
        first_generation_response_ordinal: 8,
        last_generation_response_ordinal: 16,
        effective_context_tokens: 2_048,
        runtime_reported_accelerator_bytes: 0,
        missing_effective_state_relationships: vec![
            LocalOllamaEffectiveStateMissingRelationship::GenerationBoundProviderSnapshot,
            LocalOllamaEffectiveStateMissingRelationship::EffectiveOutputConfiguration,
            LocalOllamaEffectiveStateMissingRelationship::PlatformFrameworkAndDriver,
            LocalOllamaEffectiveStateMissingRelationship::ComputeBackendAndPlacement,
        ],
        static_model_package_relationship_verified: true,
        process_retained_through_generation: true,
        package_lease_retained_through_generation: true,
        all_responses_used_retained_transport: true,
        kernel_attribution_checked_around_every_response: true,
        runtime_reported_residency_proven: true,
        effective_context_capacity_observed: true,
        process_retained_after_return: false,
        model_loaded_proven: false,
        model_used_proven: false,
        application_handler_proven: false,
        effective_runtime_state_proven: false,
        qualified: false,
    };
    evidence.binding_digest = evidence_binding_digest(&evidence).expect("legacy binding digest");
    evidence
}

macro_rules! assert_field_changes {
    ($original:ident, $field:ident, $value:expr) => {{
        let mut variant = $original.clone();
        variant.$field = $value;
        assert_ne!(
            evidence_binding_digest(&variant).expect("variant legacy binding"),
            evidence_binding_digest(&$original).expect("original legacy binding"),
            "{} did not change the legacy binding",
            stringify!($field)
        );
    }};
}

#[test]
fn original_schema_and_identity_have_literal_compatibility_vectors() {
    let evidence = fixture();
    let encoded = serde_json::to_string(&evidence).expect("legacy JSON");

    assert_eq!(evidence.schema_version(), 1);
    assert_eq!(
        evidence.binding_digest().as_str(),
        "42d2130676e910b4d3c7678fd70f2c025c57a4cd87da64511f5d724263cde3e4"
    );
    assert_eq!(
        encoded,
        concat!(
            "{\"schema_version\":1,",
            "\"binding_digest\":\"42d2130676e910b4d3c7678fd70f2c025c57a4cd87da64511f5d724263cde3e4\",",
            "\"managed_preflight_binding_digest\":\"9f25142c4d708d27b1f5b26f8bdb4ac0a6c54c90ee323c165a5906bc5ffa4e6b\",",
            "\"managed_build_binding_digest\":\"3eb53f1a5d16ed1c4ff2246eff3cc2c12206a64d41f40b9bdf0e1ffd2fe82dcf\",",
            "\"runtime_build_id\":\"fa8fd8ea74fc8daca7618c1f301d1fbb734f3893b7ffbadd708da545b1455303\",",
            "\"static_model_binding_digest\":\"4b95d1ecd214326672da4bcba9bdc75e8c556a1d1a11095cb4d4372d8aa1e82b\",",
            "\"model_package_manifest_id\":\"2cd3c038f7a07efcac2d5f7b3240f94a094865e924d43bd7f097e886a87c4dac\",",
            "\"model_artifact_id\":\"9372c470eeadd5ecd9c3c74c2b3cb633f8e2f2fad799250a0f70d652b6b825e4\",",
            "\"request_binding_digest\":\"1f58b9145b24d108d7ac38887338b3ea3229833b9c1e418250343f907bfd1047\",",
            "\"response_binding_digest\":\"a9f4b3d22a523fdada41c85c175425bcd15b32b4cd0f54d9433accd52d7195a1\",",
            "\"residency_contract_digest\":\"3afb65f8f3856fbcd00e0127d7a9d1d7850c98aa021628d32b2725827c7f96ab\",",
            "\"residency_observation_digest\":\"7822d347ee1b725aefd7ddf77c2317e5fe52abf5ed31b5837c930641e971968e\",",
            "\"post_generation_process_evidence_digest\":\"19ed40bf62c399b8492efda5b9a9184b68cf4d9d4a165b38557b9d14201d0c03\",",
            "\"post_generation_native_load_observation_digest\":\"04813282623c24a4c1d931dff92e3a72836597de80a7709698fb231deb7effaf\",",
            "\"final_isolation_evidence_digest\":\"3624d3181d5c4f8abf2f25fa708f5efa04236b79d0deafe9f292b590b2ca0f7e\",",
            "\"connection_observation_digest\":\"1e5fac867454a4fec29be85eea63d308abd770e42b169632817b0643b508877d\",",
            "\"connection_observation_count\":17,",
            "\"first_generation_response_ordinal\":8,",
            "\"last_generation_response_ordinal\":16,",
            "\"effective_context_tokens\":2048,",
            "\"runtime_reported_accelerator_bytes\":0,",
            "\"missing_effective_state_relationships\":[",
            "\"generation_bound_provider_snapshot\",",
            "\"effective_output_configuration\",",
            "\"platform_framework_and_driver\",",
            "\"compute_backend_and_placement\"],",
            "\"static_model_package_relationship_verified\":true,",
            "\"process_retained_through_generation\":true,",
            "\"package_lease_retained_through_generation\":true,",
            "\"all_responses_used_retained_transport\":true,",
            "\"kernel_attribution_checked_around_every_response\":true,",
            "\"runtime_reported_residency_proven\":true,",
            "\"effective_context_capacity_observed\":true,",
            "\"process_retained_after_return\":false,",
            "\"model_loaded_proven\":false,",
            "\"model_used_proven\":false,",
            "\"application_handler_proven\":false,",
            "\"effective_runtime_state_proven\":false,",
            "\"qualified\":false}"
        )
    );
    assert_eq!(evidence.runtime_build_id(), &evidence.runtime_build_id);
    assert_eq!(
        evidence.model_package_manifest_id(),
        &evidence.model_package_manifest_id
    );
    assert_eq!(evidence.model_artifact_id(), &evidence.model_artifact_id);
    assert_eq!(
        evidence.response_binding_digest(),
        &evidence.response_binding_digest
    );
    assert_eq!(evidence.effective_context_tokens(), 2_048);
    assert_eq!(evidence.missing_effective_state_relationships().len(), 4);
    assert!(evidence.process_retained_through_generation());
    assert!(evidence.runtime_reported_residency_proven());
    assert!(evidence.effective_context_capacity_observed());
    assert!(!evidence.process_retained_after_return());
    assert!(!evidence.model_used_proven());
    assert!(!evidence.application_handler_proven());
    assert!(!evidence.effective_runtime_state_proven());
    assert!(!evidence.qualified());
}

#[test]
fn every_legacy_identity_field_changes_the_binding() {
    let original = fixture();
    assert_field_changes!(original, schema_version, 2);
    assert_field_changes!(
        original,
        managed_preflight_binding_digest,
        Digest::sha256(b"other managed preflight")
    );
    assert_field_changes!(
        original,
        managed_build_binding_digest,
        Digest::sha256(b"other managed build")
    );
    assert_field_changes!(
        original,
        runtime_build_id,
        typed::<RuntimeBuildId>("other runtime build")
    );
    assert_field_changes!(
        original,
        static_model_binding_digest,
        Digest::sha256(b"other static")
    );
    assert_field_changes!(
        original,
        model_package_manifest_id,
        typed::<ModelPackageManifestId>("other model package")
    );
    assert_field_changes!(
        original,
        model_artifact_id,
        typed::<ArtifactId>("other model")
    );
    assert_field_changes!(
        original,
        request_binding_digest,
        Digest::sha256(b"other request")
    );
    assert_field_changes!(
        original,
        response_binding_digest,
        Digest::sha256(b"other response")
    );
    assert_field_changes!(
        original,
        residency_contract_digest,
        Digest::sha256(b"other contract")
    );
    assert_field_changes!(
        original,
        residency_observation_digest,
        Digest::sha256(b"other resident")
    );
    assert_field_changes!(
        original,
        post_generation_process_evidence_digest,
        Digest::sha256(b"other process")
    );
    assert_field_changes!(
        original,
        post_generation_native_load_observation_digest,
        Digest::sha256(b"other native")
    );
    assert_field_changes!(
        original,
        final_isolation_evidence_digest,
        Digest::sha256(b"other isolation")
    );
    assert_field_changes!(
        original,
        connection_observation_digest,
        Digest::sha256(b"other connection")
    );
    assert_field_changes!(original, connection_observation_count, 18);
    assert_field_changes!(original, first_generation_response_ordinal, 9);
    assert_field_changes!(original, last_generation_response_ordinal, 17);
    assert_field_changes!(original, effective_context_tokens, 2_049);
    assert_field_changes!(original, runtime_reported_accelerator_bytes, 1);
    assert_field_changes!(
        original,
        missing_effective_state_relationships,
        vec![LocalOllamaEffectiveStateMissingRelationship::GenerationBoundProviderSnapshot]
    );
}

#[test]
fn every_legacy_claim_field_changes_the_binding() {
    let original = fixture();
    assert_field_changes!(original, static_model_package_relationship_verified, false);
    assert_field_changes!(original, process_retained_through_generation, false);
    assert_field_changes!(original, package_lease_retained_through_generation, false);
    assert_field_changes!(original, all_responses_used_retained_transport, false);
    assert_field_changes!(
        original,
        kernel_attribution_checked_around_every_response,
        false
    );
    assert_field_changes!(original, runtime_reported_residency_proven, false);
    assert_field_changes!(original, effective_context_capacity_observed, false);
    assert_field_changes!(original, process_retained_after_return, true);
    assert_field_changes!(original, model_loaded_proven, true);
    assert_field_changes!(original, model_used_proven, true);
    assert_field_changes!(original, application_handler_proven, true);
    assert_field_changes!(original, effective_runtime_state_proven, true);
    assert_field_changes!(original, qualified, true);
}

#[test]
fn stored_binding_field_remains_excluded_from_legacy_identity() {
    let original = fixture();
    let mut changed = original.clone();
    changed.binding_digest = Digest::sha256(b"replacement stored binding");
    assert_eq!(
        evidence_binding_digest(&original).expect("original binding"),
        evidence_binding_digest(&changed).expect("changed binding")
    );
}
