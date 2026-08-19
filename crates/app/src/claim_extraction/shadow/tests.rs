use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

use rewrite_engine::ClaimShadowObserver;
use rewrite_inference::testing::{FakeInferenceBackend, FakeStructuredCompletionStep};
use rewrite_inference::{
    BackendDiscovery, BackendId, InferenceCapabilities, InventoryEntry, ReasoningPolicy,
    STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, SamplingParameters, StructuredCompletionRequest,
    StructuredCompletionResponse, UsageObservation, claim_output_contract,
};
use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
use rewrite_text_adapter::TextAdapter;
use rewrite_types::{
    CancellationToken, Digest, ExtractorManifest, GateStatus, ReasonCode, RewriteStatus,
    RewriteUnitId,
};

use super::{
    ClaimShadowJoinDisposition, ClaimShadowJoinService, PreparedClaimShadow, PreparedClaimShadowSet,
};
use crate::{
    CLAIM_PAIR_OPERATION_ID, CLAIM_PAIR_PROMPT_TEMPLATE, CandidateCheckRequest,
    CandidateCheckService, ClaimExtractionContext, ClaimExtractionError, ClaimExtractionRequest,
    ClaimShadowJoinBinding,
};

fn block_ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fake-backed join must complete immediately"),
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

fn binding() -> ClaimShadowJoinBinding {
    let (artifact_id, artifact_digest) = artifact();
    ClaimShadowJoinBinding {
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

fn unit_for(source: &[u8]) -> RewriteUnitId {
    TextAdapter::parse(source)
        .expect("fixture source")
        .document()
        .rewrite_units[0]
        .id
        .clone()
}

fn payload(unit_id: &RewriteUnitId, text: &str, predicate: &[u8]) -> String {
    serde_json::json!({
        "status": "complete",
        "unit_id": unit_id.as_str(),
        "text_digest": Digest::sha256(text.as_bytes()),
        "claims": [{
            "claim_id": Digest::sha256(predicate),
            "subject_id": null,
            "predicate_id": Digest::sha256(predicate),
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

fn structured_response(
    unit_id: &RewriteUnitId,
    text: &str,
    predicate: &[u8],
) -> StructuredCompletionResponse {
    let request = ClaimExtractionRequest {
        source: text.to_owned(),
        candidate: text.to_owned(),
        unit_id: unit_id.clone(),
        ..binding().extraction_request(unit_id.clone(), text, text)
    };
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
        payload(unit_id, text, predicate),
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
    BackendDiscovery {
        backend_id: BackendId::new("fake").expect("backend ID"),
        runtime: runtime(),
        capabilities: InferenceCapabilities {
            roles: vec![ArtifactRole::Generation],
            admitted_output_contract_digests: if admit_claim {
                vec![claim_output_contract().schema_digest]
            } else {
                Vec::new()
            },
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

fn prepare(
    backend: &FakeInferenceBackend,
    source: &str,
    candidate: &str,
) -> Result<ClaimShadowJoinDisposition, ClaimExtractionError> {
    let unit_id = unit_for(source.as_bytes());
    block_ready(ClaimShadowJoinService::new(backend).prepare(
        &binding(),
        unit_id,
        source,
        candidate,
        ClaimExtractionContext {
            cancellation: &CancellationToken::new(),
            deadline: None,
        },
    ))
}

fn admitting_backend(
    source: &str,
    candidate: &str,
    source_predicate: &[u8],
    candidate_predicate: &[u8],
) -> FakeInferenceBackend {
    let unit_id = unit_for(source.as_bytes());
    FakeInferenceBackend::new(discovery(true), []).with_structured_steps([
        FakeStructuredCompletionStep::Response(structured_response(
            &unit_id,
            source,
            source_predicate,
        )),
        FakeStructuredCompletionStep::Response(structured_response(
            &unit_id,
            candidate,
            candidate_predicate,
        )),
    ])
}

fn check_request(source: &str, candidate: &str) -> CandidateCheckRequest {
    CandidateCheckRequest {
        source: source.as_bytes().to_vec(),
        candidate: candidate.to_owned(),
        protected_terms: Vec::new(),
    }
}

fn shadow_gate_of(result: &crate::CandidateCheckResult) -> Option<&rewrite_types::GateResult> {
    result.record.assessments.first().and_then(|assessment| {
        assessment
            .gates
            .iter()
            .find(|gate| gate.gate_id == "claim_comparison_shadow")
    })
}

#[test]
fn records_complete_comparison_without_generation() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let backend = admitting_backend(source, candidate, b"greet", b"greet");
    let disposition = prepare(&backend, source, candidate).expect("join");
    let ClaimShadowJoinDisposition::Recorded(shadow) = disposition else {
        panic!("expected recorded comparison");
    };
    assert!(!shadow.comparison().counts().has_difference());
    assert!(backend.requests().expect("generation requests").is_empty());
}

#[test]
fn skips_backends_that_do_not_admit_the_claim_contract() {
    let backend = FakeInferenceBackend::new(discovery(false), []);
    assert_eq!(
        prepare(&backend, "Hello world", "Hello, world!"),
        Ok(ClaimShadowJoinDisposition::Skipped)
    );
}

#[test]
fn skips_malformed_payloads_without_failing_the_join() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let unit_id = unit_for(source.as_bytes());
    let request = binding().extraction_request(unit_id.clone(), source, candidate);
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
    let malformed = StructuredCompletionResponse::complete(
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
    .expect("framed but invalid claim JSON");
    let backend = FakeInferenceBackend::new(discovery(true), []).with_structured_steps([
        FakeStructuredCompletionStep::Response(structured_response(&unit_id, source, b"greet")),
        FakeStructuredCompletionStep::Response(malformed),
    ]);
    assert_eq!(
        prepare(&backend, source, candidate),
        Ok(ClaimShadowJoinDisposition::Skipped)
    );
}

#[test]
fn skips_incomplete_extraction_without_comparison() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let unit_id = unit_for(source.as_bytes());
    let payload = serde_json::json!({
        "status": "abstained",
        "unit_id": unit_id.as_str(),
        "text_digest": Digest::sha256(candidate.as_bytes()),
        "claims": []
    })
    .to_string();
    let abstained = StructuredCompletionResponse::complete(
        &StructuredCompletionRequest {
            schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
            artifact_id: binding().artifact_id.clone(),
            artifact_digest: binding().artifact_digest.clone(),
            input: "placeholder".to_owned(),
            output: claim_output_contract(),
            source_byte_count: u64::try_from(candidate.len()).expect("size"),
            source_byte_limit: 1_024,
            input_byte_limit: 4_096,
            context_token_limit: 2_048,
            output_token_limit: 256,
            output_byte_limit: 4_096,
            sampling: binding().sampling,
            reasoning: ReasoningPolicy::Disabled,
        },
        runtime(),
        binding().artifact_id,
        binding().artifact_digest,
        payload,
        UsageObservation {
            input_tokens: None,
            output_tokens: None,
            generation_micros: None,
        },
    )
    .expect("abstained payload");
    let backend = FakeInferenceBackend::new(discovery(true), []).with_structured_steps([
        FakeStructuredCompletionStep::Response(structured_response(&unit_id, source, b"greet")),
        FakeStructuredCompletionStep::Response(abstained),
    ]);
    assert_eq!(
        prepare(&backend, source, candidate),
        Ok(ClaimShadowJoinDisposition::Skipped)
    );
}

#[test]
fn cancellation_still_fails_the_join() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let backend = FakeInferenceBackend::new(discovery(true), []);
    let error = block_ready(ClaimShadowJoinService::new(&backend).prepare(
        &binding(),
        unit_for(b"Hello world"),
        "Hello world",
        "Hello, world!",
        ClaimExtractionContext {
            cancellation: &cancellation,
            deadline: None,
        },
    ));
    assert_eq!(error, Err(ClaimExtractionError::Cancelled));
}

#[test]
fn invalid_binding_fails_before_backend_work() {
    let mut binding = binding();
    binding.source_byte_limit = 0;
    let backend = FakeInferenceBackend::new(discovery(true), []);
    let error = block_ready(ClaimShadowJoinService::new(&backend).prepare(
        &binding,
        unit_for(b"Hello world"),
        "Hello world",
        "Hello, world!",
        ClaimExtractionContext {
            cancellation: &CancellationToken::new(),
            deadline: None,
        },
    ));
    assert_eq!(error, Err(ClaimExtractionError::InvalidRequest));
    assert!(
        backend
            .structured_requests()
            .expect("no backend work")
            .is_empty()
    );
}

#[test]
fn punctuation_pass_records_preserved_claims_without_changing_acceptance() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let backend = admitting_backend(source, candidate, b"greet", b"greet");
    let ClaimShadowJoinDisposition::Recorded(shadow) =
        prepare(&backend, source, candidate).expect("join")
    else {
        panic!("expected recorded comparison");
    };
    let result = CandidateCheckService::check_with_claim_shadow(
        check_request(source, candidate),
        &CancellationToken::new(),
        Some(&shadow),
    )
    .expect("candidate check");
    assert_eq!(result.record.status, RewriteStatus::Rewritten);
    assert_eq!(result.output, candidate.as_bytes());
    let shadow_gate = shadow_gate_of(&result).expect("shadow gate");
    assert_eq!(shadow_gate.status, GateStatus::Pass);
    assert_eq!(shadow_gate.severity, rewrite_types::Severity::Info);
    assert_eq!(shadow_gate.evidence[0].code, "claim_comparison_preserved");
}

#[test]
fn claim_conflict_cannot_reject_an_eligible_punctuation_candidate() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let backend = admitting_backend(source, candidate, b"greet", b"other");
    let ClaimShadowJoinDisposition::Recorded(shadow) =
        prepare(&backend, source, candidate).expect("join")
    else {
        panic!("expected recorded comparison");
    };
    assert!(shadow.comparison().counts().has_difference());
    let result = CandidateCheckService::check_with_claim_shadow(
        check_request(source, candidate),
        &CancellationToken::new(),
        Some(&shadow),
    )
    .expect("candidate check");
    assert_eq!(result.record.status, RewriteStatus::Rewritten);
    assert!(result.record.assessments[0].eligible);
    let shadow_gate = shadow_gate_of(&result).expect("shadow gate");
    assert_eq!(shadow_gate.status, GateStatus::Fail);
    assert_eq!(shadow_gate.severity, rewrite_types::Severity::Info);
    assert_eq!(shadow_gate.evidence[0].code, "claim_comparison_conflict");
}

#[test]
fn literal_failure_still_abstains_when_shadow_claims_are_preserved() {
    let source = "Hello world";
    let candidate = "Hello there";
    let backend = admitting_backend(source, candidate, b"greet", b"greet");
    let ClaimShadowJoinDisposition::Recorded(shadow) =
        prepare(&backend, source, candidate).expect("join")
    else {
        panic!("expected recorded comparison");
    };
    let result = CandidateCheckService::check_with_claim_shadow(
        check_request(source, candidate),
        &CancellationToken::new(),
        Some(&shadow),
    )
    .expect("candidate check");
    assert_eq!(result.record.status, RewriteStatus::Abstained);
    assert_eq!(result.record.reason, Some(ReasonCode::SemanticUncertain));
    assert_eq!(result.output, source.as_bytes());
    let shadow_gate = shadow_gate_of(&result).expect("shadow gate");
    assert_eq!(shadow_gate.status, GateStatus::Pass);
}

#[test]
fn unmatched_observer_does_not_record_a_shadow_gate() {
    let source = "Hello world";
    let candidate = "Hello, world!";
    let backend = admitting_backend(source, candidate, b"greet", b"greet");
    let ClaimShadowJoinDisposition::Recorded(shadow) =
        prepare(&backend, source, candidate).expect("join")
    else {
        panic!("expected recorded comparison");
    };
    let result = CandidateCheckService::check_with_claim_shadow(
        check_request("Pay 10 now.", "Pay 10 now!"),
        &CancellationToken::new(),
        Some(&shadow),
    )
    .expect("unrelated check");
    assert_eq!(result.record.status, RewriteStatus::Rewritten);
    assert!(shadow_gate_of(&result).is_none());
}

#[test]
fn empty_prepared_set_is_idle() {
    let set = PreparedClaimShadowSet::new(Vec::new());
    assert!(set.is_empty());
    assert!(
        ClaimShadowObserver::observe(
            &set,
            &unit_for(b"Hello world"),
            "Hello world",
            "Hello, world!",
        )
        .is_none()
    );
}

#[test]
fn from_comparison_retains_a_valid_aggregate() {
    let source = "Hello world";
    let backend = admitting_backend(source, "Hello, world!", b"greet", b"greet");
    let ClaimShadowJoinDisposition::Recorded(shadow) =
        prepare(&backend, source, "Hello, world!").expect("join")
    else {
        panic!("expected recorded comparison");
    };
    let retained = PreparedClaimShadow::from_comparison(shadow.comparison().clone())
        .expect("valid comparison");
    assert_eq!(retained.comparison(), shadow.comparison());
}
