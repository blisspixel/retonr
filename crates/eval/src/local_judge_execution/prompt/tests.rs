use rewrite_inference::{StructuredCompletionResponse, UsageObservation};
use rewrite_model::{ArtifactId, RuntimeIdentity};
use rewrite_ollama::derive_ollama_retained_session_response_id;
use serde_json::Value;

use super::*;

const A_FIRST_SEED: u64 = 11_769_455_392_107_554_815;
const B_FIRST_SEED: u64 = 16_201_717_743_242_601_372;
const A_FIRST_BINDING: &str = "247806efa2f2c7f12ca5a02bfe56f7333bc9dcfd7c53bbd693c51454ee4d0fc5";
const B_FIRST_BINDING: &str = "a8b90a2574c5ff4b35e14fdf356832ad0ffa4b506468432a1374b500a5a91ecb";

#[test]
fn one_attempt_builder_preserves_legacy_prompt_and_request_identities() {
    let a_first = build(JudgePresentation::CandidateAFirst, A_FIRST_SEED, limits())
        .expect("candidate A first request");
    let b_first = build(JudgePresentation::CandidateBFirst, B_FIRST_SEED, limits())
        .expect("candidate B first request");

    assert_eq!(a_first.binding_digest().as_str(), A_FIRST_BINDING);
    assert_eq!(b_first.binding_digest().as_str(), B_FIRST_BINDING);
    assert_eq!(a_first.sampling.seed, Some(A_FIRST_SEED));
    assert_eq!(b_first.sampling.seed, Some(B_FIRST_SEED));
    assert_eq!(a_first.artifact_id.digest(), &a_first.artifact_digest);
    assert_eq!(a_first.output, local_judge_attempt_output_contract());
    assert_eq!(a_first.reasoning, ReasoningPolicy::Disabled);

    let prompt_a: Value = serde_json::from_str(&a_first.input).expect("canonical prompt A");
    let prompt_b: Value = serde_json::from_str(&b_first.input).expect("canonical prompt B");
    assert_eq!(prompt_a["case_id"], "case-a");
    assert_eq!(prompt_a["rubric"][0]["id"], "meaning");
    assert_eq!(prompt_a["source"], "Hello world");
    assert_eq!(prompt_a["first_candidate"], "Hello, world!");
    assert_eq!(prompt_a["second_candidate"], "Hello world.");
    assert_eq!(prompt_b["first_candidate"], "Hello world.");
    assert_eq!(prompt_b["second_candidate"], "Hello, world!");
}

#[test]
fn presentation_and_explicit_seed_are_independent_identity_inputs() {
    let original = build(JudgePresentation::CandidateAFirst, 7, limits()).expect("request");
    let reordered = build(JudgePresentation::CandidateBFirst, 7, limits()).expect("reordered");
    let reseeded = build(JudgePresentation::CandidateAFirst, 8, limits()).expect("reseeded");

    assert_ne!(original.input, reordered.input);
    assert_eq!(original.input, reseeded.input);
    assert_ne!(original.binding_digest(), reordered.binding_digest());
    assert_ne!(original.binding_digest(), reseeded.binding_digest());
    assert_eq!(reseeded.sampling.seed, Some(8));
}

#[test]
fn equal_candidate_bytes_and_output_keep_transport_responses_request_bound() {
    let clause = clause("meaning");
    let build_equal = |presentation, seed| {
        build_local_judge_attempt_request(
            "case-a",
            "Hello world",
            "same candidate",
            "same candidate",
            presentation,
            &[&clause],
            &model_id(),
            &model_digest(),
            seed,
            limits(),
        )
        .expect("equal-candidate request")
    };
    let first = build_equal(JudgePresentation::CandidateAFirst, A_FIRST_SEED);
    let second = build_equal(JudgePresentation::CandidateBFirst, B_FIRST_SEED);
    assert_eq!(first.input, second.input);
    assert_ne!(first.binding_digest(), second.binding_digest());

    let complete = |request: &StructuredCompletionRequest| {
        StructuredCompletionResponse::complete(
            request,
            RuntimeIdentity {
                backend: "ollama_native".to_owned(),
                version: "0.32.15".to_owned(),
                digest: Some(Digest::sha256(b"equal-output runtime")),
            },
            request.artifact_id.clone(),
            request.artifact_digest.clone(),
            r#"{"choice":"tie","rubric_clauses":["meaning"],"source_spans":[],"first_candidate_spans":[],"second_candidate_spans":[]}"#.to_owned(),
            UsageObservation {
                input_tokens: Some(10),
                output_tokens: Some(5),
                generation_micros: Some(50),
            },
        )
        .expect("equal-output response")
    };
    let first_response = complete(&first);
    let second_response = complete(&second);
    assert_ne!(
        derive_ollama_retained_session_response_id(&first_response),
        derive_ollama_retained_session_response_id(&second_response)
    );
}

