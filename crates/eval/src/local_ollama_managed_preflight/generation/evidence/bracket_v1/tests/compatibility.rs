use rewrite_model::{ArtifactId, ModelPackageManifestId, RuntimeBuildId};
use rewrite_types::Digest;

use super::{bracket_observation_binding_digest, fixture_evidence, state_id};
use crate::LocalOllamaEffectiveStateMissingRelationship;

fn typed<T: serde::de::DeserializeOwned>(label: &str) -> T {
    serde_json::from_value(serde_json::Value::String(
        Digest::sha256(label.as_bytes()).as_str().to_owned(),
    ))
    .expect("typed digest")
}

macro_rules! assert_field_changes {
    ($original:ident, $field:ident, $value:expr) => {{
        let mut variant = $original.clone();
        variant.$field = $value;
        assert_ne!(
            bracket_observation_binding_digest(&variant).expect("variant bracket binding"),
            bracket_observation_binding_digest(&$original).expect("original bracket binding"),
            "{} did not change the bracket binding",
            stringify!($field)
        );
    }};
}

#[test]
fn bracket_v1_has_literal_identity_and_json_vectors() {
    let evidence = fixture_evidence(0);
    let encoded = serde_json::to_string(&evidence).expect("bracket JSON");

    assert_eq!(
        evidence.binding_digest().as_str(),
        "2bf99da7ea9463cba48a77a1fbfbf53dafd85470153003ca0efa3d9a18a1328a"
    );
    assert_eq!(
        evidence.bracket_observation_v1_id().digest().as_str(),
        "2bf99da7ea9463cba48a77a1fbfbf53dafd85470153003ca0efa3d9a18a1328a"
    );
    assert_eq!(
        evidence.bracket_observation_v1_id().digest(),
        evidence.binding_digest()
    );
    assert_eq!(
        encoded,
        concat!(
            "{\"schema_version\":1,",
            "\"binding_digest\":\"2bf99da7ea9463cba48a77a1fbfbf53dafd85470153003ca0efa3d9a18a1328a\",",
            "\"managed_preflight_binding_digest\":\"9f25142c4d708d27b1f5b26f8bdb4ac0a6c54c90ee323c165a5906bc5ffa4e6b\",",
            "\"managed_build_binding_digest\":\"3eb53f1a5d16ed1c4ff2246eff3cc2c12206a64d41f40b9bdf0e1ffd2fe82dcf\",",
            "\"runtime_build_id\":\"0894167806d51e44f90ca3ef94acc1357a18d893a6b4cf7df5717dbe955deb2d\",",
            "\"admitted_runtime_id\":\"f65e1a30d6aa8f6a8d7ffed0abf943c0c5e3c0266b85d860f523d0aa598060b3\",",
            "\"generation_path_id\":\"db9fa359947e408bbccbed501225151bce49e4b7d5dd68cbff5447eca72e4b3d\",",
            "\"frozen_external_component_set_id\":\"9506227305d86c5d14f141c3cb6e5951207ad290cfa7f9df122b7e7c53265cc7\",",
            "\"runtime_package_installation_generation\":11,",
            "\"model_package_installation_generation\":7,",
            "\"effective_runtime_state_id\":\"8d6e54639d47376f3034d1bf10f29a5d4fdebc369de2eced48bdde759ffd836c\",",
            "\"effective_runtime_state_join_digest\":\"281a5a6e4e0c12c10d38c44942587dd4827ccf990d8f76ee2de3c58770e3dd6c\",",
            "\"static_model_binding_digest\":\"78bbe425c47389d5c156370db5f43c9ca13531239965cfa738ec4345f2dfe1d2\",",
            "\"model_package_manifest_id\":\"860aa3258238e57c611cd8038ceaeaa6a87cbadfd72c5ccf237df602271e363d\",",
            "\"model_artifact_id\":\"bf65f1029810435008660edf1369b53b7e94bc317773a64b0aefbc7f94de3531\",",
            "\"request_binding_digest\":\"1f58b9145b24d108d7ac38887338b3ea3229833b9c1e418250343f907bfd1047\",",
            "\"response_binding_digest\":\"a9f4b3d22a523fdada41c85c175425bcd15b32b4cd0f54d9433accd52d7195a1\",",
            "\"residency_contract_digest\":\"3afb65f8f3856fbcd00e0127d7a9d1d7850c98aa021628d32b2725827c7f96ab\",",
            "\"residency_observation_digest\":\"7822d347ee1b725aefd7ddf77c2317e5fe52abf5ed31b5837c930641e971968e\",",
            "\"post_generation_process_evidence_digest\":\"19ed40bf62c399b8492efda5b9a9184b68cf4d9d4a165b38557b9d14201d0c03\",",
            "\"post_generation_native_load_observation_digest\":\"04813282623c24a4c1d931dff92e3a72836597de80a7709698fb231deb7effaf\",",
            "\"managed_input_mapping_digest\":\"866b5507ab3df30a5324413535ca57428dd11566f18c0ec3ca514b1ddcb15616\",",
            "\"managed_input_layout_digest\":\"58a0a9701e8b05e0c70711c4bbee2cbfe9c5aac5d9e4e0c49e36290535a8bc21\",",
            "\"input_bound_launch_spec_digest\":\"ccfe8dd793010a7d5de98a8f1f6dd2ad8415938f3385a116a6b0d07aed201587\",",
            "\"generation_worker_evidence_digest\":\"87eba76e7f3164534045ba922e7770fb58bbd14ad732bbf5ba6f11cc56989e6e\",",
            "\"generation_worker_native_load_observation_digest\":\"6d221eded63c5ac1bb064276b1b0f55ba493b811ea4bb9c16a23fba13f32e4db\",",
            "\"generation_worker_model_mapping_observation_digest\":\"c920187adb6aac5315eb0e69672b97ea4a908dc10d9a9d092fe16ea398461c89\",",
            "\"final_isolation_evidence_digest\":\"3624d3181d5c4f8abf2f25fa708f5efa04236b79d0deafe9f292b590b2ca0f7e\",",
            "\"connection_observation_digest\":\"1e5fac867454a4fec29be85eea63d308abd770e42b169632817b0643b508877d\",",
            "\"connection_observation_count\":17,",
            "\"first_generation_response_ordinal\":8,",
            "\"last_generation_response_ordinal\":16,",
            "\"effective_context_tokens\":2048,",
            "\"runtime_reported_accelerator_bytes\":0,",
            "\"missing_effective_state_relationships\":[],",
            "\"static_model_package_relationship_verified\":true,",
            "\"process_retained_through_generation\":true,",
            "\"runtime_package_lease_retained_through_generation\":true,",
            "\"model_package_lease_retained_through_generation\":true,",
            "\"package_leases_revalidated_immediately_after_generation\":true,",
            "\"package_leases_revalidated_after_final_observation\":true,",
            "\"private_model_input_reobserved_after_generation\":true,",
            "\"exact_model_weight_mapping_observed\":true,",
            "\"all_responses_used_retained_transport\":true,",
            "\"kernel_attribution_checked_around_every_response\":true,",
            "\"runtime_reported_residency_proven\":true,",
            "\"effective_context_capacity_observed\":true,",
            "\"process_retained_after_return\":false,",
            "\"model_loaded_proven\":true,",
            "\"model_used_proven\":false,",
            "\"application_handler_proven\":false,",
            "\"effective_runtime_state_proven\":true,",
            "\"qualified\":false}"
        )
    );
}

