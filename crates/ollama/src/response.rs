use std::{collections::BTreeSet, future::Future};

use futures_util::StreamExt as _;
use reqwest::{Response, StatusCode, header};
use rewrite_inference::{
    GenerationCandidate, GenerationRequest, InferenceError, InferenceErrorKind, InventoryEntry,
    OperationContext,
};
use rewrite_model::ArtifactId;
use rewrite_types::Digest;
use serde::de::DeserializeOwned;

use crate::{
    contract::{MAX_REFERENCE_BYTES, OllamaModelBinding},
    wire::{CandidateEnvelope, GenerateResponse, TagModel, TagsResponse},
};

const MAX_INVENTORY_ITEMS: usize = 512;

pub(crate) fn parse_inventory(tags: &TagsResponse) -> Result<Vec<InventoryEntry>, InferenceError> {
    if tags.models.len() > MAX_INVENTORY_ITEMS {
        return Err(malformed_error("inventory_too_large"));
    }
    let mut references = BTreeSet::new();
    tags.models
        .iter()
        .map(|model| {
            validate_tag(model, &mut references)?;
            let digest = parse_ollama_digest(&model.digest)?;
            Ok(InventoryEntry {
                reference: model.name.clone(),
                artifact_id: ArtifactId::from_digest(digest.clone()),
                artifact_digest: digest,
                byte_size: Some(model.size),
            })
        })
        .collect()
}

fn validate_tag<'a>(
    model: &'a TagModel,
    references: &mut BTreeSet<&'a str>,
) -> Result<(), InferenceError> {
    if !valid_text(&model.name, MAX_REFERENCE_BYTES)
        || !valid_text(&model.model, MAX_REFERENCE_BYTES)
        || model.size == 0
        || !references.insert(model.name.as_str())
    {
        return Err(malformed_error("invalid_inventory_entry"));
    }
    Ok(())
}

pub(crate) fn confirm_binding_in_tags(
    binding: &OllamaModelBinding,
    tags: &TagsResponse,
) -> Result<(), InferenceError> {
    let tag = tags
        .models
        .iter()
        .find(|model| model.name == binding.reference)
        .ok_or_else(|| compatibility_error("bound_model_missing"))?;
    let digest = parse_ollama_digest(&tag.digest)?;
    if digest != binding.artifact_digest {
        return Err(policy_error("bound_model_digest_changed"));
    }
    Ok(())
}

pub(crate) fn validate_generate_response(
    response: &GenerateResponse,
    binding: &OllamaModelBinding,
) -> Result<(), InferenceError> {
    if response.model != binding.reference || !response.done || !response.thinking.is_empty() {
        return Err(malformed_error("invalid_generation_response"));
    }
    Ok(())
}

fn parse_ollama_digest(value: &str) -> Result<Digest, InferenceError> {
    let hex = value
        .strip_prefix("sha256:")
        .ok_or_else(|| malformed_error("invalid_model_digest"))?;
    Digest::from_sha256_hex(hex.to_owned())
        .map_err(|_error| malformed_error("invalid_model_digest"))
}

pub(crate) fn parse_candidates(
    envelope: CandidateEnvelope,
    request: &GenerationRequest,
) -> Result<Vec<GenerationCandidate>, InferenceError> {
    if envelope.candidates.len() != usize::from(request.candidate_count) {
        return Err(malformed_error("candidate_count_mismatch"));
    }
    envelope
        .candidates
        .into_iter()
        .enumerate()
        .map(|(ordinal, candidate)| {
            if u64::try_from(candidate.text.len()).unwrap_or(u64::MAX)
                > request.candidate_byte_limit
            {
                return Err(malformed_error("candidate_too_large"));
            }
            Ok(GenerationCandidate {
                ordinal: u8::try_from(ordinal)
                    .map_err(|_error| malformed_error("candidate_ordinal_overflow"))?,
                text: candidate.text,
            })
        })
        .collect()
}

pub(crate) async fn decode_response<T: DeserializeOwned>(
    response: Response,
    body_limit: usize,
    context: OperationContext<'_>,
) -> Result<T, InferenceError> {
    let status = response.status();
    if !status.is_success() {
        return Err(map_status(status));
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !content_type
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(malformed_error("unexpected_content_type"));
    }
    if response
        .content_length()
        .is_some_and(|length| length > body_limit as u64)
    {
        return Err(malformed_error("response_body_too_large"));
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = await_context(context, stream.next()).await? {
        let chunk = chunk.map_err(|error| map_transport_error(&error))?;
        let new_length = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| malformed_error("response_body_too_large"))?;
        if new_length > body_limit {
            return Err(malformed_error("response_body_too_large"));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_error| malformed_error("invalid_json_response"))
}

pub(crate) async fn await_context<T>(
    context: OperationContext<'_>,
    future: impl Future<Output = T>,
) -> Result<T, InferenceError> {
    check_context(context)?;
    if let Some(deadline) = context.deadline() {
        tokio::select! {
            biased;
            () = context.cancellation().cancelled() => Err(cancelled_error()),
            () = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                Err(deadline_error())
            }
            value = future => Ok(value),
        }
    } else {
        tokio::select! {
            biased;
            () = context.cancellation().cancelled() => Err(cancelled_error()),
            value = future => Ok(value),
        }
    }
}

pub(crate) fn check_context(context: OperationContext<'_>) -> Result<(), InferenceError> {
    if context.is_cancelled() {
        Err(cancelled_error())
    } else if context.is_expired() {
        Err(deadline_error())
    } else {
        Ok(())
    }
}

pub(crate) fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

pub(crate) fn map_transport_error(error: &reqwest::Error) -> InferenceError {
    if error.is_timeout() {
        deadline_error()
    } else if error.is_connect() {
        retryable_error("connection_failed")
    } else {
        retryable_error("transport_failed")
    }
}

fn map_status(status: StatusCode) -> InferenceError {
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        retryable_error("http_transient")
    } else if status == StatusCode::NOT_FOUND {
        compatibility_error("api_not_found")
    } else {
        permanent_error("http_rejected")
    }
}

fn cancelled_error() -> InferenceError {
    InferenceError::new(InferenceErrorKind::Cancelled, "cancelled")
}

fn deadline_error() -> InferenceError {
    InferenceError::new(InferenceErrorKind::Deadline, "deadline")
}

fn retryable_error(code: &'static str) -> InferenceError {
    InferenceError::new(InferenceErrorKind::Retryable, code)
}

pub(crate) fn compatibility_error(code: &'static str) -> InferenceError {
    InferenceError::new(InferenceErrorKind::Compatibility, code)
}

pub(crate) fn policy_error(code: &'static str) -> InferenceError {
    InferenceError::new(InferenceErrorKind::Policy, code)
}

pub(crate) fn malformed_error(code: &'static str) -> InferenceError {
    InferenceError::new(InferenceErrorKind::MalformedResponse, code)
}

pub(crate) fn permanent_error(code: &'static str) -> InferenceError {
    InferenceError::new(InferenceErrorKind::Permanent, code)
}
