//! Portable pretraffic model-license assessment evidence.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    MAX_FIXED_CANONICAL_BYTES, MAX_FIXED_RECORD_JSON_BYTES, OPERATION_SCHEMA_VERSION,
    append_digest, append_u32, license_permission_tag, validate_canonical_json,
};
use super::{
    GENERATION_QUALIFICATION_LICENSE_EVIDENCE_ID_DOMAIN,
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicenseEvidenceId,
    GenerationQualificationLicensePermissionV1, GenerationQualificationOperationContractError,
    GenerationQualificationOperationPolicyV1, GenerationQualificationRequestProjectionV1,
};
use crate::{
    ArtifactId, ArtifactSetId, GenerationSystemId, GenerationSystemRecordV1,
    GenerationSystemRecordV1Relations, ModelLicenseControlId, ModelPackageFoundationId,
    ModelPackageManifestId,
};

/// Maximum JSON bytes accepted for one license-evidence record.
pub const MAX_GENERATION_QUALIFICATION_LICENSE_EVIDENCE_JSON_BYTES: usize =
    MAX_FIXED_RECORD_JSON_BYTES;
/// Maximum canonical identity bytes for one license-evidence record.
pub const MAX_GENERATION_QUALIFICATION_LICENSE_EVIDENCE_CANONICAL_BYTES: usize =
    MAX_FIXED_CANONICAL_BYTES;

/// Closed V1 model-license decision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationLicenseDecisionV1 {
    /// The exact target model is approved for local generation only.
    LocalUseOnly,
    /// The reviewed approval policy denied local generation.
    Rejected,
}

/// Closed reason for a model-license decision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationLicenseReasonV1 {
    /// The reviewed license control approves local generation.
    ApprovedLocalGeneration,
    /// The reviewed approval policy denied local generation.
    ApprovalPolicyDenied,
}

/// Exact typed records and owner-derived identities for license evidence.
#[derive(Clone, Copy)]
pub struct GenerationQualificationLicenseEvidenceV1Relations<'a> {
    /// Preregistered operation policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Complete preregistered request projection.
    pub request_projection: &'a GenerationQualificationRequestProjectionV1,
    /// Target generation system selected by the operation policy.
    pub target_generation_system: &'a GenerationSystemRecordV1,
    /// Complete typed closure of the target generation system.
    pub target_generation_system_relations: GenerationSystemRecordV1Relations<'a>,
    /// Stable model-package foundation verified for the target package.
    pub model_package_foundation_id: &'a ModelPackageFoundationId,
    /// Exact portable model-license control reviewed for local generation.
    pub model_license_control_id: &'a ModelLicenseControlId,
}

/// Assessment result supplied by the app-owned license compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationQualificationLicenseEvidenceV1Input {
    /// Derived license decision.
    pub decision: GenerationQualificationLicenseDecisionV1,
    /// Closed reason consistent with `decision`.
    pub reason: GenerationQualificationLicenseReasonV1,
}

/// Portable binding of the exact pretraffic model-license assessment.
///
/// This record is inert. It does not prove a current installation lease and
/// grants no generation, qualification, redistribution, or live-use authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationLicenseEvidenceV1 {
    schema_version: u32,
    operation_policy_id: super::GenerationQualificationOperationPolicyId,
    request_projection_id: super::GenerationQualificationRequestProjectionId,
    target_generation_system_id: GenerationSystemId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    model_package_foundation_id: ModelPackageFoundationId,
    model_license_control_id: ModelLicenseControlId,
    license_assessment_policy_id: GenerationQualificationLicenseAssessmentPolicyId,
    permission: GenerationQualificationLicensePermissionV1,
    decision: GenerationQualificationLicenseDecisionV1,
    reason: GenerationQualificationLicenseReasonV1,
    #[serde(skip)]
    id: GenerationQualificationLicenseEvidenceId,
}

impl GenerationQualificationLicenseEvidenceV1 {
    /// Derives license evidence from exact preregistered, system, and trust records.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation scope, target-system closure, or
    /// decision and reason pair is inconsistent.
    pub fn new(
        relations: GenerationQualificationLicenseEvidenceV1Relations<'_>,
        input: GenerationQualificationLicenseEvidenceV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        Self::from_relations(OPERATION_SCHEMA_VERSION, relations, input)
    }

