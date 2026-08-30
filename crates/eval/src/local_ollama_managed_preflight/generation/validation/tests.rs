use rewrite_inference::{
    ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, SamplingParameters,
    StructuredCompletionRequest, candidate_output_contract,
};
use rewrite_ollama::OllamaModelBinding;
use rewrite_types::Digest;

use super::validate_generation_binding;
use crate::{
    LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION, LocalOllamaBoundPreflightPlan,
    local_ollama_managed_preflight::test_support::package_for_version,
    local_ollama_model_binding::{
        LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, tests::exact_binding_fixture,
    },
};

fn request(model: &OllamaModelBinding) -> StructuredCompletionRequest {
    StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_id: model.artifact_id().clone(),
        artifact_digest: model.artifact_digest().clone(),
        input: "bounded fixture".to_owned(),
        output: candidate_output_contract(),
        source_byte_count: 15,
        source_byte_limit: 1024,
        input_byte_limit: 2048,
        context_token_limit: 2048,
        output_token_limit: 256,
        output_byte_limit: 4096,
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed: Some(7),
        },
        reasoning: ReasoningPolicy::Disabled,
    }
}

#[test]
fn exact_static_model_artifact_and_distinct_inventory_bind() {
    let package = package_for_version(LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION);
    let (preflight, evidence, model) = exact_binding_fixture();
    let plan = LocalOllamaBoundPreflightPlan {
        schema_version: LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION,
        preflight,
        maximum_entrypoint_bytes: 1024,
        maximum_session_body_bytes: 4 * 1024 * 1024,
        expected_entrypoint_digest: Some(package.entrypoint().artifact_id().digest().clone()),
    };
    let request = request(&model);

    validate_generation_binding(&package, &plan, &evidence, &model, &request)
        .expect("exact distinct identities bind");
    assert_ne!(model.artifact_digest(), model.inventory_digest());

    let wrong_inventory = Digest::sha256(b"wrong inventory");
    let wrong_model = OllamaModelBinding::new_with_inventory(
        model.reference(),
        model.artifact_id().clone(),
        model.artifact_digest().clone(),
        wrong_inventory,
    )
    .expect("structurally valid wrong binding");
    assert!(
        validate_generation_binding(&package, &plan, &evidence, &wrong_model, &request).is_err()
    );
    let mut oversized = request;
    oversized.input = "x".repeat(
        usize::try_from(rewrite_ollama::OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES)
            .expect("input limit")
            + 1,
    );
    oversized.input_byte_limit = u64::try_from(oversized.input.len()).expect("input length");
    assert!(oversized.validate().is_ok());
    assert!(validate_generation_binding(&package, &plan, &evidence, &model, &oversized).is_err());
}
