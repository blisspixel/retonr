use thiserror::Error;

use crate::{
    CandidateOutputPolicy, ContractError, GenerationRequest,
    STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, StructuredCompletionRequest,
};

/// Failure to derive the single-candidate structured request used by a retained session.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SingleCandidateRequestMappingError {
    /// The provider-neutral request failed its own complete validation.
    #[error("provider-neutral generation request is invalid: {0}")]
    InvalidGenerationRequest(ContractError),
    /// The first managed mapping supports exactly one declared candidate.
    #[error("managed structured request mapping requires exactly one candidate")]
    CandidateCountUnsupported,
    /// The output policy does not describe the request's exact single candidate.
    #[error("candidate output policy does not match the generation request")]
    OutputPolicyMismatch,
    /// The deterministically derived structured request was invalid.
    #[error("derived structured completion request is invalid: {0}")]
    InvalidStructuredRequest(ContractError),
    /// The supplied structured request does not exactly map the provider-neutral request.
    #[error("structured completion request does not match the generation request")]
    StructuredRequestMismatch,
}

/// Derives the exact structured wire request for one provider-neutral candidate request.
///
/// The first managed qualification protocol deliberately supports one candidate
/// per retained session. The structured response carries the complete candidate
/// envelope, so its byte ceiling is the provider-neutral policy's derived raw
/// envelope ceiling rather than the decoded per-candidate ceiling.
///
/// This pure mapping returns an inert request. It does not prove launch,
/// transport, response, model use, or qualification.
///
/// # Errors
///
/// Returns [`SingleCandidateRequestMappingError`] when the request is invalid,
/// declares any count other than one, the candidate policy does not match its
/// exact count and per-candidate ceiling, or the derived request is invalid.
pub fn derive_single_candidate_structured_request(
    request: &GenerationRequest,
    output_policy: CandidateOutputPolicy,
) -> Result<StructuredCompletionRequest, SingleCandidateRequestMappingError> {
    request
        .validate()
        .map_err(SingleCandidateRequestMappingError::InvalidGenerationRequest)?;
    if request.candidate_count != 1 {
        return Err(SingleCandidateRequestMappingError::CandidateCountUnsupported);
    }
    if output_policy.expected_count() != request.candidate_count
        || output_policy.maximum_candidate_bytes() != request.candidate_byte_limit
    {
        return Err(SingleCandidateRequestMappingError::OutputPolicyMismatch);
    }

    let structured = StructuredCompletionRequest {
        schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
        artifact_id: request.artifact_id.clone(),
        artifact_digest: request.artifact_digest.clone(),
        input: request.input.clone(),
        output: request.output.clone(),
        source_byte_count: request.source_byte_count,
        source_byte_limit: request.source_byte_limit,
        input_byte_limit: request.input_byte_limit,
        context_token_limit: request.context_token_limit,
        output_token_limit: request.output_token_limit,
        output_byte_limit: output_policy.maximum_envelope_bytes(),
        sampling: request.sampling,
        reasoning: request.reasoning,
    };
    structured
        .validate()
        .map_err(SingleCandidateRequestMappingError::InvalidStructuredRequest)?;
    Ok(structured)
}

/// Validates an existing structured request against its exact provider-neutral source.
///
/// This borrowed validation path allocates no second request and is intended for
/// authorities that already retain both bounded forms.
///
/// # Errors
///
/// Returns [`SingleCandidateRequestMappingError`] when either request is invalid,
/// the output policy differs, or any mapped field is not exactly equal.
pub fn validate_single_candidate_structured_request(
    request: &GenerationRequest,
    output_policy: CandidateOutputPolicy,
    structured: &StructuredCompletionRequest,
) -> Result<(), SingleCandidateRequestMappingError> {
    request
        .validate()
        .map_err(SingleCandidateRequestMappingError::InvalidGenerationRequest)?;
    if request.candidate_count != 1 {
        return Err(SingleCandidateRequestMappingError::CandidateCountUnsupported);
    }
    if output_policy.expected_count() != request.candidate_count
        || output_policy.maximum_candidate_bytes() != request.candidate_byte_limit
    {
        return Err(SingleCandidateRequestMappingError::OutputPolicyMismatch);
    }
    structured
        .validate()
        .map_err(SingleCandidateRequestMappingError::InvalidStructuredRequest)?;
    if structured.schema_version == STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION
        && structured.artifact_id == request.artifact_id
        && structured.artifact_digest == request.artifact_digest
        && structured.input == request.input
        && structured.output == request.output
        && structured.source_byte_count == request.source_byte_count
        && structured.source_byte_limit == request.source_byte_limit
        && structured.input_byte_limit == request.input_byte_limit
        && structured.context_token_limit == request.context_token_limit
        && structured.output_token_limit == request.output_token_limit
        && structured.output_byte_limit == output_policy.maximum_envelope_bytes()
        && structured.sampling.temperature.to_bits() == request.sampling.temperature.to_bits()
        && structured.sampling.top_p.to_bits() == request.sampling.top_p.to_bits()
        && structured.sampling.seed == request.sampling.seed
        && structured.reasoning == request.reasoning
    {
        Ok(())
    } else {
        Err(SingleCandidateRequestMappingError::StructuredRequestMismatch)
    }
}

#[cfg(test)]
mod tests;
