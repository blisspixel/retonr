use std::time::Duration;

use rewrite_inference::{InferenceError, OutputContract};
use rewrite_model::ArtifactId;
use rewrite_types::Digest;

use crate::response::{policy_error, valid_text};

pub(crate) const BACKEND_ID: &str = "ollama_native";
pub(crate) const MAX_REFERENCE_BYTES: usize = 256;
pub(crate) const MAX_VERSION_BYTES: usize = 128;
pub(crate) const MAX_METADATA_BYTES: usize = 256;
const CANDIDATE_SCHEMA: &str = r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"required":["candidates"],"properties":{"candidates":{"type":"array","minItems":1,"maxItems":16,"items":{"type":"object","additionalProperties":false,"required":["text"],"properties":{"text":{"type":"string"}}}}}}"#;

/// Resource and timeout limits applied to every Ollama request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaLimits {
    /// Maximum response bytes accepted from discovery and inspection endpoints.
    pub discovery_body_bytes: usize,
    /// Maximum response bytes accepted from generation.
    pub generation_body_bytes: usize,
    /// Maximum time allowed to establish a loopback connection.
    pub connect_timeout: Duration,
    /// Maximum elapsed time for a complete backend request.
    pub request_timeout: Duration,
    /// Maximum idle interval while reading a response body.
    pub read_timeout: Duration,
    /// Maximum concurrent operations admitted to one local runtime.
    pub max_concurrency: usize,
}

impl Default for OllamaLimits {
    fn default() -> Self {
        Self {
            discovery_body_bytes: 2 * 1024 * 1024,
            generation_body_bytes: 8 * 1024 * 1024,
            connect_timeout: Duration::from_secs(2),
            request_timeout: Duration::from_mins(2),
            read_timeout: Duration::from_secs(30),
            max_concurrency: 1,
        }
    }
}

impl OllamaLimits {
    pub(crate) fn validate(self) -> Result<Self, InferenceError> {
        if self.discovery_body_bytes == 0
            || self.generation_body_bytes == 0
            || self.connect_timeout.is_zero()
            || self.request_timeout.is_zero()
            || self.read_timeout.is_zero()
            || self.max_concurrency == 0
            || self.connect_timeout > self.request_timeout
            || self.read_timeout > self.request_timeout
        {
            return Err(policy_error("invalid_limits"));
        }
        Ok(self)
    }
}

/// Explicit binding from a mutable Ollama model reference to immutable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaModelBinding {
    pub(crate) reference: String,
    pub(crate) artifact_id: ArtifactId,
    pub(crate) artifact_digest: Digest,
}

impl OllamaModelBinding {
    /// Creates and validates an exact model binding.
    ///
    /// # Errors
    ///
    /// Returns a policy error when the reference or artifact identity is invalid.
    pub fn new(
        reference: impl Into<String>,
        artifact_id: ArtifactId,
        artifact_digest: Digest,
    ) -> Result<Self, InferenceError> {
        let reference = reference.into();
        if !valid_text(&reference, MAX_REFERENCE_BYTES) || artifact_id.digest() != &artifact_digest
        {
            return Err(policy_error("invalid_model_binding"));
        }
        Ok(Self {
            reference,
            artifact_id,
            artifact_digest,
        })
    }

    /// Returns the runtime-local model reference.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// Returns the immutable artifact identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact expected runtime digest.
    #[must_use]
    pub const fn artifact_digest(&self) -> &Digest {
        &self.artifact_digest
    }
}

/// Redacted, bounded model metadata returned by explicit inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaModelDetails {
    /// Intrinsic model format reported by the runtime.
    pub format: String,
    /// Model family reported by the runtime.
    pub family: String,
    /// Quantization level reported by the runtime.
    pub quantization: String,
    /// Bounded runtime capabilities.
    pub capabilities: Vec<String>,
    /// Digest of the exact license text without retaining the text here.
    pub license_digest: Digest,
    /// Digest of the exact runtime template without retaining the template here.
    pub template_digest: Digest,
    /// Digest of canonical detailed model metadata.
    pub metadata_digest: Digest,
}

/// Returns the exact structured-output contract supported by this adapter.
#[must_use]
pub fn candidate_output_contract() -> OutputContract {
    OutputContract {
        schema_digest: Digest::sha256(CANDIDATE_SCHEMA.as_bytes()),
        schema_json: CANDIDATE_SCHEMA.to_owned(),
    }
}
