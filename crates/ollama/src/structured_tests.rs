use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use rewrite_inference::{
    InferenceBackend, InferenceErrorKind, OperationContext, ReasoningPolicy,
    STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, SamplingParameters, StructuredCompletionFinish,
    StructuredCompletionRequest, candidate_output_contract,
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

fn tag_body() -> serde_json::Value {
    json!({
        "models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": format!("sha256:{DIGEST}")
        }]
    })
}

fn show_body() -> serde_json::Value {
    json!({
        "license": "fixture license",
        "template": "fixture template",
        "capabilities": ["completion"],
        "details": {
            "format": "gguf",
            "family": "fixture",
            "quantization_level": "Q4_K_M"
        },
        "model_info": {"fixture.context_length": 4096}
    })
}

async fn mount_common(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.13.0"})))
        .expect(2)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tag_body()))
        .expect(2)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show_body()))
        .expect(1)
        .mount(server)
        .await;
}

fn request() -> StructuredCompletionRequest {
    let binding = binding();
    StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_id: binding.artifact_id().clone(),
        artifact_digest: binding.artifact_digest().clone(),
        input: "Return this fixture as JSON.".to_owned(),
        output: candidate_output_contract(),
        source_byte_count: 13,
        source_byte_limit: 1024,
        input_byte_limit: 2048,
        context_token_limit: 4096,
        output_token_limit: 256,
        output_byte_limit: 1024,
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed: Some(7),
        },
        reasoning: ReasoningPolicy::Disabled,
    }
}

async fn mount_generation(server: &MockServer, payload: &str, done_reason: &str) {
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": MODEL,
            "response": payload,
            "thinking": "",
            "done": true,
            "done_reason": done_reason,
            "prompt_eval_count": 4,
            "eval_count": 2,
            "eval_duration": 9000
        })))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn returns_bounded_structured_json_without_domain_parsing() {
    let server = MockServer::start().await;
    mount_common(&server).await;
    let payload = r#"{"candidates":[{"text":"opaque payload"}]}"#;
    mount_generation(&server, payload, "stop").await;

    let token = CancellationToken::new();
    let response = backend(&server)
        .complete_structured(request(), context(&token))
        .await
        .expect("structured completion succeeds");
    assert_eq!(response.output_json(), payload);
    assert_eq!(response.usage().generation_micros, Some(9));
    assert_eq!(response.finish(), StructuredCompletionFinish::Complete);
    assert!(!format!("{response:?}").contains("opaque payload"));
}

#[tokio::test]
async fn rejects_oversized_invalid_or_truncated_structured_output() {
    for (payload, limit, done_reason) in [
        ("{\"too_long\":true}", 4_u64, "stop"),
        ("not json", 1024_u64, "stop"),
        ("{}", 1024_u64, "length"),
    ] {
        let server = MockServer::start().await;
        mount_common(&server).await;
        mount_generation(&server, payload, done_reason).await;
        let mut request = request();
        request.output_byte_limit = limit;
        let token = CancellationToken::new();
        let error = backend(&server)
            .complete_structured(request, context(&token))
            .await
            .expect_err("invalid structured payload is rejected");
        assert_eq!(error.kind, InferenceErrorKind::MalformedResponse);
        assert!(matches!(
            error.code.as_str(),
            "invalid_generation_response" | "invalid_structured_output"
        ));
    }
}

#[tokio::test]
async fn rejects_nonterminal_or_reasoning_structured_responses() {
    for (done, done_reason, thinking) in [
        (false, "stop", ""),
        (true, "", ""),
        (true, "stop", "model reasoning"),
    ] {
        let server = MockServer::start().await;
        mount_common(&server).await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "model": MODEL,
                "response": "{}",
                "thinking": thinking,
                "done": done,
                "done_reason": done_reason
            })))
            .expect(1)
            .mount(&server)
            .await;
        let token = CancellationToken::new();
        let error = backend(&server)
            .complete_structured(request(), context(&token))
            .await
            .expect_err("nonterminal response is rejected");
        assert_eq!(error.code, "invalid_generation_response");
    }
}