#[test]
fn every_bracket_v1_core_identity_field_changes_the_binding() {
    let original = fixture_evidence(0);
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
        admitted_runtime_id,
        Digest::sha256(b"other admitted")
    );
    assert_field_changes!(original, generation_path_id, Digest::sha256(b"other path"));
    assert_field_changes!(
        original,
        frozen_external_component_set_id,
        Digest::sha256(b"other frozen")
    );
    assert_field_changes!(original, runtime_package_installation_generation, 12);
    assert_field_changes!(original, model_package_installation_generation, 8);
    assert_field_changes!(
        original,
        effective_runtime_state_id,
        state_id("other effective state")
    );
    assert_field_changes!(
        original,
        effective_runtime_state_join_digest,
        Digest::sha256(b"other effective join")
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
}

#[test]
fn every_bracket_v1_execution_identity_field_changes_the_binding() {
    let original = fixture_evidence(0);
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
        managed_input_mapping_digest,
        Digest::sha256(b"other mapping")
    );
    assert_field_changes!(
        original,
        managed_input_layout_digest,
        Digest::sha256(b"other layout")
    );
    assert_field_changes!(
        original,
        input_bound_launch_spec_digest,
        Digest::sha256(b"other launch")
    );
    assert_field_changes!(
        original,
        generation_worker_evidence_digest,
        Digest::sha256(b"other worker")
    );
    assert_field_changes!(
        original,
        generation_worker_native_load_observation_digest,
        Digest::sha256(b"other worker native")
    );
    assert_field_changes!(
        original,
        generation_worker_model_mapping_observation_digest,
        Digest::sha256(b"other model mapping")
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
fn every_bracket_v1_claim_field_changes_the_binding() {
    let original = fixture_evidence(0);
    assert_field_changes!(original, static_model_package_relationship_verified, false);
    assert_field_changes!(original, process_retained_through_generation, false);
    assert_field_changes!(
        original,
        runtime_package_lease_retained_through_generation,
        false
    );
    assert_field_changes!(
        original,
        model_package_lease_retained_through_generation,
        false
    );
    assert_field_changes!(
        original,
        package_leases_revalidated_immediately_after_generation,
        false
    );
    assert_field_changes!(
        original,
        package_leases_revalidated_after_final_observation,
        false
    );
    assert_field_changes!(
        original,
        private_model_input_reobserved_after_generation,
        false
    );
    assert_field_changes!(original, exact_model_weight_mapping_observed, false);
    assert_field_changes!(original, all_responses_used_retained_transport, false);
    assert_field_changes!(
        original,
        kernel_attribution_checked_around_every_response,
        false
    );
    assert_field_changes!(original, runtime_reported_residency_proven, false);
    assert_field_changes!(original, effective_context_capacity_observed, false);
    assert_field_changes!(original, process_retained_after_return, true);
    assert_field_changes!(original, model_loaded_proven, false);
    assert_field_changes!(original, model_used_proven, true);
    assert_field_changes!(original, application_handler_proven, true);
    assert_field_changes!(original, effective_runtime_state_proven, false);
    assert_field_changes!(original, qualified, true);
}

#[test]
fn stored_binding_field_remains_excluded_from_bracket_v1_identity() {
    let original = fixture_evidence(0);
    let mut changed = original.clone();
    changed.binding_digest = Digest::sha256(b"replacement stored binding");
    assert_eq!(
        bracket_observation_binding_digest(&original).expect("original binding"),
        bracket_observation_binding_digest(&changed).expect("changed binding")
    );
}
