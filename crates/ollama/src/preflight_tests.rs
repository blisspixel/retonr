use std::time::{Duration, Instant};

use rewrite_inference::{InferenceErrorKind, OperationContext};
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{body_json, method, path},
};

use crate::{OllamaBackend, OllamaEndpoint, OllamaLimits, OllamaPreflightTarget};

const MODEL: &str = "fixture:latest";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn backend(server: &MockServer) -> OllamaBackend {
    let digest = Digest::from_sha256_hex(DIGEST).expect("fixture digest");
    let target = OllamaPreflightTarget::new(MODEL, digest).expect("fixture target");
    OllamaBackend::new_preflight(
        OllamaEndpoint::parse(&server.uri()).expect("wiremock endpoint"),
        vec![target],
        OllamaLimits::default(),
    )
    .expect("Ollama backend")
}

fn tags(digest: &str) -> serde_json::Value {
    json!({"models": [
        {
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": digest
        },
        {
            "name": "aaa:latest",
            "model": "aaa:latest",
            "size": 512,
            "digest": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        }
    ]})
}

fn show() -> serde_json::Value {
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

fn context(token: &CancellationToken) -> OperationContext<'_> {
    OperationContext::new(token, Some(Instant::now() + Duration::from_secs(5)))
}

async fn mount_preflight(
    server: &MockServer,
    running: serde_json::Value,
    stable_read_count: u64,
    running_read_count: u64,
    show_read_count: u64,
) {
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.14"})))
        .expect(stable_read_count)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tags(DIGEST)))
        .expect(stable_read_count)
        .mount(server)
        .await;
    if show_read_count > 0 {
        Mock::given(method("POST"))
            .and(path("/api/show"))
            .and(body_json(json!({"model": MODEL, "verbose": true})))
            .respond_with(ResponseTemplate::new(200).set_body_json(show()))
            .expect(show_read_count)
            .mount(server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(running))
        .expect(running_read_count)
        .mount(server)
        .await;
}

#[tokio::test]
async fn captures_stable_read_only_preflight_evidence() {
    let server = MockServer::start().await;
    mount_preflight(
        &server,
        json!({"models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": DIGEST,
            "size_vram": 768,
            "context_length": 4096
        }]}),
        2,
        2,
        1,
    )
    .await;
    let token = CancellationToken::new();
    let report = backend(&server)
        .preflight(context(&token))
        .await
        .expect("stable preflight");
    assert_eq!(report.runtime.version, "0.32.14");
    assert_eq!(report.inventory.len(), 2);
    assert_eq!(report.inventory[0].reference, "aaa:latest");
    assert_eq!(report.bindings[0].inventory_digest.as_str(), DIGEST);
    assert_eq!(report.bindings[0].details.family, "fixture");
    assert_eq!(report.running[0].context_tokens, 4096);
    let serialized = serde_json::to_string(&report).expect("serialize preflight");
    assert!(!serialized.contains("fixture license"));
    assert!(!serialized.contains("fixture template"));
}

#[tokio::test]
async fn rejects_runtime_drift() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.14"})))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.15"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tags(DIGEST)))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .expect(2)
        .mount(&server)
        .await;
    let token = CancellationToken::new();
    let error = backend(&server)
        .preflight(context(&token))
        .await
        .expect_err("runtime drift fails closed");
    assert_eq!(error.kind, InferenceErrorKind::Compatibility);
    assert_eq!(error.code, "runtime_changed_during_preflight");
}

#[tokio::test]
async fn rejects_inventory_drift() {
    let inventory_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.14"})))
        .expect(2)
        .mount(&inventory_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tags(DIGEST)))
        .up_to_n_times(1)
        .mount(&inventory_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tags(
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        )))
        .expect(1)
        .mount(&inventory_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show()))
        .expect(1)
        .mount(&inventory_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .expect(2)
        .mount(&inventory_server)
        .await;
    let token = CancellationToken::new();
    let inventory_error = backend(&inventory_server)
        .preflight(context(&token))
        .await
        .expect_err("inventory drift fails closed");
    assert_eq!(inventory_error.kind, InferenceErrorKind::Compatibility);
    assert_eq!(inventory_error.code, "runtime_changed_during_preflight");
}

