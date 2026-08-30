//! Canonical typed request profile for one generation qualification case.

use std::fmt;

use rewrite_types::{Digest, RewriteMode};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    GenerationCaseManifestV1, GenerationDeterministicCaseContractV1, GenerationSystemRecordV1,
};

/// Current canonical generation-case request-profile schema.
pub const GENERATION_CASE_REQUEST_PROFILE_SCHEMA_VERSION: u32 = 1;
/// Maximum canonical JSON bytes accepted for one generation-case request profile.
pub const MAX_GENERATION_CASE_REQUEST_PROFILE_JSON_BYTES: usize = 1_024;
/// Domain for the canonical generation-case language digest.
pub const GENERATION_CASE_LANGUAGE_DIGEST_DOMAIN: &[u8] = b"retonr:generation-case-language:v1\0";
/// Domain for the canonical generation-case rewrite-mode digest.
pub const GENERATION_CASE_REWRITE_MODE_DIGEST_DOMAIN: &[u8] =
    b"retonr:generation-case-rewrite-mode:v1\0";
/// Domain for the canonical generation-case format digest.
pub const GENERATION_CASE_FORMAT_DIGEST_DOMAIN: &[u8] = b"retonr:generation-case-format:v1\0";

/// Closed language admitted by the first generation-case request profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationCaseLanguageV1 {
    /// English source and output.
    English,
}

/// Closed source format admitted by the first generation-case request profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationCaseFormatV1 {
    /// Plain UTF-8 text interpreted as one whole-document rewrite unit.
    PlainUtf8Text,
}

/// Closed rewrite-unit selection policy for the first request profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationCaseUnitPolicyV1 {
    /// Require exactly one whole-document unit from the text adapter.
    ExactlyOneWholeDocument,
}

/// Closed style-context policy for the first request profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationCaseStylePolicyV1 {
    /// Supply an empty style context and serialize its status as unavailable.
    Unavailable,
}

/// Closed edit atomicity for the first request profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationCaseAtomicityV1 {
    /// Accept or reject the complete document as one transaction.
    Document,
}

/// Closed protected-value planning policy for the first request profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationCaseProtectionPolicyV1 {
    /// Run deterministic protection with the exact case-contract protected terms.
    ExactContractTerms,
}

/// Canonical typed preimage for case and generation-system request selectors.
///
/// This inert record gives meaning to the three legacy digest slots. Equality to
/// arbitrary digest text cannot construct or recover a supported profile.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationCaseRequestProfileV1 {
    schema_version: u32,
    language: GenerationCaseLanguageV1,
    rewrite_mode: RewriteMode,
    format: GenerationCaseFormatV1,
    unit_policy: GenerationCaseUnitPolicyV1,
    style_policy: GenerationCaseStylePolicyV1,
    atomicity: GenerationCaseAtomicityV1,
    protection_policy: GenerationCaseProtectionPolicyV1,
}

/// Failure to decode or bind one canonical generation-case request profile.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationCaseRequestProfileError {
    /// Encoded JSON exceeds the fixed predecode ceiling.
    #[error("generation-case request profile exceeds its JSON byte limit")]
    EncodedRecordTooLarge,
    /// JSON is malformed, incomplete, duplicated, trailing, or otherwise invalid.
    #[error("generation-case request profile encoding is invalid")]
    InvalidEncoding,
    /// JSON differs from the one canonical encoding.
    #[error("generation-case request profile encoding is noncanonical")]
    NonCanonicalEncoding,
    /// The portable schema version is unsupported.
    #[error("generation-case request profile schema is unsupported")]
    UnsupportedSchema,
    /// The deterministic contract does not bind the exact case manifest.
    #[error("generation-case request profile case contract does not match")]
    CaseContractMismatch,
    /// One or more records do not bind the canonical English language digest.
    #[error("generation-case request profile language does not match")]
    LanguageMismatch,
    /// One or more records do not bind the explicit canonical rewrite-mode digest.
    #[error("generation-case request profile rewrite mode does not match")]
    RewriteModeMismatch,
    /// One or more records do not bind the canonical plain UTF-8 format digest.
    #[error("generation-case request profile format does not match")]
    FormatMismatch,
}

