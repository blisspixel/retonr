use rewrite_engine::ClaimEvidenceComparator;
use rewrite_inference::{
    InferenceBackend, InferenceErrorKind, OperationContext, ReasoningPolicy,
    STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, StructuredCompletionRequest,
    claim_output_contract,
};
use rewrite_types::{ClaimEvidenceSet, ClaimExtractionStatus, Digest};

mod contract;
mod parse;
mod shadow;

pub use contract::{
    CLAIM_PAIR_OPERATION_ID, CLAIM_PAIR_PROMPT_TEMPLATE, ClaimExtractionContext,
    ClaimExtractionError, ClaimExtractionPair, ClaimExtractionRequest, ClaimShadowJoinBinding,
};
pub use shadow::{
    ClaimShadowJoinDisposition, ClaimShadowJoinService, PreparedClaimShadow, PreparedClaimShadowSet,
};

use crate::MAX_CANDIDATE_CHECK_BYTES;
use parse::{claims_from_payload, parse_payload};

/// Application service that extracts claims from source and candidate independently.
///
/// The operation may record a deterministic comparison when both sides complete.
/// It has no engine authority and does not decide a rewrite.
pub struct ClaimExtractionService<'a> {
    backend: &'a dyn InferenceBackend,
}

impl<'a> ClaimExtractionService<'a> {
    /// Creates a service over an already constructed inference backend.
    #[must_use]
    pub const fn new(backend: &'a dyn InferenceBackend) -> Self {
        Self { backend }
    }

    /// Extracts source claims, then candidate claims, under one cancellation token.
    ///
    /// Either side is examined only through the bound claim-output contract. A
    /// cancellation or deadline observed before both sides complete returns no
    /// evidence. Comparison evidence is not semantic proof.
    ///
    /// # Errors
    ///
    /// Returns [`ClaimExtractionError`] when the request or manifest is invalid,
    /// the backend is unavailable, a payload is malformed, or cancellation is
    /// observed.
    pub async fn extract(
        &self,
        request: ClaimExtractionRequest,
        context: ClaimExtractionContext<'_>,
    ) -> Result<ClaimExtractionPair, ClaimExtractionError> {
        validate_request(&request)?;
        ensure_context(context)?;
        let discovery = self
            .backend
            .discover(OperationContext::new(
                context.cancellation,
                context.deadline,
            ))
            .await
            .map_err(map_backend_error)?;
        let output = claim_output_contract();
        let inventoried = discovery.inventory.iter().any(|entry| {
            entry.artifact_id == request.artifact_id
                && entry.artifact_digest == request.artifact_digest
        });
        if !discovery.capabilities.admits_output(&output) || !inventoried {
            return Err(ClaimExtractionError::Unavailable);
        }

        let source = self.extract_one(&request, &request.source, context).await?;
        ensure_context(context)?;
        let candidate = self
            .extract_one(&request, &request.candidate, context)
            .await?;
        let comparison = if source.extraction_status() == ClaimExtractionStatus::Complete
            && candidate.extraction_status() == ClaimExtractionStatus::Complete
        {
            Some(
                ClaimEvidenceComparator::compare(&source, &candidate)
                    .map_err(ClaimExtractionError::Comparison)?,
            )
        } else {
            None
        };
        Ok(ClaimExtractionPair {
            source,
            candidate,
            comparison,
        })
    }

    async fn extract_one(
        &self,
        request: &ClaimExtractionRequest,
        text: &str,
        context: ClaimExtractionContext<'_>,
    ) -> Result<ClaimEvidenceSet, ClaimExtractionError> {
        ensure_context(context)?;
        let output = claim_output_contract();
        let input = filled_prompt(&request.unit_id, text);
        let completion = StructuredCompletionRequest {
            schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
            artifact_id: request.artifact_id.clone(),
            artifact_digest: request.artifact_digest.clone(),
            input,
            output,
            source_byte_count: u64::try_from(text.len())
                .map_err(|_| ClaimExtractionError::InvalidRequest)?,
            source_byte_limit: request.source_byte_limit,
            input_byte_limit: request.input_byte_limit,
            context_token_limit: request.context_token_limit,
            output_token_limit: request.output_token_limit,
            output_byte_limit: request.output_byte_limit,
            sampling: request.sampling,
            reasoning: ReasoningPolicy::Disabled,
        };
        let response = self
            .backend
            .complete_structured(
                completion,
                OperationContext::new(context.cancellation, context.deadline),
            )
            .await
            .map_err(map_backend_error)?;
        if response.artifact_id() != &request.artifact_id
            || response.artifact_digest() != &request.artifact_digest
        {
            return Err(ClaimExtractionError::Unavailable);
        }
        let payload = parse_payload(response.output_json())?;
        let (status, claims) = claims_from_payload(payload, &request.unit_id, text)?;
        ClaimEvidenceSet::new(
            request.manifest.extractor_id(),
            request.manifest.extractor_version(),
            request.manifest.manifest_digest(),
            status,
            request.minimum_confidence_ppm,
            request.unit_id.clone(),
            text,
            claims,
        )
        .map_err(ClaimExtractionError::InvalidEvidence)
    }
}

