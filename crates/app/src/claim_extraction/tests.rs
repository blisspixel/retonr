use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

use rewrite_inference::testing::{FakeInferenceBackend, FakeStructuredCompletionStep};
use rewrite_inference::{
    BackendDiscovery, BackendId, InferenceCapabilities, InferenceError, InferenceErrorKind,
    InventoryEntry, ReasoningPolicy, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
    SamplingParameters, StructuredCompletionRequest, StructuredCompletionResponse,
    UsageObservation, claim_output_contract,
};
use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
use rewrite_types::{
    CancellationToken, ClaimExtractionStatus, Digest, DocumentId, ExtractorManifest, RewriteUnitId,
};

use super::{
    CLAIM_PAIR_OPERATION_ID, CLAIM_PAIR_PROMPT_TEMPLATE, ClaimExtractionContext,
    ClaimExtractionError, ClaimExtractionRequest, ClaimExtractionService,
};

fn block_ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fake-backed extraction must complete immediately"),
    }
}

fn artifact() -> (ArtifactId, Digest) {
    let digest = Digest::sha256(b"extractor-artifact");
    (ArtifactId::from_digest(digest.clone()), digest)
}

fn runtime() -> RuntimeIdentity {
    RuntimeIdentity {
        backend: "fake".to_owned(),
        version: "1".to_owned(),
        digest: None,
    }
}

fn manifest() -> ExtractorManifest {
    ExtractorManifest::new(
        "literal-claims",
        "1.0.0",
        Digest::sha256(b"subject-policy"),
        Digest::sha256(CLAIM_PAIR_PROMPT_TEMPLATE.as_bytes()),
        claim_output_contract().schema_digest,
        Digest::sha256(CLAIM_PAIR_OPERATION_ID.as_bytes()),
        Digest::sha256(b"confidence-policy"),
        Digest::sha256(b"language-policy"),
    )
    .expect("valid extractor")
}

fn unit() -> RewriteUnitId {
    RewriteUnitId::new(&DocumentId::from_digest(&Digest::sha256(b"pair-doc")), 0)
}

fn request(source: &str, candidate: &str) -> ClaimExtractionRequest {
    let (artifact_id, artifact_digest) = artifact();
    ClaimExtractionRequest {
        source: source.to_owned(),
        candidate: candidate.to_owned(),
        unit_id: unit(),
        manifest: manifest(),
        artifact_id,
        artifact_digest,
        minimum_confidence_ppm: 500_000,
        source_byte_limit: 1_024,
        input_byte_limit: 4_096,
        context_token_limit: 2_048,
        output_token_limit: 256,
        output_byte_limit: 4_096,
        sampling: SamplingParameters {
            temperature: 0.0,
            top_p: 1.0,
            seed: Some(1),
        },
    }
}

fn payload(text: &str, status: &str) -> String {
    serde_json::json!({
        "status": status,
        "unit_id": unit().as_str(),
        "text_digest": Digest::sha256(text.as_bytes()),
        "claims": [{
            "claim_id": Digest::sha256(text.as_bytes()),
            "subject_id": null,
            "predicate_id": Digest::sha256(b"predicate"),
            "object_id": null,
            "polarity": "affirmed",
            "modality": "asserted",
            "condition_count": 0,
            "attributed": false,
            "evidence_spans": [{"start": 0, "end": text.len()}],
            "confidence_ppm": 800_000
        }]
    })
    .to_string()
}

fn structured_response(text: &str, status: &str) -> StructuredCompletionResponse {
    let request = request("Pay 10 now.", "Pay 10 now.");
    let completion = StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_id: request.artifact_id.clone(),
        artifact_digest: request.artifact_digest.clone(),
        input: "placeholder".to_owned(),
        output: claim_output_contract(),
        source_byte_count: u64::try_from(text.len()).expect("text size"),
        source_byte_limit: request.source_byte_limit,
        input_byte_limit: request.input_byte_limit,
        context_token_limit: request.context_token_limit,
        output_token_limit: request.output_token_limit,
        output_byte_limit: request.output_byte_limit,
        sampling: request.sampling,
        reasoning: ReasoningPolicy::Disabled,
    };
    StructuredCompletionResponse::complete(
        &completion,
        runtime(),
        request.artifact_id,
        request.artifact_digest,
        payload(text, status),
        UsageObservation {
            input_tokens: Some(8),
            output_tokens: Some(4),
            generation_micros: Some(3),
        },
    )
    .expect("structured response")
}

