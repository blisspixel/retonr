use std::{collections::VecDeque, sync::Mutex};

use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    AttachedProcessLease, AttachedProcessObserver, AttachedProcessWitnessError,
    AttachedProcessWitnessLimits, ListenerEndpoint, RetainedTcpConnection,
    RetainedTcpConnectionEvidence, RetainedTcpConnectionEvidenceInput,
    TcpConnectionAttributionKind, TcpConnectionSharingLimitation,
};
use rewrite_types::{CancellationToken, Digest};
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use crate::{
    LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION, LocalOllamaModelPlan, LocalOllamaPreflightMode,
    LocalOllamaPreflightPlan,
};

use super::{
    LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION, LocalOllamaBoundPreflightError,
    LocalOllamaBoundPreflightPlan, LocalOllamaBoundProcessEvidenceLevel,
    MAX_LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_BYTES, parse_local_ollama_bound_preflight_plan,
    run_with_observer,
};

const MODEL: &str = "fixture:latest";
const INVENTORY_DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn process_evidence(label: &str) -> AttachedProcessEvidence {
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
    .expect("valid fake process evidence")
}

fn connection_evidence(process: &AttachedProcessEvidence) -> RetainedTcpConnectionEvidence {
    RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
        attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
        sharing_limitation: TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable,
        process_evidence_digest: process.evidence_digest().clone(),
        platform_connection_digest: digest("stable connection"),
    })
    .expect("valid fake connection evidence")
}

