//! Portable pretraffic platform assessment evidence.

use std::fmt;

use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    MAX_FIXED_CANONICAL_BYTES, MAX_FIXED_RECORD_JSON_BYTES, OPERATION_SCHEMA_VERSION,
    append_digest, append_u32, validate_canonical_json,
};
use super::{
    GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_ID_DOMAIN,
    GenerationQualificationOperationContractError, GenerationQualificationOperationPolicyV1,
    GenerationQualificationPlatformAssessmentPolicyId, GenerationQualificationPlatformEvidenceId,
    GenerationQualificationRequestProjectionV1,
};
use crate::{
    FrozenExternalComponentSetId, GenerationSystemId, GenerationSystemRecordV1,
    GenerationSystemRecordV1Relations, ManagedGenerationPathId, RuntimeAbi, RuntimeAdmissionJoinId,
    RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget,
};

/// Maximum JSON bytes accepted for one platform-evidence record.
pub const MAX_GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_JSON_BYTES: usize =
    MAX_FIXED_RECORD_JSON_BYTES;
/// Maximum canonical identity bytes for one platform-evidence record.
pub const MAX_GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_CANONICAL_BYTES: usize =
    MAX_FIXED_CANONICAL_BYTES;

/// Closed result of the pretraffic platform assessment.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationPlatformStatusV1 {
    /// The exact reviewed managed Linux native CPU profile is supported.
    Supported,
    /// The exact target or reviewed platform policy rejects this operation.
    Rejected,
}

/// Closed reason for a platform assessment result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationPlatformReasonV1 {
    /// The reviewed managed Linux native CPU profile matched exactly.
    ReviewedManagedLinuxNativeCpu,
    /// The operating-system family is unsupported.
    UnsupportedOperatingSystem,
    /// The instruction-set architecture is unsupported.
    UnsupportedArchitecture,
    /// The application binary interface is unsupported.
    UnsupportedAbi,
    /// The execution class is unsupported.
    UnsupportedExecutionClass,
    /// The hardware envelope is unsupported.
    UnsupportedHardwareEnvelope,
    /// The reviewed platform assessment policy denied the operation.
    AssessmentPolicyDenied,
}

/// Exact typed records against which platform evidence is derived and revalidated.
#[derive(Clone, Copy)]
pub struct GenerationQualificationPlatformEvidenceV1Relations<'a> {
    /// Preregistered operation policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Complete preregistered request projection.
    pub request_projection: &'a GenerationQualificationRequestProjectionV1,
    /// Target generation system selected by the operation policy.
    pub target_generation_system: &'a GenerationSystemRecordV1,
    /// Complete typed closure of the target generation system.
    pub target_generation_system_relations: GenerationSystemRecordV1Relations<'a>,
}

/// Assessment result supplied by the app-owned platform compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationQualificationPlatformEvidenceV1Input {
    /// Derived assessment status.
    pub status: GenerationQualificationPlatformStatusV1,
    /// Closed reason consistent with `status`.
    pub reason: GenerationQualificationPlatformReasonV1,
}

/// Portable binding of the exact pretraffic platform assessment.
///
/// This record is inert. It does not prove that a live worker ran and grants no
/// launch, generation, qualification, or live-use authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationPlatformEvidenceV1 {
    schema_version: u32,
    operation_policy_id: super::GenerationQualificationOperationPolicyId,
    request_projection_id: super::GenerationQualificationRequestProjectionId,
    target_generation_system_id: GenerationSystemId,
    runtime_target: RuntimeTarget,
    operating_system_digest: Digest,
    architecture_digest: Digest,
    execution_class_digest: Digest,
    hardware_envelope_digest: Digest,
    runtime_admission_join_id: RuntimeAdmissionJoinId,
    managed_generation_path_id: ManagedGenerationPathId,
    frozen_external_component_set_id: FrozenExternalComponentSetId,
    platform_assessment_policy_id: GenerationQualificationPlatformAssessmentPolicyId,
    status: GenerationQualificationPlatformStatusV1,
    reason: GenerationQualificationPlatformReasonV1,
    #[serde(skip)]
    id: GenerationQualificationPlatformEvidenceId,
}

