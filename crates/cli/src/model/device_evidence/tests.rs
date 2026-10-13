use serde_json::json;

use super::super::ModelOutput;
use super::inspect::inspect;
use super::{KIND, MAXIMUM_SHORT_TEXT_BYTES, SCHEMA};
use crate::contract::{EXIT_COMPATIBILITY, EXIT_USAGE, ErrorBody, ErrorCategory, ErrorCode};

fn sample() -> serde_json::Value {
    json!({
        "schema": SCHEMA,
        "kind": KIND,
        "disclaimer": "This is a fitr measurement of one model on one device. It is not a retonr qualification, activation, or license decision.",
        "sister": "https://github.com/blisspixel/retonr",
        "fitr_version": "0.2.0",
        "model": "demo:8b",
        "quant": "Q4_K_M",
        "family": "qwen3",
        "param_size": "8B",
        "level": "full",
        "repeats": 3,
        "device": {
            "host": "secret-laptop",
            "os": "windows",
            "cpu": "secret-cpu",
            "ram_gb": 32.0,
            "gpu": "demo-gpu",
            "gpu_driver": "secret-driver",
            "ollama": "0.32.14",
            "inference_device": "GPU 100%",
            "gpu_backend": "cuda",
            "vram_gb": 8.0,
            "config": { "OLLAMA_MODELS": "C:\\\\secret\\\\models" }
        },
        "device_key": "opaque-key",
        "profile": "default",
        "needs": {
            "structured_output": { "state": "PASS", "why": "6/7" },
            "vision": { "state": "n/a", "why": "text-only" }
        },
        "serves": ["structured_output"],
        "use_for": "JSON pipelines",
        "plumbing": "healthy",
        "result": "C:\\\\Users\\\\secret\\\\.fitr\\\\results\\\\demo.json"
    })
}

#[test]
fn accepted_evidence_is_measurement_not_qualification() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("demo.retonr.json");
    std::fs::write(&path, sample().to_string()).expect("write evidence");
    let report = inspect(&path).expect("inspect");
    assert_eq!(report.kind, KIND);
    assert!(!report.qualified);
    assert_eq!(report.qualification, "absent");
    assert_eq!(report.model, "demo:8b");
    assert_eq!(report.device.gpu.as_deref(), Some("demo-gpu"));
    assert_eq!(report.device.runtime.as_deref(), Some("0.32.14"));
    assert_eq!(report.needs[0].name, "structured_output");
    assert_eq!(report.needs[1].state, "n/a");
    let encoded = serde_json::to_string(&report).expect("serialize");
    assert!(!encoded.contains("secret-laptop"));
    assert!(!encoded.contains("secret-cpu"));
    assert!(!encoded.contains("OLLAMA_MODELS"));
    assert!(!encoded.contains("C:\\\\Users"));
    assert!(!encoded.contains("secret-driver"));
    assert!(encoded.contains("\"qualified\":false"));
}

#[test]
fn dishonest_disclaimer_is_unsupported() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("bad.json");
    let mut value = sample();
    value["disclaimer"] = json!("ready to activate");
    std::fs::write(&path, value.to_string()).expect("write");
    let Err(failure) = inspect(&path) else {
        panic!("dishonest disclaimer should refuse");
    };
    assert_eq!(
        failure.exit_code,
        std::process::ExitCode::from(EXIT_COMPATIBILITY)
    );
}

#[test]
fn forwarded_fields_are_bounded_and_terminal_safe() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("unsafe.json");
    let mut value = sample();
    value["model"] = json!("demo\u{202e}:8b");
    std::fs::write(&path, value.to_string()).expect("write unsafe evidence");
    let Err(failure) = inspect(&path) else {
        panic!("directionality must be refused");
    };
    assert_eq!(
        failure.body,
        ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidManifest, false)
    );

    let mut value = sample();
    value["model"] = json!("x".repeat(MAXIMUM_SHORT_TEXT_BYTES + 1));
    std::fs::write(&path, value.to_string()).expect("write oversized evidence");
    let Err(failure) = inspect(&path) else {
        panic!("oversized model must be refused");
    };
    assert_eq!(
        failure.body,
        ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidManifest, false)
    );

    let mut value = sample();
    value["device"]["ram_gb"] = json!(-1.0);
    std::fs::write(&path, value.to_string()).expect("write invalid measurement");
    let Err(failure) = inspect(&path) else {
        panic!("negative memory must be refused");
    };
    assert_eq!(
        failure.body,
        ErrorBody::new(ErrorCategory::Usage, ErrorCode::InvalidManifest, false)
    );
}

#[test]
fn inconclusive_need_is_accepted_measurement() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("gemma.retonr.json");
    let mut value = sample();
    value["fitr_version"] = json!("0.11.3");
    value["model"] = json!("gemma4:e4b-it-qat");
    value["needs"]["fast_and_decent"] = json!({
        "state": "INCONCLUSIVE",
        "why": "no cache receipt"
    });
    std::fs::write(&path, value.to_string()).expect("write evidence");
    let report = inspect(&path).expect("inconclusive evidence");
    assert!(
        report
            .needs
            .iter()
            .any(|need| { need.name == "fast_and_decent" && need.state == "INCONCLUSIVE" })
    );
    let text = ModelOutput::device_evidence(&report).text;
    assert!(text.contains("level: full"));
    assert!(text.contains("ram_gb: 32"));
    assert!(text.contains("vram_gb: 8"));
    assert!(text.contains("inference_device: GPU 100%"));
    assert!(text.contains("serves: structured_output"));
    assert!(text.contains("use_for: JSON pipelines"));
    assert!(text.contains("state=INCONCLUSIVE"));
    assert!(!text.contains("secret"));
}

#[test]
fn unsupported_need_state_names_only_a_safe_token() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("maybe.json");
    let mut value = sample();
    value["needs"]["fast_and_decent"] = json!({
        "state": "MAYBE",
        "why": "secret-why-text"
    });
    std::fs::write(&path, value.to_string()).expect("write");
    let Err(failure) = inspect(&path) else {
        panic!("unknown state must be refused");
    };
    assert_eq!(failure.exit_code, std::process::ExitCode::from(EXIT_USAGE));
    assert_eq!(
        failure.body.diagnostic(),
        Some("needs.fast_and_decent.state: unsupported value MAYBE")
    );
    assert!(!failure.message.contains("secret-why-text"));

    let mut value = sample();
    value["needs"]["fast_and_decent"] = json!({
        "state": "secret prose should stay hidden",
        "why": "also hidden"
    });
    std::fs::write(&path, value.to_string()).expect("write unsafe state");
    let Err(failure) = inspect(&path) else {
        panic!("unsafe state must be refused");
    };
    assert!(failure.body.diagnostic().is_none());
    assert!(!failure.message.contains("secret prose"));
    assert!(!failure.message.contains("also hidden"));
}
