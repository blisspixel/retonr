use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Digest, MAX_EXTRACTOR_ID_BYTES};

/// Current extractor-manifest contract version.
pub const EXTRACTOR_MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded JSON bytes admitted by the extractor-manifest decoder.
pub const MAX_EXTRACTOR_MANIFEST_JSON_BYTES: usize = 16_384;

/// Content-addressed extractor implementation identity.
///
/// The record binds one extractor to exact prompt and contract digests. It has
/// no authorization method and does not qualify a runtime or artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractorManifest {
    schema_version: u32,
    extractor_id: String,
    extractor_version: String,
    subject_digest: Digest,
    prompt_digest: Digest,
    claim_output_contract_digest: Digest,
    claim_operation_contract_digest: Digest,
    confidence_policy_digest: Digest,
    language_policy_digest: Digest,
}

impl ExtractorManifest {
    /// Creates and validates a version 1 extractor manifest.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractorManifestError`] when identity fields are empty,
    /// oversized, or noncanonical.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor binds one atomic extractor identity"
    )]
    pub fn new(
        extractor_id: impl Into<String>,
        extractor_version: impl Into<String>,
        subject_digest: Digest,
        prompt_digest: Digest,
        claim_output_contract_digest: Digest,
        claim_operation_contract_digest: Digest,
        confidence_policy_digest: Digest,
        language_policy_digest: Digest,
    ) -> Result<Self, ExtractorManifestError> {
        Self::from_wire(
            EXTRACTOR_MANIFEST_SCHEMA_VERSION,
            extractor_id.into(),
            extractor_version.into(),
            subject_digest,
            prompt_digest,
            claim_output_contract_digest,
            claim_operation_contract_digest,
            confidence_policy_digest,
            language_policy_digest,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the decoder reconstructs one atomic extractor identity"
    )]
    fn from_wire(
        schema_version: u32,
        extractor_id: String,
        extractor_version: String,
        subject_digest: Digest,
        prompt_digest: Digest,
        claim_output_contract_digest: Digest,
        claim_operation_contract_digest: Digest,
        confidence_policy_digest: Digest,
        language_policy_digest: Digest,
    ) -> Result<Self, ExtractorManifestError> {
        if schema_version != EXTRACTOR_MANIFEST_SCHEMA_VERSION {
            return Err(ExtractorManifestError::UnsupportedSchema(schema_version));
        }
        if !valid_extractor_component(&extractor_id)
            || !valid_extractor_component(&extractor_version)
        {
            return Err(ExtractorManifestError::InvalidIdentity);
        }
        Ok(Self {
            schema_version,
            extractor_id,
            extractor_version,
            subject_digest,
            prompt_digest,
            claim_output_contract_digest,
            claim_operation_contract_digest,
            confidence_policy_digest,
            language_policy_digest,
        })
    }

    /// Parses a byte-bounded JSON manifest and revalidates every field.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractorManifestError`] when the input is oversized, malformed,
    /// or fails identity validation.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, ExtractorManifestError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            extractor_id: String,
            extractor_version: String,
            subject_digest: Digest,
            prompt_digest: Digest,
            claim_output_contract_digest: Digest,
            claim_operation_contract_digest: Digest,
            confidence_policy_digest: Digest,
            language_policy_digest: Digest,
        }

        if bytes.len() > MAX_EXTRACTOR_MANIFEST_JSON_BYTES {
            return Err(ExtractorManifestError::EncodedManifestTooLarge);
        }
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|_| ExtractorManifestError::InvalidEncoding)?;
        Self::from_wire(
            wire.schema_version,
            wire.extractor_id,
            wire.extractor_version,
            wire.subject_digest,
            wire.prompt_digest,
            wire.claim_output_contract_digest,
            wire.claim_operation_contract_digest,
            wire.confidence_policy_digest,
            wire.language_policy_digest,
        )
    }

    /// Returns the content-derived identity of this manifest.
    ///
    /// # Panics
    ///
    /// Panics if a validated manifest cannot be serialized. That path is
    /// unreachable for records created through the public constructor.
    #[must_use]
    pub fn manifest_digest(&self) -> Digest {
        let encoded = serde_json::to_string(self).expect("extractor manifest serialization");
        Digest::sha256(encoded.as_bytes())
    }

    /// Returns the extractor identifier.
    #[must_use]
    pub fn extractor_id(&self) -> &str {
        &self.extractor_id
    }

    /// Returns the extractor version.
    #[must_use]
    pub fn extractor_version(&self) -> &str {
        &self.extractor_version
    }

    /// Returns the bound claim-output contract digest.
    #[must_use]
    pub const fn claim_output_contract_digest(&self) -> &Digest {
        &self.claim_output_contract_digest
    }

    /// Returns the bound subject-policy digest.
    #[must_use]
    pub const fn subject_digest(&self) -> &Digest {
        &self.subject_digest
    }

    /// Returns the bound prompt-template digest.
    #[must_use]
    pub const fn prompt_digest(&self) -> &Digest {
        &self.prompt_digest
    }

    /// Returns the bound claim-operation contract digest.
    #[must_use]
    pub const fn claim_operation_contract_digest(&self) -> &Digest {
        &self.claim_operation_contract_digest
    }

    /// Returns the bound confidence-policy digest.
    #[must_use]
    pub const fn confidence_policy_digest(&self) -> &Digest {
        &self.confidence_policy_digest
    }

    /// Returns the bound language-policy digest.
    #[must_use]
    pub const fn language_policy_digest(&self) -> &Digest {
        &self.language_policy_digest
    }
}