impl GenerationQualificationPlatformEvidenceV1 {
    /// Derives platform evidence from exact preregistered and target-system records.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation scope, target-system closure, or
    /// status and reason pair is inconsistent.
    pub fn new(
        relations: GenerationQualificationPlatformEvidenceV1Relations<'_>,
        input: GenerationQualificationPlatformEvidenceV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        Self::from_relations(OPERATION_SCHEMA_VERSION, relations, input)
    }

    fn from_relations(
        schema_version: u32,
        relations: GenerationQualificationPlatformEvidenceV1Relations<'_>,
        input: GenerationQualificationPlatformEvidenceV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        if schema_version != OPERATION_SCHEMA_VERSION {
            return Err(GenerationQualificationOperationContractError::UnsupportedSchema);
        }
        validate_relations(relations)?;
        let runtime_target = relations
            .target_generation_system_relations
            .runtime_package_manifest
            .target();
        validate_status_reason(input.status, input.reason, runtime_target)?;

        let target = relations.target_generation_system;
        let mut record = Self {
            schema_version,
            operation_policy_id: relations.operation_policy.operation_policy_id().clone(),
            request_projection_id: relations.request_projection.request_projection_id().clone(),
            target_generation_system_id: target.generation_system_id().clone(),
            runtime_target,
            operating_system_digest: target.operating_system_digest().clone(),
            architecture_digest: target.architecture_digest().clone(),
            execution_class_digest: target.execution_class_digest().clone(),
            hardware_envelope_digest: target.hardware_envelope_digest().clone(),
            runtime_admission_join_id: target.runtime_admission_join_id().clone(),
            managed_generation_path_id: target.managed_generation_path_id().clone(),
            frozen_external_component_set_id: target.frozen_external_component_set_id().clone(),
            platform_assessment_policy_id: relations
                .operation_policy
                .platform_assessment_policy_id()
                .clone(),
            status: input.status,
            reason: input.reason,
            id: GenerationQualificationPlatformEvidenceId::from_canonical_bytes(
                b"uninitialized platform evidence",
            ),
        };
        let canonical = record.canonical_bytes();
        if canonical.len() > MAX_GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_CANONICAL_BYTES {
            return Err(GenerationQualificationOperationContractError::CanonicalEncodingTooLarge);
        }
        record.id = GenerationQualificationPlatformEvidenceId::from_canonical_bytes(&canonical);
        Ok(record)
    }