#[tokio::test]
async fn discards_structured_output_when_artifact_identity_drifts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.13.0"})))
        .expect(2)
        .mount(&server)
        .await;
    let calls = Arc::new(AtomicUsize::new(0));
    let sequence = Arc::clone(&calls);
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(move |_: &wiremock::Request| {
            let digest = if sequence.fetch_add(1, Ordering::SeqCst) == 0 {
                DIGEST
            } else {
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            };
            let mut body = tag_body();
            body["models"][0]["digest"] = json!(format!("sha256:{digest}"));
            ResponseTemplate::new(200).set_body_json(body)
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show_body()))
        .expect(1)
        .mount(&server)
        .await;
    mount_generation(&server, "{}", "stop").await;
    let token = CancellationToken::new();
    let error = backend(&server)
        .complete_structured(request(), context(&token))
        .await
        .expect_err("drifted output is discarded");
    assert_eq!(error.code, "bound_model_digest_changed");
}

#[tokio::test]
async fn observes_structured_cancellation_before_network_work() {
    let server = MockServer::start().await;
    let token = CancellationToken::new();
    token.cancel();
    let error = backend(&server)
        .complete_structured(request(), context(&token))
        .await
        .expect_err("cancelled completion fails");
    assert_eq!(error.kind, InferenceErrorKind::Cancelled);
    assert!(
        server
            .received_requests()
            .await
            .expect("request recording enabled")
            .is_empty()
    );
}

#[tokio::test]
async fn rejects_unadmitted_structured_contract_without_network_work() {
    let server = MockServer::start().await;
    let mut request = request();
    request.output.schema_json = r#"{"type":"object"}"#.to_owned();
    request.output.schema_digest = Digest::sha256(request.output.schema_json.as_bytes());
    let token = CancellationToken::new();
    let error = backend(&server)
        .complete_structured(request, context(&token))
        .await
        .expect_err("unadmitted contract is rejected");
    assert_eq!(error.kind, InferenceErrorKind::Compatibility);
    assert_eq!(error.code, "unsupported_output_contract");
    assert!(
        server
            .received_requests()
            .await
            .expect("request recording enabled")
            .is_empty()
    );
}

#[tokio::test]
async fn cancellation_discards_in_flight_structured_output() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.13.0"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tag_body()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show_body()))
        .expect(1)
        .mount(&server)
        .await;
    let token = CancellationToken::new();
    let cancelling = token.clone();
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(move |_: &wiremock::Request| {
            cancelling.cancel();
            ResponseTemplate::new(200)
                .set_body_json(json!({
                    "model": MODEL,
                    "response": "{}",
                    "thinking": "",
                    "done": true,
                    "done_reason": "stop"
                }))
                .set_delay(Duration::from_mins(1))
        })
        .expect(1)
        .mount(&server)
        .await;
    let error = backend(&server)
        .complete_structured(request(), OperationContext::new(&token, None))
        .await
        .expect_err("in-flight cancellation discards output");
    assert_eq!(error.kind, InferenceErrorKind::Cancelled);
}

#[tokio::test]
async fn discards_structured_output_when_runtime_identity_drifts() {
    let server = MockServer::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let sequence = Arc::clone(&calls);
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(move |_: &wiremock::Request| {
            let version = if sequence.fetch_add(1, Ordering::SeqCst) == 0 {
                "0.13.0"
            } else {
                "0.13.1"
            };
            ResponseTemplate::new(200).set_body_json(json!({"version": version}))
        })
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tag_body()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show_body()))
        .expect(1)
        .mount(&server)
        .await;
    mount_generation(&server, "{}", "stop").await;
    let token = CancellationToken::new();
    let error = backend(&server)
        .complete_structured(request(), context(&token))
        .await
        .expect_err("runtime-drifted output is discarded");
    assert_eq!(error.kind, InferenceErrorKind::Compatibility);
    assert_eq!(error.code, "runtime_changed_during_generation");
}
