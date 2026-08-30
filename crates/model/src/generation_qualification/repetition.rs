use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{append_digest, append_u32, validate_canonical_json};
use super::{
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GENERATION_REPETITION_ID_DOMAIN,
    GenerationQualificationContractError, GenerationRepetitionId, GenerationSuiteManifestId,
    GenerationSuiteManifestV1,
};

/// Maximum JSON bytes accepted for one generation-repetition record.
pub const MAX_GENERATION_REPETITION_JSON_BYTES: usize = 16_384;
const MAX_GENERATION_REPETITION_CANONICAL_BYTES: usize = 512;

/// Inert portable declaration of one suite repetition.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationRepetitionRecordV1 {
    schema_version: u32,
    suite_manifest_id: GenerationSuiteManifestId,
    repetition_ordinal: u32,
    repetition_policy_digest: Digest,
    #[serde(skip)]
    id: GenerationRepetitionId,
}

impl GenerationRepetitionRecordV1 {
    /// Creates one repetition bound to the exact supplied suite.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] if canonical identity
    /// construction exceeds its fixed ceiling.
    pub fn new(
        suite: &GenerationSuiteManifestV1,
        repetition_ordinal: u32,
        repetition_policy_digest: Digest,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::from_wire(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            suite,
            suite.suite_manifest_id().clone(),
            repetition_ordinal,
            repetition_policy_digest,
        )
    }

    fn from_wire(
        schema_version: u32,
        suite: &GenerationSuiteManifestV1,
        suite_manifest_id: GenerationSuiteManifestId,
        repetition_ordinal: u32,
        repetition_policy_digest: Digest,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        if &suite_manifest_id != suite.suite_manifest_id() {
            return Err(GenerationQualificationContractError::SuiteMismatch);
        }
        let canonical = canonical_bytes(
            schema_version,
            &suite_manifest_id,
            repetition_ordinal,
            &repetition_policy_digest,
        );
        if canonical.len() > MAX_GENERATION_REPETITION_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            schema_version,
            suite_manifest_id,
            repetition_ordinal,
            repetition_policy_digest,
            id: GenerationRepetitionId(Digest::sha256(&canonical)),
        })
    }

    /// Parses one canonical repetition and rechecks its suite relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for excessive, malformed,
    /// noncanonical, unsupported, or cross-suite input.
    pub fn from_json_bytes(
        bytes: &[u8],
        suite: &GenerationSuiteManifestV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            suite_manifest_id: GenerationSuiteManifestId,
            repetition_ordinal: u32,
            repetition_policy_digest: Digest,
        }

        if bytes.len() > MAX_GENERATION_REPETITION_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let record = Self::from_wire(
            wire.schema_version,
            suite,
            wire.suite_manifest_id,
            wire.repetition_ordinal,
            wire.repetition_policy_digest,
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact suite-manifest identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }

    /// Returns the zero-based repetition ordinal.
    #[must_use]
    pub const fn repetition_ordinal(&self) -> u32 {
        self.repetition_ordinal
    }

    /// Returns the repetition-policy digest.
    #[must_use]
    pub const fn repetition_policy_digest(&self) -> &Digest {
        &self.repetition_policy_digest
    }

    /// Returns the content-derived repetition identity.
    #[must_use]
    pub const fn repetition_id(&self) -> &GenerationRepetitionId {
        &self.id
    }
}

impl fmt::Debug for GenerationRepetitionRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationRepetitionRecordV1")
            .field("schema_version", &self.schema_version)
            .field("repetition_id", &self.id)
            .field("repetition_ordinal", &self.repetition_ordinal)
            .finish_non_exhaustive()
    }
}

fn canonical_bytes(
    schema_version: u32,
    suite_manifest_id: &GenerationSuiteManifestId,
    repetition_ordinal: u32,
    policy_digest: &Digest,
) -> Vec<u8> {
    let mut output = GENERATION_REPETITION_ID_DOMAIN.to_vec();
    append_u32(&mut output, schema_version);
    append_digest(&mut output, suite_manifest_id.digest());
    append_u32(&mut output, repetition_ordinal);
    append_digest(&mut output, policy_digest);
    output
}
