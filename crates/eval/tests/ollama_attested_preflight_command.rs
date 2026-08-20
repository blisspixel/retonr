#![cfg(any(target_os = "linux", windows))]

use std::fs;

use assert_cmd::Command;
use serde_json::json;
use tempfile::tempdir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const MODEL: &str = "fixture:latest";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[tokio::test]
async fn attached_preflight_runs_as_a_process_without_overstating_evidence() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.14"})))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": DIGEST
        }]})))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "license": "private fixture license",
            "template": "private fixture template",
            "capabilities": ["completion"],
            "details": {
                "format": "gguf",
                "family": "fixture",
                "quantization_level": "Q4_K_M"
            },
            "model_info": {"fixture.context_length": 4096}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .expect(2)
        .mount(&server)
        .await;

    let directory = tempdir().expect("temporary directory");
    let plan = directory.path().join("attached-preflight.json");
    fs::write(
        &plan,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "preflight": {
                "schema_version": 1,
                "plan_id": "attached-process-preflight-v1",
                "mode": "observe",
                "endpoint": server.uri(),
                "expected_runtime_version": "0.32.14",
                "require_idle": true,
                "models": [{
                    "reference": MODEL,
                    "inventory_digest": DIGEST
                }]
            },
            "maximum_entrypoint_bytes": 536_870_912
        }))
        .expect("serialize plan"),
    )
    .expect("write plan");

    let output = tokio::task::spawn_blocking(move || {
        Command::cargo_bin("rewrite-eval")
            .expect("compiled evaluation runner")
            .arg("--ollama-attested-preflight")
            .arg(plan)
            .output()
            .expect("run evaluation process")
    })
    .await
    .expect("join evaluation process");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8 report");
    assert!(stdout.contains("\"process_evidence_level\": \"observed_native_listener\""));
    assert!(stdout.contains("\"response_bound\": false"));
    assert!(stdout.contains("\"qualified\": false"));
    assert!(!stdout.contains("\"runtime_build_id\""));
    assert!(!stdout.contains("private fixture license"));
    assert!(!stdout.contains("private fixture template"));
}
