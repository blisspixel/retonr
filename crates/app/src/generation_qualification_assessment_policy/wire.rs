use serde::{Deserialize, Serialize, de::DeserializeOwned};

use rewrite_model::RuntimeTarget;

use super::{
    GenerationQualificationAssessmentPolicyError,
    MAX_GENERATION_QUALIFICATION_ASSESSMENT_POLICY_JSON_BYTES,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AuthorityWire {
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PlatformPolicyKindWire {
    GenerationQualificationPlatformAssessment,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PlatformDecisionRuleWire {
    ExactReviewedProfileAndCurrentHost,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PlatformProfileWire {
    ManagedLinuxNativeCpu,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlatformPolicyWire {
    pub(super) authority: AuthorityWire,
    pub(super) policy: PlatformPolicyKindWire,
    pub(super) decision_rule: PlatformDecisionRuleWire,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) profile: PlatformProfileWire,
    pub(super) runtime_target: RuntimeTarget,
    pub(super) operating_system_digest: String,
    pub(super) architecture_digest: String,
    pub(super) execution_class_digest: String,
    pub(super) hardware_envelope_digest: String,
    pub(super) schema_version: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LicensePolicyKindWire {
    GenerationQualificationLicenseAssessment,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LicenseDecisionRuleWire {
    ExactControlAndProductionApproval,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LicensePermissionWire {
    LocalGeneration,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LicensePolicyWire {
    pub(super) authority: AuthorityWire,
    pub(super) policy: LicensePolicyKindWire,
    pub(super) decision_rule: LicenseDecisionRuleWire,
    pub(super) model_license_control_id: String,
    pub(super) permission: LicensePermissionWire,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) schema_version: u32,
}

pub(super) fn parse_platform(
    bytes: &[u8],
) -> Result<PlatformPolicyWire, GenerationQualificationAssessmentPolicyError> {
    parse_canonical(bytes)
}

pub(super) fn parse_license(
    bytes: &[u8],
) -> Result<LicensePolicyWire, GenerationQualificationAssessmentPolicyError> {
    parse_canonical(bytes)
}

#[cfg(any(test, feature = "test-support"))]
pub(super) fn encode_platform(
    value: &PlatformPolicyWire,
) -> Result<Vec<u8>, GenerationQualificationAssessmentPolicyError> {
    encode(value)
}

#[cfg(any(test, feature = "test-support"))]
pub(super) fn encode_license(
    value: &LicensePolicyWire,
) -> Result<Vec<u8>, GenerationQualificationAssessmentPolicyError> {
    encode(value)
}

fn parse_canonical<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
) -> Result<T, GenerationQualificationAssessmentPolicyError> {
    if bytes.is_empty() {
        return Err(GenerationQualificationAssessmentPolicyError::InvalidEncoding);
    }
    if bytes.len() > MAX_GENERATION_QUALIFICATION_ASSESSMENT_POLICY_JSON_BYTES {
        return Err(GenerationQualificationAssessmentPolicyError::LimitExceeded);
    }
    let value = serde_json::from_slice(bytes)
        .map_err(|_error| GenerationQualificationAssessmentPolicyError::InvalidEncoding)?;
    if encode(&value)? != bytes {
        return Err(GenerationQualificationAssessmentPolicyError::NonCanonicalEncoding);
    }
    Ok(value)
}

fn encode<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, GenerationQualificationAssessmentPolicyError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_error| GenerationQualificationAssessmentPolicyError::InvalidEncoding)?;
    if bytes.len() > MAX_GENERATION_QUALIFICATION_ASSESSMENT_POLICY_JSON_BYTES {
        Err(GenerationQualificationAssessmentPolicyError::LimitExceeded)
    } else {
        Ok(bytes)
    }
}
