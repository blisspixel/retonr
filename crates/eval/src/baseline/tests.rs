use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

use rewrite_inference::testing::{FakeGenerationStep, FakeInferenceBackend};
use rewrite_inference::{
    BackendDiscovery, BackendId, GenerationCandidate, GenerationResponse, InferenceCapabilities,
    OutputContract, ReasoningPolicy, SamplingParameters, UsageObservation,
};
use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
use rewrite_types::{CancellationToken, Digest, RewriteStatus};

use super::{
    BASELINE_SCHEMA_VERSION, BaselineDefinition, BaselineError, BaselineInferencePolicy,
    BaselineKind, BaselineStatusCounts, parse_baseline_definition, run_baseline,
    run_offline_baseline,
};
use crate::{EvaluationSuite, parse_suite};

const NO_REWRITE_DEFINITION: &str = include_str!("../../fixtures/no_rewrite_baseline_v1.json");

fn block_ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fake-backed baseline must complete immediately"),
    }
}

fn suite() -> EvaluationSuite {
    parse_suite(
        r#"{
            "schema_version": 2,
            "cases": [{
                "id": "case-1",
                "category": "positive_literal",
                "source": "Hello world",
                "candidate": "Hello, world!",
                "reference_judgment": "acceptable",
                "expected_status": "rewritten",
                "expected_reason": null,
                "expected_output": "candidate"
            }]
        }"#,
    )
    .expect("valid suite")
}

fn generative_fixtures() -> (BaselineDefinition, BackendDiscovery, GenerationResponse) {
    let digest = Digest::sha256(b"artifact");
    let artifact_id = ArtifactId::from_digest(digest.clone());
    let runtime = RuntimeIdentity {
        backend: "fake".to_owned(),
        version: "1".to_owned(),
        digest: None,
    };
    let schema_json = "{\"type\":\"object\"}".to_owned();
    let prompt_template = "Rewrite conservatively.".to_owned();
    let definition = BaselineDefinition {
        schema_version: BASELINE_SCHEMA_VERSION,
        id: "direct-fixture".to_owned(),
        kind: BaselineKind::DirectPrompt,
        inference: Some(BaselineInferencePolicy {
            artifact_id: artifact_id.clone(),
            artifact_digest: digest.clone(),
            prompt_template_digest: Digest::sha256(prompt_template.as_bytes()),
            prompt_template,
            output: OutputContract {
                schema_digest: Digest::sha256(schema_json.as_bytes()),
                schema_json,
            },
            source_byte_limit: 4_096,
            input_byte_limit: 8_192,
            context_token_limit: 8_192,
            output_token_limit: 512,
            candidate_byte_limit: 4_096,
            sampling: SamplingParameters {
                temperature: 0.0,
                top_p: 1.0,
                seed: Some(1),
            },
            reasoning: ReasoningPolicy::Disabled,
        }),
        style_description: None,
        retrieved_examples: Vec::new(),
    };
    let discovery = BackendDiscovery {
        backend_id: BackendId::new("fake").expect("valid ID"),
        runtime: runtime.clone(),
        capabilities: InferenceCapabilities {
            roles: vec![ArtifactRole::Generation],
            admitted_output_contract_digests: vec![
                definition
                    .inference
                    .as_ref()
                    .expect("inference policy")
                    .output
                    .schema_digest
                    .clone(),
            ],
            seed: true,
            disable_reasoning: true,
        },
        inventory: vec![rewrite_inference::InventoryEntry {
            reference: "fixture:latest".to_owned(),
            artifact_id: artifact_id.clone(),
            artifact_digest: digest.clone(),
            byte_size: Some(8),
        }],
    };
    let response = GenerationResponse {
        runtime,
        artifact_id,
        artifact_digest: digest,
        candidates: vec![GenerationCandidate {
            ordinal: 0,
            text: "Hello, world!".to_owned(),
        }],
        usage: UsageObservation {
            input_tokens: Some(1),
            output_tokens: Some(1),
            generation_micros: Some(1),
        },
    };
    (definition, discovery, response)
}