impl GenerationCaseRequestProfileV1 {
    /// Creates the fixed V1 profile with one explicit rewrite mode.
    #[must_use]
    pub const fn new(rewrite_mode: RewriteMode) -> Self {
        Self {
            schema_version: GENERATION_CASE_REQUEST_PROFILE_SCHEMA_VERSION,
            language: GenerationCaseLanguageV1::English,
            rewrite_mode,
            format: GenerationCaseFormatV1::PlainUtf8Text,
            unit_policy: GenerationCaseUnitPolicyV1::ExactlyOneWholeDocument,
            style_policy: GenerationCaseStylePolicyV1::Unavailable,
            atomicity: GenerationCaseAtomicityV1::Document,
            protection_policy: GenerationCaseProtectionPolicyV1::ExactContractTerms,
        }
    }

    /// Decodes canonical bounded JSON and validates every exact record relationship.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, noncanonical,
    /// unsupported, cross-case, or arbitrary legacy digest input.
    pub fn from_json_bytes(
        bytes: &[u8],
        case: &GenerationCaseManifestV1,
        contract: &GenerationDeterministicCaseContractV1,
        generation_system: &GenerationSystemRecordV1,
    ) -> Result<Self, GenerationCaseRequestProfileError> {
        if bytes.len() > MAX_GENERATION_CASE_REQUEST_PROFILE_JSON_BYTES {
            return Err(GenerationCaseRequestProfileError::EncodedRecordTooLarge);
        }
        let wire: ProfileWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationCaseRequestProfileError::InvalidEncoding)?;
        if wire.schema_version != GENERATION_CASE_REQUEST_PROFILE_SCHEMA_VERSION {
            return Err(GenerationCaseRequestProfileError::UnsupportedSchema);
        }
        let value = Self::new(wire.rewrite_mode);
        if !wire.matches(&value) {
            return Err(GenerationCaseRequestProfileError::InvalidEncoding);
        }
        if value.to_canonical_json_bytes()? != bytes {
            return Err(GenerationCaseRequestProfileError::NonCanonicalEncoding);
        }
        value.validate_against(case, contract, generation_system)?;
        Ok(value)
    }

    /// Serializes the one canonical bounded JSON representation.
    ///
    /// # Errors
    ///
    /// Returns an error only if serialization fails or exceeds the fixed ceiling.
    pub fn to_canonical_json_bytes(&self) -> Result<Vec<u8>, GenerationCaseRequestProfileError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| GenerationCaseRequestProfileError::InvalidEncoding)?;
        if bytes.len() > MAX_GENERATION_CASE_REQUEST_PROFILE_JSON_BYTES {
            Err(GenerationCaseRequestProfileError::EncodedRecordTooLarge)
        } else {
            Ok(bytes)
        }
    }

    /// Validates the profile against one exact case, contract, and generation system.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless the contract binds the case and every
    /// record carries the canonical digest derived from this typed profile.
    pub fn validate_against(
        &self,
        case: &GenerationCaseManifestV1,
        contract: &GenerationDeterministicCaseContractV1,
        generation_system: &GenerationSystemRecordV1,
    ) -> Result<(), GenerationCaseRequestProfileError> {
        if self.schema_version != GENERATION_CASE_REQUEST_PROFILE_SCHEMA_VERSION {
            return Err(GenerationCaseRequestProfileError::UnsupportedSchema);
        }
        contract
            .validate_case_manifest(case)
            .map_err(|_| GenerationCaseRequestProfileError::CaseContractMismatch)?;

        let language = self.language_digest();
        if case.language_digest() != &language
            || contract.language_digest() != &language
            || generation_system.language_digest() != &language
        {
            return Err(GenerationCaseRequestProfileError::LanguageMismatch);
        }
        let rewrite_mode = self.rewrite_mode_digest();
        if case.mode_digest() != &rewrite_mode
            || contract.mode_digest() != &rewrite_mode
            || generation_system.mode_digest() != &rewrite_mode
        {
            return Err(GenerationCaseRequestProfileError::RewriteModeMismatch);
        }
        let format = self.format_digest();
        if case.format_digest() != &format
            || contract.format_digest() != &format
            || generation_system.format_digest() != &format
        {
            return Err(GenerationCaseRequestProfileError::FormatMismatch);
        }
        Ok(())
    }

    /// Returns the canonical profile schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the fixed English language selector.
    #[must_use]
    pub const fn language(&self) -> GenerationCaseLanguageV1 {
        self.language
    }

    /// Returns the explicitly selected rewrite mode.
    #[must_use]
    pub const fn rewrite_mode(&self) -> RewriteMode {
        self.rewrite_mode
    }

    /// Returns the fixed plain UTF-8 text format selector.
    #[must_use]
    pub const fn format(&self) -> GenerationCaseFormatV1 {
        self.format
    }

    /// Returns the fixed exactly-one whole-document unit policy.
    #[must_use]
    pub const fn unit_policy(&self) -> GenerationCaseUnitPolicyV1 {
        self.unit_policy
    }

    /// Returns the fixed unavailable style policy.
    #[must_use]
    pub const fn style_policy(&self) -> GenerationCaseStylePolicyV1 {
        self.style_policy
    }

    /// Returns the fixed document atomicity.
    #[must_use]
    pub const fn atomicity(&self) -> GenerationCaseAtomicityV1 {
        self.atomicity
    }

    /// Returns the fixed exact-contract-terms protection policy.
    #[must_use]
    pub const fn protection_policy(&self) -> GenerationCaseProtectionPolicyV1 {
        self.protection_policy
    }

    /// Derives the canonical English language digest.
    #[must_use]
    pub fn language_digest(&self) -> Digest {
        tagged_digest(GENERATION_CASE_LANGUAGE_DIGEST_DOMAIN, 0)
    }

    /// Derives the canonical digest for the explicit rewrite mode.
    #[must_use]
    pub fn rewrite_mode_digest(&self) -> Digest {
        let tag = match self.rewrite_mode {
            RewriteMode::Literal => 0,
            RewriteMode::Pure => 1,
            RewriteMode::Balanced => 2,
            RewriteMode::Strong => 3,
        };
        tagged_digest(GENERATION_CASE_REWRITE_MODE_DIGEST_DOMAIN, tag)
    }

    /// Derives the canonical plain UTF-8 format digest.
    #[must_use]
    pub fn format_digest(&self) -> Digest {
        tagged_digest(GENERATION_CASE_FORMAT_DIGEST_DOMAIN, 0)
    }
}

