use serde::{Deserialize, Serialize, de::DeserializeOwned};

use rewrite_types::Digest;

use super::{
    GENERATION_SYSTEM_POLICY_CONTROL_PROCEDURE_ID,
    GENERATION_SYSTEM_POLICY_CONTROL_PROCEDURE_VERSION,
    GENERATION_SYSTEM_POLICY_CONTROL_SCHEMA_VERSION, GENERATION_SYSTEM_POLICY_REVIEW_PROCEDURE_ID,
    GENERATION_SYSTEM_POLICY_REVIEW_PROCEDURE_VERSION,
    GENERATION_SYSTEM_POLICY_REVIEW_SCHEMA_VERSION, GenerationSystemPolicyBindingsV1,
    GenerationSystemPolicyError, GenerationSystemPolicyPermission, GenerationSystemPolicyPurpose,
    MAX_GENERATION_SYSTEM_POLICY_CONTROL_JSON_BYTES,
    MAX_GENERATION_SYSTEM_POLICY_REVIEW_JSON_BYTES,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReviewWire {
    pub(super) decision: ReviewDecisionWire,
    pub(super) bindings: GenerationSystemPolicyBindingsV1,
    pub(super) permission: GenerationSystemPolicyPermission,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) purpose: GenerationSystemPolicyPurpose,
    pub(super) schema_version: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ReviewDecisionWire {
    Approved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlWire {
    pub(super) authority: AuthorityWire,
    pub(super) control: ControlKindWire,
    pub(super) permission: GenerationSystemPolicyPermission,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) purpose: GenerationSystemPolicyPurpose,
    pub(super) review: ReviewWire,
    pub(super) review_evidence_digest: Digest,
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
    GenerationSystemPolicy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ControlStatusWire {
    Approved,
}

pub(super) fn expected_review(
    bindings: &GenerationSystemPolicyBindingsV1,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
) -> ReviewWire {
    ReviewWire {
        decision: ReviewDecisionWire::Approved,
        bindings: bindings.clone(),
        permission,
        procedure_id: GENERATION_SYSTEM_POLICY_REVIEW_PROCEDURE_ID.to_owned(),
        procedure_version: GENERATION_SYSTEM_POLICY_REVIEW_PROCEDURE_VERSION,
        purpose,
        schema_version: GENERATION_SYSTEM_POLICY_REVIEW_SCHEMA_VERSION,
    }
}

pub(super) fn expected_control(
    review: ReviewWire,
    review_evidence_digest: Digest,
    permission: GenerationSystemPolicyPermission,
    purpose: GenerationSystemPolicyPurpose,
) -> ControlWire {
    ControlWire {
        authority: AuthorityWire::None,
        control: ControlKindWire::GenerationSystemPolicy,
        permission,
        procedure_id: GENERATION_SYSTEM_POLICY_CONTROL_PROCEDURE_ID.to_owned(),
        procedure_version: GENERATION_SYSTEM_POLICY_CONTROL_PROCEDURE_VERSION,
        purpose,
        review,
        review_evidence_digest,
        schema_version: GENERATION_SYSTEM_POLICY_CONTROL_SCHEMA_VERSION,
        status: ControlStatusWire::Approved,
    }
}

pub(super) fn parse_review(bytes: &[u8]) -> Result<ReviewWire, GenerationSystemPolicyError> {
    parse_canonical(bytes, MAX_GENERATION_SYSTEM_POLICY_REVIEW_JSON_BYTES)
}

pub(super) fn parse_control(bytes: &[u8]) -> Result<ControlWire, GenerationSystemPolicyError> {
    parse_canonical(bytes, MAX_GENERATION_SYSTEM_POLICY_CONTROL_JSON_BYTES)
}

pub(super) fn encode_review(value: &ReviewWire) -> Result<Vec<u8>, GenerationSystemPolicyError> {
    encode(value, MAX_GENERATION_SYSTEM_POLICY_REVIEW_JSON_BYTES)
}

pub(super) fn encode_control(value: &ControlWire) -> Result<Vec<u8>, GenerationSystemPolicyError> {
    encode(value, MAX_GENERATION_SYSTEM_POLICY_CONTROL_JSON_BYTES)
}

fn parse_canonical<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, GenerationSystemPolicyError> {
    if bytes.is_empty() {
        return Err(GenerationSystemPolicyError::InvalidEncoding);
    }
    if bytes.len() > maximum {
        return Err(GenerationSystemPolicyError::LimitExceeded);
    }
    let value = serde_json::from_slice(bytes)
        .map_err(|_error| GenerationSystemPolicyError::InvalidEncoding)?;
    if encode(&value, maximum)? != bytes {
        return Err(GenerationSystemPolicyError::InvalidEncoding);
    }
    Ok(value)
}

fn encode<T: Serialize>(value: &T, maximum: usize) -> Result<Vec<u8>, GenerationSystemPolicyError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_error| GenerationSystemPolicyError::InvalidEncoding)?;
    if bytes.len() > maximum {
        Err(GenerationSystemPolicyError::LimitExceeded)
    } else {
        Ok(bytes)
    }
}
