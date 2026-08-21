#![cfg(any(target_os = "linux", windows))]

use std::fs;

use assert_cmd::Command;
use serde_json::{Value, json};
use tempfile::tempdir;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

const MODEL: &str = "fixture:latest";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

async fn assert_preflight_requests(server: &MockServer) {
    let requests = server
        .received_requests()
        .await
        .expect("request recording enabled");
    let request_count = |request_method: &str, request_path: &str| {
        requests
            .iter()
            .filter(|request| {
                request.method.as_str() == request_method && request.url.path() == request_path
            })
            .count()
    };
    assert_eq!(requests.len(), 7);
    assert_eq!(request_count("GET", "/api/version"), 2);
    assert_eq!(request_count("GET", "/api/tags"), 2);
    assert_eq!(request_count("POST", "/api/show"), 1);
    assert_eq!(request_count("GET", "/api/ps"), 2);
}

async fn mount_preflight(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.14"})))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": DIGEST
        }]})))
        .mount(server)
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
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .mount(server)
        .await;
}

#[tokio::test]
async fn bound_preflight_process_emits_redacted_inert_connection_evidence() {
    let server = MockServer::start().await;
    mount_preflight(&server).await;
    let directory = tempdir().expect("temporary directory");
    let plan = directory.path().join("bound-preflight.json");
    fs::write(
        &plan,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "preflight": {
                "schema_version": 1,
                "plan_id": "bound-process-preflight-v1",
                "mode": "observe",
                "endpoint": server.uri(),
                "expected_runtime_version": "0.32.14",
                "require_idle": true,
                "models": [{
                    "reference": MODEL,
                    "inventory_digest": DIGEST
                }]
            },
            "maximum_entrypoint_bytes": 536_870_912,
            "maximum_session_body_bytes": 16_777_216
        }))
        .expect("serialize plan"),
    )
    .expect("write plan");

    let output = tokio::task::spawn_blocking(move || {
        Command::cargo_bin("rewrite-eval")
            .expect("compiled evaluation runner")
            .arg("--ollama-bound-preflight")
            .arg(plan)
            .output()
            .expect("run evaluation process")
    })
    .await
    .expect("join evaluation process");

    #[cfg(target_os = "linux")]
    if !output.status.success() {
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).expect("utf-8 error");
        assert!(
            stderr
                == "error: bound Ollama native witness failed: attached process witness process access was denied\n"
                || stderr
                    == "error: bound Ollama native witness failed: attached process witness listener snapshot is incomplete\n"
                || stderr
                    == "error: bound Ollama native witness failed: attached process witness connection snapshot is incomplete\n",
            "unexpected Linux failure: {stderr}"
        );
        assert!(
            server
                .received_requests()
                .await
                .expect("request recording enabled")
                .is_empty(),
            "native witness failure must precede HTTP"
        );
        return;
    }
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_preflight_requests(&server).await;
    let report: Value = serde_json::from_slice(&output.stdout).expect("bound report JSON");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(
        report["process_evidence_level"],
        "observed_native_connection_attribution"
    );
    assert!(report.get("process_witness").is_some());
    assert!(report.get("connection_witness").is_some());
    assert!(report.get("final_process_witness").is_none());
    assert!(report.get("final_connection_witness").is_none());
    assert_eq!(
        report["connection_observations"]
            .as_array()
            .expect("connection observations")
            .len(),
        8
    );
    assert_eq!(report["all_responses_used_retained_transport"], true);
    assert_eq!(
        report["kernel_attribution_checked_after_each_response"],
        true
    );
    assert_eq!(report["exclusive_socket_owner_proven"], false);
    assert_eq!(report["application_handler_proven"], false);
    assert_eq!(report["qualified"], false);
    assert!(!report.to_string().contains(&server.uri()));
    assert!(!report.to_string().contains("private fixture license"));
    assert!(!report.to_string().contains("private fixture template"));
    assert_no_endpoint_keys(&report);
}

fn assert_no_endpoint_keys(value: &Value) {
    match value {
        Value::Array(values) => values.iter().for_each(assert_no_endpoint_keys),
        Value::Object(object) => {
            for (key, value) in object {
                assert!(!matches!(
                    key.as_str(),
                    "address" | "addresses" | "client" | "server" | "port" | "ports"
                ));
                assert_no_endpoint_keys(value);
            }
        }
        _ => {}
    }
}