    /// Parses bounded canonical JSON and rechecks the complete typed closure.
    /// `expected_input` must come from the independent app-owned assessment;
    /// neither status nor reason is trusted from the serialized record.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized, malformed, noncanonical, unsupported, or
    /// relationship-inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationQualificationPlatformEvidenceV1Relations<'_>,
        expected_input: GenerationQualificationPlatformEvidenceV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_JSON_BYTES {
            return Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge);
        }
        let wire: PlatformEvidenceWire = serde_json::from_slice(bytes)
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
    /// Returns an error when any dependency, system fact, assessment result, or
    /// derived identity differs.
    pub fn validate_against(
        &self,
        relations: GenerationQualificationPlatformEvidenceV1Relations<'_>,
        expected_input: GenerationQualificationPlatformEvidenceV1Input,
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

    /// Returns the assessed native runtime target.
    #[must_use]
    pub const fn runtime_target(&self) -> RuntimeTarget {
        self.runtime_target
    }

    /// Returns the target system's operating-system digest.
    #[must_use]
    pub const fn operating_system_digest(&self) -> &Digest {
        &self.operating_system_digest
    }

    /// Returns the target system's architecture digest.
    #[must_use]
    pub const fn architecture_digest(&self) -> &Digest {
        &self.architecture_digest
    }

    /// Returns the target system's execution-class digest.
    #[must_use]
    pub const fn execution_class_digest(&self) -> &Digest {
        &self.execution_class_digest
    }

    /// Returns the target system's hardware-envelope digest.
    #[must_use]
    pub const fn hardware_envelope_digest(&self) -> &Digest {
        &self.hardware_envelope_digest
    }

    /// Returns the target system's runtime-admission join identity.
    #[must_use]
    pub const fn runtime_admission_join_id(&self) -> &RuntimeAdmissionJoinId {
        &self.runtime_admission_join_id
    }

    /// Returns the target system's managed generation-path identity.
    #[must_use]
    pub const fn managed_generation_path_id(&self) -> &ManagedGenerationPathId {
        &self.managed_generation_path_id
    }

    /// Returns the target system's frozen external-component set identity.
    #[must_use]
    pub const fn frozen_external_component_set_id(&self) -> &FrozenExternalComponentSetId {
        &self.frozen_external_component_set_id
    }

    /// Returns the app-reviewed platform assessment-policy identity.
    #[must_use]
    pub const fn platform_assessment_policy_id(
        &self,
    ) -> &GenerationQualificationPlatformAssessmentPolicyId {
        &self.platform_assessment_policy_id
    }

    /// Returns the derived platform status.
    #[must_use]
    pub const fn status(&self) -> GenerationQualificationPlatformStatusV1 {
        self.status
    }

    /// Returns the closed platform reason.
    #[must_use]
    pub const fn reason(&self) -> GenerationQualificationPlatformReasonV1 {
        self.reason
    }

    /// Returns the content-derived platform-evidence identity.
    #[must_use]
    pub const fn platform_evidence_id(&self) -> &GenerationQualificationPlatformEvidenceId {
        &self.id
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        append_digest(&mut output, self.operation_policy_id.digest());
        append_digest(&mut output, self.request_projection_id.digest());
        append_digest(&mut output, self.target_generation_system_id.digest());
        output.push(operating_system_tag(self.runtime_target.operating_system()));
        output.push(architecture_tag(self.runtime_target.architecture()));
        output.push(abi_tag(self.runtime_target.abi()));
        append_digest(&mut output, &self.operating_system_digest);
        append_digest(&mut output, &self.architecture_digest);
        append_digest(&mut output, &self.execution_class_digest);
        append_digest(&mut output, &self.hardware_envelope_digest);
        append_digest(&mut output, self.runtime_admission_join_id.digest());
        append_digest(&mut output, self.managed_generation_path_id.digest());
        append_digest(&mut output, self.frozen_external_component_set_id.digest());
        append_digest(&mut output, self.platform_assessment_policy_id.digest());
        output.push(platform_status_tag(self.status));
        output.push(platform_reason_tag(self.reason));
        output
    }
}

impl fmt::Debug for GenerationQualificationPlatformEvidenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationPlatformEvidenceV1")
            .field("schema_version", &self.schema_version)
            .field("platform_evidence_id", &self.id)
            .field("status", &self.status)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

fn validate_relations(
    relations: GenerationQualificationPlatformEvidenceV1Relations<'_>,
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
    {
        return Err(GenerationQualificationOperationContractError::ScopeMismatch);
    }
    Ok(())
}

