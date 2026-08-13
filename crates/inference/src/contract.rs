use core::str::FromStr;
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
use rewrite_types::Digest;

use crate::ContractError;

/// Current bounded-generation request contract version.
pub const GENERATION_REQUEST_SCHEMA_VERSION: u32 = 1;
/// Maximum candidates allowed by the inference boundary.
pub const MAX_INFERENCE_CANDIDATES: u8 = 16;
const MAX_DISCOVERED_OUTPUT_CONTRACTS: usize = 64;
/// Maximum serialized JSON Schema bytes allowed in one request.
pub const MAX_OUTPUT_SCHEMA_BYTES: usize = 64 * 1024;

/// Stable identifier for one backend implementation.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct BackendId(String);

impl BackendId {
    /// Creates a validated backend identifier.
    ///
    /// # Errors
    ///
    /// Returns [`BackendIdError`] unless the value is a lowercase machine label.
    pub fn new(value: impl Into<String>) -> Result<Self, BackendIdError> {
        value.into().parse()
    }

    /// Returns the machine label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for BackendId {
    type Err = BackendIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !value.is_empty()
            && value.len() <= 64
            && value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
            })
        {
            Ok(Self(value.to_owned()))
        } else {
            Err(BackendIdError)
        }
    }
}

impl<'de> Deserialize<'de> for BackendId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Invalid backend machine label.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("invalid backend identifier")]
pub struct BackendIdError;

/// Transport capabilities admitted by this exact adapter implementation.
///
/// These backend-wide declarations are necessary transport checks, not
/// per-artifact qualification or product support claims.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferenceCapabilities {
    /// Artifact roles this adapter is prepared to transport.
    pub roles: Vec<ArtifactRole>,
    /// Exact structured-output contracts admitted by this backend instance.
    ///
    /// An empty list means no structured-output contract is available. Callers
    /// must require an exact digest match and must not infer support from the
    /// runtime's generic JSON mode.
    pub admitted_output_contract_digests: Vec<Digest>,
    /// Whether a deterministic seed is accepted.
    pub seed: bool,
    /// Whether reasoning output can be disabled.
    pub disable_reasoning: bool,
}

impl InferenceCapabilities {
    /// Returns whether the adapter admits the exact output contract.
    #[must_use]
    pub fn admits_output(&self, output: &OutputContract) -> bool {
        self.validate().is_ok()
            && self
                .admitted_output_contract_digests
                .iter()
                .any(|digest| digest == &output.schema_digest)
    }

    /// Validates canonical capability ordering and bounds.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when capability entries are duplicated,
    /// unordered, or unbounded.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.roles.len() > ArtifactRole::ALL.len()
            || self.admitted_output_contract_digests.len() > MAX_DISCOVERED_OUTPUT_CONTRACTS
            || !self.roles.windows(2).all(|pair| pair[0] < pair[1])
            || !self
                .admitted_output_contract_digests
                .windows(2)
                .all(|pair| pair[0].as_str() < pair[1].as_str())
        {
            return Err(ContractError::InvalidCapabilities);
        }
        Ok(())
    }
}

/// One installed artifact reported by runtime discovery.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryEntry {
    /// Runtime-local mutable reference, shown only for inspection.
    pub reference: String,
    /// Resolved immutable artifact identity.
    pub artifact_id: ArtifactId,
    /// Digest resolved immediately from the runtime.
    pub artifact_digest: Digest,
    /// Runtime-reported byte size when available.
    pub byte_size: Option<u64>,
}

/// Exact discovery result required before generation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackendDiscovery {
    /// Backend implementation identifier.
    pub backend_id: BackendId,
    /// Exact runtime identity.
    pub runtime: RuntimeIdentity,
    /// Confirmed runtime capabilities.
    pub capabilities: InferenceCapabilities,
    /// Installed artifacts resolved during discovery.
    pub inventory: Vec<InventoryEntry>,
}

