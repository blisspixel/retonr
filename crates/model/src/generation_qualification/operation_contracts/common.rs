use serde::Serialize;

use super::GenerationQualificationOperationContractError;

/// Maximum bytes accepted for one canonical assessment policy.
pub const MAX_GENERATION_QUALIFICATION_POLICY_BYTES: usize = 64 * 1_024;
pub(super) const OPERATION_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_FIXED_RECORD_JSON_BYTES: usize = 16 * 1_024;
pub(super) const MAX_FIXED_CANONICAL_BYTES: usize = 4 * 1_024;
pub(super) const MAX_PROJECTION_JSON_BYTES: usize = 4 * 1_024 * 1_024;
pub(super) const MAX_PROJECTION_CANONICAL_BYTES: usize = 512 * 1_024;

/// Fixed license permission for generation qualification V1.
#[derive(
    Clone, Copy, Debug, serde::Deserialize, Eq, schemars::JsonSchema, PartialEq, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationLicensePermissionV1 {
    /// Local model generation without redistribution.
    LocalGeneration,
}

pub(super) const fn license_permission_tag(
    value: GenerationQualificationLicensePermissionV1,
) -> u8 {
    match value {
        GenerationQualificationLicensePermissionV1::LocalGeneration => 0,
    }
}

pub(super) fn validate_canonical_json<T: Serialize>(
    bytes: &[u8],
    value: &T,
) -> Result<(), GenerationQualificationOperationContractError> {
    let canonical = serde_json::to_vec(value)
        .map_err(|_| GenerationQualificationOperationContractError::InvalidEncoding)?;
    if canonical == bytes {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::NonCanonicalEncoding)
    }
}

pub(super) fn append_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

pub(super) fn append_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

pub(super) fn append_digest(output: &mut Vec<u8>, value: &rewrite_types::Digest) {
    output.extend_from_slice(value.as_str().as_bytes());
}
