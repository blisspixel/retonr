use rewrite_inference::InferenceErrorKind;
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use super::{
    LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION, LocalOllamaModelPlan, LocalOllamaPreflightError,
    LocalOllamaPreflightMode, LocalOllamaPreflightPlan, MAX_LOCAL_OLLAMA_MODELS,
    MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES, parse_local_ollama_preflight_plan,
    run_local_ollama_preflight,
};

const MODEL: &str = "fixture:latest";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn plan(server: &MockServer) -> LocalOllamaPreflightPlan {
    plan_at(&server.uri())
}

fn plan_at(endpoint: &str) -> LocalOllamaPreflightPlan {
    LocalOllamaPreflightPlan {
        schema_version: LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
        plan_id: "fixture-preflight-v1".to_owned(),
        mode: LocalOllamaPreflightMode::Observe,
        endpoint: endpoint.to_owned(),
        expected_runtime_version: "0.32.14".to_owned(),
        require_idle: true,
        models: vec![LocalOllamaModelPlan {
            reference: MODEL.to_owned(),
            inventory_digest: Digest::from_sha256_hex(DIGEST).expect("fixture digest"),
            expected_details: None,
        }],
    }
}

fn tags() -> serde_json::Value {
    json!({"models": [{
        "name": MODEL,
        "model": MODEL,
        "size": 1024,
        "digest": DIGEST
    }]})
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

async fn mount(server: &MockServer) {
    mount_with_running(server, json!({"models": []})).await;
}

async fn mount_with_running(server: &MockServer, running: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.14"})))
        .expect(2)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(tags()))
        .expect(2)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(show()))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(running))
        .expect(2)
        .mount(server)
        .await;
}

#[tokio::test]
async fn observes_then_verifies_frozen_details_without_qualification() {
    let server = MockServer::start().await;
    mount(&server).await;
    let observed = run_local_ollama_preflight(&plan(&server), &CancellationToken::new())
        .await
        .expect("observe preflight");
    assert!(!observed.qualified);
    assert!(!observed.is_verified());
    assert!(observed.observed.running.is_empty());
    assert_eq!(observed.observed.bindings[0].details.family, "fixture");

    let verify_server = MockServer::start().await;
    mount(&verify_server).await;
    let mut verify = plan(&verify_server);
    verify.mode = LocalOllamaPreflightMode::Verify;
    verify.models[0].expected_details = Some(observed.observed.bindings[0].details.clone());
    let verified = run_local_ollama_preflight(&verify, &CancellationToken::new())
        .await
        .expect("verify preflight");
    assert!(verified.is_verified());
    assert!(!verified.qualified);
    let encoded = serde_json::to_string(&verified).expect("serialize report");
    assert!(!encoded.contains("fixture license"));
    assert!(!encoded.contains("fixture template"));
}

#[tokio::test]
async fn rejects_version_detail_and_idle_mismatches() {
    let version_server = MockServer::start().await;
    mount(&version_server).await;
    let mut wrong_version = plan(&version_server);
    wrong_version.expected_runtime_version = "0.32.13".to_owned();
    assert!(matches!(
        run_local_ollama_preflight(&wrong_version, &CancellationToken::new()).await,
        Err(LocalOllamaPreflightError::RuntimeVersionMismatch)
    ));

    let detail_server = MockServer::start().await;
    mount(&detail_server).await;
    let mut wrong_details = plan(&detail_server);
    wrong_details.mode = LocalOllamaPreflightMode::Verify;
    wrong_details.models[0].expected_details = Some(rewrite_ollama::OllamaModelDetails {
        format: "gguf".to_owned(),
        family: "other".to_owned(),
        quantization: "Q4_K_M".to_owned(),
        capabilities: vec!["completion".to_owned()],
        license_digest: Digest::sha256(b"fixture license"),
        template_digest: Digest::sha256(b"fixture template"),
        metadata_digest: Digest::sha256(b"metadata"),
    });
    assert!(matches!(
        run_local_ollama_preflight(&wrong_details, &CancellationToken::new()).await,
        Err(LocalOllamaPreflightError::ModelDetailsMismatch)
    ));

    let resident_server = MockServer::start().await;
    mount_with_running(
        &resident_server,
        json!({"models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": DIGEST,
            "size_vram": 768,
            "context_length": 4096
        }]}),
    )
    .await;
    assert!(matches!(
        run_local_ollama_preflight(&plan(&resident_server), &CancellationToken::new()).await,
        Err(LocalOllamaPreflightError::RuntimeNotIdle)
    ));
}

#[tokio::test]
async fn cancellation_prevents_preflight_network_work() {
    let server = MockServer::start().await;
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let error = run_local_ollama_preflight(&plan(&server), &cancellation)
        .await
        .expect_err("pre-cancelled preflight fails");
    assert!(matches!(
        error,
        LocalOllamaPreflightError::Backend(error)
            if error.kind == InferenceErrorKind::Cancelled && error.code == "cancelled"
    ));
}

