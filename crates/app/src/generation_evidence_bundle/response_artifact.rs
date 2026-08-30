use std::fmt;

use rewrite_inference::{
    ContractError, StructuredCompletionRequest, StructuredCompletionResponse, UsageObservation,
};
use rewrite_model::{
    ArtifactId, OllamaRetainedSessionResponseId, RuntimeIdentity,
    StructuredCompletionRequestBindingId,
};
use rewrite_ollama::{OllamaSessionExecutionReceipt, derive_ollama_retained_session_response_id};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Schema version for one canonical retained structured-response artifact.
pub const RETAINED_STRUCTURED_RESPONSE_ARTIFACT_SCHEMA_VERSION: u32 = 1;
/// Hard encoded-byte ceiling for one retained structured-response artifact.
pub const MAX_RETAINED_STRUCTURED_RESPONSE_ARTIFACT_JSON_BYTES: usize = 256 * 1_024 * 1_024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RetainedStructuredResponseFinishV1 {
    Complete,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedStructuredResponseArtifactWireV1 {
    schema_version: u32,
    response_id: OllamaRetainedSessionResponseId,
    runtime: RuntimeIdentity,
    artifact_id: ArtifactId,
    artifact_digest: Digest,
    structured_request_binding_id: StructuredCompletionRequestBindingId,
    output_json: String,
    usage: UsageObservation,
    finish: RetainedStructuredResponseFinishV1,
}

/// Canonical exact-byte artifact for one retained structured response.
///
/// This inert record preserves every field in the unchanged Ollama retained
/// response V1 identity. It proves no transport, model use, handler placement,
/// effective runtime identity, or qualification.
#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedStructuredResponseArtifactV1 {
    schema_version: u32,
    response_id: OllamaRetainedSessionResponseId,
    runtime: RuntimeIdentity,
    artifact_id: ArtifactId,
    artifact_digest: Digest,
    structured_request_binding_id: StructuredCompletionRequestBindingId,
    output_json: String,
    usage: UsageObservation,
    finish: RetainedStructuredResponseFinishV1,
    #[serde(skip)]
    canonical_json: Vec<u8>,
    #[serde(skip)]
    content_artifact_id: ArtifactId,
    #[serde(skip)]
    canonical_json_byte_size: u64,
}

impl RetainedStructuredResponseArtifactV1 {
    /// Compiles one artifact from an exact response and its retained-session receipt.
    ///
    /// # Errors
    ///
    /// Returns [`RetainedStructuredResponseArtifactError`] if the receipt does not
    /// bind the response's exact request and response identities, or if canonical
    /// encoding exceeds the hard evidence ceiling.
    pub fn from_retained_response(
        response: &StructuredCompletionResponse,
        receipt: &OllamaSessionExecutionReceipt,
    ) -> Result<Self, RetainedStructuredResponseArtifactError> {
        let response_id = derive_ollama_retained_session_response_id(response);
        if receipt.request_digest() != response.request_binding_digest()
            || receipt.response_digest() != response_id.digest()
            || receipt.retained_response_id() != response_id
        {
            return Err(RetainedStructuredResponseArtifactError::RelationshipMismatch);
        }
        Self::build(response, response_id)
    }

