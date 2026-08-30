use rewrite_model::ArtifactId;
use rewrite_ollama::derive_ollama_retained_session_response_id;
use rewrite_types::Digest;

use crate::ReleasedGenerationEffectivePackageV2;

use super::{completion, validate_response_binding};

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn retained_resource_authorities_remain_send_and_sync() {
    assert_send_sync::<ReleasedGenerationEffectivePackageV2>();
    assert_send_sync::<super::VerifiedGenerationQualificationResourceAttemptObservation>();
}

#[test]
fn portable_identical_sessions_cannot_be_cross_paired_at_the_app_boundary() {
    let artifact = ArtifactId::from_digest(Digest::sha256(b"identical session model"));
    let first = completion(
        artifact.clone(),
        "identical session request",
        r#"{"candidates":[{"text":"same"}]}"#,
    );
    let second = completion(
        artifact,
        "identical session request",
        r#"{"candidates":[{"text":"same"}]}"#,
    );
    assert_eq!(
        derive_ollama_retained_session_response_id(first.completion.response()),
        derive_ollama_retained_session_response_id(second.completion.response())
    );
    assert_eq!(
        first.completion.resident_execution_receipt(),
        second.completion.resident_execution_receipt()
    );
    assert!(validate_response_binding(&first.completion).is_ok());
    assert!(validate_response_binding(&second.completion).is_ok());

    let source = include_str!("../../generation_qualification_resource_attempt_observation.rs");
    let input = source
        .split("pub struct GenerationQualificationResourceAttemptObservationInput")
        .nth(1)
        .and_then(|suffix| suffix.split("pub struct Verified").next())
        .expect("public input section");
    assert!(input.contains("pub completion: OllamaResidentResourceObservedCompletion"));
    assert!(!input.contains("pub response:"));
    assert!(!input.contains("pub receipt:"));
    assert!(!input.contains("pub provider_observation:"));
    assert!(!source.contains("pub fn into_response"));
    assert!(!source.contains("pub fn into_resident_execution_receipt"));
}
