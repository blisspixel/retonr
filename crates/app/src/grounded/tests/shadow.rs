use rewrite_grounded::GroundedStrategy;
use rewrite_inference::testing::{
    FakeGenerationStep, FakeInferenceBackend, FakeStructuredCompletionStep,
};
use rewrite_inference::{
    BackendDiscovery, BackendId, GenerationCandidate, GenerationResponse, InferenceCapabilities,
    InferenceError, InferenceErrorKind, ReasoningPolicy,
    STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, SamplingParameters, StructuredCompletionRequest,
    StructuredCompletionResponse, UsageObservation, claim_output_contract,
};
use rewrite_model::{ArtifactRole, RuntimeIdentity};
use rewrite_text_adapter::TextAdapter;
use rewrite_types::{
    CancellationToken, Digest, ExtractorManifest, GateStatus, ReasonCode, RewriteStatus, Severity,
};

use super::{block_ready, fixtures, request};
use crate::{
    CLAIM_PAIR_OPERATION_ID, CLAIM_PAIR_PROMPT_TEMPLATE, ClaimShadowJoinBinding,
    GroundedRewriteService,
};

fn extractor_manifest() -> ExtractorManifest {
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

fn shadow_binding(strategy: &GroundedStrategy) -> ClaimShadowJoinBinding {
    let policy = strategy.policy();
    ClaimShadowJoinBinding {
        manifest: extractor_manifest(),
        artifact_id: policy.artifact_id.clone(),
        artifact_digest: policy.artifact_digest.clone(),
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

fn source_unit_id(source: &[u8]) -> rewrite_types::RewriteUnitId {
    TextAdapter::parse(source)
        .expect("fixture source")
        .document()
        .rewrite_units[0]
        .id
        .clone()
}

fn claim_payload(unit_id: &rewrite_types::RewriteUnitId, text: &str, predicate: &[u8]) -> String {
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

fn claim_response(
    strategy: &GroundedStrategy,
    text: &str,
    predicate: &[u8],
) -> StructuredCompletionResponse {
    let policy = strategy.policy();
    let unit_id = source_unit_id(b"Version 2 works.");
    let completion = StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_id: policy.artifact_id.clone(),
        artifact_digest: policy.artifact_digest.clone(),
        input: "placeholder".to_owned(),
        output: claim_output_contract(),
        source_byte_count: u64::try_from(text.len()).expect("text size"),
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
        reasoning: ReasoningPolicy::Disabled,
    };
    StructuredCompletionResponse::complete(
        &completion,
        RuntimeIdentity {
            backend: "fake".to_owned(),
            version: "1".to_owned(),
            digest: None,
        },
        policy.artifact_id.clone(),
        policy.artifact_digest.clone(),
        claim_payload(&unit_id, text, predicate),
        UsageObservation {
            input_tokens: Some(4),
            output_tokens: Some(2),
            generation_micros: Some(1),
        },
    )
    .expect("claim response")
}

fn claim_capable_fake(
    strategy: &GroundedStrategy,
    candidate: &str,
    structured: impl IntoIterator<Item = FakeStructuredCompletionStep>,
) -> FakeInferenceBackend {
    let digest = strategy.policy().artifact_digest.clone();
    let artifact_id = strategy.policy().artifact_id.clone();
    let runtime = RuntimeIdentity {
        backend: "fake".to_owned(),
        version: "1".to_owned(),
        digest: None,
    };
    let response = GenerationResponse {
        runtime: runtime.clone(),
        artifact_id: artifact_id.clone(),
        artifact_digest: digest.clone(),
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
    FakeInferenceBackend::new(
        BackendDiscovery {
            backend_id: BackendId::new("fake").expect("backend ID"),
            runtime,
            capabilities: InferenceCapabilities {
                roles: vec![ArtifactRole::Generation],
                admitted_output_contract_digests: {
                    let mut digests = vec![
                        strategy.policy().output.schema_digest.clone(),
                        claim_output_contract().schema_digest,
                    ];
                    digests.sort_by(|left, right| left.as_str().cmp(right.as_str()));
                    digests
                },
                seed: true,
                disable_reasoning: true,
            },
            inventory: vec![rewrite_inference::InventoryEntry {
                reference: "fixture:latest".to_owned(),
                artifact_id,
                artifact_digest: digest,
                byte_size: Some(8),
            }],
        },
        [FakeGenerationStep::Response(response)],
    )
    .with_structured_steps(structured)
}

fn fixtures_with_claims(
    candidate: &str,
    source_text: &str,
    restored_text: &str,
    source_predicate: &[u8],
    candidate_predicate: &[u8],
) -> (GroundedStrategy, FakeInferenceBackend) {
    let (strategy, _) = fixtures(candidate);
    let fake = claim_capable_fake(
        &strategy,
        candidate,
        [
            FakeStructuredCompletionStep::Response(claim_response(
                &strategy,
                source_text,
                source_predicate,
            )),
            FakeStructuredCompletionStep::Response(claim_response(
                &strategy,
                restored_text,
                candidate_predicate,
            )),
        ],
    );
    (strategy, fake)
}

fn shadow_gate_of(result: &crate::GroundedRewriteResult) -> Option<&rewrite_types::GateResult> {
    result.record.assessments.first().and_then(|assessment| {
        assessment
            .gates
            .iter()
            .find(|gate| gate.gate_id == "claim_comparison_shadow")
    })
}

#[test]
fn candidate_only_backend_skips_shadow_without_changing_acceptance() {
    let (strategy, fake) = fixtures("Version {{PROTECTED_NUMBER_0001}} works!");
    let mut request = request();
    request.claim_shadow = Some(shadow_binding(&strategy));
    let service = GroundedRewriteService::new(strategy, &fake);
    let result = block_ready(service.rewrite(request, &CancellationToken::new(), None))
        .expect("grounded rewrite");
    assert_eq!(result.record.status, RewriteStatus::Rewritten);
    assert_eq!(result.output, b"Version 2 works!");
    assert!(shadow_gate_of(&result).is_none());
}

#[test]
fn recorded_shadow_conflict_cannot_reject_a_literal_pass() {
    let (strategy, fake) = fixtures_with_claims(
        "Version {{PROTECTED_NUMBER_0001}} works!",
        "Version 2 works.",
        "Version 2 works!",
        b"version-works",
        b"other-claim",
    );
    let mut request = request();
    request.claim_shadow = Some(shadow_binding(&strategy));
    let service = GroundedRewriteService::new(strategy, &fake);
    let result = block_ready(service.rewrite(request, &CancellationToken::new(), None))
        .expect("grounded rewrite");
    assert_eq!(result.record.status, RewriteStatus::Rewritten);
    assert_eq!(result.output, b"Version 2 works!");
    let shadow = shadow_gate_of(&result).expect("shadow gate");
    assert_eq!(shadow.status, GateStatus::Fail);
    assert_eq!(shadow.severity, Severity::Info);
    assert_eq!(shadow.evidence[0].code, "claim_comparison_conflict");
}

#[test]
fn recorded_shadow_preservation_still_abstains_on_lexical_change() {
    let (strategy, fake) = fixtures_with_claims(
        "Version {{PROTECTED_NUMBER_0001}} succeeds!",
        "Version 2 works.",
        "Version 2 succeeds!",
        b"version-works",
        b"version-works",
    );
    let mut request = request();
    request.claim_shadow = Some(shadow_binding(&strategy));
    let service = GroundedRewriteService::new(strategy, &fake);
    let result = block_ready(service.rewrite(request, &CancellationToken::new(), None))
        .expect("candidate rejection is a successful transaction");
    assert_eq!(result.record.status, RewriteStatus::Abstained);
    assert_eq!(result.record.reason, Some(ReasonCode::SemanticUncertain));
    assert_eq!(result.output, b"Version 2 works.");
    let shadow = shadow_gate_of(&result).expect("shadow gate");
    assert_eq!(shadow.status, GateStatus::Pass);
}

#[test]
fn extraction_cancellation_after_generation_abstains() {
    let candidate = "Version {{PROTECTED_NUMBER_0001}} works!";
    let (strategy, _) = fixtures(candidate);
    let fake = claim_capable_fake(
        &strategy,
        candidate,
        [FakeStructuredCompletionStep::Error(InferenceError::new(
            InferenceErrorKind::Cancelled,
            "cancelled_before_structured_completion",
        ))],
    );
    let mut request = request();
    request.claim_shadow = Some(shadow_binding(&strategy));
    let service = GroundedRewriteService::new(strategy, &fake);
    let result = block_ready(service.rewrite(request, &CancellationToken::new(), None))
        .expect("extraction cancellation is a safe outcome");
    assert_eq!(result.record.status, RewriteStatus::Abstained);
    assert_eq!(result.record.reason, Some(ReasonCode::Cancelled));
    assert_eq!(result.output, b"Version 2 works.");
    assert!(result.record.generation.is_some());
    assert!(shadow_gate_of(&result).is_none());
}
