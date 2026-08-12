use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

use rewrite_grounded::{GROUNDED_POLICY_SCHEMA_VERSION, GroundedPolicy, GroundedStrategy};
use rewrite_inference::testing::{FakeGenerationStep, FakeInferenceBackend};
use rewrite_inference::{
    BackendDiscovery, BackendId, GenerationCandidate, GenerationResponse, InferenceCapabilities,
    OutputContract, ReasoningPolicy, SamplingParameters, UsageObservation,
};
use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
use rewrite_types::{CancellationToken, Digest, ReasonCode, RewriteMode, RewriteStatus};

use super::{GroundedRewriteRequest, GroundedRewriteService};

fn block_ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fake-backed service must complete immediately"),
    }
}

fn fixtures(candidate: &str) -> (GroundedStrategy, FakeInferenceBackend) {
    let digest = Digest::sha256(b"artifact");
    let artifact_id = ArtifactId::from_digest(digest.clone());
    let runtime = RuntimeIdentity {
        backend: "fake".to_owned(),
        version: "1".to_owned(),
        digest: None,
    };
    let prompt_template = "Rewrite conservatively and preserve every sentinel.".to_owned();
    let schema_json = "{\"type\":\"object\"}".to_owned();
    let strategy = GroundedStrategy::new(GroundedPolicy {
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
    })
    .expect("grounded policy");
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
            text: candidate.to_owned(),
        }],
        usage: UsageObservation {
            input_tokens: Some(12),
            output_tokens: Some(6),
            generation_micros: Some(10),
        },
    };
    (
        strategy,
        FakeInferenceBackend::new(discovery, [FakeGenerationStep::Response(response)]),
    )
}

fn request() -> GroundedRewriteRequest {
    GroundedRewriteRequest {
        source: b"Version 2 works.".to_vec(),
        protected_terms: Vec::new(),
        mode: RewriteMode::Literal,
        style_context: String::new(),
    }
}

#[test]
fn accepts_only_after_grounded_candidate_passes_common_gates() {
    let (strategy, fake) = fixtures("Version {{PROTECTED_NUMBER_0001}} works!");
    let service = GroundedRewriteService::new(strategy, &fake);
    let result = block_ready(service.rewrite(request(), &CancellationToken::new(), None))
        .expect("grounded rewrite");
    assert_eq!(result.record.status, RewriteStatus::Rewritten);
    assert_eq!(result.output, b"Version 2 works!");
    assert!(result.trace.is_some());
}

#[test]
fn rejects_lexical_change_and_returns_exact_original() {
    let (strategy, fake) = fixtures("Version {{PROTECTED_NUMBER_0001}} succeeds!");
    let service = GroundedRewriteService::new(strategy, &fake);
    let result = block_ready(service.rewrite(request(), &CancellationToken::new(), None))
        .expect("candidate rejection is a successful transaction");
    assert_eq!(result.record.status, RewriteStatus::Abstained);
    assert_eq!(result.record.reason, Some(ReasonCode::SemanticUncertain));
    assert_eq!(result.output, b"Version 2 works.");
    assert!(result.trace.is_some());
}

#[test]
fn backend_cancellation_becomes_safe_cancelled_abstention() {
    let (strategy, fake) = fixtures("unused");
    let observed = fake.requests().expect("initial request state");
    assert!(observed.is_empty());
    let token = CancellationToken::new();
    token.cancel();
    let service = GroundedRewriteService::new(strategy, &fake);
    let result = block_ready(service.rewrite(request(), &token, None))
        .expect("cancellation is a safe outcome");
    assert_eq!(result.record.status, RewriteStatus::Abstained);
    assert_eq!(result.record.reason, Some(ReasonCode::Cancelled));
    assert_eq!(result.output, b"Version 2 works.");
    assert!(result.trace.is_none());
}
