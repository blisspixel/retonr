use std::collections::HashSet;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{append_count, append_digest, append_u32, validate_canonical_json};
use super::{
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GENERATION_SUITE_MANIFEST_ID_DOMAIN, GenerationCaseId,
    GenerationCaseManifestV1, GenerationQualificationContractError, GenerationSuiteManifestId,
};

/// Maximum bytes in a canonical generation machine key.
pub const MAX_GENERATION_MACHINE_KEY_BYTES: usize = 128;
/// Maximum cases in one generation-suite manifest.
pub const MAX_GENERATION_SUITE_CASES: usize = 256;
/// Maximum JSON bytes accepted for one generation-suite manifest.
pub const MAX_GENERATION_SUITE_JSON_BYTES: usize = 4 * 1_024 * 1_024;
const MAX_GENERATION_SUITE_CANONICAL_BYTES: usize = 32 * 1_024;

/// Inert portable declaration of one ordered generation qualification suite.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationSuiteManifestV1 {
    schema_version: u32,
    protocol_digest: Digest,
    case_ids: Vec<GenerationCaseId>,
    #[serde(skip)]
    id: GenerationSuiteManifestId,
}

impl GenerationSuiteManifestV1 {
    /// Creates a suite from cases in exact semantic protocol order.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for an empty, excessive,
    /// duplicate, or otherwise noncanonical case sequence.
    pub fn new(
        protocol_digest: Digest,
        cases: &[GenerationCaseManifestV1],
    ) -> Result<Self, GenerationQualificationContractError> {
        validate_cases(cases)?;
        Self::from_wire(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            protocol_digest,
            cases.iter().map(|case| case.case_id().clone()).collect(),
            cases,
        )
    }

    fn from_wire(
        schema_version: u32,
        protocol_digest: Digest,
        case_ids: Vec<GenerationCaseId>,
        cases: &[GenerationCaseManifestV1],
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        validate_cases(cases)?;
        if case_ids.len() != cases.len()
            || case_ids
                .iter()
                .zip(cases)
                .any(|(id, case)| id != case.case_id())
        {
            return Err(GenerationQualificationContractError::CaseMismatch);
        }
        let canonical = canonical_bytes(schema_version, &protocol_digest, &case_ids)?;
        if canonical.len() > MAX_GENERATION_SUITE_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            schema_version,
            protocol_digest,
            case_ids,
            id: GenerationSuiteManifestId(Digest::sha256(&canonical)),
        })
    }

    /// Parses one canonical suite and rechecks every supplied case and its order.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for excessive, malformed,
    /// noncanonical, unsupported, duplicate, missing, extra, or reordered input.
    pub fn from_json_bytes(
        bytes: &[u8],
        cases: &[GenerationCaseManifestV1],
    ) -> Result<Self, GenerationQualificationContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            protocol_digest: Digest,
            case_ids: Vec<GenerationCaseId>,
        }

        if bytes.len() > MAX_GENERATION_SUITE_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let record = Self::from_wire(
            wire.schema_version,
            wire.protocol_digest,
            wire.case_ids,
            cases,
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact protocol digest.
    #[must_use]
    pub const fn protocol_digest(&self) -> &Digest {
        &self.protocol_digest
    }

    /// Returns case identities in semantic protocol order.
    #[must_use]
    pub fn case_ids(&self) -> &[GenerationCaseId] {
        &self.case_ids
    }

    /// Returns the content-derived suite-manifest identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.id
    }
}

impl fmt::Debug for GenerationSuiteManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationSuiteManifestV1")
            .field("schema_version", &self.schema_version)
            .field("suite_manifest_id", &self.id)
            .field("case_count", &self.case_ids.len())
            .finish_non_exhaustive()
    }
}

fn validate_cases(
    cases: &[GenerationCaseManifestV1],
) -> Result<(), GenerationQualificationContractError> {
    if cases.is_empty() || cases.len() > MAX_GENERATION_SUITE_CASES {
        return Err(GenerationQualificationContractError::InvalidCollectionSize);
    }
    let mut ids = HashSet::with_capacity(cases.len());
    let mut keys = HashSet::with_capacity(cases.len());
    if cases
        .iter()
        .any(|case| !ids.insert(case.case_id()) || !keys.insert(case.case_key()))
    {
        return Err(GenerationQualificationContractError::DuplicateEntry);
    }
    Ok(())
}

fn canonical_bytes(
    schema_version: u32,
    protocol_digest: &Digest,
    case_ids: &[GenerationCaseId],
) -> Result<Vec<u8>, GenerationQualificationContractError> {
    let mut output = GENERATION_SUITE_MANIFEST_ID_DOMAIN.to_vec();
    append_u32(&mut output, schema_version);
    append_digest(&mut output, protocol_digest);
    append_count(&mut output, case_ids.len())?;
    for id in case_ids {
        append_digest(&mut output, id.digest());
    }
    Ok(output)
}
