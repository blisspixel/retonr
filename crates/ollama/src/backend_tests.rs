use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use rewrite_inference::{
    GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest, InferenceBackend, InferenceErrorKind,
    OperationContext, ReasoningPolicy, SamplingParameters, candidate_output_contract,
};
use rewrite_model::ArtifactId;
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, body_partial_json, method, path},
};

use crate::{OllamaBackend, OllamaEndpoint, OllamaLimits, OllamaModelBinding};

const MODEL: &str = "fixture:latest";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn binding() -> OllamaModelBinding {
    let digest = Digest::from_sha256_hex(DIGEST).expect("fixture digest");
    OllamaModelBinding::new(MODEL, ArtifactId::from_digest(digest.clone()), digest)
        .expect("fixture binding")
}

fn context(token: &CancellationToken) -> OperationContext<'_> {
    OperationContext::new(token, Some(Instant::now() + Duration::from_secs(5)))
}

fn backend(server: &MockServer) -> OllamaBackend {
    backend_with_limits(server, OllamaLimits::default())
}

fn backend_with_limits(server: &MockServer, limits: OllamaLimits) -> OllamaBackend {
    OllamaBackend::new(
        OllamaEndpoint::parse(&server.uri()).expect("wiremock endpoint"),
        vec![binding()],
        limits,
    )
    .expect("Ollama backend")
}

fn tag_body(digest: &str) -> serde_json::Value {
    json!({
        "models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": format!("sha256:{digest}")
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

async fn mount_common(server: &MockServer, version_count: u64, tag_count: u64) {
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.13.0"})))
        .expect(version_count)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tag_body(DIGEST)))
        .expect(tag_count)
        .mount(server)
        .await;
}

fn request(binding: &OllamaModelBinding) -> GenerationRequest {
    GenerationRequest {
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
    }
}

#[tokio::test]
async fn discovers_only_exact_bound_generation_models() {
    let server = MockServer::start().await;
    mount_common(&server, 1, 1).await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .and(body_json(json!({"model": MODEL, "verbose": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json(show_body()))
        .expect(1)
        .mount(&server)
        .await;
    let token = CancellationToken::new();
    let discovery = backend(&server)
        .discover(context(&token))
        .await
        .expect("discovery succeeds");
    assert_eq!(discovery.backend_id.as_str(), "ollama_native");
    assert_eq!(discovery.runtime.version, "0.13.0");
    assert_eq!(discovery.inventory.len(), 1);
    assert_eq!(discovery.inventory[0].artifact_digest.as_str(), DIGEST);
    assert_eq!(
        discovery.capabilities.roles,
        vec![rewrite_model::ArtifactRole::Generation]
    );
    assert!(
        !discovery
            .capabilities
            .roles
            .contains(&rewrite_model::ArtifactRole::ClaimExtraction)
    );
    assert_eq!(
        discovery.capabilities.admitted_output_contract_digests,
        vec![candidate_output_contract().schema_digest]
    );
}

#[tokio::test]
async fn generates_bounded_candidates_and_rechecks_identity() {
    let server = MockServer::start().await;
    mount_common(&server, 2, 2).await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show_body()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .and(body_partial_json(json!({
            "model": MODEL,
            "stream": false,
            "think": false,
            "raw": false,
            "options": {
                "temperature": 0.2,
                "top_p": 0.9,
                "seed": 7,
                "num_ctx": 4096,
                "num_predict": 256,
                "stop": []
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": MODEL,
            "response": "{\"candidates\":[{\"text\":\"Clear replacement.\"}]}",
            "thinking": "",
            "done": true,
            "done_reason": "stop",
            "prompt_eval_count": 9,
            "eval_count": 3,
            "eval_duration": 12000,
            "total_duration": 18000
        })))
        .expect(1)
        .mount(&server)
        .await;
    let binding = binding();
    let token = CancellationToken::new();
    let response = backend(&server)
        .generate(request(&binding), context(&token))
        .await
        .expect("generation succeeds");
    assert_eq!(response.candidates[0].text, "Clear replacement.");
    assert_eq!(response.usage.generation_micros, Some(12));
}

#[tokio::test]
async fn rejects_digest_drift_before_generation() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tag_body(
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        )))
        .expect(1)
        .mount(&server)
        .await;
    let binding = binding();
    let token = CancellationToken::new();
    let error = backend(&server)
        .generate(request(&binding), context(&token))
        .await
        .expect_err("digest drift is rejected");
    assert_eq!(error.kind, InferenceErrorKind::Policy);
    assert_eq!(error.code, "bound_model_digest_changed");
}

#[tokio::test]
async fn observes_cancellation_before_network_work() {
    let server = MockServer::start().await;
    let token = CancellationToken::new();
    token.cancel();
    let error = backend(&server)
        .discover(context(&token))
        .await
        .expect_err("cancelled discovery fails");
    assert_eq!(error.kind, InferenceErrorKind::Cancelled);
}

#[tokio::test]
async fn rejects_redirects_without_following_location() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", "/api/redirected-version"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let token = CancellationToken::new();
    let error = backend(&server)
        .discover(context(&token))
        .await
        .expect_err("redirect is rejected");
    assert_eq!(error.kind, InferenceErrorKind::Permanent);
    assert_eq!(error.code, "http_rejected");
}

#[tokio::test]
async fn rejects_oversized_and_wrong_content_type_responses() {
    let oversized_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.13.0"})))
        .expect(1)
        .mount(&oversized_server)
        .await;
    let token = CancellationToken::new();
    let error = backend_with_limits(
        &oversized_server,
        OllamaLimits {
            discovery_body_bytes: 8,
            ..OllamaLimits::default()
        },
    )
    .discover(context(&token))
    .await
    .expect_err("oversized body is rejected");
    assert_eq!(error.kind, InferenceErrorKind::MalformedResponse);
    assert_eq!(error.code, "response_body_too_large");

    let content_type_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(r#"{"version":"0.13.0"}"#, "text/plain"),
        )
        .expect(1)
        .mount(&content_type_server)
        .await;
    let error = backend(&content_type_server)
        .discover(context(&token))
        .await
        .expect_err("wrong content type is rejected");
    assert_eq!(error.kind, InferenceErrorKind::MalformedResponse);
    assert_eq!(error.code, "unexpected_content_type");
}

#[tokio::test]
async fn classifies_disconnected_transport_without_content() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with_err(|_: &wiremock::Request| {
            std::io::Error::new(std::io::ErrorKind::ConnectionReset, "fixture reset")
        })
        .expect(1)
        .mount(&server)
        .await;
    let token = CancellationToken::new();
    let error = backend(&server)
        .discover(context(&token))
        .await
        .expect_err("disconnect is classified");
    assert_eq!(error.kind, InferenceErrorKind::Retryable);
    assert!(!error.code.contains("fixture"));
}

