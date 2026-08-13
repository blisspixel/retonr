use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_model::{ArtifactId, RuntimeIdentity};
use rewrite_types::Digest;

use crate::{ContractError, OutputContract, ReasoningPolicy, SamplingParameters, UsageObservation};

/// Current structured-completion request contract version.
pub const STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION: u32 = 1;

/// Fully explicit request for one bounded structured JSON payload.
#[derive(Clone, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredCompletionRequest {
    /// Request contract version.
    pub schema_version: u32,
    /// Exact artifact selected by the caller.
    pub artifact_id: ArtifactId,
    /// Digest rechecked around inference.
    pub artifact_digest: Digest,
    /// Complete bounded prompt or backend input.
    pub input: String,
    /// Exact structured-output contract.
    pub output: OutputContract,
    /// Observed source bytes represented inside the complete input.
    pub source_byte_count: u64,
    /// Qualified source-byte envelope.
    pub source_byte_limit: u64,
    /// Maximum complete prompt or backend-input text bytes.
    pub input_byte_limit: u64,
    /// Qualified context envelope.
    pub context_token_limit: u32,
    /// Maximum generated tokens requested from the backend.
    pub output_token_limit: u32,
    /// Maximum UTF-8 JSON bytes accepted from the backend payload.
    pub output_byte_limit: u64,
    /// Explicit sampling policy.
    pub sampling: SamplingParameters,
    /// Explicit reasoning-output policy.
    pub reasoning: ReasoningPolicy,
}

impl fmt::Debug for StructuredCompletionRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StructuredCompletionRequest")
            .field("schema_version", &self.schema_version)
            .field("artifact_id", &self.artifact_id)
            .field("artifact_digest", &self.artifact_digest)
            .field("input_bytes", &self.input.len())
            .field("output_schema_digest", &self.output.schema_digest)
            .field("source_byte_count", &self.source_byte_count)
            .field("source_byte_limit", &self.source_byte_limit)
            .field("input_byte_limit", &self.input_byte_limit)
            .field("context_token_limit", &self.context_token_limit)
            .field("output_token_limit", &self.output_token_limit)
            .field("output_byte_limit", &self.output_byte_limit)
            .field("sampling", &self.sampling)
            .field("reasoning", &self.reasoning)
            .finish()
    }
}

impl StructuredCompletionRequest {
    /// Validates exact identity, resource limits, sampling, and output schema.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when the request is unsupported or inconsistent.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION {
            return Err(ContractError::UnsupportedSchema);
        }
        if self.artifact_id.digest() != &self.artifact_digest {
            return Err(ContractError::ArtifactMismatch);
        }
        if self.source_byte_count > self.source_byte_limit
            || self.source_byte_limit == 0
            || self.input_byte_limit == 0
            || self.context_token_limit == 0
            || self.output_token_limit == 0
            || self.output_byte_limit == 0
            || u64::try_from(self.input.len()).unwrap_or(u64::MAX) > self.input_byte_limit
        {
            return Err(ContractError::InvalidLimits);
        }
        if !self.sampling.temperature.is_finite()
            || !(0.0..=2.0).contains(&self.sampling.temperature)
            || !self.sampling.top_p.is_finite()
            || !(0.0..=1.0).contains(&self.sampling.top_p)
        {
            return Err(ContractError::InvalidSampling);
        }
        self.output.validate()
    }

    /// Returns a canonical digest binding the full effective request.
    ///
    /// The digest is an equality binding, not anonymization. Short predictable
    /// inputs can still be guessed by dictionary attack.
    #[must_use]
    pub fn binding_digest(&self) -> Digest {
        let mut material = b"retonr:structured-completion-request:v1\0".to_vec();
        append_u32(&mut material, self.schema_version);
        append_digest(&mut material, self.artifact_id.digest());
        append_digest(&mut material, &self.artifact_digest);
        append_bytes(&mut material, self.input.as_bytes());
        append_digest(&mut material, &self.output.schema_digest);
        append_bytes(&mut material, self.output.schema_json.as_bytes());
        append_u64(&mut material, self.source_byte_count);
        append_u64(&mut material, self.source_byte_limit);
        append_u64(&mut material, self.input_byte_limit);
        append_u32(&mut material, self.context_token_limit);
        append_u32(&mut material, self.output_token_limit);
        append_u64(&mut material, self.output_byte_limit);
        material.extend_from_slice(&self.sampling.temperature.to_bits().to_be_bytes());
        material.extend_from_slice(&self.sampling.top_p.to_bits().to_be_bytes());
        append_optional_u64(&mut material, self.sampling.seed);
        material.push(match self.reasoning {
            ReasoningPolicy::Disabled => 0,
            ReasoningPolicy::Discard => 1,
        });
        Digest::sha256(&material)
    }
}