/// Structured-output requirement passed to a backend.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputContract {
    /// Digest of the exact schema bytes.
    pub schema_digest: Digest,
    /// Bounded UTF-8 JSON Schema.
    pub schema_json: String,
}

impl OutputContract {
    /// Validates one complete JSON Schema object and its exact byte digest.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when the schema is empty, oversized, malformed,
    /// not a JSON object, contains trailing data, or has a mismatched digest.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_json.is_empty()
            || self.schema_json.len() > MAX_OUTPUT_SCHEMA_BYTES
            || Digest::sha256(self.schema_json.as_bytes()) != self.schema_digest
        {
            return Err(ContractError::InvalidOutputContract);
        }
        let mut deserializer = serde_json::Deserializer::from_str(&self.schema_json);
        let value = serde_json::Value::deserialize(&mut deserializer)
            .map_err(|_error| ContractError::InvalidOutputContract)?;
        if !value.is_object() || deserializer.end().is_err() {
            return Err(ContractError::InvalidOutputContract);
        }
        Ok(())
    }
}

/// Explicit policy for backend reasoning output.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningPolicy {
    /// Backend must disable reasoning output.
    Disabled,
    /// Backend may reason internally, but no reasoning text is returned or retained.
    Discard,
}

/// Explicit sampling values for a qualified generation request.
#[derive(Clone, Copy, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingParameters {
    /// Temperature in the inclusive range zero to two.
    pub temperature: f32,
    /// Nucleus-sampling probability in the inclusive range zero to one.
    pub top_p: f32,
    /// Deterministic seed when supported by the runtime.
    pub seed: Option<u64>,
}

/// Fully explicit, bounded generation request.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationRequest {
    /// Request contract version.
    pub schema_version: u32,
    /// Exact qualified artifact.
    pub artifact_id: ArtifactId,
    /// Digest rechecked around generation.
    pub artifact_digest: Digest,
    /// Complete bounded prompt or backend input.
    pub input: String,
    /// Structured-output requirement.
    pub output: OutputContract,
    /// Requested number of independent candidates.
    pub candidate_count: u8,
    /// Observed source bytes represented inside the complete backend input.
    pub source_byte_count: u64,
    /// Qualified source-byte envelope.
    pub source_byte_limit: u64,
    /// Maximum complete prompt or backend-input text bytes accepted by policy.
    pub input_byte_limit: u64,
    /// Qualified context envelope.
    pub context_token_limit: u32,
    /// Maximum generated tokens requested from the backend.
    pub output_token_limit: u32,
    /// Maximum bytes accepted for each candidate.
    pub candidate_byte_limit: u64,
    /// Explicit sampling policy.
    pub sampling: SamplingParameters,
    /// Explicit reasoning-output policy.
    pub reasoning: ReasoningPolicy,
}

impl GenerationRequest {
    /// Validates the complete request before backend work begins.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError`] when identity, bounds, sampling, or schema
    /// invariants are invalid.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != GENERATION_REQUEST_SCHEMA_VERSION {
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
            || self.candidate_byte_limit == 0
            || u64::try_from(self.input.len()).unwrap_or(u64::MAX) > self.input_byte_limit
        {
            return Err(ContractError::InvalidLimits);
        }
        if self.candidate_count == 0 || self.candidate_count > MAX_INFERENCE_CANDIDATES {
            return Err(ContractError::InvalidCandidateCount);
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
}

/// One candidate payload returned by a backend.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationCandidate {
    /// Zero-based request-local candidate ordinal.
    pub ordinal: u8,
    /// Untrusted candidate text to pass through validation.
    pub text: String,
}

/// Optional resource observations reported by a backend.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageObservation {
    /// Runtime-reported input token count.
    pub input_tokens: Option<u64>,
    /// Runtime-reported output token count.
    pub output_tokens: Option<u64>,
    /// Runtime-reported generation duration in microseconds.
    pub generation_micros: Option<u64>,
}

