use std::sync::Mutex;

use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    AttachedProcessLease, AttachedProcessObserver, AttachedProcessWitnessError,
    AttachedProcessWitnessLimits, ListenerEndpoint,
};
use rewrite_types::{CancellationToken, Digest};
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{
    LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION, LocalOllamaModelPlan, LocalOllamaPreflightMode,
    LocalOllamaPreflightPlan,
};

use super::{
    LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_SCHEMA_VERSION, LocalOllamaAttestedPreflightError,
    LocalOllamaAttestedPreflightPlan, LocalOllamaProcessEvidenceLevel,
    MAX_LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_BYTES, parse_local_ollama_attested_preflight_plan,
    run_with_observer,
};

const MODEL: &str = "fixture:latest";
const INVENTORY_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn evidence(label: &str) -> AttachedProcessEvidence {
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::WindowsOwnerPidProcessHandle,
        owner_pid: 42,
        process_instance_digest: digest(&format!("process {label}")),
        ownership_snapshot_digest: digest(&format!("listener {label}")),
        entrypoint_object_digest: digest(&format!("object {label}")),
        entrypoint_digest: digest(&format!("entrypoint {label}")),
        entrypoint_bytes: 4096,
        platform_evidence_digest: digest(&format!("platform {label}")),
    })
    .expect("valid fake evidence")
}

fn plan(server: &MockServer) -> LocalOllamaAttestedPreflightPlan {
    LocalOllamaAttestedPreflightPlan {
        schema_version: LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_SCHEMA_VERSION,
        preflight: LocalOllamaPreflightPlan {
            schema_version: LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
            plan_id: "attached-fixture-v1".to_owned(),
            mode: LocalOllamaPreflightMode::Observe,
            endpoint: server.uri(),
            expected_runtime_version: "0.32.14".to_owned(),
            require_idle: true,
            models: vec![LocalOllamaModelPlan {
                reference: MODEL.to_owned(),
                inventory_digest: Digest::from_sha256_hex(INVENTORY_DIGEST)
                    .expect("inventory digest"),
                expected_details: None,
            }],
        },
        maximum_entrypoint_bytes: 16 * 1024 * 1024,
        expected_entrypoint_digest: None,
    }
}

async fn mount(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"version": "0.32.14"})))
        .expect(2)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": [{
            "name": MODEL,
            "model": MODEL,
            "size": 1024,
            "digest": INVENTORY_DIGEST
        }]})))
        .expect(2)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "license": "private license canary",
            "template": "private template canary",
            "capabilities": ["completion"],
            "details": {
                "format": "gguf",
                "family": "fixture",
                "quantization_level": "Q4_K_M"
            },
            "model_info": {"fixture.context_length": 4096}
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/ps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"models": []})))
        .expect(2)
        .mount(server)
        .await;
}

struct FakeObserver {
    lease: Mutex<Option<FakeLease>>,
}

impl FakeObserver {
    fn stable() -> Self {
        let evidence = evidence("stable");
        Self {
            lease: Mutex::new(Some(FakeLease {
                initial: evidence.clone(),
                final_result: Ok(evidence),
            })),
        }
    }

    fn drifting(error: AttachedProcessWitnessError) -> Self {
        Self {
            lease: Mutex::new(Some(FakeLease {
                initial: evidence("initial"),
                final_result: Err(error),
            })),
        }
    }
}

impl AttachedProcessObserver for FakeObserver {
    type Lease = FakeLease;

    fn attach(
        &self,
        _endpoint: ListenerEndpoint,
        _limits: AttachedProcessWitnessLimits,
        _cancellation: &CancellationToken,
    ) -> Result<Self::Lease, AttachedProcessWitnessError> {
        self.lease
            .lock()
            .map_err(|_error| AttachedProcessWitnessError::PlatformObservationFailed)?
            .take()
            .ok_or(AttachedProcessWitnessError::PlatformObservationFailed)
    }
}

struct FakeLease {
    initial: AttachedProcessEvidence,
    final_result: Result<AttachedProcessEvidence, AttachedProcessWitnessError>,
}

impl AttachedProcessLease for FakeLease {
    fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.initial
    }

    fn reobserve(
        &mut self,
        _cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        self.final_result.clone()
    }
}