    fn from_relations(
        schema_version: u32,
        relations: GenerationQualificationLicenseEvidenceV1Relations<'_>,
        input: GenerationQualificationLicenseEvidenceV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        if schema_version != OPERATION_SCHEMA_VERSION {
            return Err(GenerationQualificationOperationContractError::UnsupportedSchema);
        }
        validate_relations(relations)?;
        validate_decision_reason(input.decision, input.reason)?;

        let target = relations.target_generation_system;
        let mut record = Self {
            schema_version,
            operation_policy_id: relations.operation_policy.operation_policy_id().clone(),
            request_projection_id: relations.request_projection.request_projection_id().clone(),
            target_generation_system_id: target.generation_system_id().clone(),
            model_artifact_set_id: target.model_artifact_set_id().clone(),
            model_package_manifest_id: target.model_package_manifest_id().clone(),
            model_artifact_id: target.model_artifact_id().clone(),
            model_package_foundation_id: relations.model_package_foundation_id.clone(),
            model_license_control_id: relations.model_license_control_id.clone(),
            license_assessment_policy_id: relations
                .operation_policy
                .license_assessment_policy_id()
                .clone(),
            permission: relations.operation_policy.required_license_permission(),
            decision: input.decision,
            reason: input.reason,
            id: GenerationQualificationLicenseEvidenceId::from_canonical_bytes(
                b"uninitialized license evidence",
            ),
        };
        let canonical = record.canonical_bytes();
        if canonical.len() > MAX_GENERATION_QUALIFICATION_LICENSE_EVIDENCE_CANONICAL_BYTES {
            return Err(GenerationQualificationOperationContractError::CanonicalEncodingTooLarge);
        }
        record.id = GenerationQualificationLicenseEvidenceId::from_canonical_bytes(&canonical);
        Ok(record)
    }

    /// Parses bounded canonical JSON and rechecks the complete typed closure.
    /// `expected_input` must come from the independent app-owned assessment;
    /// neither decision nor reason is trusted from the serialized record.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, noncanonical, unsupported, or
    /// relationship-inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationQualificationLicenseEvidenceV1Relations<'_>,
        expected_input: GenerationQualificationLicenseEvidenceV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_LICENSE_EVIDENCE_JSON_BYTES {
            return Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge);
        }
        let wire: LicenseEvidenceWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationOperationContractError::InvalidEncoding)?;
        if wire.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(GenerationQualificationOperationContractError::UnsupportedSchema);
        }

        let record = Self::from_relations(wire.schema_version, relations, expected_input)?;
        if !wire.relationships_equal(&record) {
            return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
        }
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Revalidates this record against the current exact typed closure.
    /// `expected_input` must be freshly supplied by the app-owned assessment
    /// boundary rather than reconstructed from this record.
    ///
    /// # Errors
    ///
    /// Returns an error when any dependency, system or package fact, assessment
    /// result, or derived identity differs.
    pub fn validate_against(
        &self,
        relations: GenerationQualificationLicenseEvidenceV1Relations<'_>,
        expected_input: GenerationQualificationLicenseEvidenceV1Input,
    ) -> Result<(), GenerationQualificationOperationContractError> {
        let expected = Self::from_relations(self.schema_version, relations, expected_input)?;
        if self != &expected {
            return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
        }
        Ok(())
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact operation-policy identity.
    #[must_use]
    pub const fn operation_policy_id(&self) -> &super::GenerationQualificationOperationPolicyId {
        &self.operation_policy_id
    }

    /// Returns the complete request-projection identity.
    #[must_use]
    pub const fn request_projection_id(
        &self,
    ) -> &super::GenerationQualificationRequestProjectionId {
        &self.request_projection_id
    }

    /// Returns the target generation-system identity.
    #[must_use]
    pub const fn target_generation_system_id(&self) -> &GenerationSystemId {
        &self.target_generation_system_id
    }

    /// Returns the target model artifact-set identity.
    #[must_use]
    pub const fn model_artifact_set_id(&self) -> &ArtifactSetId {
        &self.model_artifact_set_id
    }

    /// Returns the exact semantic model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the selected target model artifact identity.
    #[must_use]
    pub const fn model_artifact_id(&self) -> &ArtifactId {
        &self.model_artifact_id
    }

    /// Returns the stable model-package foundation identity.
    #[must_use]
    pub const fn model_package_foundation_id(&self) -> &ModelPackageFoundationId {
        &self.model_package_foundation_id
    }

    /// Returns the exact model-license control identity.
    #[must_use]
    pub const fn model_license_control_id(&self) -> &ModelLicenseControlId {
        &self.model_license_control_id
    }

    /// Returns the app-reviewed license assessment-policy identity.
    #[must_use]
    pub const fn license_assessment_policy_id(
        &self,
    ) -> &GenerationQualificationLicenseAssessmentPolicyId {
        &self.license_assessment_policy_id
    }

    /// Returns the fixed local-generation permission.
    #[must_use]
    pub const fn permission(&self) -> GenerationQualificationLicensePermissionV1 {
        self.permission
    }

    /// Returns the derived license decision.
    #[must_use]
    pub const fn decision(&self) -> GenerationQualificationLicenseDecisionV1 {
        self.decision
    }

    /// Returns the closed license reason.
    #[must_use]
    pub const fn reason(&self) -> GenerationQualificationLicenseReasonV1 {
        self.reason
    }

    /// Returns the content-derived license-evidence identity.
    #[must_use]
    pub const fn license_evidence_id(&self) -> &GenerationQualificationLicenseEvidenceId {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = GENERATION_QUALIFICATION_LICENSE_EVIDENCE_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.operation_policy_id.digest());
        append_digest(&mut output, self.request_projection_id.digest());
        append_digest(&mut output, self.target_generation_system_id.digest());
        append_digest(&mut output, self.model_artifact_set_id.digest());
        append_digest(&mut output, self.model_package_manifest_id.digest());
        append_digest(&mut output, self.model_artifact_id.digest());
        append_digest(&mut output, self.model_package_foundation_id.digest());
        append_digest(&mut output, self.model_license_control_id.digest());
        append_digest(&mut output, self.license_assessment_policy_id.digest());
        output.push(license_permission_tag(self.permission));
        output.push(license_decision_tag(self.decision));
        output.push(license_reason_tag(self.reason));
        output
    }
}

