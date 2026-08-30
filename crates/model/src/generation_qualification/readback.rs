use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{append_digest, append_u32, append_u64, validate_canonical_json};
use super::{
    CANDIDATE_GENERATION_EVIDENCE_READBACK_ID_DOMAIN, CandidateGenerationEvidenceBundleId,
    CandidateGenerationEvidenceBundleManifestV1, CandidateGenerationEvidenceBundleReadbackId,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationContractError,
};

/// Maximum JSON bytes accepted for one evidence-bundle readback record.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_READBACK_JSON_BYTES: usize = 16_384;
const MAX_READBACK_CANONICAL_BYTES: usize = 1_024;

/// Closed publication mode for a persisted candidate evidence bundle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationEvidenceBundlePublicationModeV1 {
    /// The publisher atomically created a previously absent destination.
    CreateNewNoReplace,
}

/// Closed persistable readback status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateGenerationEvidenceBundleReadbackStatusV1 {
    /// A fresh bounded lease rehashed and matched the exact bundle.
    Verified,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadbackWire {
    schema_version: u32,
    bundle_id: CandidateGenerationEvidenceBundleId,
    entry_count: u32,
    aggregate_byte_count: u64,
    publication_mode: CandidateGenerationEvidenceBundlePublicationModeV1,
    status: CandidateGenerationEvidenceBundleReadbackStatusV1,
}

/// Inert portable result of an upper-layer bounded evidence-bundle readback.
///
/// Construction does not perform filesystem work and grants no bundle lease or
/// candidate authority. The app boundary must retain the exact verified lease.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationEvidenceBundleReadbackV1 {
    schema_version: u32,
    bundle_id: CandidateGenerationEvidenceBundleId,
    entry_count: u32,
    aggregate_byte_count: u64,
    publication_mode: CandidateGenerationEvidenceBundlePublicationModeV1,
    status: CandidateGenerationEvidenceBundleReadbackStatusV1,
    #[serde(skip)]
    id: CandidateGenerationEvidenceBundleReadbackId,
}

impl CandidateGenerationEvidenceBundleReadbackV1 {
    /// Derives the inert fixed-status record for an exact bundle manifest.
    ///
    /// Upper layers may call this only after successful no-replace publication and
    /// fresh bounded readback. This model-owned constructor does not prove either.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] only if the fixed canonical
    /// identity exceeds its contract ceiling.
    pub fn new(
        bundle: &CandidateGenerationEvidenceBundleManifestV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::build(GENERATION_QUALIFICATION_SCHEMA_VERSION, bundle, None)
    }

    fn build(
        schema_version: u32,
        bundle: &CandidateGenerationEvidenceBundleManifestV1,
        wire: Option<&ReadbackWire>,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        if wire.is_some_and(|wire| {
            wire.bundle_id != *bundle.evidence_bundle_id()
                || wire.entry_count != bundle.entry_count()
                || wire.aggregate_byte_count != bundle.aggregate_byte_count()
                || wire.publication_mode
                    != CandidateGenerationEvidenceBundlePublicationModeV1::CreateNewNoReplace
                || wire.status != CandidateGenerationEvidenceBundleReadbackStatusV1::Verified
        }) {
            return Err(GenerationQualificationContractError::ReadbackRelationshipMismatch);
        }
        let mut value = Self {
            schema_version,
            bundle_id: bundle.evidence_bundle_id().clone(),
            entry_count: bundle.entry_count(),
            aggregate_byte_count: bundle.aggregate_byte_count(),
            publication_mode:
                CandidateGenerationEvidenceBundlePublicationModeV1::CreateNewNoReplace,
            status: CandidateGenerationEvidenceBundleReadbackStatusV1::Verified,
            id: CandidateGenerationEvidenceBundleReadbackId(Digest::sha256(
                b"uninitialized readback",
            )),
        };
        let canonical = value.canonical_bytes();
        if canonical.len() > MAX_READBACK_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        value.id = CandidateGenerationEvidenceBundleReadbackId(Digest::sha256(&canonical));
        Ok(value)
    }

    /// Parses canonical bounded JSON against the exact recomputed bundle.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// future-schema, noncanonical, stale, or substituted records.
    pub fn from_json_bytes(
        bytes: &[u8],
        bundle: &CandidateGenerationEvidenceBundleManifestV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        if bytes.len() > MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_READBACK_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: ReadbackWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        let value = Self::build(wire.schema_version, bundle, Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact bundle identity.
    #[must_use]
    pub const fn bundle_id(&self) -> &CandidateGenerationEvidenceBundleId {
        &self.bundle_id
    }
    /// Returns the verified entry count.
    #[must_use]
    pub const fn entry_count(&self) -> u32 {
        self.entry_count
    }
    /// Returns the verified aggregate bytes.
    #[must_use]
    pub const fn aggregate_byte_count(&self) -> u64 {
        self.aggregate_byte_count
    }
    /// Returns the fixed publication mode.
    #[must_use]
    pub const fn publication_mode(&self) -> CandidateGenerationEvidenceBundlePublicationModeV1 {
        self.publication_mode
    }
    /// Returns the fixed readback status.
    #[must_use]
    pub const fn status(&self) -> CandidateGenerationEvidenceBundleReadbackStatusV1 {
        self.status
    }
    /// Returns the content-derived readback identity.
    #[must_use]
    pub const fn readback_id(&self) -> &CandidateGenerationEvidenceBundleReadbackId {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = CANDIDATE_GENERATION_EVIDENCE_READBACK_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.bundle_id.digest());
        append_u32(&mut output, self.entry_count);
        append_u64(&mut output, self.aggregate_byte_count);
        output.push(0);
        output.push(0);
        output
    }
}

impl fmt::Debug for CandidateGenerationEvidenceBundleReadbackV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationEvidenceBundleReadbackV1")
            .field("readback_id", &self.id)
            .field("bundle_id", &self.bundle_id)
            .field("entry_count", &self.entry_count)
            .field("aggregate_byte_count", &self.aggregate_byte_count)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}
