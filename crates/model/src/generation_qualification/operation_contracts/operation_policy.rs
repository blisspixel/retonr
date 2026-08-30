//! Plan-wide preregistered qualification-operation policy.

use std::fmt;

use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    MAX_FIXED_CANONICAL_BYTES, MAX_FIXED_RECORD_JSON_BYTES, OPERATION_SCHEMA_VERSION,
    append_digest, append_u32, license_permission_tag, validate_canonical_json,
};
use super::{
    GENERATION_QUALIFICATION_OPERATION_POLICY_ID_DOMAIN,
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicensePermissionV1,
    GenerationQualificationOperationContractError, GenerationQualificationOperationPolicyId,
    GenerationQualificationPlatformAssessmentPolicyId,
};
use crate::generation_qualification::{
    GenerationQualificationPlanId, GenerationQualificationPlanV1, GenerationRepetitionRecordV1,
    GenerationSuiteManifestId, GenerationSuiteManifestV1, GenerationSystemId,
    GenerationSystemRecordV1, GenerationSystemRecordV1Relations, PlannedCandidateAttemptV1,
};

mod limits;
mod validation;

use limits::append_limits;
pub use limits::*;
use validation::validate_relations;

/// Failure-policy identity domain shared with the frozen qualification plan.
pub const GENERATION_QUALIFICATION_PLAN_FAILURE_POLICY_DOMAIN: &[u8] =
    b"retonr:generation-qualification-plan-failure-policy:v1\0";
/// Maximum JSON bytes accepted for one operation-policy record.
pub const MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_JSON_BYTES: usize =
    MAX_FIXED_RECORD_JSON_BYTES;
/// Maximum canonical identity bytes accepted for one operation policy.
pub const MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_CANONICAL_BYTES: usize =
    MAX_FIXED_CANONICAL_BYTES;
/// Closed qualification decision rule.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationDecisionRuleV1 {
    /// Every required exact relationship and verified phase policy must pass.
    AllRequiredEvidencePasses,
}

const fn decision_rule_tag(value: GenerationQualificationDecisionRuleV1) -> u8 {
    match value {
        GenerationQualificationDecisionRuleV1::AllRequiredEvidencePasses => 0,
    }
}

/// One generation system and its complete exact typed relationship closure.
#[derive(Clone, Copy)]
pub struct GenerationQualificationOperationSystemRelationsV1<'a> {
    /// Exact stable generation-system record.
    pub generation_system: &'a GenerationSystemRecordV1,
    /// Complete runtime, state, package, and effective-package relationships.
    pub relations: GenerationSystemRecordV1Relations<'a>,
}

/// Exact records needed to construct or decode an operation policy.
#[derive(Clone, Copy)]
pub struct GenerationQualificationOperationPolicyV1Relations<'a> {
    /// Exact suite selected by the frozen plan.
    pub suite: &'a GenerationSuiteManifestV1,
    /// Exact frozen qualification plan.
    pub plan: &'a GenerationQualificationPlanV1,
    /// Every contiguous preregistered suite repetition.
    pub repetitions: &'a [GenerationRepetitionRecordV1],
    /// Every planned attempt in exact plan order.
    pub planned_attempts: &'a [PlannedCandidateAttemptV1],
    /// The explicit target generation system and effective state.
    pub target_system: GenerationQualificationOperationSystemRelationsV1<'a>,
    /// The sole baseline generation system and effective state.
    pub baseline_system: GenerationQualificationOperationSystemRelationsV1<'a>,
}

/// Caller-supplied policy facts not derived from exact plan relationships.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationQualificationOperationPolicyV1Input {
    /// Common exact operation ceilings.
    pub limits: GenerationQualificationOperationLimitsV1,
    /// Closed qualification decision rule.
    pub decision_rule: GenerationQualificationDecisionRuleV1,
    /// App-reviewed platform assessment policy identity.
    pub platform_assessment_policy_id: GenerationQualificationPlatformAssessmentPolicyId,
    /// Fixed license permission required for V1 generation.
    pub required_license_permission: GenerationQualificationLicensePermissionV1,
    /// App-reviewed license assessment policy identity.
    pub license_assessment_policy_id: GenerationQualificationLicenseAssessmentPolicyId,
    /// Attempt-ledger phase policy digest.
    pub attempt_ledger_policy_digest: Digest,
    /// Repeatability phase policy digest.
    pub repeatability_policy_digest: Digest,
    /// Resource phase policy digest.
    pub resource_policy_digest: Digest,
    /// Human-adjudication phase policy digest.
    pub human_adjudication_policy_digest: Digest,
}