impl fmt::Debug for GenerationQualificationLicenseEvidenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationLicenseEvidenceV1")
            .field("schema_version", &self.schema_version)
            .field("license_evidence_id", &self.id)
            .field("decision", &self.decision)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

fn validate_relations(
    relations: GenerationQualificationLicenseEvidenceV1Relations<'_>,
) -> Result<(), GenerationQualificationOperationContractError> {
    relations
        .target_generation_system
        .validate_against(relations.target_generation_system_relations)
        .map_err(|_| GenerationQualificationOperationContractError::RelationshipMismatch)?;
    let system_id = relations.target_generation_system.generation_system_id();
    if relations.request_projection.operation_policy_id()
        != relations.operation_policy.operation_policy_id()
        || relations.operation_policy.target_generation_system_id() != system_id
        || relations.request_projection.target_generation_system_id() != system_id
        || relations.operation_policy.required_license_permission()
            != GenerationQualificationLicensePermissionV1::LocalGeneration
    {
        return Err(GenerationQualificationOperationContractError::ScopeMismatch);
    }
    Ok(())
}

fn validate_decision_reason(
    decision: GenerationQualificationLicenseDecisionV1,
    reason: GenerationQualificationLicenseReasonV1,
) -> Result<(), GenerationQualificationOperationContractError> {
    let valid = matches!(
        (decision, reason),
        (
            GenerationQualificationLicenseDecisionV1::LocalUseOnly,
            GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration
        ) | (
            GenerationQualificationLicenseDecisionV1::Rejected,
            GenerationQualificationLicenseReasonV1::ApprovalPolicyDenied
        )
    );
    if valid {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::InvalidLicenseClosure)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LicenseEvidenceWire {
    schema_version: u32,
    operation_policy_id: super::GenerationQualificationOperationPolicyId,
    request_projection_id: super::GenerationQualificationRequestProjectionId,
    target_generation_system_id: GenerationSystemId,
    model_artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    model_artifact_id: ArtifactId,
    model_package_foundation_id: ModelPackageFoundationId,
    model_license_control_id: ModelLicenseControlId,
    license_assessment_policy_id: GenerationQualificationLicenseAssessmentPolicyId,
    permission: GenerationQualificationLicensePermissionV1,
    decision: GenerationQualificationLicenseDecisionV1,
    reason: GenerationQualificationLicenseReasonV1,
}

impl LicenseEvidenceWire {
    fn relationships_equal(&self, expected: &GenerationQualificationLicenseEvidenceV1) -> bool {
        self.operation_policy_id == expected.operation_policy_id
            && self.request_projection_id == expected.request_projection_id
            && self.target_generation_system_id == expected.target_generation_system_id
            && self.model_artifact_set_id == expected.model_artifact_set_id
            && self.model_package_manifest_id == expected.model_package_manifest_id
            && self.model_artifact_id == expected.model_artifact_id
            && self.model_package_foundation_id == expected.model_package_foundation_id
            && self.model_license_control_id == expected.model_license_control_id
            && self.license_assessment_policy_id == expected.license_assessment_policy_id
            && self.permission == expected.permission
            && self.decision == expected.decision
            && self.reason == expected.reason
    }
}

const fn license_decision_tag(value: GenerationQualificationLicenseDecisionV1) -> u8 {
    match value {
        GenerationQualificationLicenseDecisionV1::LocalUseOnly => 0,
        GenerationQualificationLicenseDecisionV1::Rejected => 1,
    }
}

const fn license_reason_tag(value: GenerationQualificationLicenseReasonV1) -> u8 {
    match value {
        GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration => 0,
        GenerationQualificationLicenseReasonV1::ApprovalPolicyDenied => 1,
    }
}

#[cfg(test)]
#[path = "license_evidence/tests.rs"]
mod tests;
