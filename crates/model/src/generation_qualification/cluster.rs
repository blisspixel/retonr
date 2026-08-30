use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::codec::{
    append_digest, append_text, append_u32, valid_machine_key, validate_canonical_json,
};
use super::{
    GENERATION_CLUSTER_ID_DOMAIN, GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationClusterId,
    GenerationQualificationContractError,
};

/// Maximum JSON bytes accepted for one generation-cluster record.
pub const MAX_GENERATION_CLUSTER_JSON_BYTES: usize = 16_384;
const MAX_GENERATION_CLUSTER_CANONICAL_BYTES: usize = 512;

/// Inert portable declaration of one generation qualification cluster.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationClusterRecordV1 {
    schema_version: u32,
    cluster_key: String,
    cluster_policy_digest: Digest,
    #[serde(skip)]
    id: GenerationClusterId,
}

impl GenerationClusterRecordV1 {
    /// Creates one bounded canonical cluster declaration.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] unless the key is a
    /// canonical lowercase machine key within the fixed ceiling.
    pub fn new(
        cluster_key: impl Into<String>,
        cluster_policy_digest: Digest,
    ) -> Result<Self, GenerationQualificationContractError> {
        Self::from_wire(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            cluster_key.into(),
            cluster_policy_digest,
        )
    }

    fn from_wire(
        schema_version: u32,
        cluster_key: String,
        cluster_policy_digest: Digest,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        if !valid_machine_key(&cluster_key, super::MAX_GENERATION_MACHINE_KEY_BYTES) {
            return Err(GenerationQualificationContractError::InvalidMachineKey);
        }
        let canonical = canonical_bytes(schema_version, &cluster_key, &cluster_policy_digest);
        if canonical.len() > MAX_GENERATION_CLUSTER_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            schema_version,
            cluster_key,
            cluster_policy_digest,
            id: GenerationClusterId(Digest::sha256(&canonical)),
        })
    }

    /// Parses one exact canonical bounded JSON cluster record.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for excessive, malformed,
    /// noncanonical, unsupported, or invalid input.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, GenerationQualificationContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            cluster_key: String,
            cluster_policy_digest: Digest,
        }

        if bytes.len() > MAX_GENERATION_CLUSTER_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        let record = Self::from_wire(
            wire.schema_version,
            wire.cluster_key,
            wire.cluster_policy_digest,
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the canonical cluster machine key.
    #[must_use]
    pub fn cluster_key(&self) -> &str {
        &self.cluster_key
    }

    /// Returns the exact cluster-policy digest.
    #[must_use]
    pub const fn cluster_policy_digest(&self) -> &Digest {
        &self.cluster_policy_digest
    }

    /// Returns the content-derived cluster identity.
    #[must_use]
    pub const fn cluster_id(&self) -> &GenerationClusterId {
        &self.id
    }
}

impl fmt::Debug for GenerationClusterRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationClusterRecordV1")
            .field("schema_version", &self.schema_version)
            .field("cluster_id", &self.id)
            .finish_non_exhaustive()
    }
}

fn canonical_bytes(schema_version: u32, cluster_key: &str, policy_digest: &Digest) -> Vec<u8> {
    let mut output = GENERATION_CLUSTER_ID_DOMAIN.to_vec();
    append_u32(&mut output, schema_version);
    append_text(&mut output, cluster_key);
    append_digest(&mut output, policy_digest);
    output
}