/// Complete bounded response from an inference backend.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationResponse {
    /// Runtime identity observed for this response.
    pub runtime: RuntimeIdentity,
    /// Artifact identity used for generation.
    pub artifact_id: ArtifactId,
    /// Artifact digest rechecked after generation.
    pub artifact_digest: Digest,
    /// Untrusted bounded candidate payloads.
    pub candidates: Vec<GenerationCandidate>,
    /// Optional resource observations.
    pub usage: UsageObservation,
}

#[cfg(test)]
mod tests {
    use rewrite_model::{ArtifactId, ArtifactRole, RuntimeIdentity};
    use rewrite_types::Digest;

    use super::{
        BackendId, GENERATION_REQUEST_SCHEMA_VERSION, GenerationRequest, InferenceCapabilities,
        OutputContract, ReasoningPolicy, SamplingParameters, UsageObservation,
    };
    use crate::{
        ContractError, STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION, StructuredCompletionFinish,
        StructuredCompletionRequest, StructuredCompletionResponse,
    };

    fn request() -> GenerationRequest {
        let digest = Digest::sha256(b"artifact");
        let schema_json = "{\"type\":\"object\"}".to_owned();
        GenerationRequest {
            schema_version: GENERATION_REQUEST_SCHEMA_VERSION,
            artifact_id: ArtifactId::from_digest(digest.clone()),
            artifact_digest: digest,
            input: "bounded input".to_owned(),
            output: OutputContract {
                schema_digest: Digest::sha256(schema_json.as_bytes()),
                schema_json,
            },
            candidate_count: 1,
            source_byte_count: 13,
            source_byte_limit: 1_024,
            input_byte_limit: 2_048,
            context_token_limit: 2_048,
            output_token_limit: 256,
            candidate_byte_limit: 1_024,
            sampling: SamplingParameters {
                temperature: 0.2,
                top_p: 0.9,
                seed: Some(7),
            },
            reasoning: ReasoningPolicy::Disabled,
        }
    }

    #[test]
    fn backend_id_rejects_untrusted_labels() {
        assert!(BackendId::new("ollama_native").is_ok());
        assert!(BackendId::new("Ollama Native").is_err());
        assert!(serde_json::from_str::<BackendId>("\"../backend\"").is_err());
    }

    #[test]
    fn request_rejects_mismatched_identity_and_schema() {
        let mut value = request();
        value.validate().expect("fixture request is valid");
        value.artifact_digest = Digest::sha256(b"other");
        assert_eq!(value.validate(), Err(ContractError::ArtifactMismatch));

        let mut value = request();
        value.output.schema_json.push(' ');
        assert_eq!(value.validate(), Err(ContractError::InvalidOutputContract));

        for schema_json in ["null", "[]", "{} {}", "not json"] {
            let mut value = request();
            value.output = OutputContract {
                schema_digest: Digest::sha256(schema_json.as_bytes()),
                schema_json: schema_json.to_owned(),
            };
            assert_eq!(value.validate(), Err(ContractError::InvalidOutputContract));
        }
    }

    #[test]
    fn capabilities_require_exact_canonical_output_contracts() {
        let first = Digest::sha256(b"first");
        let second = Digest::sha256(b"second");
        let output = OutputContract {
            schema_digest: first.clone(),
            schema_json: "{}".to_owned(),
        };
        let mut capabilities = InferenceCapabilities {
            roles: vec![ArtifactRole::Generation],
            admitted_output_contract_digests: vec![first.clone(), second.clone()],
            seed: true,
            disable_reasoning: true,
        };
        capabilities
            .admitted_output_contract_digests
            .sort_unstable_by(|left, right| left.as_str().cmp(right.as_str()));
        capabilities.validate().expect("canonical capabilities");
        assert!(capabilities.admits_output(&output));

        capabilities.admitted_output_contract_digests = vec![first.clone(), first.clone()];
        assert!(!capabilities.admits_output(&output));
        assert_eq!(
            capabilities.validate(),
            Err(ContractError::InvalidCapabilities)
        );

        capabilities.admitted_output_contract_digests = vec![first, second];
        capabilities
            .admitted_output_contract_digests
            .sort_unstable_by(|left, right| right.as_str().cmp(left.as_str()));
        assert_eq!(
            capabilities.validate(),
            Err(ContractError::InvalidCapabilities)
        );
    }