fn discovery(admit_claim: bool) -> BackendDiscovery {
    let (artifact_id, artifact_digest) = artifact();
    let admitted = if admit_claim {
        vec![claim_output_contract().schema_digest]
    } else {
        Vec::new()
    };
    BackendDiscovery {
        backend_id: BackendId::new("fake").expect("backend ID"),
        runtime: runtime(),
        capabilities: InferenceCapabilities {
            roles: vec![ArtifactRole::Generation],
            admitted_output_contract_digests: admitted,
            seed: true,
            disable_reasoning: true,
        },
        inventory: vec![InventoryEntry {
            reference: "fixture:latest".to_owned(),
            artifact_id,
            artifact_digest,
            byte_size: Some(8),
        }],
    }
}

fn extract(
    backend: &FakeInferenceBackend,
    request: ClaimExtractionRequest,
    cancellation: &CancellationToken,
) -> Result<super::ClaimExtractionPair, ClaimExtractionError> {
    let service = ClaimExtractionService::new(backend);
    block_ready(service.extract(
        request,
        ClaimExtractionContext {
            cancellation,
            deadline: None,
        },
    ))
}

#[test]
fn extracts_source_and_candidate_independently_without_generation() {
    let source = "Pay 10 now.";
    let candidate = "Pay 10 promptly.";
    let backend = FakeInferenceBackend::new(discovery(true), []).with_structured_steps([
        FakeStructuredCompletionStep::Response(structured_response(source, "complete")),
        FakeStructuredCompletionStep::Response(structured_response(candidate, "complete")),
    ]);
    let result = extract(
        &backend,
        request(source, candidate),
        &CancellationToken::new(),
    )
    .expect("pair extraction");
    assert_eq!(
        result.source.extraction_status(),
        ClaimExtractionStatus::Complete
    );
    assert_eq!(
        result.candidate.extraction_status(),
        ClaimExtractionStatus::Complete
    );
    assert_eq!(
        result.source.text_digest(),
        &Digest::sha256(source.as_bytes())
    );
    assert_eq!(
        result.candidate.text_digest(),
        &Digest::sha256(candidate.as_bytes())
    );
    assert!(result.comparison.is_some());
    assert!(backend.requests().expect("generation requests").is_empty());
    let structured = backend.structured_requests().expect("structured requests");
    assert_eq!(structured.len(), 2);
    assert!(structured[0].input.contains(source));
    assert!(structured[1].input.contains(candidate));
    assert!(!structured[0].input.contains(candidate));
    assert_eq!(
        structured[0].output.schema_digest,
        claim_output_contract().schema_digest
    );
}

#[test]
fn rejects_manifest_that_does_not_bind_the_pair_operation() {
    let mut request = request("Pay 10 now.", "Pay 10 now.");
    request.manifest = ExtractorManifest::new(
        "literal-claims",
        "1.0.0",
        Digest::sha256(b"subject-policy"),
        Digest::sha256(CLAIM_PAIR_PROMPT_TEMPLATE.as_bytes()),
        claim_output_contract().schema_digest,
        Digest::sha256(b"other-operation"),
        Digest::sha256(b"confidence-policy"),
        Digest::sha256(b"language-policy"),
    )
    .expect("valid but unbound extractor");
    let backend = FakeInferenceBackend::new(discovery(true), []);
    assert_eq!(
        extract(&backend, request, &CancellationToken::new()),
        Err(ClaimExtractionError::ManifestMismatch)
    );
    assert!(
        backend
            .structured_requests()
            .expect("no backend work")
            .is_empty()
    );
}