#[tokio::test]
async fn stable_witness_brackets_preflight_without_claiming_response_binding() {
    let server = MockServer::start().await;
    mount(&server).await;
    let report = run_with_observer(
        &plan(&server),
        &CancellationToken::new(),
        &FakeObserver::stable(),
    )
    .await
    .expect("attached preflight");
    assert_eq!(
        report.process_evidence_level,
        LocalOllamaProcessEvidenceLevel::ObservedNativeListener
    );
    assert!(!report.response_bound);
    assert!(!report.qualified);
    assert!(!report.preflight.qualified);
    let encoded = serde_json::to_string(&report).expect("serialize report");
    assert!(!encoded.contains("private license canary"));
    assert!(!encoded.contains("private template canary"));
    assert!(!encoded.contains("entrypoint_path"));
    assert!(!encoded.contains("artifact_id"));
}

#[tokio::test]
async fn verify_mode_requires_and_checks_the_entrypoint_digest() {
    let server = MockServer::start().await;
    let mut verify = plan(&server);
    verify.preflight.mode = LocalOllamaPreflightMode::Verify;
    verify.preflight.models[0].expected_details = Some(rewrite_ollama::OllamaModelDetails {
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        quantization: "Q4_K_M".to_owned(),
        capabilities: vec!["completion".to_owned()],
        license_digest: digest("license"),
        template_digest: digest("template"),
        metadata_digest: digest("metadata"),
    });
    verify.expected_entrypoint_digest = Some(digest("wrong entrypoint"));
    let error = run_with_observer(&verify, &CancellationToken::new(), &FakeObserver::stable())
        .await
        .expect_err("entrypoint mismatch");
    assert!(matches!(
        error,
        LocalOllamaAttestedPreflightError::Witness(
            AttachedProcessWitnessError::EntrypointDigestMismatch
        )
    ));
    assert!(
        server
            .received_requests()
            .await
            .expect("requests")
            .is_empty()
    );
}

#[tokio::test]
async fn native_drift_has_priority_over_an_api_failure() {
    let server = MockServer::start().await;
    let error = run_with_observer(
        &plan(&server),
        &CancellationToken::new(),
        &FakeObserver::drifting(AttachedProcessWitnessError::ListenerRebound),
    )
    .await
    .expect_err("drift wins");
    assert!(matches!(
        error,
        LocalOllamaAttestedPreflightError::Witness(AttachedProcessWitnessError::ListenerRebound)
    ));
}

#[test]
fn parser_rejects_unknown_fields_mode_mismatch_and_oversize() {
    let unknown = br#"{
        "schema_version":1,
        "preflight":{},
        "maximum_entrypoint_bytes":1,
        "extra":true
    }"#;
    assert!(matches!(
        parse_local_ollama_attested_preflight_plan(unknown),
        Err(LocalOllamaAttestedPreflightError::InvalidJson)
    ));

    let server_uri = "http://127.0.0.1:11434";
    let mut invalid = LocalOllamaAttestedPreflightPlan {
        schema_version: LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_SCHEMA_VERSION,
        preflight: LocalOllamaPreflightPlan {
            schema_version: LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
            plan_id: "fixture".to_owned(),
            mode: LocalOllamaPreflightMode::Observe,
            endpoint: server_uri.to_owned(),
            expected_runtime_version: "0.32.14".to_owned(),
            require_idle: true,
            models: vec![LocalOllamaModelPlan {
                reference: MODEL.to_owned(),
                inventory_digest: Digest::from_sha256_hex(INVENTORY_DIGEST)
                    .expect("inventory digest"),
                expected_details: None,
            }],
        },
        maximum_entrypoint_bytes: 4096,
        expected_entrypoint_digest: Some(digest("unexpected")),
    };
    let encoded = serde_json::to_vec(&invalid).expect("serialize invalid plan");
    assert!(matches!(
        parse_local_ollama_attested_preflight_plan(&encoded),
        Err(LocalOllamaAttestedPreflightError::InvalidPlan)
    ));
    invalid.expected_entrypoint_digest = None;
    invalid.maximum_entrypoint_bytes = 0;
    let encoded = serde_json::to_vec(&invalid).expect("serialize zero limit");
    assert!(matches!(
        parse_local_ollama_attested_preflight_plan(&encoded),
        Err(LocalOllamaAttestedPreflightError::InvalidPlan)
    ));
    assert!(matches!(
        parse_local_ollama_attested_preflight_plan(&vec![
            b' ';
            MAX_LOCAL_OLLAMA_ATTESTED_PREFLIGHT_PLAN_BYTES
                + 1
        ]),
        Err(LocalOllamaAttestedPreflightError::TooLarge)
    ));
}