    #[test]
    fn structured_response_binds_identity_bounds_and_complete_json() {
        let generation = request();
        let mut request = StructuredCompletionRequest {
            schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
            artifact_id: generation.artifact_id,
            artifact_digest: generation.artifact_digest,
            input: "private source input".to_owned(),
            output: generation.output,
            source_byte_count: generation.source_byte_count,
            source_byte_limit: generation.source_byte_limit,
            input_byte_limit: generation.input_byte_limit,
            context_token_limit: generation.context_token_limit,
            output_token_limit: generation.output_token_limit,
            output_byte_limit: 32,
            sampling: generation.sampling,
            reasoning: generation.reasoning,
        };
        let runtime = RuntimeIdentity {
            backend: "fake".to_owned(),
            version: "1".to_owned(),
            digest: None,
        };
        let response = StructuredCompletionResponse::complete(
            &request,
            runtime.clone(),
            request.artifact_id.clone(),
            request.artifact_digest.clone(),
            "{\"private\":true}".to_owned(),
            UsageObservation {
                input_tokens: Some(1),
                output_tokens: Some(2),
                generation_micros: Some(3),
            },
        )
        .expect("complete structured response");
        assert_eq!(response.finish(), StructuredCompletionFinish::Complete);
        assert_eq!(response.request_binding_digest(), &request.binding_digest());
        assert!(!format!("{request:?} {response:?}").contains("private"));

        let prior_binding = response.request_binding_digest().clone();
        request.sampling.seed = Some(99);
        assert_ne!(prior_binding, request.binding_digest());

        request.output_byte_limit = 1;
        assert_eq!(
            StructuredCompletionResponse::complete(
                &request,
                runtime,
                request.artifact_id.clone(),
                request.artifact_digest.clone(),
                "{}".to_owned(),
                response.usage(),
            ),
            Err(ContractError::InvalidStructuredResponse)
        );
    }

    #[test]
    fn structured_response_rejects_trailing_or_mismatched_payloads() {
        let generation = request();
        let request = StructuredCompletionRequest {
            schema_version: STRUCTURED_COMPLETION_REQUEST_SCHEMA_VERSION,
            artifact_id: generation.artifact_id,
            artifact_digest: generation.artifact_digest,
            input: generation.input,
            output: generation.output,
            source_byte_count: generation.source_byte_count,
            source_byte_limit: generation.source_byte_limit,
            input_byte_limit: generation.input_byte_limit,
            context_token_limit: generation.context_token_limit,
            output_token_limit: generation.output_token_limit,
            output_byte_limit: 32,
            sampling: generation.sampling,
            reasoning: generation.reasoning,
        };
        let runtime = RuntimeIdentity {
            backend: "fake".to_owned(),
            version: "1".to_owned(),
            digest: None,
        };
        for (artifact_id, output) in [
            (request.artifact_id.clone(), "{} {}"),
            (ArtifactId::from_digest(Digest::sha256(b"other")), "{}"),
        ] {
            assert_eq!(
                StructuredCompletionResponse::complete(
                    &request,
                    runtime.clone(),
                    artifact_id,
                    request.artifact_digest.clone(),
                    output.to_owned(),
                    UsageObservation {
                        input_tokens: None,
                        output_tokens: None,
                        generation_micros: None,
                    },
                ),
                Err(ContractError::InvalidStructuredResponse)
            );
        }
    }
}