/// Terminal status assigned by the transport after complete response validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredCompletionFinish {
    /// The transport completed without truncation and the payload is valid JSON.
    Complete,
}

/// One bounded structured payload with exact observed backend identity.
///
/// Content fields are private and the debug representation omits generated JSON.
#[derive(Clone, Eq, PartialEq)]
pub struct StructuredCompletionResponse {
    runtime: RuntimeIdentity,
    artifact_id: ArtifactId,
    artifact_digest: Digest,
    request_binding_digest: Digest,
    output_json: String,
    usage: UsageObservation,
    finish: StructuredCompletionFinish,
}

impl StructuredCompletionResponse {
    /// Constructs a response after exact binding, byte, and JSON validation.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] for inconsistent identity, size, or JSON framing.
    pub fn complete(
        request: &StructuredCompletionRequest,
        runtime: RuntimeIdentity,
        artifact_id: ArtifactId,
        artifact_digest: Digest,
        output_json: String,
        usage: UsageObservation,
    ) -> Result<Self, ContractError> {
        request.validate()?;
        let output_bytes = u64::try_from(output_json.len()).unwrap_or(u64::MAX);
        if artifact_id != request.artifact_id
            || artifact_digest != request.artifact_digest
            || output_bytes > request.output_byte_limit
            || !is_complete_json(&output_json)
        {
            return Err(ContractError::InvalidStructuredResponse);
        }
        Ok(Self {
            runtime,
            artifact_id,
            artifact_digest,
            request_binding_digest: request.binding_digest(),
            output_json,
            usage,
            finish: StructuredCompletionFinish::Complete,
        })
    }

    /// Returns the observed runtime identity.
    #[must_use]
    pub const fn runtime(&self) -> &RuntimeIdentity {
        &self.runtime
    }

    /// Returns the exact artifact identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact artifact digest rechecked after inference.
    #[must_use]
    pub const fn artifact_digest(&self) -> &Digest {
        &self.artifact_digest
    }

    /// Returns the digest binding this response to its complete request.
    #[must_use]
    pub const fn request_binding_digest(&self) -> &Digest {
        &self.request_binding_digest
    }

    /// Borrows the bounded untrusted JSON payload.
    #[must_use]
    pub fn output_json(&self) -> &str {
        &self.output_json
    }

    /// Returns optional resource observations.
    #[must_use]
    pub const fn usage(&self) -> UsageObservation {
        self.usage
    }

    /// Returns the transport-derived completion status.
    #[must_use]
    pub const fn finish(&self) -> StructuredCompletionFinish {
        self.finish
    }

    /// Consumes the response and returns the bounded untrusted JSON payload.
    #[must_use]
    pub fn into_output_json(self) -> String {
        self.output_json
    }
}

impl fmt::Debug for StructuredCompletionResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StructuredCompletionResponse")
            .field("runtime", &self.runtime)
            .field("artifact_id", &self.artifact_id)
            .field("artifact_digest", &self.artifact_digest)
            .field("request_binding_digest", &self.request_binding_digest)
            .field("output_bytes", &self.output_json.len())
            .field("usage", &self.usage)
            .field("finish", &self.finish)
            .finish()
    }
}

fn is_complete_json(value: &str) -> bool {
    let mut deserializer = serde_json::Deserializer::from_str(value);
    serde::de::IgnoredAny::deserialize(&mut deserializer).is_ok() && deserializer.end().is_ok()
}

fn append_u32(material: &mut Vec<u8>, value: u32) {
    material.extend_from_slice(&value.to_be_bytes());
}

fn append_u64(material: &mut Vec<u8>, value: u64) {
    material.extend_from_slice(&value.to_be_bytes());
}

fn append_bytes(material: &mut Vec<u8>, value: &[u8]) {
    append_u64(material, value.len() as u64);
    material.extend_from_slice(value);
}

fn append_digest(material: &mut Vec<u8>, value: &Digest) {
    material.extend_from_slice(value.as_str().as_bytes());
}

fn append_optional_u64(material: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            material.push(1);
            append_u64(material, value);
        }
        None => material.push(0),
    }
}