    /// Decodes canonical bounded JSON against an exact structured request and response identity.
    ///
    /// The decoder reconstructs [`StructuredCompletionResponse`], repeats its
    /// contract validation, and independently rederives the unchanged Ollama
    /// retained-response identity.
    ///
    /// # Errors
    ///
    /// Returns [`RetainedStructuredResponseArtifactError`] for oversized,
    /// malformed, unsupported, noncanonical, or substituted evidence.
    pub fn from_json_bytes(
        bytes: &[u8],
        expected_request: &StructuredCompletionRequest,
        expected_response_id: &OllamaRetainedSessionResponseId,
    ) -> Result<Self, RetainedStructuredResponseArtifactError> {
        validate_encoded_size(bytes.len())?;
        let wire: RetainedStructuredResponseArtifactWireV1 = serde_json::from_slice(bytes)
            .map_err(|_| RetainedStructuredResponseArtifactError::InvalidEncoding)?;
        if wire.schema_version != RETAINED_STRUCTURED_RESPONSE_ARTIFACT_SCHEMA_VERSION {
            return Err(RetainedStructuredResponseArtifactError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        if &wire.response_id != expected_response_id {
            return Err(RetainedStructuredResponseArtifactError::RelationshipMismatch);
        }
        let decoded = Self {
            schema_version: wire.schema_version,
            response_id: wire.response_id,
            runtime: wire.runtime,
            artifact_id: wire.artifact_id,
            artifact_digest: wire.artifact_digest,
            structured_request_binding_id: wire.structured_request_binding_id,
            output_json: wire.output_json,
            usage: wire.usage,
            finish: wire.finish,
            canonical_json: Vec::new(),
            content_artifact_id: ArtifactId::from_digest(Digest::sha256(b"uninitialized")),
            canonical_json_byte_size: 0,
        };
        let response = decoded.reconstruct_response(expected_request)?;
        let observed_response_id = derive_ollama_retained_session_response_id(&response);
        let artifact = Self::build(&response, observed_response_id)?;
        if artifact.canonical_json != bytes {
            return Err(RetainedStructuredResponseArtifactError::NonCanonicalEncoding);
        }
        Ok(artifact)
    }

    fn build(
        response: &StructuredCompletionResponse,
        response_id: OllamaRetainedSessionResponseId,
    ) -> Result<Self, RetainedStructuredResponseArtifactError> {
        let mut artifact = Self {
            schema_version: RETAINED_STRUCTURED_RESPONSE_ARTIFACT_SCHEMA_VERSION,
            response_id,
            runtime: response.runtime().clone(),
            artifact_id: response.artifact_id().clone(),
            artifact_digest: response.artifact_digest().clone(),
            structured_request_binding_id:
                StructuredCompletionRequestBindingId::from_derived_digest(
                    response.request_binding_digest().clone(),
                ),
            output_json: response.output_json().to_owned(),
            usage: response.usage(),
            finish: RetainedStructuredResponseFinishV1::Complete,
            canonical_json: Vec::new(),
            content_artifact_id: ArtifactId::from_digest(Digest::sha256(b"uninitialized")),
            canonical_json_byte_size: 0,
        };
        let canonical_json = serde_json::to_vec(&artifact)
            .map_err(|_| RetainedStructuredResponseArtifactError::InvalidEncoding)?;
        validate_encoded_size(canonical_json.len())?;
        let canonical_json_byte_size = u64::try_from(canonical_json.len())
            .map_err(|_| RetainedStructuredResponseArtifactError::EncodedRecordTooLarge)?;
        artifact.content_artifact_id = ArtifactId::from_digest(Digest::sha256(&canonical_json));
        artifact.canonical_json_byte_size = canonical_json_byte_size;
        artifact.canonical_json = canonical_json;
        Ok(artifact)
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the unchanged retained-session response identity.
    #[must_use]
    pub const fn response_id(&self) -> &OllamaRetainedSessionResponseId {
        &self.response_id
    }

    /// Returns the exact structured-request binding identity.
    #[must_use]
    pub const fn structured_request_binding_id(&self) -> &StructuredCompletionRequestBindingId {
        &self.structured_request_binding_id
    }

    /// Reconstructs the validated structured response against its exact request.
    ///
    /// # Errors
    ///
    /// Returns [`RetainedStructuredResponseArtifactError`] if the request binding,
    /// finish state, structured-response contract, or retained response identity
    /// does not match.
    pub fn reconstruct_response(
        &self,
        expected_request: &StructuredCompletionRequest,
    ) -> Result<StructuredCompletionResponse, RetainedStructuredResponseArtifactError> {
        if self.structured_request_binding_id != expected_request.structured_request_binding_id()
            || self.finish != RetainedStructuredResponseFinishV1::Complete
        {
            return Err(RetainedStructuredResponseArtifactError::RelationshipMismatch);
        }
        let response = StructuredCompletionResponse::complete(
            expected_request,
            self.runtime.clone(),
            self.artifact_id.clone(),
            self.artifact_digest.clone(),
            self.output_json.clone(),
            self.usage,
        )
        .map_err(RetainedStructuredResponseArtifactError::ResponseContract)?;
        if derive_ollama_retained_session_response_id(&response) != self.response_id {
            return Err(RetainedStructuredResponseArtifactError::RelationshipMismatch);
        }
        Ok(response)
    }

    /// Returns the provider artifact identity bound into the structured response.
    #[must_use]
    pub const fn provider_artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact observed usage bound into the structured response.
    #[must_use]
    pub const fn usage(&self) -> UsageObservation {
        self.usage
    }

    /// Returns the exact untrusted structured output JSON.
    #[must_use]
    pub fn output_json(&self) -> &str {
        &self.output_json
    }

    /// Returns the complete canonical JSON artifact bytes.
    #[must_use]
    pub fn canonical_json_bytes(&self) -> &[u8] {
        &self.canonical_json
    }

    /// Returns the raw SHA-256 artifact identity of the canonical JSON bytes.
    #[must_use]
    pub const fn content_artifact_id(&self) -> &ArtifactId {
        &self.content_artifact_id
    }

    /// Returns the exact canonical JSON byte size.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.canonical_json_byte_size
    }
}

impl fmt::Debug for RetainedStructuredResponseArtifactV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedStructuredResponseArtifactV1")
            .field("response_id", &self.response_id)
            .field("content_artifact_id", &self.content_artifact_id)
            .field("byte_size", &self.canonical_json_byte_size)
            .finish_non_exhaustive()
    }
}

/// Failure while compiling or decoding one retained structured-response artifact.
#[derive(Debug, Error)]
pub enum RetainedStructuredResponseArtifactError {
    /// The encoded artifact exceeds its hard byte ceiling.
    #[error("retained structured-response artifact exceeds its byte limit")]
    EncodedRecordTooLarge,
    /// The JSON is malformed, duplicated, unknown-field-bearing, or not representable.
    #[error("retained structured-response artifact encoding is invalid")]
    InvalidEncoding,
    /// The JSON names an unsupported schema version.
    #[error("retained structured-response artifact schema version {0} is unsupported")]
    UnsupportedSchema(u32),
    /// The JSON does not use the one exact canonical representation.
    #[error("retained structured-response artifact encoding is not canonical")]
    NonCanonicalEncoding,
    /// The response, receipt, request, or expected response identity does not match.
    #[error("retained structured-response artifact relationship does not match")]
    RelationshipMismatch,
    /// Reconstructing the structured response failed its inference contract.
    #[error("retained structured-response artifact response contract is invalid")]
    ResponseContract(#[source] ContractError),
}

fn validate_encoded_size(length: usize) -> Result<(), RetainedStructuredResponseArtifactError> {
    if length > MAX_RETAINED_STRUCTURED_RESPONSE_ARTIFACT_JSON_BYTES {
        Err(RetainedStructuredResponseArtifactError::EncodedRecordTooLarge)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
