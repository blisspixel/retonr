use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{
    append_digest, append_text, append_u32, append_u64, valid_machine_key, validate_canonical_json,
};
use super::{
    GENERATION_CASE_ID_DOMAIN, GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationCaseId,
    GenerationClusterId, GenerationClusterRecordV1, GenerationQualificationContractError,
};
use crate::ArtifactId;

/// Maximum JSON bytes accepted for one generation-case manifest.
pub const MAX_GENERATION_CASE_JSON_BYTES: usize = 16_384;
const MAX_GENERATION_CASE_CANONICAL_BYTES: usize = 1_024;

/// Caller-supplied content facts for one generation case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationCaseManifestV1Input {
    /// Canonical case machine key.
    pub case_key: String,
    /// Exact immutable source artifact identity.
    pub source_artifact_id: ArtifactId,
    /// Exact source digest, which must equal the artifact identity digest.
    pub source_digest: Digest,
    /// Nonzero UTF-8 source byte count.
    pub source_byte_count: u64,
    /// Digest of the immutable case contract.
    pub case_contract_digest: Digest,
    /// Digest of the case language declaration.
    pub language_digest: Digest,
    /// Digest of the case operation mode.
    pub mode_digest: Digest,
    /// Digest of the case format contract.
    pub format_digest: Digest,
}

/// Inert portable declaration of one exact generation qualification case.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationCaseManifestV1 {
    schema_version: u32,
    case_key: String,
    cluster_id: GenerationClusterId,
    source_artifact_id: ArtifactId,
    source_digest: Digest,
    source_byte_count: u64,
    case_contract_digest: Digest,
    language_digest: Digest,
    mode_digest: Digest,
    format_digest: Digest,
    #[serde(skip)]
    id: GenerationCaseId,
}

impl GenerationCaseManifestV1 {
    /// Creates one exact case bound to the supplied cluster record.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for an invalid machine
    /// key, zero source size, mismatched source digest, or excessive identity.
    pub fn new(
        cluster: &GenerationClusterRecordV1,
        input: GenerationCaseManifestV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::from_wire(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            cluster,
            cluster.cluster_id().clone(),
            input,
        )
    }

    fn from_wire(
        schema_version: u32,
        cluster: &GenerationClusterRecordV1,
        cluster_id: GenerationClusterId,
        input: GenerationCaseManifestV1Input,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        if &cluster_id != cluster.cluster_id() {
            return Err(GenerationQualificationContractError::ClusterMismatch);
        }
        if !valid_machine_key(&input.case_key, super::MAX_GENERATION_MACHINE_KEY_BYTES) {
            return Err(GenerationQualificationContractError::InvalidMachineKey);
        }
        if input.source_byte_count == 0 || input.source_artifact_id.digest() != &input.source_digest
        {
            return Err(GenerationQualificationContractError::InvalidSourceBinding);
        }
        let canonical = canonical_bytes(schema_version, &cluster_id, &input);
        if canonical.len() > MAX_GENERATION_CASE_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            schema_version,
            case_key: input.case_key,
            cluster_id,
            source_artifact_id: input.source_artifact_id,
            source_digest: input.source_digest,
            source_byte_count: input.source_byte_count,
            case_contract_digest: input.case_contract_digest,
            language_digest: input.language_digest,
            mode_digest: input.mode_digest,
            format_digest: input.format_digest,
            id: GenerationCaseId(Digest::sha256(&canonical)),
        })
    }

    /// Parses one canonical bounded case and rechecks its cluster relationship.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for excessive, malformed,
    /// noncanonical, invalid, unsupported, or cross-cluster input.
    pub fn from_json_bytes(
        bytes: &[u8],
        cluster: &GenerationClusterRecordV1,
    ) -> Result<Self, GenerationQualificationContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            case_key: String,
            cluster_id: GenerationClusterId,
            source_artifact_id: ArtifactId,
            source_digest: Digest,
            source_byte_count: u64,
            case_contract_digest: Digest,
            language_digest: Digest,
            mode_digest: Digest,
            format_digest: Digest,
        }

        if bytes.len() > MAX_GENERATION_CASE_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let record = Self::from_wire(
            wire.schema_version,
            cluster,
            wire.cluster_id,
            GenerationCaseManifestV1Input {
                case_key: wire.case_key,
                source_artifact_id: wire.source_artifact_id,
                source_digest: wire.source_digest,
                source_byte_count: wire.source_byte_count,
                case_contract_digest: wire.case_contract_digest,
                language_digest: wire.language_digest,
                mode_digest: wire.mode_digest,
                format_digest: wire.format_digest,
            },
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the canonical case machine key.
    #[must_use]
    pub fn case_key(&self) -> &str {
        &self.case_key
    }
    /// Returns the exact cluster identity.
    #[must_use]
    pub const fn cluster_id(&self) -> &GenerationClusterId {
        &self.cluster_id
    }
    /// Returns the exact source artifact identity.
    #[must_use]
    pub const fn source_artifact_id(&self) -> &ArtifactId {
        &self.source_artifact_id
    }
    /// Returns the exact source digest.
    #[must_use]
    pub const fn source_digest(&self) -> &Digest {
        &self.source_digest
    }
    /// Returns the exact source byte count.
    #[must_use]
    pub const fn source_byte_count(&self) -> u64 {
        self.source_byte_count
    }
    /// Returns the immutable case-contract digest.
    #[must_use]
    pub const fn case_contract_digest(&self) -> &Digest {
        &self.case_contract_digest
    }
    /// Returns the language declaration digest.
    #[must_use]
    pub const fn language_digest(&self) -> &Digest {
        &self.language_digest
    }
    /// Returns the operation-mode digest.
    #[must_use]
    pub const fn mode_digest(&self) -> &Digest {
        &self.mode_digest
    }
    /// Returns the format-contract digest.
    #[must_use]
    pub const fn format_digest(&self) -> &Digest {
        &self.format_digest
    }
    /// Returns the content-derived case identity.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.id
    }
}

impl fmt::Debug for GenerationCaseManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationCaseManifestV1")
            .field("schema_version", &self.schema_version)
            .field("case_id", &self.id)
            .field("source_byte_count", &self.source_byte_count)
            .finish_non_exhaustive()
    }
}

fn canonical_bytes(
    schema_version: u32,
    cluster_id: &GenerationClusterId,
    input: &GenerationCaseManifestV1Input,
) -> Vec<u8> {
    let mut output = GENERATION_CASE_ID_DOMAIN.to_vec();
    append_u32(&mut output, schema_version);
    append_text(&mut output, &input.case_key);
    append_digest(&mut output, cluster_id.digest());
    append_digest(&mut output, input.source_artifact_id.digest());
    append_digest(&mut output, &input.source_digest);
    append_u64(&mut output, input.source_byte_count);
    for digest in [
        &input.case_contract_digest,
        &input.language_digest,
        &input.mode_digest,
        &input.format_digest,
    ] {
        append_digest(&mut output, digest);
    }
    output
}