#[tokio::test]
async fn expired_deadline_prevents_network_work() {
    let server = MockServer::start().await;
    let token = CancellationToken::new();
    let operation = OperationContext::new(&token, Some(Instant::now()));
    let error = backend(&server)
        .discover(operation)
        .await
        .expect_err("deadline ends request");
    assert_eq!(error.kind, InferenceErrorKind::Deadline);
    assert!(
        server
            .received_requests()
            .await
            .expect("request recording enabled")
            .is_empty()
    );
}

#[tokio::test]
async fn cancellation_terminates_in_flight_discovery() {
    let server = MockServer::start().await;
    let token = CancellationToken::new();
    let cancelling = token.clone();
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(move |_: &wiremock::Request| {
            cancelling.cancel();
            ResponseTemplate::new(200)
                .set_body_json(json!({"version": "0.13.0"}))
                .set_delay(Duration::from_mins(1))
        })
        .expect(1)
        .mount(&server)
        .await;
    let error = backend(&server)
        .discover(OperationContext::new(&token, None))
        .await
        .expect_err("cancellation ends request");
    assert_eq!(error.kind, InferenceErrorKind::Cancelled);
}

#[tokio::test]
async fn discards_generation_when_digest_changes_after_response() {
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
            ResponseTemplate::new(200).set_body_json(tag_body(digest))
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
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": MODEL,
            "response": "{\"candidates\":[{\"text\":\"discard me\"}]}",
            "thinking": "",
            "done": true,
            "done_reason": "stop"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let binding = binding();
    let token = CancellationToken::new();
    let error = backend(&server)
        .generate(request(&binding), context(&token))
        .await
        .expect_err("post-call digest drift discards response");
    assert_eq!(error.kind, InferenceErrorKind::Policy);
    assert_eq!(error.code, "bound_model_digest_changed");
}

#[tokio::test]
async fn rejects_truncated_response_body() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("listener address");
    let server = tokio::spawn(async move {
        let (mut socket, _peer) = listener.accept().await.expect("accept request");
        let mut request = [0_u8; 1024];
        let _read = socket.read(&mut request).await.expect("read request");
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 64\r\n\r\n{\"version\":",
            )
            .await
            .expect("write partial response");
        socket.shutdown().await.expect("close partial response");
    });
    let endpoint =
        OllamaEndpoint::parse(&format!("http://{address}")).expect("test loopback endpoint");
    let backend = OllamaBackend::new(endpoint, vec![binding()], OllamaLimits::default())
        .expect("test backend");
    let token = CancellationToken::new();
    let error = backend
        .discover(context(&token))
        .await
        .expect_err("truncated body is rejected");
    server.await.expect("test server joins");
    assert_eq!(error.kind, InferenceErrorKind::Retryable);
    assert_eq!(error.code, "transport_failed");
}

#[tokio::test]
async fn deadline_terminates_response_stalled_after_headers() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("bind test listener");
    let address = listener.local_addr().expect("listener address");
    let (headers_sent, headers_received) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _peer) = listener.accept().await.expect("accept request");
        let mut request = [0_u8; 1024];
        let _read = socket.read(&mut request).await.expect("read request");
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 64\r\n\r\n",
            )
            .await
            .expect("write response headers");
        headers_sent.send(()).expect("signal response headers");
        std::future::pending::<()>().await;
    });
    let endpoint =
        OllamaEndpoint::parse(&format!("http://{address}")).expect("test loopback endpoint");
    let backend = OllamaBackend::new(
        endpoint,
        vec![binding()],
        OllamaLimits {
            request_timeout: Duration::from_mins(3),
            read_timeout: Duration::from_mins(2),
            ..OllamaLimits::default()
        },
    )
    .expect("test backend");
    let token = CancellationToken::new();
    let operation = tokio::spawn(async move {
        let context = OperationContext::new(&token, Some(Instant::now() + Duration::from_mins(1)));
        backend.discover(context).await
    });
    headers_received.await.expect("response headers observed");
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(61)).await;
    let error = operation
        .await
        .expect("operation joins")
        .expect_err("deadline ends stalled body");
    server.abort();
    assert_eq!(error.kind, InferenceErrorKind::Deadline);
    assert_eq!(error.code, "deadline");
}