/// Portable preregistered policy for one complete qualification operation.
///
/// This record is inert. It proves structural closure against exact typed inputs
/// but grants no authority to launch a runtime or qualify a generation system.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationOperationPolicyV1 {
    schema_version: u32,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    target_generation_system_id: GenerationSystemId,
    baseline_generation_system_id: GenerationSystemId,
    limits: GenerationQualificationOperationLimitsV1,
    decision_rule: GenerationQualificationDecisionRuleV1,
    platform_assessment_policy_id: GenerationQualificationPlatformAssessmentPolicyId,
    required_license_permission: GenerationQualificationLicensePermissionV1,
    license_assessment_policy_id: GenerationQualificationLicenseAssessmentPolicyId,
    attempt_ledger_policy_digest: Digest,
    repeatability_policy_digest: Digest,
    resource_policy_digest: Digest,
    human_adjudication_policy_digest: Digest,
    #[serde(skip)]
    id: GenerationQualificationOperationPolicyId,
}

impl GenerationQualificationOperationPolicyV1 {
    /// Creates one policy from the exact frozen suite, plan, systems, and attempts.
    ///
    /// # Errors
    ///
    /// Returns a content-free contract error for any invalid bound, substituted
    /// relationship, incomplete plan cross product, or failure-policy mismatch.
    pub fn new(
        relations: GenerationQualificationOperationPolicyV1Relations<'_>,
        input: GenerationQualificationOperationPolicyV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        Self::build(
            OPERATION_SCHEMA_VERSION,
            relations.plan.qualification_plan_id().clone(),
            relations.suite.suite_manifest_id().clone(),
            relations
                .target_system
                .generation_system
                .generation_system_id()
                .clone(),
            relations
                .baseline_system
                .generation_system
                .generation_system_id()
                .clone(),
            relations,
            input,
        )
    }

    fn build(
        schema_version: u32,
        generation_qualification_plan_id: GenerationQualificationPlanId,
        suite_manifest_id: GenerationSuiteManifestId,
        target_generation_system_id: GenerationSystemId,
        baseline_generation_system_id: GenerationSystemId,
        relations: GenerationQualificationOperationPolicyV1Relations<'_>,
        input: GenerationQualificationOperationPolicyV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        if schema_version != OPERATION_SCHEMA_VERSION {
            return Err(GenerationQualificationOperationContractError::UnsupportedSchema);
        }
        if &generation_qualification_plan_id != relations.plan.qualification_plan_id()
            || &suite_manifest_id != relations.suite.suite_manifest_id()
            || &target_generation_system_id
                != relations
                    .target_system
                    .generation_system
                    .generation_system_id()
            || &baseline_generation_system_id
                != relations
                    .baseline_system
                    .generation_system
                    .generation_system_id()
        {
            return Err(GenerationQualificationOperationContractError::ScopeMismatch);
        }
        validate_relations(relations, &input)?;
        let mut record = Self {
            schema_version,
            generation_qualification_plan_id,
            suite_manifest_id,
            target_generation_system_id,
            baseline_generation_system_id,
            limits: input.limits,
            decision_rule: input.decision_rule,
            platform_assessment_policy_id: input.platform_assessment_policy_id,
            required_license_permission: input.required_license_permission,
            license_assessment_policy_id: input.license_assessment_policy_id,
            attempt_ledger_policy_digest: input.attempt_ledger_policy_digest,
            repeatability_policy_digest: input.repeatability_policy_digest,
            resource_policy_digest: input.resource_policy_digest,
            human_adjudication_policy_digest: input.human_adjudication_policy_digest,
            id: GenerationQualificationOperationPolicyId::from_canonical_bytes(b"uninitialized"),
        };
        let canonical = record.canonical_bytes();
        if canonical.len() > MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_CANONICAL_BYTES {
            return Err(GenerationQualificationOperationContractError::CanonicalEncodingTooLarge);
        }
        record.id = GenerationQualificationOperationPolicyId::from_canonical_bytes(&canonical);
        Ok(record)
    }