fn validate_request(request: &ClaimExtractionRequest) -> Result<(), ClaimExtractionError> {
    let output = claim_output_contract();
    let prompt_digest = Digest::sha256(CLAIM_PAIR_PROMPT_TEMPLATE.as_bytes());
    let operation_digest = Digest::sha256(CLAIM_PAIR_OPERATION_ID.as_bytes());
    if request.manifest.claim_output_contract_digest() != &output.schema_digest
        || request.manifest.prompt_digest() != &prompt_digest
        || request.manifest.claim_operation_contract_digest() != &operation_digest
    {
        return Err(ClaimExtractionError::ManifestMismatch);
    }
    if request.artifact_id.digest() != &request.artifact_digest
        || request.source_byte_limit == 0
        || request.input_byte_limit == 0
        || request.context_token_limit == 0
        || request.output_token_limit == 0
        || request.output_byte_limit == 0
        || request.minimum_confidence_ppm > rewrite_types::CLAIM_CONFIDENCE_PARTS_PER_MILLION
        || !request.sampling.temperature.is_finite()
        || !(0.0..=2.0).contains(&request.sampling.temperature)
        || !request.sampling.top_p.is_finite()
        || !(0.0..=1.0).contains(&request.sampling.top_p)
    {
        return Err(ClaimExtractionError::InvalidRequest);
    }
    enforce_text_limit(&request.source, request.source_byte_limit)?;
    enforce_text_limit(&request.candidate, request.source_byte_limit)?;
    let source_input = filled_prompt(&request.unit_id, &request.source);
    let candidate_input = filled_prompt(&request.unit_id, &request.candidate);
    if source_input.len() > usize::try_from(request.input_byte_limit).unwrap_or(usize::MAX)
        || candidate_input.len() > usize::try_from(request.input_byte_limit).unwrap_or(usize::MAX)
    {
        return Err(ClaimExtractionError::InvalidRequest);
    }
    Ok(())
}

fn enforce_text_limit(text: &str, source_byte_limit: u64) -> Result<(), ClaimExtractionError> {
    let actual = text.len();
    let product = MAX_CANDIDATE_CHECK_BYTES;
    let request_limit = usize::try_from(source_byte_limit).unwrap_or(usize::MAX);
    let maximum = product.min(request_limit);
    if actual > maximum {
        Err(ClaimExtractionError::TextTooLarge { actual, maximum })
    } else {
        Ok(())
    }
}

fn filled_prompt(unit_id: &rewrite_types::RewriteUnitId, text: &str) -> String {
    format!(
        "{CLAIM_PAIR_PROMPT_TEMPLATE}unit_id={}\ntext_digest={}\n---\n{text}",
        unit_id.as_str(),
        Digest::sha256(text.as_bytes())
    )
}

fn ensure_context(context: ClaimExtractionContext<'_>) -> Result<(), ClaimExtractionError> {
    if context.cancellation.is_cancelled() {
        Err(ClaimExtractionError::Cancelled)
    } else if context
        .deadline
        .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    {
        Err(ClaimExtractionError::Deadline)
    } else {
        Ok(())
    }
}

fn map_backend_error(error: rewrite_inference::InferenceError) -> ClaimExtractionError {
    match error.kind {
        InferenceErrorKind::Cancelled => ClaimExtractionError::Cancelled,
        InferenceErrorKind::Deadline => ClaimExtractionError::Deadline,
        InferenceErrorKind::Compatibility | InferenceErrorKind::Policy => {
            ClaimExtractionError::Unavailable
        }
        InferenceErrorKind::MalformedResponse => ClaimExtractionError::InvalidPayload,
        _ => ClaimExtractionError::Backend(error),
    }
}

#[cfg(test)]
mod tests;