fn plan_at(endpoint: &str) -> LocalOllamaBoundPreflightPlan {
    LocalOllamaBoundPreflightPlan {
        schema_version: LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_SCHEMA_VERSION,
        preflight: LocalOllamaPreflightPlan {
            schema_version: LOCAL_OLLAMA_PREFLIGHT_PLAN_SCHEMA_VERSION,
            plan_id: "bound-fixture-v1".to_owned(),
            mode: LocalOllamaPreflightMode::Observe,
            endpoint: endpoint.to_owned(),
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
        maximum_session_body_bytes: 16 * 1024 * 1024,
        expected_entrypoint_digest: None,
    }
}

async fn mount_stable(server: &MockServer) {
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
    fn stable(connection_observations: usize) -> Self {
        let process = process_evidence("stable");
        let connection = connection_evidence(&process);
        Self {
            lease: Mutex::new(Some(FakeLease {
                initial: process.clone(),
                final_result: Ok(process),
                connection_results: std::iter::repeat_n(Ok(connection), connection_observations)
                    .collect(),
            })),
        }
    }

    fn with_results(
        final_result: Result<AttachedProcessEvidence, AttachedProcessWitnessError>,
        connection_results: Vec<Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>>,
    ) -> Self {
        Self {
            lease: Mutex::new(Some(FakeLease {
                initial: process_evidence("stable"),
                final_result,
                connection_results: connection_results.into(),
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
    connection_results:
        VecDeque<Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>>,
}

impl FakeLease {
    fn next_connection(
        &mut self,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.connection_results
            .pop_front()
            .unwrap_or(Err(AttachedProcessWitnessError::PlatformObservationFailed))
    }
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

    fn observe_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.next_connection()
    }

    fn reobserve_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _initial: &RetainedTcpConnectionEvidence,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.next_connection()
    }
}

#[tokio::test]
async fn stable_bound_preflight_records_exact_redacted_connection_count() {
    let server = MockServer::start().await;
    mount_stable(&server).await;
    let report = run_with_observer(
        &plan_at(&server.uri()),
        &CancellationToken::new(),
        &FakeObserver::stable(8),
    )
    .await
    .expect("bound preflight");

    assert_eq!(
        report.process_evidence_level,
        LocalOllamaBoundProcessEvidenceLevel::ObservedNativeConnectionAttribution
    );
    assert_eq!(report.connection_observations.len(), 8);
    assert_eq!(
        report.connection_observations.last(),
        Some(&report.connection_witness)
    );
    assert!(report.all_responses_used_retained_transport);
    assert!(report.kernel_attribution_checked_after_each_response);
    assert!(!report.exclusive_socket_owner_proven);
    assert!(!report.application_handler_proven);
    assert!(!report.preflight.qualified);
    assert!(!report.qualified);
    assert_ne!(report.binding_digest, report.plan_digest);

    let encoded = serde_json::to_string(&report).expect("serialize bound report");
    assert!(!encoded.contains(&server.uri()));
    assert!(!encoded.contains("private license canary"));
    assert!(!encoded.contains("private template canary"));
    assert_no_endpoint_keys(&serde_json::from_str(&encoded).expect("report JSON"));
}

#[tokio::test]
async fn listener_or_process_drift_wins_over_connection_observation_failure() {
    let server = MockServer::start().await;
    let process = process_evidence("stable");
    let connection = connection_evidence(&process);
    let observer = FakeObserver::with_results(
        Err(AttachedProcessWitnessError::ListenerRebound),
        vec![Ok(connection)],
    );
    let error = run_with_observer(
        &plan_at(&server.uri()),
        &CancellationToken::new(),
        &observer,
    )
    .await
    .expect_err("final native drift wins");
    assert!(matches!(
        error,
        LocalOllamaBoundPreflightError::Witness(AttachedProcessWitnessError::ListenerRebound)
    ));
}

#[tokio::test]
async fn failed_attempt_reobservation_has_priority_over_http_failure() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("connection", "close")
                .set_body_json(json!({"version": "0.32.14"})),
        )
        .mount(&server)
        .await;
    let process = process_evidence("stable");
    let connection = connection_evidence(&process);
    let observer = FakeObserver::with_results(
        Ok(process),
        vec![
            Ok(connection),
            Err(AttachedProcessWitnessError::ConnectionChanged),
        ],
    );
    let error = run_with_observer(
        &plan_at(&server.uri()),
        &CancellationToken::new(),
        &observer,
    )
    .await
    .expect_err("connection witness wins");
    assert!(matches!(
        error,
        LocalOllamaBoundPreflightError::Witness(AttachedProcessWitnessError::ConnectionChanged)
    ));
}

#[tokio::test]
async fn verify_mode_checks_expected_entrypoint_before_connect() {
    let mut plan = plan_at("http://127.0.0.1:9");
    plan.preflight.mode = LocalOllamaPreflightMode::Verify;
    plan.preflight.models[0].expected_details = Some(rewrite_ollama::OllamaModelDetails {
        format: "gguf".to_owned(),
        family: "fixture".to_owned(),
        quantization: "Q4_K_M".to_owned(),
        capabilities: vec!["completion".to_owned()],
        license_digest: digest("license"),
        template_digest: digest("template"),
        metadata_digest: digest("metadata"),
    });
    plan.expected_entrypoint_digest = Some(digest("wrong entrypoint"));
    let error = run_with_observer(&plan, &CancellationToken::new(), &FakeObserver::stable(0))
        .await
        .expect_err("entrypoint mismatch before connect");
    assert!(matches!(
        error,
        LocalOllamaBoundPreflightError::Witness(
            AttachedProcessWitnessError::EntrypointDigestMismatch
        )
    ));
}

#[test]
fn parser_rejects_unknown_fields_mode_mismatch_and_invalid_limits() {
    let unknown = br#"{
        "schema_version":1,
        "preflight":{},
        "maximum_entrypoint_bytes":1,
        "maximum_session_body_bytes":1,
        "extra":true
    }"#;
    assert!(matches!(
        parse_local_ollama_bound_preflight_plan(unknown),
        Err(LocalOllamaBoundPreflightError::InvalidJson)
    ));

    let mut invalid = plan_at("http://127.0.0.1:11434");
    invalid.expected_entrypoint_digest = Some(digest("unexpected"));
    assert_plan_error(&invalid, &LocalOllamaBoundPreflightError::InvalidPlan);
    invalid.expected_entrypoint_digest = None;
    invalid.maximum_entrypoint_bytes = 0;
    assert_plan_error(&invalid, &LocalOllamaBoundPreflightError::InvalidPlan);
    invalid.maximum_entrypoint_bytes = 4096;
    invalid.maximum_session_body_bytes = 0;
    assert_plan_error(&invalid, &LocalOllamaBoundPreflightError::InvalidPlan);
    invalid.maximum_session_body_bytes = 256 * 1024 * 1024 + 1;
    assert_plan_error(&invalid, &LocalOllamaBoundPreflightError::InvalidPlan);
    assert!(matches!(
        parse_local_ollama_bound_preflight_plan(&vec![
            b' ';
            MAX_LOCAL_OLLAMA_BOUND_PREFLIGHT_PLAN_BYTES
                + 1
        ]),
        Err(LocalOllamaBoundPreflightError::TooLarge)
    ));
}

fn assert_plan_error(
    plan: &LocalOllamaBoundPreflightPlan,
    expected: &LocalOllamaBoundPreflightError,
) {
    let encoded = serde_json::to_vec(plan).expect("serialize invalid plan");
    let actual = parse_local_ollama_bound_preflight_plan(&encoded)
        .expect_err("invalid bound plan fails closed");
    assert_eq!(actual.to_string(), expected.to_string());
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