#[test]
fn content_and_complete_prompt_limits_fail_before_request_release() {
    let mut bounded = limits();
    bounded.maximum_source_bytes = 10;
    assert_eq!(
        build(JudgePresentation::CandidateAFirst, 1, bounded),
        Err(LocalJudgeAttemptBuildError::InputLimitExceeded)
    );

    bounded = limits();
    bounded.maximum_candidate_bytes = 12;
    assert_eq!(
        build(JudgePresentation::CandidateAFirst, 1, bounded),
        Err(LocalJudgeAttemptBuildError::InputLimitExceeded)
    );
    assert_eq!(
        build(JudgePresentation::CandidateBFirst, 1, bounded),
        Err(LocalJudgeAttemptBuildError::InputLimitExceeded)
    );

    let request = build(JudgePresentation::CandidateAFirst, 1, limits()).expect("request");
    bounded = limits();
    bounded.maximum_input_bytes = request.input.len() as u64;
    assert!(build(JudgePresentation::CandidateAFirst, 1, bounded).is_ok());
    bounded.maximum_input_bytes -= 1;
    assert_eq!(
        build(JudgePresentation::CandidateAFirst, 1, bounded),
        Err(LocalJudgeAttemptBuildError::InputLimitExceeded)
    );
}

#[test]
fn invalid_limits_model_binding_and_rubric_are_closed_failures() {
    let mut invalid_limits = limits();
    invalid_limits.maximum_candidate_bytes = 0;
    assert_eq!(
        build(JudgePresentation::CandidateAFirst, 1, invalid_limits),
        Err(LocalJudgeAttemptBuildError::InvalidRequest)
    );

    let digest = Digest::sha256(b"different model");
    let meaning = clause("meaning");
    assert_eq!(
        build_local_judge_attempt_request(
            "case-a",
            "Hello world",
            "Hello, world!",
            "Hello world.",
            JudgePresentation::CandidateAFirst,
            &[&meaning],
            &model_id(),
            &digest,
            1,
            limits(),
        ),
        Err(LocalJudgeAttemptBuildError::InvalidRequest)
    );
    assert_eq!(
        build_with_clauses(&[]),
        Err(LocalJudgeAttemptBuildError::RubricMismatch)
    );
    let zeta = clause("zeta");
    let alpha = clause("alpha");
    assert_eq!(
        build_with_clauses(&[&zeta, &alpha]),
        Err(LocalJudgeAttemptBuildError::RubricMismatch)
    );
    assert_eq!(
        build_with_clauses(&[&alpha, &alpha]),
        Err(LocalJudgeAttemptBuildError::RubricMismatch)
    );
}

#[test]
fn legacy_error_mapping_preserves_every_closed_builder_kind() {
    assert!(matches!(
        map_build_error::<()>(LocalJudgeAttemptBuildError::RubricMismatch),
        LocalJudgeExecutionError::RubricMismatch
    ));
    assert!(matches!(
        map_build_error::<()>(LocalJudgeAttemptBuildError::InputLimitExceeded),
        LocalJudgeExecutionError::InputLimitExceeded
    ));
    assert!(matches!(
        map_build_error::<()>(LocalJudgeAttemptBuildError::PromptEncoding),
        LocalJudgeExecutionError::PromptEncoding
    ));
    assert!(matches!(
        map_build_error::<()>(LocalJudgeAttemptBuildError::InvalidRequest),
        LocalJudgeExecutionError::InvalidRequest
    ));
}

fn build(
    presentation: JudgePresentation,
    seed: u64,
    limits: LocalJudgeAttemptLimits,
) -> Result<StructuredCompletionRequest, LocalJudgeAttemptBuildError> {
    let clause = clause("meaning");
    build_local_judge_attempt_request(
        "case-a",
        "Hello world",
        "Hello, world!",
        "Hello world.",
        presentation,
        &[&clause],
        &model_id(),
        &model_digest(),
        seed,
        limits,
    )
}

fn build_with_clauses(
    clauses: &[&LocalJudgeRubricClause],
) -> Result<StructuredCompletionRequest, LocalJudgeAttemptBuildError> {
    build_local_judge_attempt_request(
        "case-a",
        "source",
        "candidate a",
        "candidate b",
        JudgePresentation::CandidateAFirst,
        clauses,
        &model_id(),
        &model_digest(),
        1,
        limits(),
    )
}

fn clause(id: &str) -> LocalJudgeRubricClause {
    LocalJudgeRubricClause {
        id: id.to_owned(),
        instruction: "Prefer the candidate that preserves the source meaning.".to_owned(),
    }
}

fn model_id() -> ArtifactId {
    ArtifactId::from_digest(model_digest())
}

fn model_digest() -> Digest {
    Digest::from_sha256_hex("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .expect("model digest")
}

const fn limits() -> LocalJudgeAttemptLimits {
    LocalJudgeAttemptLimits {
        maximum_source_bytes: 4_096,
        maximum_candidate_bytes: 4_096,
        maximum_input_bytes: 16_384,
        context_token_limit: 8_192,
        output_token_limit: 512,
        maximum_response_bytes: 4_096,
    }
}
