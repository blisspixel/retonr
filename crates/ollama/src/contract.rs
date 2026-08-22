use std::time::Duration;

use rewrite_inference::InferenceError;
use rewrite_model::{ArtifactId, RuntimeIdentity};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

use crate::response::{policy_error, valid_text};

pub(crate) const BACKEND_ID: &str = "ollama_native";
pub(crate) const MAX_REFERENCE_BYTES: usize = 256;
pub(crate) const MAX_VERSION_BYTES: usize = 128;
pub(crate) const MAX_METADATA_BYTES: usize = 256;
const MAX_DISCOVERY_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_GENERATION_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_REQUEST_TIMEOUT: Duration = Duration::from_hours(24);
pub(crate) const MAX_PREFLIGHT_TARGETS: usize = 64;

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
            discovery_body_bytes: 16 * 1024 * 1024,
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
            || self.discovery_body_bytes > MAX_DISCOVERY_BODY_BYTES
            || self.generation_body_bytes > MAX_GENERATION_BODY_BYTES
            || self.connect_timeout.is_zero()
            || self.request_timeout.is_zero()
            || self.read_timeout.is_zero()
            || self.request_timeout > MAX_REQUEST_TIMEOUT
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
    pub(crate) inventory_digest: Digest,
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
        Self::new_with_inventory(
            reference,
            artifact_id,
            artifact_digest.clone(),
            artifact_digest,
        )
    }

    /// Creates a binding with distinct immutable artifact and mutable inventory digests.
    ///
    /// The artifact digest binds completion requests to exact managed model bytes.
    /// The inventory digest is used only to verify the runtime-local Ollama address
    /// before and around execution.
    ///
    /// # Errors
    ///
    /// Returns a policy error when the reference or artifact identity is invalid.
    pub fn new_with_inventory(
        reference: impl Into<String>,
        artifact_id: ArtifactId,
        artifact_digest: Digest,
        inventory_digest: Digest,
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
            inventory_digest,
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

    /// Returns the exact Ollama inventory digest expected at the mutable address.
    #[must_use]
    pub const fn inventory_digest(&self) -> &Digest {
        &self.inventory_digest
    }
}

/// Runtime-local model address and expected inventory digest for read-only preflight.
///
/// This target deliberately carries no Retonr artifact identity. An Ollama inventory digest is
/// insufficient to identify a complete artifact set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OllamaPreflightTarget {
    pub(crate) reference: String,
    pub(crate) inventory_digest: Digest,
}

impl OllamaPreflightTarget {
    /// Creates and validates one read-only preflight target.
    ///
    /// # Errors
    ///
    /// Returns a policy error when the runtime-local reference is invalid.
    pub fn new(
        reference: impl Into<String>,
        inventory_digest: Digest,
    ) -> Result<Self, InferenceError> {
        let reference = reference.into();
        if !valid_text(&reference, MAX_REFERENCE_BYTES) {
            return Err(policy_error("invalid_preflight_target"));
        }
        Ok(Self {
            reference,
            inventory_digest,
        })
    }

    /// Returns the runtime-local mutable model reference.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }

    /// Returns the exact expected Ollama inventory digest.
    #[must_use]
    pub const fn inventory_digest(&self) -> &Digest {
        &self.inventory_digest
    }
}

/// Redacted, bounded model metadata returned by explicit inspection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

/// One bounded model-residency observation from Ollama.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OllamaRunningModel {
    /// Runtime-local mutable reference.
    pub reference: String,
    /// Runtime-reported package digest.
    pub inventory_digest: Digest,
    /// Runtime-reported total loaded-model memory bytes.
    pub byte_size: u64,
    /// Runtime-reported bytes resident in accelerator memory.
    pub accelerator_bytes: u64,
    /// Effective context length reported for the loaded model.
    pub context_tokens: u32,
}

/// One bounded inventory observation without implying Retonr artifact identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OllamaInventoryEntry {
    /// Runtime-local mutable reference.
    pub reference: String,
    /// Runtime-reported package digest.
    pub inventory_digest: Digest,
    /// Runtime-reported package byte size.
    pub byte_size: u64,
}

/// Exact configured binding and bounded model-description evidence observed in one preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OllamaPreflightBinding {
    /// Runtime-local mutable reference that was resolved twice.
    pub reference: String,
    /// Exact configured runtime inventory digest.
    pub inventory_digest: Digest,
    /// Content-redacted model-description evidence.
    pub details: OllamaModelDetails,
}

/// Coherent read-only Ollama discovery evidence captured without generation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OllamaPreflight {
    /// Runtime identity observed before and after the preflight.
    pub runtime: RuntimeIdentity,
    /// Complete bounded runtime inventory.
    pub inventory: Vec<OllamaInventoryEntry>,
    /// Exact configured bindings and their redacted model-description evidence.
    pub bindings: Vec<OllamaPreflightBinding>,
    /// Models resident when the preflight inspected the runtime.
    pub running: Vec<OllamaRunningModel>,
}