fn valid_extractor_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_EXTRACTOR_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+' | b':')
        })
}

/// Extractor-manifest validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ExtractorManifestError {
    /// Encoded input exceeds the fixed pre-decode byte ceiling.
    #[error("encoded extractor manifest exceeds its limit")]
    EncodedManifestTooLarge,
    /// Encoded JSON is malformed or contains an unknown field.
    #[error("extractor manifest encoding is invalid")]
    InvalidEncoding,
    /// The extractor-manifest schema version is unsupported.
    #[error("unsupported extractor manifest schema {0}")]
    UnsupportedSchema(u32),
    /// Extractor identifier or version is empty, oversized, or noncanonical.
    #[error("extractor identity is invalid")]
    InvalidIdentity,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> ExtractorManifest {
        ExtractorManifest::new(
            "literal-claims",
            "1.0.0",
            Digest::sha256(b"subject"),
            Digest::sha256(b"prompt"),
            Digest::sha256(b"claim-output"),
            Digest::sha256(b"claim-operation"),
            Digest::sha256(b"confidence"),
            Digest::sha256(b"language"),
        )
        .expect("valid extractor")
    }

    #[test]
    fn round_trips_and_rejects_unknown_fields() {
        let manifest = fixture();
        let encoded = serde_json::to_vec(&manifest).expect("serialize");
        assert_eq!(
            ExtractorManifest::from_json_bytes(&encoded).expect("parse"),
            manifest
        );
        assert_eq!(manifest.extractor_id(), "literal-claims");
        assert_eq!(manifest.subject_digest(), &Digest::sha256(b"subject"));
        assert_eq!(manifest.prompt_digest(), &Digest::sha256(b"prompt"));
        assert_eq!(
            manifest.claim_operation_contract_digest(),
            &Digest::sha256(b"claim-operation")
        );
        let digest = manifest.manifest_digest();
        assert_eq!(digest.as_str().len(), 64);

        let mut unknown: serde_json::Value = serde_json::from_slice(&encoded).expect("json");
        unknown["authorizes"] = serde_json::json!(true);
        assert_eq!(
            ExtractorManifest::from_json_bytes(
                &serde_json::to_vec(&unknown).expect("serialize unknown")
            ),
            Err(ExtractorManifestError::InvalidEncoding)
        );
    }

    #[test]
    fn rejects_empty_identity_and_has_no_authorize_surface() {
        assert_eq!(
            ExtractorManifest::new(
                "",
                "1",
                Digest::sha256(b"s"),
                Digest::sha256(b"p"),
                Digest::sha256(b"o"),
                Digest::sha256(b"op"),
                Digest::sha256(b"c"),
                Digest::sha256(b"l"),
            )
            .err(),
            Some(ExtractorManifestError::InvalidIdentity)
        );
        let encoded = serde_json::to_string(&fixture()).expect("serialize");
        assert!(!encoded.contains("authorizes"));
    }
}