#[test]
fn no_rewrite_never_requires_a_backend() {
    let definition = BaselineDefinition {
        schema_version: BASELINE_SCHEMA_VERSION,
        id: "no-rewrite".to_owned(),
        kind: BaselineKind::NoRewrite,
        inference: None,
        style_description: None,
        retrieved_examples: Vec::new(),
    };
    let report = block_ready(run_baseline(
        &definition,
        &suite(),
        None,
        &CancellationToken::new(),
    ))
    .expect("no-rewrite baseline");
    assert_eq!(
        report.statuses,
        BaselineStatusCounts {
            unchanged: 1,
            ..BaselineStatusCounts::default()
        }
    );
}

#[test]
fn direct_baseline_uses_inference_port_and_common_validation() {
    let (definition, discovery, response) = generative_fixtures();
    let fake = FakeInferenceBackend::new(discovery, [FakeGenerationStep::Response(response)]);
    let report = block_ready(run_baseline(
        &definition,
        &suite(),
        Some(&fake),
        &CancellationToken::new(),
    ))
    .expect("direct baseline");
    assert_eq!(report.statuses.rewritten, 1);
    assert_eq!(report.cases[0].status, Some(RewriteStatus::Rewritten));
    assert_eq!(fake.requests().expect("request record").len(), 1);
    let input = &fake.requests().expect("request record")[0].input;
    assert!(input.contains("\"masked_source\":\"Hello world\""));
    let serialized = serde_json::to_string(&report).expect("serialize report");
    assert!(!serialized.contains("Hello world"));
}

#[test]
fn checked_in_no_rewrite_definition_runs_offline() {
    let definition =
        parse_baseline_definition(NO_REWRITE_DEFINITION).expect("checked-in definition");
    assert_eq!(definition.schema_version, BASELINE_SCHEMA_VERSION);
    assert_eq!(definition.id, "no-rewrite-v1");
    assert_eq!(definition.kind, BaselineKind::NoRewrite);
    let report = run_offline_baseline(&definition, &suite()).expect("offline no-rewrite");
    assert!(report.is_success());
    assert_eq!(report.total, 1);
    assert_eq!(report.statuses.unchanged, 1);
    assert_eq!(report.statuses.failed, 0);
    let serialized = serde_json::to_string(&report).expect("serialize report");
    assert!(!serialized.contains("Hello world"));
    assert!(!serialized.contains("Hello, world!"));
}

#[test]
fn generative_definition_round_trips_and_stays_offline_closed() {
    let (definition, _, _) = generative_fixtures();
    let encoded = serde_json::to_string(&definition).expect("definition serializes");
    let parsed = parse_baseline_definition(&encoded).expect("generative definition is valid");
    assert_eq!(parsed, definition);
    assert_eq!(
        run_offline_baseline(&parsed, &suite()),
        Err(BaselineError::MissingBackend)
    );
}

#[test]
fn rejects_untrusted_baseline_definitions() {
    assert!(matches!(
        parse_baseline_definition(&"x".repeat(super::MAX_BASELINE_DEFINITION_BYTES + 1)),
        Err(BaselineError::TooLarge)
    ));
    assert!(matches!(
        parse_baseline_definition("{"),
        Err(BaselineError::InvalidJson)
    ));
    assert!(matches!(
        parse_baseline_definition(
            r#"{"schema_version":1,"id":"no-rewrite-v1","kind":"no_rewrite","notes":"secret"}"#
        ),
        Err(BaselineError::InvalidJson)
    ));
    assert!(matches!(
        parse_baseline_definition(
            r#"{"schema_version":2,"id":"no-rewrite-v1","kind":"no_rewrite"}"#
        ),
        Err(BaselineError::UnsupportedSchema)
    ));
    assert!(matches!(
        parse_baseline_definition(r#"{"schema_version":1,"id":"No Rewrite","kind":"no_rewrite"}"#),
        Err(BaselineError::InvalidIdentifier)
    ));
    assert!(matches!(
        parse_baseline_definition(
            r#"{"schema_version":1,"id":"direct-prompt-v1","kind":"direct_prompt"}"#
        ),
        Err(BaselineError::InvalidConfiguration)
    ));
}
