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

use super::{GroundedRewriteRequest, GroundedRewriteSelection, GroundedRewriteService};
use crate::AppError;

pub(super) fn block_ready<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fake-backed service must complete immediately"),
    }
}

pub(super) fn fixtures(candidate: &str) -> (GroundedStrategy, FakeInferenceBackend) {
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
            admitted_output_contract_digests: vec![strategy.policy().output.schema_digest.clone()],
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

pub(super) fn request() -> GroundedRewriteRequest {
    GroundedRewriteRequest {
        source: b"Version 2 works.".to_vec(),
        protected_terms: Vec::new(),
        mode: RewriteMode::Literal,
        style_context: String::new(),
        claim_shadow: None,
    }
}

#[test]
fn current_selection_fails_closed_without_a_qualified_artifact() {
    assert!(matches!(
        GroundedRewriteSelection::require_selected(),
        Err(AppError::GroundedUnavailable)
    ));
    assert!(matches!(
        GroundedRewriteSelection::require_ready(None, None),
        Err(AppError::GroundedUnavailable)
    ));
    let requested = rewrite_model::ArtifactId::from_digest(rewrite_types::Digest::sha256(b"none"));
    assert!(matches!(
        GroundedRewriteSelection::require_ready(None, Some(&requested)),
        Err(AppError::GroundedSelectionMismatch)
    ));
    GroundedRewriteSelection::validate_source(b"Hello world\n").expect("valid source");
    assert!(matches!(
        GroundedRewriteSelection::validate_source(b"a\xff"),
        Err(AppError::TextAdapter(_))
    ));
}

#[test]
fn imported_repository_without_activation_is_not_ready() {
    use crate::{ArtifactImportLimits, ArtifactRepository, OfflineArtifactImportRequest};

    let directory = tempfile::tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let bytes = b"grounded selection fixture";
    std::fs::write(&source, bytes).expect("write artifact");
    let digest = Digest::sha256(bytes);
    let artifact_id = ArtifactId::from_digest(digest.clone());
    let data = directory.path().join("data");
    let repository = ArtifactRepository::new(&data).expect("derive repository");
    repository
        .import(
            &OfflineArtifactImportRequest {
                source,
                manifest: rewrite_model::ArtifactManifest {
                    schema_version: rewrite_model::ARTIFACT_MANIFEST_SCHEMA_VERSION,
                    artifact_id: artifact_id.clone(),
                    source: rewrite_model::ArtifactSource {
                        origin: "fixture/model".to_owned(),
                        revision: "fixture".to_owned(),
                    },
                    artifact_digest: digest,
                    byte_size: u64::try_from(bytes.len()).expect("fixture size"),
                    format: "gguf".to_owned(),
                    family: "fixture".to_owned(),
                    architecture: None,
                    quantization: None,
                    tokenizer: None,
                    licenses: vec![rewrite_model::LicenseRecord {
                        component: "weights".to_owned(),
                        identifier: "Apache-2.0".to_owned(),
                        text_digest: Digest::sha256(b"license"),
                    }],
                    declared_capabilities: rewrite_model::DeclaredCapabilities {
                        roles: vec![ArtifactRole::Generation],
                        languages: vec!["en".to_owned()],
                        context_tokens: Some(8_192),
                    },
                },
            },
            ArtifactImportLimits {
                maximum_artifact_bytes: 1024,
                maximum_storage_entries: 8,
            },
            &CancellationToken::new(),
        )
        .expect("import");
    assert!(matches!(
        GroundedRewriteSelection::require_ready(Some(&data), None),
        Err(AppError::GroundedUnavailable)
    ));
    assert!(matches!(
        GroundedRewriteSelection::require_ready(Some(&data), Some(&artifact_id)),
        Err(AppError::GroundedSelectionMismatch)
    ));
}

#[test]
fn accepts_only_after_grounded_candidate_passes_common_gates() {
    let (strategy, fake) = fixtures("Version {{PROTECTED_NUMBER_0001}} works!");
    let service = GroundedRewriteService::new(strategy, &fake);
    let result = block_ready(service.rewrite(request(), &CancellationToken::new(), None))
        .expect("grounded rewrite");
    assert_eq!(result.record.status, RewriteStatus::Rewritten);
    assert_eq!(result.output, b"Version 2 works!");
    let generation = result.record.generation.expect("generation provenance");
    assert_eq!(generation.runtime.backend, "fake");
    assert_eq!(generation.candidate_count, 1);
    let encoded = serde_json::to_string(&generation).expect("generation serializes");
    assert!(!encoded.contains("Version"));
    assert!(!encoded.contains("PROTECTED"));
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
    assert!(result.record.generation.is_some());
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
    assert!(result.record.generation.is_none());
}

#[test]
fn ambiguous_source_mapping_abstains_without_backend_work() {
    let (strategy, fake) = fixtures("unused");
    let service = GroundedRewriteService::new(strategy, &fake);
    let request = GroundedRewriteRequest {
        source: b"ada@example.com $12.ada@example.com $150".to_vec(),
        protected_terms: Vec::new(),
        mode: RewriteMode::Literal,
        style_context: String::new(),
        claim_shadow: None,
    };
    let result = block_ready(service.rewrite(request, &CancellationToken::new(), None))
        .expect("source ambiguity is a successful abstention");
    assert_eq!(result.record.status, RewriteStatus::Abstained);
    assert_eq!(result.record.reason, Some(ReasonCode::SentinelIntegrity));
    assert_eq!(result.output, b"ada@example.com $12.ada@example.com $150");
    assert!(result.record.generation.is_none());
    assert!(fake.requests().expect("request log").is_empty());
}

mod shadow;
