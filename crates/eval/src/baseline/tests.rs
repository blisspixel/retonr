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
    BASELINE_SCHEMA_VERSION, BaselineDefinition, BaselineInferencePolicy, BaselineKind,
    BaselineStatusCounts, run_baseline,
};
use crate::{EvaluationSuite, parse_suite};

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
            structured_output: true,
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
    let serialized = serde_json::to_string(&report).expect("serialize report");
    assert!(!serialized.contains("Hello world"));
}