    /// Parses canonical bounded JSON and revalidates every exact relationship.
    ///
    /// # Errors
    ///
    /// Returns a content-free contract error for oversized, malformed,
    /// noncanonical, unsupported, substituted, incomplete, or invalid input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationQualificationOperationPolicyV1Relations<'_>,
        expected_input: &GenerationQualificationOperationPolicyV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            generation_qualification_plan_id: GenerationQualificationPlanId,
            suite_manifest_id: GenerationSuiteManifestId,
            target_generation_system_id: GenerationSystemId,
            baseline_generation_system_id: GenerationSystemId,
            limits: GenerationQualificationOperationLimitsV1,
            decision_rule: GenerationQualificationDecisionRuleV1,
            platform_assessment_policy_id: GenerationQualificationPlatformAssessmentPolicyId,
            required_license_permission: GenerationQualificationLicensePermissionV1,
            license_assessment_policy_id: GenerationQualificationLicenseAssessmentPolicyId,
            attempt_ledger_policy_digest: Digest,
            repeatability_policy_digest: Digest,
            resource_policy_digest: Digest,
            human_adjudication_policy_digest: Digest,
        }

        if bytes.len() > MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_JSON_BYTES {
            return Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationOperationContractError::InvalidEncoding)?;
        if wire.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(GenerationQualificationOperationContractError::UnsupportedSchema);
        }
        if wire.limits != expected_input.limits
            || wire.decision_rule != expected_input.decision_rule
            || wire.platform_assessment_policy_id != expected_input.platform_assessment_policy_id
            || wire.required_license_permission != expected_input.required_license_permission
            || wire.license_assessment_policy_id != expected_input.license_assessment_policy_id
            || wire.attempt_ledger_policy_digest != expected_input.attempt_ledger_policy_digest
            || wire.repeatability_policy_digest != expected_input.repeatability_policy_digest
            || wire.resource_policy_digest != expected_input.resource_policy_digest
            || wire.human_adjudication_policy_digest
                != expected_input.human_adjudication_policy_digest
        {
            return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
        }
        let record = Self::build(
            wire.schema_version,
            wire.generation_qualification_plan_id,
            wire.suite_manifest_id,
            wire.target_generation_system_id,
            wire.baseline_generation_system_id,
            relations,
            expected_input.clone(),
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Revalidates this policy against fresh exact relationship records.
    ///
    /// # Errors
    ///
    /// Returns a content-free error if any relationship or derived identity differs.
    pub fn validate_against(
        &self,
        relations: GenerationQualificationOperationPolicyV1Relations<'_>,
        expected_input: &GenerationQualificationOperationPolicyV1Input,
    ) -> Result<(), GenerationQualificationOperationContractError> {
        let expected = Self::new(relations, expected_input.clone())?;
        if &expected == self {
            Ok(())
        } else {
            Err(GenerationQualificationOperationContractError::RelationshipMismatch)
        }
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the content-derived operation-policy identity.
    #[must_use]
    pub const fn operation_policy_id(&self) -> &GenerationQualificationOperationPolicyId {
        &self.id
    }
    /// Returns the exact frozen qualification-plan identity.
    #[must_use]
    pub const fn generation_qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.generation_qualification_plan_id
    }
    /// Returns the exact suite-manifest identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the explicit target generation-system identity.
    #[must_use]
    pub const fn target_generation_system_id(&self) -> &GenerationSystemId {
        &self.target_generation_system_id
    }
    /// Returns the sole baseline generation-system identity.
    #[must_use]
    pub const fn baseline_generation_system_id(&self) -> &GenerationSystemId {
        &self.baseline_generation_system_id
    }
    /// Returns the exact common operation ceilings.
    #[must_use]
    pub const fn limits(&self) -> GenerationQualificationOperationLimitsV1 {
        self.limits
    }
    /// Returns the closed qualification decision rule.
    #[must_use]
    pub const fn decision_rule(&self) -> GenerationQualificationDecisionRuleV1 {
        self.decision_rule
    }
    /// Returns the platform-assessment policy identity.
    #[must_use]
    pub const fn platform_assessment_policy_id(
        &self,
    ) -> &GenerationQualificationPlatformAssessmentPolicyId {
        &self.platform_assessment_policy_id
    }
    /// Returns the fixed required license permission.
    #[must_use]
    pub const fn required_license_permission(&self) -> GenerationQualificationLicensePermissionV1 {
        self.required_license_permission
    }
    /// Returns the license-assessment policy identity.
    #[must_use]
    pub const fn license_assessment_policy_id(
        &self,
    ) -> &GenerationQualificationLicenseAssessmentPolicyId {
        &self.license_assessment_policy_id
    }
    /// Returns the attempt-ledger phase policy digest.
    #[must_use]
    pub const fn attempt_ledger_policy_digest(&self) -> &Digest {
        &self.attempt_ledger_policy_digest
    }
    /// Returns the repeatability phase policy digest.
    #[must_use]
    pub const fn repeatability_policy_digest(&self) -> &Digest {
        &self.repeatability_policy_digest
    }
    /// Returns the resource phase policy digest.
    #[must_use]
    pub const fn resource_policy_digest(&self) -> &Digest {
        &self.resource_policy_digest
    }
    /// Returns the human-adjudication phase policy digest.
    #[must_use]
    pub const fn human_adjudication_policy_digest(&self) -> &Digest {
        &self.human_adjudication_policy_digest
    }
    /// Returns the complete failure-policy digest required on the frozen plan.
    #[must_use]
    pub fn plan_failure_policy_digest(&self) -> Digest {
        generation_qualification_plan_failure_policy_digest(
            self.decision_rule,
            &self.platform_assessment_policy_id,
            self.required_license_permission,
            &self.license_assessment_policy_id,
            &self.attempt_ledger_policy_digest,
            &self.repeatability_policy_digest,
            &self.resource_policy_digest,
            &self.human_adjudication_policy_digest,
        )
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = GENERATION_QUALIFICATION_OPERATION_POLICY_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.generation_qualification_plan_id.digest(),
            self.suite_manifest_id.digest(),
            self.target_generation_system_id.digest(),
            self.baseline_generation_system_id.digest(),
        ] {
            append_digest(&mut output, digest);
        }
        append_limits(&mut output, self.limits);
        output.push(decision_rule_tag(self.decision_rule));
        append_digest(&mut output, self.platform_assessment_policy_id.digest());
        output.push(license_permission_tag(self.required_license_permission));
        append_digest(&mut output, self.license_assessment_policy_id.digest());
        for digest in [
            &self.attempt_ledger_policy_digest,
            &self.repeatability_policy_digest,
            &self.resource_policy_digest,
            &self.human_adjudication_policy_digest,
        ] {
            append_digest(&mut output, digest);
        }
        output
    }
}

