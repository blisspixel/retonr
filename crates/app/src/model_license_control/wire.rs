use rewrite_model::{ArtifactId, PackageSourceKind};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::{
    MAX_MODEL_LICENSE_CONTROL_JSON_BYTES, MAX_MODEL_LICENSE_REVIEW_JSON_BYTES,
    ModelLicenseControlError, ModelLicensePermission,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewWire {
    pub(super) decision: ReviewDecisionWire,
    pub(super) foundation: FoundationWire,
    pub(super) license_members: Vec<LicenseMemberWire>,
    pub(super) permission: ModelLicensePermission,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) schema_version: u32,
    pub(super) source: SourceWire,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReviewDecisionWire {
    Approved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FoundationWire {
    pub(super) artifact_set_id: Digest,
    pub(super) descriptor_mapping_digest: Digest,
    pub(super) foundation_id: Digest,
    pub(super) logical_binding_digest: Digest,
    pub(super) model_package_manifest_id: Digest,
    pub(super) package_source_id: Digest,
    pub(super) provenance_manifest: LicenseMemberWire,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceWire {
    pub(super) kind: PackageSourceKind,
    pub(super) locator: String,
    pub(super) provenance_digest: Digest,
    pub(super) revision: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LicenseMemberWire {
    pub(super) artifact_id: ArtifactId,
    pub(super) byte_size: u64,
    pub(super) relative_path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlWire {
    pub(super) authority: AuthorityWire,
    pub(super) control: ControlKindWire,
    pub(super) evidence: ControlEvidenceWire,
    pub(super) permission: ModelLicensePermission,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) review: ReviewWire,
    pub(super) schema_version: u32,
    pub(super) status: ControlStatusWire,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AuthorityWire {
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ControlKindWire {
    ModelLicense,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ControlStatusWire {
    Approved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlEvidenceWire {
    pub(super) artifact_set_id: Digest,
    pub(super) foundation_id: Digest,
    pub(super) license_member_count: u32,
    pub(super) model_package_manifest_id: Digest,
    pub(super) package_source_id: Digest,
    pub(super) review_evidence_digest: Digest,
}

pub(super) fn parse_review(bytes: &[u8]) -> Result<ReviewWire, ModelLicenseControlError> {
    parse_canonical(bytes, MAX_MODEL_LICENSE_REVIEW_JSON_BYTES)
}

pub(super) fn parse_control(bytes: &[u8]) -> Result<ControlWire, ModelLicenseControlError> {
    parse_canonical(bytes, MAX_MODEL_LICENSE_CONTROL_JSON_BYTES)
}

pub(super) fn encode_review(value: &ReviewWire) -> Result<Vec<u8>, ModelLicenseControlError> {
    encode(value, MAX_MODEL_LICENSE_REVIEW_JSON_BYTES)
}

pub(super) fn encode_control(value: &ControlWire) -> Result<Vec<u8>, ModelLicenseControlError> {
    encode(value, MAX_MODEL_LICENSE_CONTROL_JSON_BYTES)
}

fn parse_canonical<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, ModelLicenseControlError> {
    if bytes.is_empty() {
        return Err(ModelLicenseControlError::InvalidEncoding);
    }
    if bytes.len() > maximum {
        return Err(ModelLicenseControlError::LimitExceeded);
    }
    let value = serde_json::from_slice(bytes)
        .map_err(|_error| ModelLicenseControlError::InvalidEncoding)?;
    if encode(&value, maximum)? != bytes {
        return Err(ModelLicenseControlError::InvalidEncoding);
    }
    Ok(value)
}

fn encode<T: Serialize>(value: &T, maximum: usize) -> Result<Vec<u8>, ModelLicenseControlError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_error| ModelLicenseControlError::InvalidEncoding)?;
    if bytes.len() > maximum {
        Err(ModelLicenseControlError::LimitExceeded)
    } else {
        Ok(bytes)
    }
}
