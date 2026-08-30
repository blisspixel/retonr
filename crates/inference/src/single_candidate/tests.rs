use rewrite_model::ArtifactId;
use rewrite_types::Digest;

use super::{
    SingleCandidateRequestMappingError, derive_single_candidate_structured_request,
    validate_single_candidate_structured_request,
};
use crate::{
    CandidateOutputPolicy, ContractError, GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest,
    OutputContract, ReasoningPolicy, SamplingParameters,
};

fn request() -> GenerationRequest {
    let artifact_digest = Digest::sha256(b"single candidate artifact");
    let schema_json = "{\"type\":\"object\"}".to_owned();
    GenerationRequest {
        schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
        artifact_id: ArtifactId::from_digest(artifact_digest.clone()),
        artifact_digest,
        input: "bounded grounded request".to_owned(),
        output: OutputContract {
            schema_digest: Digest::sha256(schema_json.as_bytes()),
            schema_json,
        },
        candidate_count: 1,
        source_byte_count: 16,
        source_byte_limit: 1_024,
        input_byte_limit: 2_048,
        context_token_limit: 4_096,
        output_token_limit: 256,
        candidate_byte_limit: 1_024,
        sampling: SamplingParameters {
            temperature: 0.25,
            top_p: 0.9,
            seed: Some(7),
        },
        reasoning: ReasoningPolicy::Disabled,
    }
}

#[test]
fn maps_every_shared_field_and_uses_the_derived_envelope_ceiling() {
    let request = request();
    let policy = CandidateOutputPolicy::new(1, 1_024, 768).expect("candidate policy");
    let structured = derive_single_candidate_structured_request(&request, policy)
        .expect("single-candidate mapping");

    assert_eq!(structured.artifact_id, request.artifact_id);
    assert_eq!(structured.artifact_digest, request.artifact_digest);
    assert_eq!(structured.input, request.input);
    assert_eq!(structured.output, request.output);
    assert_eq!(structured.source_byte_count, request.source_byte_count);
    assert_eq!(structured.source_byte_limit, request.source_byte_limit);
    assert_eq!(structured.input_byte_limit, request.input_byte_limit);
    assert_eq!(structured.context_token_limit, request.context_token_limit);
    assert_eq!(structured.output_token_limit, request.output_token_limit);
    assert_eq!(
        structured.output_byte_limit,
        policy.maximum_envelope_bytes()
    );
    assert_eq!(structured.sampling, request.sampling);
    assert_eq!(structured.reasoning, request.reasoning);
    assert_ne!(
        structured.structured_request_binding_id().digest(),
        request.generation_request_binding_id().digest()
    );
    validate_single_candidate_structured_request(&request, policy, &structured)
        .expect("borrowed exact mapping validation");
}

#[test]
fn borrowed_validation_rejects_every_substituted_mapped_field() {
    let request = request();
    let policy = CandidateOutputPolicy::new(1, 1_024, 768).expect("candidate policy");
    let structured = derive_single_candidate_structured_request(&request, policy)
        .expect("single-candidate mapping");
    let substitutions = [
        {
            let mut value = structured.clone();
            value.input = "foreign input".to_owned();
            value
        },
        {
            let mut value = structured.clone();
            let digest = Digest::sha256(b"foreign artifact");
            value.artifact_id = ArtifactId::from_digest(digest.clone());
            value.artifact_digest = digest;
            value
        },
        {
            let mut value = structured.clone();
            value.output_byte_limit = value.output_byte_limit.saturating_sub(1);
            value
        },
        {
            let mut value = structured.clone();
            value.sampling.seed = Some(8);
            value
        },
    ];

    for substituted in substitutions {
        assert_eq!(
            validate_single_candidate_structured_request(&request, policy, &substituted),
            Err(SingleCandidateRequestMappingError::StructuredRequestMismatch)
        );
    }
}

#[test]
fn borrowed_validation_distinguishes_signed_zero_sampling_bits() {
    let mut request = request();
    request.sampling.temperature = 0.0;
    request.sampling.top_p = 0.0;
    let policy = CandidateOutputPolicy::new(1, 1_024, 768).expect("candidate policy");
    let structured = derive_single_candidate_structured_request(&request, policy)
        .expect("single-candidate mapping");

    for substituted in [
        {
            let mut value = structured.clone();
            value.sampling.temperature = -0.0;
            value
        },
        {
            let mut value = structured.clone();
            value.sampling.top_p = -0.0;
            value
        },
    ] {
        assert_ne!(
            substituted.structured_request_binding_id(),
            structured.structured_request_binding_id()
        );
        assert_eq!(
            validate_single_candidate_structured_request(&request, policy, &substituted),
            Err(SingleCandidateRequestMappingError::StructuredRequestMismatch)
        );
    }
}

#[test]
fn borrowed_validation_preserves_input_and_policy_error_classes() {
    let request = request();
    let policy = CandidateOutputPolicy::new(1, 1_024, 768).expect("candidate policy");
    let structured = derive_single_candidate_structured_request(&request, policy)
        .expect("single-candidate mapping");

    assert_eq!(
        validate_single_candidate_structured_request(
            &request,
            CandidateOutputPolicy::new(1, 512, 512).expect("foreign policy"),
            &structured,
        ),
        Err(SingleCandidateRequestMappingError::OutputPolicyMismatch)
    );

    let mut invalid = structured;
    invalid.output_byte_limit = 0;
    assert!(matches!(
        validate_single_candidate_structured_request(&request, policy, &invalid),
        Err(SingleCandidateRequestMappingError::InvalidStructuredRequest(_))
    ));
}

#[test]
fn rejects_multiple_candidates_without_fabricating_request_local_ordinals() {
    let mut request = request();
    request.candidate_count = 2;
    assert_eq!(
        derive_single_candidate_structured_request(
            &request,
            CandidateOutputPolicy::new(2, 1_024, 2_048).expect("candidate policy")
        ),
        Err(SingleCandidateRequestMappingError::CandidateCountUnsupported)
    );
}

#[test]
fn rejects_policy_substitution_and_invalid_provider_requests() {
    let request = request();
    for policy in [
        CandidateOutputPolicy::new(2, 1_024, 2_048).expect("wrong count policy"),
        CandidateOutputPolicy::new(1, 512, 512).expect("wrong byte policy"),
    ] {
        assert_eq!(
            derive_single_candidate_structured_request(&request, policy),
            Err(SingleCandidateRequestMappingError::OutputPolicyMismatch)
        );
    }

    let mut invalid = request;
    invalid.artifact_digest = Digest::sha256(b"substituted artifact");
    assert_eq!(
        derive_single_candidate_structured_request(
            &invalid,
            CandidateOutputPolicy::new(1, 1_024, 1_024).expect("candidate policy")
        ),
        Err(
            SingleCandidateRequestMappingError::InvalidGenerationRequest(
                ContractError::ArtifactMismatch
            )
        )
    );
}
