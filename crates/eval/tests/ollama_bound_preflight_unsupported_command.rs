#![cfg(target_os = "macos")]

use std::fs;

use assert_cmd::Command;
use serde_json::json;
use tempfile::tempdir;

const MODEL: &str = "fixture:latest";
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

#[test]
fn bound_preflight_fails_closed_before_http_on_macos() {
    let directory = tempdir().expect("temporary directory");
    let plan = directory.path().join("bound-preflight.json");
    fs::write(
        &plan,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "preflight": {
                "schema_version": 1,
                "plan_id": "unsupported-bound-process-preflight-v1",
                "mode": "observe",
                "endpoint": "http://127.0.0.1:9",
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

    let output = Command::cargo_bin("rewrite-eval")
        .expect("compiled evaluation runner")
        .arg("--ollama-bound-preflight")
        .arg(plan)
        .output()
        .expect("run evaluation process");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("utf-8 error"),
        "error: bound Ollama native witness failed: attached process witness is unsupported on this platform\n"
    );
}