#[tokio::test]
async fn rejects_residency_drift() {
    let residency_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.14"})))
        .expect(2)
        .mount(&residency_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tags(DIGEST)))
        .expect(2)
        .mount(&residency_server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show()))
        .expect(1)
        .mount(&residency_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .up_to_n_times(1)
        .mount(&residency_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": DIGEST,
            "size_vram": 768,
            "context_length": 4096
        }]})))
        .expect(1)
        .mount(&residency_server)
        .await;
    let token = CancellationToken::new();
    let residency_error = backend(&residency_server)
        .preflight(context(&token))
        .await
        .expect_err("residency drift fails closed");
    assert_eq!(residency_error.kind, InferenceErrorKind::Compatibility);
    assert_eq!(residency_error.code, "runtime_changed_during_preflight");
}

#[tokio::test]
async fn rejects_remote_or_invalid_running_state() {
    for running in [
        json!({"models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": DIGEST,
            "size_vram": 768,
            "context_length": 0
        }]}),
        json!({"models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": DIGEST,
            "size_vram": 768,
            "context_length": 4096,
            "remote_model": "cloud-model",
            "remote_host": "https://example.invalid"
        }]}),
    ] {
        let server = MockServer::start().await;
        mount_preflight(&server, running, 1, 1, 0).await;
        let token = CancellationToken::new();
        let error = backend(&server)
            .preflight(context(&token))
            .await
            .expect_err("invalid residency fails closed");
        assert_eq!(error.kind, InferenceErrorKind::MalformedResponse);
        assert_eq!(error.code, "invalid_running_inventory_entry");
    }
}

#[test]
fn rejects_response_limits_above_fixed_adapter_ceilings() {
    let endpoint = OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback endpoint");
    let discovery_error = OllamaBackend::new(
        endpoint.clone(),
        Vec::new(),
        OllamaLimits {
            discovery_body_bytes: 16 * 1024 * 1024 + 1,
            ..OllamaLimits::default()
        },
    )
    .expect_err("oversized discovery limit is rejected");
    assert_eq!(discovery_error.kind, InferenceErrorKind::Policy);
    assert_eq!(discovery_error.code, "invalid_limits");

    let generation_error = OllamaBackend::new(
        endpoint,
        Vec::new(),
        OllamaLimits {
            generation_body_bytes: 16 * 1024 * 1024 + 1,
            ..OllamaLimits::default()
        },
    )
    .expect_err("oversized generation limit is rejected");
    assert_eq!(generation_error.kind, InferenceErrorKind::Policy);
    assert_eq!(generation_error.code, "invalid_limits");
}

#[tokio::test]
async fn rejects_invalid_preflight_configuration_before_network_work() {
    let endpoint = OllamaEndpoint::parse("http://127.0.0.1:11434").expect("loopback endpoint");
    let digest = Digest::from_sha256_hex(DIGEST).expect("fixture digest");
    assert!(OllamaPreflightTarget::new("", digest.clone()).is_err());

    let empty_error =
        OllamaBackend::new_preflight(endpoint.clone(), Vec::new(), OllamaLimits::default())
            .expect_err("empty preflight target set is rejected");
    assert_eq!(empty_error.code, "invalid_preflight_targets");

    let duplicate_error = OllamaBackend::new_preflight(
        endpoint.clone(),
        vec![
            OllamaPreflightTarget::new("a:latest", digest.clone()).expect("first target"),
            OllamaPreflightTarget::new("b:latest", digest).expect("second target"),
        ],
        OllamaLimits::default(),
    )
    .expect_err("duplicate preflight digests are rejected");
    assert_eq!(duplicate_error.code, "duplicate_preflight_target");

    let too_many = (0_u8..65)
        .map(|index| {
            OllamaPreflightTarget::new(format!("fixture-{index}"), Digest::sha256(&[index]))
                .expect("bounded target")
        })
        .collect();
    let oversized_error =
        OllamaBackend::new_preflight(endpoint.clone(), too_many, OllamaLimits::default())
            .expect_err("oversized preflight target set is rejected");
    assert_eq!(oversized_error.code, "invalid_preflight_targets");

    let backend = OllamaBackend::new(endpoint, Vec::new(), OllamaLimits::default())
        .expect("generation adapter without bindings");
    let token = CancellationToken::new();
    let unconfigured_error = backend
        .preflight(context(&token))
        .await
        .expect_err("generation adapter cannot run preflight");
    assert_eq!(unconfigured_error.code, "preflight_not_configured");
}
