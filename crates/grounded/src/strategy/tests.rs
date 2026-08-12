use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

use rewrite_inference::testing::{FakeGenerationStep, FakeInferenceBackend};
use rewrite_inference::{
    BackendDiscovery, BackendId, GenerationCandidate, GenerationResponse, InferenceCapabilities,
    OperationContext, OutputContract, ReasoningPolicy, SamplingParameters, UsageObservation,
};
use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
use rewrite_types::{CancellationToken, Digest, DocumentId, RewriteMode, RewriteUnitId};

use super::GroundedStrategy;
use crate::{
    GROUNDED_POLICY_SCHEMA_VERSION, GroundedError, GroundedPolicy, GroundedRequest,
    GroundedSentinel, GroundedSentinelKind,
};

fn block_ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fake-backed strategy must complete immediately"),
    }
}

fn fixtures() -> (
    GroundedPolicy,
    GroundedRequest,
    BackendDiscovery,
    GenerationResponse,
) {
    let digest = Digest::sha256(b"artifact");
    let artifact_id = ArtifactId::from_digest(digest.clone());
    let runtime = RuntimeIdentity {
        backend: "fake".to_owned(),
        version: "1".to_owned(),
        digest: None,
    };
    let prompt_template = "Rewrite the masked source conservatively.".to_owned();
    let schema_json = "{\"type\":\"object\"}".to_owned();
    let policy = GroundedPolicy {
        schema_version: GROUNDED_POLICY_SCHEMA_VERSION,
        artifact_id: artifact_id.clone(),
        artifact_digest: digest.clone(),
        prompt_template_digest: Digest::sha256(prompt_template.as_bytes()),
        prompt_template,
        output: OutputContract {
            schema_digest: Digest::sha256(schema_json.as_bytes()),
            schema_json,
        },
        candidate_count: 1,
        source_byte_limit: 1024,
        input_byte_limit: 4096,
        context_token_limit: 4096,
        output_token_limit: 256,
        candidate_byte_limit: 1024,
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed: Some(1),
        },
        reasoning: ReasoningPolicy::Disabled,
    };
    let document_id = DocumentId::from_digest(&Digest::sha256(b"source"));
    let request = GroundedRequest {
        unit_id: RewriteUnitId::new(&document_id, 0),
        masked_source: "Version {{PROTECTED_NUMBER_0001}} works.".to_owned(),
        sentinels: vec![GroundedSentinel {
            token: "{{PROTECTED_NUMBER_0001}}".to_owned(),
            kind: GroundedSentinelKind::Number,
        }],
        mode: RewriteMode::Pure,
        style_context: String::new(),
    };
    let discovery = BackendDiscovery {
        backend_id: BackendId::new("fake").expect("backend ID"),
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
            text: "Version {{PROTECTED_NUMBER_0001}} works!".to_owned(),
        }],
        usage: UsageObservation {
            input_tokens: Some(20),
            output_tokens: Some(8),
            generation_micros: Some(10),
        },
    };
    (policy, request, discovery, response)
}

#[test]
fn produces_masked_candidates_with_redacted_trace() {
    let (policy, request, discovery, response) = fixtures();
    let strategy = GroundedStrategy::new(policy).expect("strategy policy");
    let fake = FakeInferenceBackend::new(discovery, [FakeGenerationStep::Response(response)]);
    let token = CancellationToken::new();
    let result =
        block_ready(strategy.generate(&request, &fake, OperationContext::new(&token, None)))
            .expect("grounded generation");
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(
        result.candidates[0].text_kind,
        rewrite_types::CandidateTextKind::Masked
    );
    let trace = serde_json::to_string(&result.trace).expect("trace serializes");
    assert!(!trace.contains("Version"));
    assert!(!trace.contains("PROTECTED"));
    let observed = fake.requests().expect("observed request");
    assert_eq!(
        observed[0].source_byte_count,
        u64::try_from(request.masked_source.len()).expect("fixture length")
    );
}

#[test]
fn rejects_missing_exact_artifact_without_generation() {
    let (policy, request, mut discovery, response) = fixtures();
    discovery.inventory.clear();
    let strategy = GroundedStrategy::new(policy).expect("strategy policy");
    let fake = FakeInferenceBackend::new(discovery, [FakeGenerationStep::Response(response)]);
    let token = CancellationToken::new();
    let error =
        block_ready(strategy.generate(&request, &fake, OperationContext::new(&token, None)))
            .expect_err("missing artifact is rejected");
    assert_eq!(error, GroundedError::Unavailable);
    assert!(fake.requests().expect("request observations").is_empty());
}