#[test]
fn parser_rejects_unknown_fields_noncanonical_models_and_mode_mismatch() {
    let unknown = br#"{
        "schema_version":1,
        "plan_id":"fixture",
        "mode":"observe",
        "endpoint":"http://127.0.0.1:11434",
        "expected_runtime_version":"0.32.14",
        "require_idle":true,
        "models":[],
        "extra":true
    }"#;
    assert!(matches!(
        parse_local_ollama_preflight_plan(unknown),
        Err(LocalOllamaPreflightError::InvalidJson)
    ));

    let invalid_endpoint = plan_at("http://localhost:11434");
    let bytes = serde_json::to_vec(&invalid_endpoint).expect("serialize invalid endpoint");
    assert!(matches!(
        parse_local_ollama_preflight_plan(&bytes),
        Err(LocalOllamaPreflightError::InvalidEndpoint)
    ));

    let mut invalid = plan_at("http://127.0.0.1:11434");
    invalid.models.push(invalid.models[0].clone());
    let bytes = serde_json::to_vec(&invalid).expect("serialize invalid plan");
    assert!(matches!(
        parse_local_ollama_preflight_plan(&bytes),
        Err(LocalOllamaPreflightError::InvalidPlan)
    ));

    invalid.models.truncate(1);
    let mut duplicate_digest = invalid.models[0].clone();
    duplicate_digest.reference = "zzz:latest".to_owned();
    invalid.models.push(duplicate_digest);
    let bytes = serde_json::to_vec(&invalid).expect("serialize duplicate digest");
    assert!(matches!(
        parse_local_ollama_preflight_plan(&bytes),
        Err(LocalOllamaPreflightError::InvalidPlan)
    ));

    invalid.models.truncate(1);
    invalid.mode = LocalOllamaPreflightMode::Verify;
    let bytes = serde_json::to_vec(&invalid).expect("serialize mode mismatch");
    assert!(matches!(
        parse_local_ollama_preflight_plan(&bytes),
        Err(LocalOllamaPreflightError::InvalidPlan)
    ));
}

#[test]
fn parser_enforces_fixed_schema_size_count_order_and_metadata_bounds() {
    let oversized = vec![b' '; MAX_LOCAL_OLLAMA_PREFLIGHT_PLAN_BYTES + 1];
    assert!(matches!(
        parse_local_ollama_preflight_plan(&oversized),
        Err(LocalOllamaPreflightError::TooLarge)
    ));

    let mut unsupported = plan_at("http://127.0.0.1:11434");
    unsupported.schema_version += 1;
    assert_plan_error(&unsupported, &LocalOllamaPreflightError::UnsupportedSchema);

    let mut invalid_label = plan_at("http://127.0.0.1:11434");
    invalid_label.plan_id = "Not-Canonical".to_owned();
    assert_plan_error(&invalid_label, &LocalOllamaPreflightError::InvalidPlan);

    let mut empty = plan_at("http://127.0.0.1:11434");
    empty.models.clear();
    assert_plan_error(&empty, &LocalOllamaPreflightError::InvalidPlan);

    let mut too_many = plan_at("http://127.0.0.1:11434");
    too_many.models = (0..=MAX_LOCAL_OLLAMA_MODELS)
        .map(|index| LocalOllamaModelPlan {
            reference: format!("fixture-{index:02}:latest"),
            inventory_digest: Digest::sha256(&index.to_le_bytes()),
            expected_details: None,
        })
        .collect();
    assert_plan_error(&too_many, &LocalOllamaPreflightError::InvalidPlan);

    let mut unordered = plan_at("http://127.0.0.1:11434");
    let mut second = unordered.models[0].clone();
    second.reference = "aaa:latest".to_owned();
    second.inventory_digest = Digest::sha256(b"second model");
    unordered.models.push(second);
    assert_plan_error(&unordered, &LocalOllamaPreflightError::InvalidPlan);

    let mut unsorted_metadata = plan_at("http://127.0.0.1:11434");
    unsorted_metadata.mode = LocalOllamaPreflightMode::Verify;
    unsorted_metadata.models[0].expected_details = Some(rewrite_ollama::OllamaModelDetails {
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        quantization: "Q4_K_M".to_owned(),
        capabilities: vec!["vision".to_owned(), "completion".to_owned()],
        license_digest: Digest::sha256(b"license"),
        template_digest: Digest::sha256(b"template"),
        metadata_digest: Digest::sha256(b"metadata"),
    });
    assert_plan_error(&unsorted_metadata, &LocalOllamaPreflightError::InvalidPlan);
}

fn assert_plan_error(plan: &LocalOllamaPreflightPlan, expected: &LocalOllamaPreflightError) {
    let bytes = serde_json::to_vec(plan).expect("serialize rejected plan");
    let actual = parse_local_ollama_preflight_plan(&bytes).expect_err("plan must fail closed");
    assert_eq!(
        std::mem::discriminant(&actual),
        std::mem::discriminant(expected)
    );
}
