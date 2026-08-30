use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::{
    GENERATION_QUALIFICATION_HUMAN_ADJUDICATION_POLICY_PROCEDURE_ID,
    GENERATION_QUALIFICATION_HUMAN_ADJUDICATION_POLICY_PROCEDURE_VERSION,
    GENERATION_QUALIFICATION_PHASE_POLICY_SCHEMA_VERSION,
    GENERATION_QUALIFICATION_RESOURCE_POLICY_PROCEDURE_ID,
    GENERATION_QUALIFICATION_RESOURCE_POLICY_PROCEDURE_VERSION,
    GenerationQualificationPhasePolicyError, GenerationQualificationResourcePolicyLimitsV1,
    MAX_GENERATION_QUALIFICATION_PHASE_POLICY_JSON_BYTES,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AuthorityWire {
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ResourceDecisionRuleWire {
    AllCompleteTargetObservationsWithinDeclaredLimits,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ResourceMeasurementProfileWire {
    ManagedLocalGenerationV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RequiredProviderObservationWire {
    PromptTokenCount,
    GeneratedTokenCount,
    TotalDurationNanoseconds,
    LoadDurationNanoseconds,
    PromptEvaluationDurationNanoseconds,
    EvaluationDurationNanoseconds,
}

pub(super) const REQUIRED_PROVIDER_OBSERVATIONS: [RequiredProviderObservationWire; 6] = [
    RequiredProviderObservationWire::PromptTokenCount,
    RequiredProviderObservationWire::GeneratedTokenCount,
    RequiredProviderObservationWire::TotalDurationNanoseconds,
    RequiredProviderObservationWire::LoadDurationNanoseconds,
    RequiredProviderObservationWire::PromptEvaluationDurationNanoseconds,
    RequiredProviderObservationWire::EvaluationDurationNanoseconds,
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResourcePolicyWire {
    pub(super) authority: AuthorityWire,
    pub(super) decision_rule: ResourceDecisionRuleWire,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) measurement_profile: ResourceMeasurementProfileWire,
    pub(super) maximum_attempt_elapsed_nanoseconds: u64,
    pub(super) maximum_first_response_nanoseconds: u64,
    pub(super) maximum_cleanup_nanoseconds: u64,
    pub(super) maximum_worker_high_water_resident_bytes: u64,
    pub(super) maximum_installed_footprint_bytes: u64,
    pub(super) required_provider_observations: Vec<RequiredProviderObservationWire>,
    pub(super) schema_version: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum HumanDecisionRuleWire {
    TwoIndependentBlindedReviewsThenRoleSeparatedAdjudication,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum PresentationRuleWire {
    DeterministicBlindedCandidatePairV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EligibleCaseRuleWire {
    AllCasesInAllPassedRepetitions,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AdjudicationTriggerWire {
    DisagreementTieOrAbstention,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AllowedOutcomeWire {
    Acceptable,
    Unacceptable,
    Abstain,
}

pub(super) const ALLOWED_OUTCOMES: [AllowedOutcomeWire; 3] = [
    AllowedOutcomeWire::Acceptable,
    AllowedOutcomeWire::Unacceptable,
    AllowedOutcomeWire::Abstain,
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HumanPolicyWire {
    pub(super) authority: AuthorityWire,
    pub(super) decision_rule: HumanDecisionRuleWire,
    pub(super) procedure_id: String,
    pub(super) procedure_version: u32,
    pub(super) presentation_rule: PresentationRuleWire,
    pub(super) presentation_seed: u64,
    pub(super) eligible_case_rule: EligibleCaseRuleWire,
    pub(super) primary_reviewer_count: u32,
    pub(super) require_distinct_primary_reviewers: bool,
    pub(super) require_role_separated_adjudicator: bool,
    pub(super) adjudication_trigger: AdjudicationTriggerWire,
    pub(super) allowed_outcomes: Vec<AllowedOutcomeWire>,
    pub(super) schema_version: u32,
}

pub(super) fn expected_resource(
    limits: GenerationQualificationResourcePolicyLimitsV1,
) -> ResourcePolicyWire {
    ResourcePolicyWire {
        authority: AuthorityWire::None,
        decision_rule: ResourceDecisionRuleWire::AllCompleteTargetObservationsWithinDeclaredLimits,
        procedure_id: GENERATION_QUALIFICATION_RESOURCE_POLICY_PROCEDURE_ID.to_owned(),
        procedure_version: GENERATION_QUALIFICATION_RESOURCE_POLICY_PROCEDURE_VERSION,
        measurement_profile: ResourceMeasurementProfileWire::ManagedLocalGenerationV1,
        maximum_attempt_elapsed_nanoseconds: limits.maximum_attempt_elapsed_nanoseconds,
        maximum_first_response_nanoseconds: limits.maximum_first_response_nanoseconds,
        maximum_cleanup_nanoseconds: limits.maximum_cleanup_nanoseconds,
        maximum_worker_high_water_resident_bytes: limits.maximum_worker_high_water_resident_bytes,
        maximum_installed_footprint_bytes: limits.maximum_installed_footprint_bytes,
        required_provider_observations: REQUIRED_PROVIDER_OBSERVATIONS.to_vec(),
        schema_version: GENERATION_QUALIFICATION_PHASE_POLICY_SCHEMA_VERSION,
    }
}

pub(super) fn expected_human(presentation_seed: u64) -> HumanPolicyWire {
    HumanPolicyWire {
        authority: AuthorityWire::None,
        decision_rule:
            HumanDecisionRuleWire::TwoIndependentBlindedReviewsThenRoleSeparatedAdjudication,
        procedure_id: GENERATION_QUALIFICATION_HUMAN_ADJUDICATION_POLICY_PROCEDURE_ID.to_owned(),
        procedure_version: GENERATION_QUALIFICATION_HUMAN_ADJUDICATION_POLICY_PROCEDURE_VERSION,
        presentation_rule: PresentationRuleWire::DeterministicBlindedCandidatePairV1,
        presentation_seed,
        eligible_case_rule: EligibleCaseRuleWire::AllCasesInAllPassedRepetitions,
        primary_reviewer_count: 2,
        require_distinct_primary_reviewers: true,
        require_role_separated_adjudicator: true,
        adjudication_trigger: AdjudicationTriggerWire::DisagreementTieOrAbstention,
        allowed_outcomes: ALLOWED_OUTCOMES.to_vec(),
        schema_version: GENERATION_QUALIFICATION_PHASE_POLICY_SCHEMA_VERSION,
    }
}

pub(super) fn parse_resource(
    bytes: &[u8],
) -> Result<ResourcePolicyWire, GenerationQualificationPhasePolicyError> {
    parse_canonical(bytes)
}

pub(super) fn parse_human(
    bytes: &[u8],
) -> Result<HumanPolicyWire, GenerationQualificationPhasePolicyError> {
    parse_canonical(bytes)
}

#[cfg(test)]
pub(super) fn encode_resource(
    value: &ResourcePolicyWire,
) -> Result<Vec<u8>, GenerationQualificationPhasePolicyError> {
    encode(value)
}

#[cfg(test)]
pub(super) fn encode_human(
    value: &HumanPolicyWire,
) -> Result<Vec<u8>, GenerationQualificationPhasePolicyError> {
    encode(value)
}

fn parse_canonical<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
) -> Result<T, GenerationQualificationPhasePolicyError> {
    if bytes.is_empty() {
        return Err(GenerationQualificationPhasePolicyError::InvalidEncoding);
    }
    if bytes.len() > MAX_GENERATION_QUALIFICATION_PHASE_POLICY_JSON_BYTES {
        return Err(GenerationQualificationPhasePolicyError::LimitExceeded);
    }
    let value = serde_json::from_slice(bytes)
        .map_err(|_error| GenerationQualificationPhasePolicyError::InvalidEncoding)?;
    if encode(&value)? != bytes {
        return Err(GenerationQualificationPhasePolicyError::NonCanonicalEncoding);
    }
    Ok(value)
}

fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, GenerationQualificationPhasePolicyError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_error| GenerationQualificationPhasePolicyError::InvalidEncoding)?;
    if bytes.len() > MAX_GENERATION_QUALIFICATION_PHASE_POLICY_JSON_BYTES {
        Err(GenerationQualificationPhasePolicyError::LimitExceeded)
    } else {
        Ok(bytes)
    }
}