impl fmt::Debug for GenerationQualificationOperationPolicyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationOperationPolicyV1")
            .field("schema_version", &self.schema_version)
            .field("operation_policy_id", &self.id)
            .field("decision_rule", &self.decision_rule)
            .finish_non_exhaustive()
    }
}

/// Derives the exact complete failure-policy digest frozen into the plan.
#[must_use]
#[expect(
    clippy::too_many_arguments,
    reason = "the arguments mirror the frozen digest contract"
)]
pub fn generation_qualification_plan_failure_policy_digest(
    decision_rule: GenerationQualificationDecisionRuleV1,
    platform_assessment_policy_id: &GenerationQualificationPlatformAssessmentPolicyId,
    required_license_permission: GenerationQualificationLicensePermissionV1,
    license_assessment_policy_id: &GenerationQualificationLicenseAssessmentPolicyId,
    attempt_ledger_policy_digest: &Digest,
    repeatability_policy_digest: &Digest,
    resource_policy_digest: &Digest,
    human_adjudication_policy_digest: &Digest,
) -> Digest {
    let mut material = GENERATION_QUALIFICATION_PLAN_FAILURE_POLICY_DOMAIN.to_vec();
    append_u32(&mut material, OPERATION_SCHEMA_VERSION);
    material.push(decision_rule_tag(decision_rule));
    append_digest(&mut material, platform_assessment_policy_id.digest());
    material.push(license_permission_tag(required_license_permission));
    append_digest(&mut material, license_assessment_policy_id.digest());
    for digest in [
        attempt_ledger_policy_digest,
        repeatability_policy_digest,
        resource_policy_digest,
        human_adjudication_policy_digest,
    ] {
        append_digest(&mut material, digest);
    }
    Digest::sha256(&material)
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
