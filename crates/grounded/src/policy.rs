use std::collections::BTreeSet;

use rewrite_inference::{OutputContract, ReasoningPolicy, SamplingParameters};
use rewrite_model::ArtifactId;
use rewrite_types::{Digest, RewriteMode, RewriteUnitId};
use serde::{Deserialize, Serialize};

use crate::GroundedError;

/// Current grounded-strategy policy schema version.
pub const GROUNDED_POLICY_SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_PROMPT_TEMPLATE_BYTES: usize = 16 * 1024;
pub(crate) const MAX_STYLE_CONTEXT_BYTES: usize = 16 * 1024;
const MAX_SENTINELS: usize = 128;
const MAX_SENTINEL_TOKEN_BYTES: usize = 96;
const MAX_GROUNDED_CANDIDATES: u8 = 16;
const MAX_OUTPUT_SCHEMA_BYTES: usize = 64 * 1024;

/// Exact, versioned inference policy for grounded candidate generation.
#[derive(Clone, Debug, PartialEq)]
pub struct GroundedPolicy {
    /// Policy schema version.
    pub schema_version: u32,
    /// Exact qualified generation artifact.
    pub artifact_id: ArtifactId,
    /// Digest rechecked around generation.
    pub artifact_digest: Digest,
    /// Versioned instruction template placed before structured untrusted input.
    pub prompt_template: String,
    /// Digest of the exact prompt template bytes.
    pub prompt_template_digest: Digest,
    /// Exact structured-output contract.
    pub output: OutputContract,
    /// Number of independent candidates requested.
    pub candidate_count: u8,
    /// Qualified maximum masked-source bytes.
    pub source_byte_limit: u64,
    /// Maximum complete serialized backend-input bytes.
    pub input_byte_limit: u64,
    /// Explicit backend context setting.
    pub context_token_limit: u32,
    /// Maximum generated tokens requested from the backend.
    pub output_token_limit: u32,
    /// Maximum accepted bytes per candidate.
    pub candidate_byte_limit: u64,
    /// Explicit sampling parameters.
    pub sampling: SamplingParameters,
    /// Explicit reasoning-output policy.
    pub reasoning: ReasoningPolicy,
}

impl GroundedPolicy {
    pub(crate) fn validate(&self) -> Result<(), GroundedError> {
        if self.schema_version != GROUNDED_POLICY_SCHEMA_VERSION {
            return Err(GroundedError::UnsupportedPolicySchema);
        }
        if self.artifact_id.digest() != &self.artifact_digest
            || !valid_multiline_text(&self.prompt_template, MAX_PROMPT_TEMPLATE_BYTES, false)
            || Digest::sha256(self.prompt_template.as_bytes()) != self.prompt_template_digest
            || self.candidate_count == 0
            || self.candidate_count > MAX_GROUNDED_CANDIDATES
            || self.source_byte_limit == 0
            || self.input_byte_limit == 0
            || self.context_token_limit == 0
            || self.output_token_limit == 0
            || self.candidate_byte_limit == 0
            || self.output.schema_json.is_empty()
            || self.output.schema_json.len() > MAX_OUTPUT_SCHEMA_BYTES
            || Digest::sha256(self.output.schema_json.as_bytes()) != self.output.schema_digest
            || !self.sampling.temperature.is_finite()
            || !(0.0..=2.0).contains(&self.sampling.temperature)
            || !self.sampling.top_p.is_finite()
            || !(0.0..=1.0).contains(&self.sampling.top_p)
        {
            return Err(GroundedError::InvalidPolicy);
        }
        Ok(())
    }
}

/// Typed sentinel disclosed to the model without its protected surface value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundedSentinel {
    /// Exact engine-issued token that must remain byte-identical.
    pub token: String,
    /// Category of protected value represented by the token.
    pub kind: GroundedSentinelKind,
}

/// Protected-value category visible to the grounded strategy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundedSentinelKind {
    /// Caller-declared exact term.
    DeclaredTerm,
    /// HTTP or HTTPS URL.
    Url,
    /// Email address.
    Email,
    /// Numeric literal.
    Number,
}

/// One bounded request for masked candidate generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroundedRequest {
    /// Unit for which candidates are requested.
    pub unit_id: RewriteUnitId,
    /// Source with protected surfaces replaced by engine-issued sentinels.
    pub masked_source: String,
    /// Sentinels the model may copy, without original protected surfaces.
    pub sentinels: Vec<GroundedSentinel>,
    /// Requested rewrite strength.
    pub mode: RewriteMode,
    /// Explicit style context, or an empty string when unavailable.
    pub style_context: String,
}

impl GroundedRequest {
    pub(crate) fn validate(&self, policy: &GroundedPolicy) -> Result<(), GroundedError> {
        if !valid_multiline_text(&self.masked_source, usize::MAX, false)
            || u64::try_from(self.masked_source.len()).unwrap_or(u64::MAX)
                > policy.source_byte_limit
            || !valid_multiline_text(&self.style_context, MAX_STYLE_CONTEXT_BYTES, true)
            || self.sentinels.len() > MAX_SENTINELS
        {
            return Err(GroundedError::InvalidRequest);
        }
        let mut tokens = BTreeSet::new();
        if self
            .sentinels
            .iter()
            .enumerate()
            .any(|(ordinal, sentinel)| {
                !valid_token(sentinel, ordinal) || !tokens.insert(sentinel.token.as_str())
            })
        {
            return Err(GroundedError::InvalidRequest);
        }
        Ok(())
    }
}

fn valid_token(sentinel: &GroundedSentinel, ordinal: usize) -> bool {
    let label = match sentinel.kind {
        GroundedSentinelKind::DeclaredTerm => "TERM",
        GroundedSentinelKind::Url => "URL",
        GroundedSentinelKind::Email => "EMAIL",
        GroundedSentinelKind::Number => "NUMBER",
    };
    let expected = format!("{{{{PROTECTED_{label}_{:04}}}}}", ordinal + 1);
    sentinel.token.len() <= MAX_SENTINEL_TOKEN_BYTES && sentinel.token == expected
}

fn valid_multiline_text(value: &str, maximum: usize, empty_allowed: bool) -> bool {
    (empty_allowed || !value.is_empty())
        && value.len() <= maximum
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
}
