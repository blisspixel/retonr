use std::time::{Duration, Instant};

use rewrite_inference::{
    GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest, InferenceBackend, OperationContext,
    ReasoningPolicy, SamplingParameters, candidate_output_contract,
};
use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{OllamaBackend, OllamaEndpoint, OllamaLimits, OllamaModelBinding};

const MODEL: &str = "fixture:latest";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn binding() -> OllamaModelBinding {
    let digest = Digest::from_sha256_hex(DIGEST).expect("fixture digest");
    OllamaModelBinding::new(MODEL, ArtifactId::from_digest(digest.clone()), digest)
        .expect("fixture binding")
}

fn backend(server: &MockServer) -> OllamaBackend {
    OllamaBackend::new(
        OllamaEndpoint::parse(&server.uri()).expect("wiremock endpoint"),
        vec![binding()],
        OllamaLimits::default(),
    )
    .expect("Ollama backend")
}

fn context(token: &CancellationToken) -> OperationContext<'_> {
    OperationContext::new(token, Some(Instant::now() + Duration::from_secs(5)))
}

fn local_tag() -> serde_json::Value {
    json!({"models": [{
        "name": MODEL,
        "model": MODEL,
        "size": 1024,
        "digest": format!("sha256:{DIGEST}")
    }]})
}

fn local_show() -> serde_json::Value {
    json!({
        "license": "fixture license",
        "template": "fixture template",
        "capabilities": ["completion"],
        "details": {"format": "gguf", "family": "fixture", "quantization_level": "Q4_K_M"},
        "model_info": {"fixture.context_length": 4096}
    })
}

async fn mount_get(server: &MockServer, endpoint: &str, body: serde_json::Value, count: u64) {
    Mock::given(method("GET"))
        .and(path(endpoint))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(count)
        .mount(server)
        .await;
}

#[tokio::test]
async fn rejects_remote_inventory_and_model_metadata() {
    let token = CancellationToken::new();
    let inventory_server = MockServer::start().await;
    mount_get(
        &inventory_server,
        "/api/version",
        json!({"version": "0.13.0"}),
        1,
    )
    .await;
    let mut remote_tag = local_tag();
    remote_tag["models"][0]["remote_model"] = json!("cloud-model");
    remote_tag["models"][0]["remote_host"] = json!("https://example.invalid");
    mount_get(&inventory_server, "/api/tags", remote_tag, 1).await;
    let error = backend(&inventory_server)
        .discover(context(&token))
        .await
        .expect_err("remote inventory is rejected");
    assert_eq!(error.code, "invalid_inventory_entry");

    let show_server = MockServer::start().await;
    mount_get(
        &show_server,
        "/api/version",
        json!({"version": "0.13.0"}),
        1,
    )
    .await;
    mount_get(&show_server, "/api/tags", local_tag(), 1).await;
    let mut remote_show = local_show();
    remote_show["remote_model"] = json!("cloud-model");
    remote_show["remote_host"] = json!("https://example.invalid");
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(remote_show))
        .expect(1)
        .mount(&show_server)
        .await;
    let error = backend(&show_server)
        .discover(context(&token))
        .await
        .expect_err("remote model metadata is rejected");
    assert_eq!(error.code, "invalid_model_metadata");
}

#[tokio::test]
async fn rejects_remote_generation_response() {
    let server = MockServer::start().await;
    mount_get(&server, "/api/version", json!({"version": "0.13.0"}), 2).await;
    mount_get(&server, "/api/tags", local_tag(), 2).await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(local_show()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": MODEL,
            "remote_model": "cloud-model",
            "remote_host": "https://example.invalid",
            "response": "{\"candidates\":[{\"text\":\"discard\"}]}",
            "thinking": "",
            "done": true,
            "done_reason": "stop"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let binding = binding();
    let request = GenerationRequest {
        schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
        artifact_id: binding.artifact_id().clone(),
        artifact_digest: binding.artifact_digest().clone(),
        input: "Rewrite this fixture.".to_owned(),
        output: candidate_output_contract(),
        candidate_count: 1,
        source_byte_count: 21,
        source_byte_limit: 1024,
        input_byte_limit: 2048,
        context_token_limit: 4096,
        output_token_limit: 256,
        candidate_byte_limit: 1024,
        sampling: SamplingParameters {
            temperature: 0.2,
            top_p: 0.9,
            seed: Some(7),
        },
        reasoning: ReasoningPolicy::Disabled,
    };
    let token = CancellationToken::new();
    let error = backend(&server)
        .generate(request, context(&token))
        .await
        .expect_err("remote generation is rejected");
    assert_eq!(error.code, "invalid_generation_response");
}