impl fmt::Debug for GenerationCaseRequestProfileV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationCaseRequestProfileV1")
            .field("schema_version", &self.schema_version)
            .field("language", &self.language)
            .field("rewrite_mode", &self.rewrite_mode)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileWire {
    schema_version: u32,
    language: GenerationCaseLanguageV1,
    rewrite_mode: RewriteMode,
    format: GenerationCaseFormatV1,
    unit_policy: GenerationCaseUnitPolicyV1,
    style_policy: GenerationCaseStylePolicyV1,
    atomicity: GenerationCaseAtomicityV1,
    protection_policy: GenerationCaseProtectionPolicyV1,
}

impl ProfileWire {
    fn matches(&self, value: &GenerationCaseRequestProfileV1) -> bool {
        self.schema_version == value.schema_version
            && self.language == value.language
            && self.rewrite_mode == value.rewrite_mode
            && self.format == value.format
            && self.unit_policy == value.unit_policy
            && self.style_policy == value.style_policy
            && self.atomicity == value.atomicity
            && self.protection_policy == value.protection_policy
    }
}

fn tagged_digest(domain: &[u8], tag: u8) -> Digest {
    let mut material = Vec::with_capacity(domain.len() + 5);
    material.extend_from_slice(domain);
    material.extend_from_slice(&GENERATION_CASE_REQUEST_PROFILE_SCHEMA_VERSION.to_be_bytes());
    material.push(tag);
    Digest::sha256(&material)
}

#[cfg(test)]
#[path = "case_request_profile/tests.rs"]
mod tests;