fn validate_status_reason(
    status: GenerationQualificationPlatformStatusV1,
    reason: GenerationQualificationPlatformReasonV1,
    target: RuntimeTarget,
) -> Result<(), GenerationQualificationOperationContractError> {
    let reviewed_target = target.operating_system() == RuntimeOperatingSystem::Linux
        && target.architecture() == RuntimeArchitecture::X86_64
        && target.abi() == RuntimeAbi::LinuxGnuLibc;
    let valid = match (status, reason) {
        (
            GenerationQualificationPlatformStatusV1::Supported,
            GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu,
        )
        | (
            GenerationQualificationPlatformStatusV1::Rejected,
            GenerationQualificationPlatformReasonV1::UnsupportedExecutionClass
            | GenerationQualificationPlatformReasonV1::UnsupportedHardwareEnvelope
            | GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied,
        ) => reviewed_target,
        (
            GenerationQualificationPlatformStatusV1::Rejected,
            GenerationQualificationPlatformReasonV1::UnsupportedOperatingSystem,
        ) => target.operating_system() != RuntimeOperatingSystem::Linux,
        (
            GenerationQualificationPlatformStatusV1::Rejected,
            GenerationQualificationPlatformReasonV1::UnsupportedArchitecture,
        ) => {
            target.operating_system() == RuntimeOperatingSystem::Linux
                && target.architecture() != RuntimeArchitecture::X86_64
        }
        (
            GenerationQualificationPlatformStatusV1::Rejected,
            GenerationQualificationPlatformReasonV1::UnsupportedAbi,
        ) => {
            target.operating_system() == RuntimeOperatingSystem::Linux
                && target.architecture() == RuntimeArchitecture::X86_64
                && target.abi() != RuntimeAbi::LinuxGnuLibc
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::InvalidPlatformClosure)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlatformEvidenceWire {
    schema_version: u32,
    operation_policy_id: super::GenerationQualificationOperationPolicyId,
    request_projection_id: super::GenerationQualificationRequestProjectionId,
    target_generation_system_id: GenerationSystemId,
    runtime_target: RuntimeTarget,
    operating_system_digest: Digest,
    architecture_digest: Digest,
    execution_class_digest: Digest,
    hardware_envelope_digest: Digest,
    runtime_admission_join_id: RuntimeAdmissionJoinId,
    managed_generation_path_id: ManagedGenerationPathId,
    frozen_external_component_set_id: FrozenExternalComponentSetId,
    platform_assessment_policy_id: GenerationQualificationPlatformAssessmentPolicyId,
    status: GenerationQualificationPlatformStatusV1,
    reason: GenerationQualificationPlatformReasonV1,
}

impl PlatformEvidenceWire {
    fn relationships_equal(&self, expected: &GenerationQualificationPlatformEvidenceV1) -> bool {
        self.operation_policy_id == expected.operation_policy_id
            && self.request_projection_id == expected.request_projection_id
            && self.target_generation_system_id == expected.target_generation_system_id
            && self.runtime_target == expected.runtime_target
            && self.operating_system_digest == expected.operating_system_digest
            && self.architecture_digest == expected.architecture_digest
            && self.execution_class_digest == expected.execution_class_digest
            && self.hardware_envelope_digest == expected.hardware_envelope_digest
            && self.runtime_admission_join_id == expected.runtime_admission_join_id
            && self.managed_generation_path_id == expected.managed_generation_path_id
            && self.frozen_external_component_set_id == expected.frozen_external_component_set_id
            && self.platform_assessment_policy_id == expected.platform_assessment_policy_id
            && self.status == expected.status
            && self.reason == expected.reason
    }
}

const fn platform_status_tag(value: GenerationQualificationPlatformStatusV1) -> u8 {
    match value {
        GenerationQualificationPlatformStatusV1::Supported => 0,
        GenerationQualificationPlatformStatusV1::Rejected => 1,
    }
}

const fn platform_reason_tag(value: GenerationQualificationPlatformReasonV1) -> u8 {
    match value {
        GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu => 0,
        GenerationQualificationPlatformReasonV1::UnsupportedOperatingSystem => 1,
        GenerationQualificationPlatformReasonV1::UnsupportedArchitecture => 2,
        GenerationQualificationPlatformReasonV1::UnsupportedAbi => 3,
        GenerationQualificationPlatformReasonV1::UnsupportedExecutionClass => 4,
        GenerationQualificationPlatformReasonV1::UnsupportedHardwareEnvelope => 5,
        GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied => 6,
    }
}

const fn operating_system_tag(value: RuntimeOperatingSystem) -> u8 {
    match value {
        RuntimeOperatingSystem::Windows => 0,
        RuntimeOperatingSystem::MacOs => 1,
        RuntimeOperatingSystem::Linux => 2,
    }
}

const fn architecture_tag(value: RuntimeArchitecture) -> u8 {
    match value {
        RuntimeArchitecture::X86_64 => 0,
        RuntimeArchitecture::Aarch64 => 1,
    }
}

const fn abi_tag(value: RuntimeAbi) -> u8 {
    match value {
        RuntimeAbi::WindowsMsvc => 0,
        RuntimeAbi::WindowsGnu => 1,
        RuntimeAbi::LinuxGnuLibc => 2,
        RuntimeAbi::LinuxMusl => 3,
        RuntimeAbi::Darwin => 4,
    }
}

#[cfg(test)]
#[path = "platform_evidence/tests.rs"]
mod tests;