#[test]
fn refuses_backends_that_do_not_admit_the_claim_contract() {
    let backend = FakeInferenceBackend::new(discovery(false), []);
    assert_eq!(
        extract(
            &backend,
            request("Pay 10 now.", "Pay 10 now."),
            &CancellationToken::new()
        ),
        Err(ClaimExtractionError::Unavailable)
    );
}

#[test]
fn cancellation_on_the_second_side_returns_no_pair() {
    let source = "Pay 10 now.";
    let candidate = "Pay 10 promptly.";
    let backend = FakeInferenceBackend::new(discovery(true), []).with_structured_steps([
        FakeStructuredCompletionStep::Response(structured_response(source, "complete")),
        FakeStructuredCompletionStep::Error(InferenceError::new(
            InferenceErrorKind::Cancelled,
            "cancelled_before_structured_completion",
        )),
    ]);
    assert_eq!(
        extract(
            &backend,
            request(source, candidate),
            &CancellationToken::new()
        ),
        Err(ClaimExtractionError::Cancelled)
    );
}

#[test]
fn cancellation_before_start_returns_no_backend_work() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let backend = FakeInferenceBackend::new(discovery(true), []).with_structured_steps([
        FakeStructuredCompletionStep::Response(structured_response("Pay 10 now.", "complete")),
        FakeStructuredCompletionStep::Response(structured_response("Pay 10 now.", "complete")),
    ]);
    assert_eq!(
        extract(
            &backend,
            request("Pay 10 now.", "Pay 10 now."),
            &cancellation
        ),
        Err(ClaimExtractionError::Cancelled)
    );
    assert!(
        backend
            .structured_requests()
            .expect("no backend work")
            .is_empty()
    );
}

#[test]
fn incomplete_candidate_skips_comparison_without_engine_authority() {
    let source = "Pay 10 now.";
    let candidate = "Pay 10 promptly.";
    let backend = FakeInferenceBackend::new(discovery(true), []).with_structured_steps([
        FakeStructuredCompletionStep::Response(structured_response(source, "complete")),
        FakeStructuredCompletionStep::Response(structured_response(candidate, "abstained")),
    ]);
    let result = extract(
        &backend,
        request(source, candidate),
        &CancellationToken::new(),
    )
    .expect("pair extraction without comparison");
    assert_eq!(
        result.candidate.extraction_status(),
        ClaimExtractionStatus::Abstained
    );
    assert!(result.comparison.is_none());
}

#[test]
fn malformed_second_payload_fails_closed_without_a_partial_pair() {
    let source = "Pay 10 now.";
    let candidate = "Pay 10 promptly.";
    let backend = FakeInferenceBackend::new(discovery(true), []).with_structured_steps([
        FakeStructuredCompletionStep::Response(structured_response(source, "complete")),
        FakeStructuredCompletionStep::Response({
            let request = request(source, candidate);
            let completion = StructuredCompletionRequest {
                schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
                artifact_id: request.artifact_id.clone(),
                artifact_digest: request.artifact_digest.clone(),
                input: "placeholder".to_owned(),
                output: claim_output_contract(),
                source_byte_count: 1,
                source_byte_limit: request.source_byte_limit,
                input_byte_limit: request.input_byte_limit,
                context_token_limit: request.context_token_limit,
                output_token_limit: request.output_token_limit,
                output_byte_limit: request.output_byte_limit,
                sampling: request.sampling,
                reasoning: ReasoningPolicy::Disabled,
            };
            StructuredCompletionResponse::complete(
                &completion,
                runtime(),
                request.artifact_id,
                request.artifact_digest,
                "{\"status\":\"complete\"}".to_owned(),
                UsageObservation {
                    input_tokens: None,
                    output_tokens: None,
                    generation_micros: None,
                },
            )
            .expect("framed but invalid claim JSON")
        }),
    ]);
    assert_eq!(
        extract(
            &backend,
            request(source, candidate),
            &CancellationToken::new()
        ),
        Err(ClaimExtractionError::InvalidPayload)
    );
}
