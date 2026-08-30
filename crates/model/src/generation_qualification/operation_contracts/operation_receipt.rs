//! Terminal receipt for one preregistered qualification operation.

use std::fmt;

use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    MAX_FIXED_CANONICAL_BYTES, MAX_FIXED_RECORD_JSON_BYTES, OPERATION_SCHEMA_VERSION,
    append_digest, append_u32, append_u64, validate_canonical_json,
};
use super::{
    GENERATION_QUALIFICATION_OPERATION_RECEIPT_ID_DOMAIN, GenerationQualificationLicenseEvidenceId,
    GenerationQualificationLicenseEvidenceV1, GenerationQualificationOperationContractError,
    GenerationQualificationOperationPolicyId, GenerationQualificationOperationPolicyV1,
    GenerationQualificationOperationReceiptId, GenerationQualificationPlatformEvidenceId,
    GenerationQualificationPlatformEvidenceV1, GenerationQualificationRequestProjectionId,
    GenerationQualificationRequestProjectionV1,
};
use crate::generation_qualification::{
    GenerationAttemptLedgerManifestId, GenerationAttemptLedgerManifestV1,
    GenerationHumanAdjudicationEvidenceManifestId, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationRepeatabilityEvidenceManifestId, GenerationRepeatabilityEvidenceManifestV1,
    GenerationResourceEvidenceManifestId, GenerationResourceEvidenceManifestV1, GenerationSystemId,
};

mod validation;

use validation::{
    validate_deadline, validate_phase_progression, validate_scope, validate_terminal,
};

/// Maximum JSON bytes accepted for one operation receipt.
pub const MAX_GENERATION_QUALIFICATION_OPERATION_RECEIPT_JSON_BYTES: usize =
    MAX_FIXED_RECORD_JSON_BYTES;
/// Maximum canonical identity bytes for one operation receipt.
pub const MAX_GENERATION_QUALIFICATION_OPERATION_RECEIPT_CANONICAL_BYTES: usize =
    MAX_FIXED_CANONICAL_BYTES;

/// Closed terminal outcome of one qualification operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationOperationTerminalStatusV1 {
    /// A policy-directed positive or negative evaluation result was reached.
    Completed,
    /// Cancellation was observed before the deadline.
    Cancelled,
    /// Final elapsed time reached or exceeded the preregistered deadline.
    DeadlineExceeded,
    /// Another operational or mandatory-finalization failure occurred.
    Failed,
}

/// Closed mandatory-finalization outcome for one qualification operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationOperationFinalizationStatusV1 {
    /// No managed runtime, candidate, or judge authority required finalization.
    NotRequired,
    /// Every independently required finalizer passed.
    Passed,
    /// At least one independently required finalizer failed.
    Failed,
}

/// Runner-derived terminal facts not already owned by exact portable evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationQualificationOperationReceiptV1Input {
    /// Checked elapsed nanoseconds from the one monotonic operation start.
    pub elapsed_nanoseconds: u64,
    /// Peak simultaneous candidate or judge attempt scopes.
    pub peak_concurrent_attempts: u32,
    /// Derived terminal status.
    pub terminal_status: GenerationQualificationOperationTerminalStatusV1,
    /// Derived mandatory-finalization status.
    pub finalization_status: GenerationQualificationOperationFinalizationStatusV1,
}

/// Exact records required to construct or decode one operation receipt.
#[derive(Clone, Copy)]
pub struct GenerationQualificationOperationReceiptV1Relations<'a> {
    /// Exact preregistered operation policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Complete preregistered request projection.
    pub request_projection: &'a GenerationQualificationRequestProjectionV1,
    /// Exact pretraffic platform assessment.
    pub platform_evidence: &'a GenerationQualificationPlatformEvidenceV1,
    /// Exact pretraffic license assessment.
    pub license_evidence: &'a GenerationQualificationLicenseEvidenceV1,
    /// Final attempt-ledger manifest.
    pub attempt_ledger_manifest: &'a GenerationAttemptLedgerManifestV1,
    /// Final repeatability manifest.
    pub repeatability_manifest: &'a GenerationRepeatabilityEvidenceManifestV1,
    /// Final resource-evidence manifest.
    pub resource_manifest: &'a GenerationResourceEvidenceManifestV1,
    /// Final human-adjudication manifest.
    pub human_adjudication_manifest: &'a GenerationHumanAdjudicationEvidenceManifestV1,
}

/// Inert durable terminal receipt for one preregistered operation.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationOperationReceiptV1 {
    schema_version: u32,
    target_generation_system_id: GenerationSystemId,
    baseline_generation_system_id: GenerationSystemId,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    request_projection_id: GenerationQualificationRequestProjectionId,
    platform_evidence_id: GenerationQualificationPlatformEvidenceId,
    license_evidence_id: GenerationQualificationLicenseEvidenceId,
    attempt_ledger_manifest_id: GenerationAttemptLedgerManifestId,
    attempt_ledger_root_digest: Digest,
    repeatability_evidence_manifest_id: GenerationRepeatabilityEvidenceManifestId,
    repeatability_evidence_root_digest: Digest,
    resource_evidence_manifest_id: GenerationResourceEvidenceManifestId,
    resource_evidence_root_digest: Digest,
    human_adjudication_evidence_manifest_id: GenerationHumanAdjudicationEvidenceManifestId,
    human_adjudication_evidence_root_digest: Digest,
    elapsed_nanoseconds: u64,
    peak_concurrent_attempts: u32,
    terminal_status: GenerationQualificationOperationTerminalStatusV1,
    finalization_status: GenerationQualificationOperationFinalizationStatusV1,
    #[serde(skip)]
    id: GenerationQualificationOperationReceiptId,
}

impl GenerationQualificationOperationReceiptV1 {
    /// Creates one terminal receipt from exact typed dependencies.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless scope, phase progression, deadline,
    /// concurrency, terminal status, and finalization form one exact closure.
    pub fn new(
        relations: GenerationQualificationOperationReceiptV1Relations<'_>,
        input: GenerationQualificationOperationReceiptV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        Self::build(relations, input, None)
    }

    /// Decodes bounded canonical JSON and rederives every typed relationship.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, noncanonical,
    /// unsupported, substituted, deadline-inconsistent, or invalid terminal input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationQualificationOperationReceiptV1Relations<'_>,
        input: GenerationQualificationOperationReceiptV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_OPERATION_RECEIPT_JSON_BYTES {
            return Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge);
        }
        let wire: OperationReceiptWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationOperationContractError::InvalidEncoding)?;
        if wire.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(GenerationQualificationOperationContractError::UnsupportedSchema);
        }
        let value = Self::build(relations, input, Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    fn build(
        relations: GenerationQualificationOperationReceiptV1Relations<'_>,
        input: GenerationQualificationOperationReceiptV1Input,
        wire: Option<&OperationReceiptWire>,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        validate_scope(relations)?;
        validate_phase_progression(relations, input)?;
        validate_deadline(relations.operation_policy, input)?;
        validate_terminal(relations, input)?;
        let mut value = Self {
            schema_version: OPERATION_SCHEMA_VERSION,
            target_generation_system_id: relations
                .operation_policy
                .target_generation_system_id()
                .clone(),
            baseline_generation_system_id: relations
                .operation_policy
                .baseline_generation_system_id()
                .clone(),
            operation_policy_id: relations.operation_policy.operation_policy_id().clone(),
            request_projection_id: relations.request_projection.request_projection_id().clone(),
            platform_evidence_id: relations.platform_evidence.platform_evidence_id().clone(),
            license_evidence_id: relations.license_evidence.license_evidence_id().clone(),
            attempt_ledger_manifest_id: relations
                .attempt_ledger_manifest
                .attempt_ledger_manifest_id()
                .clone(),
            attempt_ledger_root_digest: relations
                .attempt_ledger_manifest
                .evidence_root_digest()
                .clone(),
            repeatability_evidence_manifest_id: relations
                .repeatability_manifest
                .repeatability_evidence_manifest_id()
                .clone(),
            repeatability_evidence_root_digest: relations
                .repeatability_manifest
                .evidence_root_digest()
                .clone(),
            resource_evidence_manifest_id: relations
                .resource_manifest
                .resource_evidence_manifest_id()
                .clone(),
            resource_evidence_root_digest: relations
                .resource_manifest
                .evidence_root_digest()
                .clone(),
            human_adjudication_evidence_manifest_id: relations
                .human_adjudication_manifest
                .human_adjudication_evidence_manifest_id()
                .clone(),
            human_adjudication_evidence_root_digest: relations
                .human_adjudication_manifest
                .evidence_root_digest()
                .clone(),
            elapsed_nanoseconds: input.elapsed_nanoseconds,
            peak_concurrent_attempts: input.peak_concurrent_attempts,
            terminal_status: input.terminal_status,
            finalization_status: input.finalization_status,
            id: GenerationQualificationOperationReceiptId::from_canonical_bytes(
                b"uninitialized qualification operation receipt",
            ),
        };
        if wire.is_some_and(|wire| !wire.matches(&value)) {
            return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
        }
        let canonical = value.canonical_bytes();
        if canonical.len() > MAX_GENERATION_QUALIFICATION_OPERATION_RECEIPT_CANONICAL_BYTES {
            return Err(GenerationQualificationOperationContractError::CanonicalEncodingTooLarge);
        }
        value.id = GenerationQualificationOperationReceiptId::from_canonical_bytes(&canonical);
        Ok(value)
    }

    /// Revalidates this receipt against fresh exact dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when any dependency or derived terminal fact differs.
    pub fn validate_against(
        &self,
        relations: GenerationQualificationOperationReceiptV1Relations<'_>,
        input: GenerationQualificationOperationReceiptV1Input,
    ) -> Result<(), GenerationQualificationOperationContractError> {
        let expected = Self::new(relations, input)?;
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
    /// Returns the content-derived receipt identity.
    #[must_use]
    pub const fn operation_receipt_id(&self) -> &GenerationQualificationOperationReceiptId {
        &self.id
    }
    /// Returns the exact target generation system.
    #[must_use]
    pub const fn target_generation_system_id(&self) -> &GenerationSystemId {
        &self.target_generation_system_id
    }
    /// Returns the exact baseline generation system.
    #[must_use]
    pub const fn baseline_generation_system_id(&self) -> &GenerationSystemId {
        &self.baseline_generation_system_id
    }
    /// Returns the exact operation policy.
    #[must_use]
    pub const fn operation_policy_id(&self) -> &GenerationQualificationOperationPolicyId {
        &self.operation_policy_id
    }
    /// Returns the complete request projection.
    #[must_use]
    pub const fn request_projection_id(&self) -> &GenerationQualificationRequestProjectionId {
        &self.request_projection_id
    }
    /// Returns the exact platform evidence.
    #[must_use]
    pub const fn platform_evidence_id(&self) -> &GenerationQualificationPlatformEvidenceId {
        &self.platform_evidence_id
    }
    /// Returns the exact license evidence.
    #[must_use]
    pub const fn license_evidence_id(&self) -> &GenerationQualificationLicenseEvidenceId {
        &self.license_evidence_id
    }
    /// Returns the final attempt-ledger identity and root.
    #[must_use]
    pub const fn attempt_ledger_manifest(&self) -> (&GenerationAttemptLedgerManifestId, &Digest) {
        (
            &self.attempt_ledger_manifest_id,
            &self.attempt_ledger_root_digest,
        )
    }
    /// Returns the final repeatability identity and root.
    #[must_use]
    pub const fn repeatability_evidence_manifest(
        &self,
    ) -> (&GenerationRepeatabilityEvidenceManifestId, &Digest) {
        (
            &self.repeatability_evidence_manifest_id,
            &self.repeatability_evidence_root_digest,
        )
    }
    /// Returns the final resource-evidence identity and root.
    #[must_use]
    pub const fn resource_evidence_manifest(
        &self,
    ) -> (&GenerationResourceEvidenceManifestId, &Digest) {
        (
            &self.resource_evidence_manifest_id,
            &self.resource_evidence_root_digest,
        )
    }
    /// Returns the final human-adjudication identity and root.
    #[must_use]
    pub const fn human_adjudication_evidence_manifest(
        &self,
    ) -> (&GenerationHumanAdjudicationEvidenceManifestId, &Digest) {
        (
            &self.human_adjudication_evidence_manifest_id,
            &self.human_adjudication_evidence_root_digest,
        )
    }
    /// Returns checked elapsed operation nanoseconds.
    #[must_use]
    pub const fn elapsed_nanoseconds(&self) -> u64 {
        self.elapsed_nanoseconds
    }
    /// Returns peak simultaneous managed attempt scopes.
    #[must_use]
    pub const fn peak_concurrent_attempts(&self) -> u32 {
        self.peak_concurrent_attempts
    }
    /// Returns the derived terminal status.
    #[must_use]
    pub const fn terminal_status(&self) -> GenerationQualificationOperationTerminalStatusV1 {
        self.terminal_status
    }
    /// Returns the derived finalization status.
    #[must_use]
    pub const fn finalization_status(
        &self,
    ) -> GenerationQualificationOperationFinalizationStatusV1 {
        self.finalization_status
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = GENERATION_QUALIFICATION_OPERATION_RECEIPT_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.target_generation_system_id.digest(),
            self.baseline_generation_system_id.digest(),
            self.operation_policy_id.digest(),
            self.request_projection_id.digest(),
            self.platform_evidence_id.digest(),
            self.license_evidence_id.digest(),
            self.attempt_ledger_manifest_id.digest(),
            &self.attempt_ledger_root_digest,
            self.repeatability_evidence_manifest_id.digest(),
            &self.repeatability_evidence_root_digest,
            self.resource_evidence_manifest_id.digest(),
            &self.resource_evidence_root_digest,
            self.human_adjudication_evidence_manifest_id.digest(),
            &self.human_adjudication_evidence_root_digest,
        ] {
            append_digest(&mut output, digest);
        }
        append_u64(&mut output, self.elapsed_nanoseconds);
        append_u32(&mut output, self.peak_concurrent_attempts);
        output.push(terminal_tag(self.terminal_status));
        output.push(finalization_tag(self.finalization_status));
        output
    }
}

impl fmt::Debug for GenerationQualificationOperationReceiptV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationOperationReceiptV1")
            .field("operation_receipt_id", &self.id)
            .field("elapsed_nanoseconds", &self.elapsed_nanoseconds)
            .field("peak_concurrent_attempts", &self.peak_concurrent_attempts)
            .field("terminal_status", &self.terminal_status)
            .field("finalization_status", &self.finalization_status)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationReceiptWire {
    schema_version: u32,
    target_generation_system_id: GenerationSystemId,
    baseline_generation_system_id: GenerationSystemId,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    request_projection_id: GenerationQualificationRequestProjectionId,
    platform_evidence_id: GenerationQualificationPlatformEvidenceId,
    license_evidence_id: GenerationQualificationLicenseEvidenceId,
    attempt_ledger_manifest_id: GenerationAttemptLedgerManifestId,
    attempt_ledger_root_digest: Digest,
    repeatability_evidence_manifest_id: GenerationRepeatabilityEvidenceManifestId,
    repeatability_evidence_root_digest: Digest,
    resource_evidence_manifest_id: GenerationResourceEvidenceManifestId,
    resource_evidence_root_digest: Digest,
    human_adjudication_evidence_manifest_id: GenerationHumanAdjudicationEvidenceManifestId,
    human_adjudication_evidence_root_digest: Digest,
    elapsed_nanoseconds: u64,
    peak_concurrent_attempts: u32,
    terminal_status: GenerationQualificationOperationTerminalStatusV1,
    finalization_status: GenerationQualificationOperationFinalizationStatusV1,
}

impl OperationReceiptWire {
    fn matches(&self, value: &GenerationQualificationOperationReceiptV1) -> bool {
        self.schema_version == value.schema_version
            && self.target_generation_system_id == value.target_generation_system_id
            && self.baseline_generation_system_id == value.baseline_generation_system_id
            && self.operation_policy_id == value.operation_policy_id
            && self.request_projection_id == value.request_projection_id
            && self.platform_evidence_id == value.platform_evidence_id
            && self.license_evidence_id == value.license_evidence_id
            && self.attempt_ledger_manifest_id == value.attempt_ledger_manifest_id
            && self.attempt_ledger_root_digest == value.attempt_ledger_root_digest
            && self.repeatability_evidence_manifest_id == value.repeatability_evidence_manifest_id
            && self.repeatability_evidence_root_digest == value.repeatability_evidence_root_digest
            && self.resource_evidence_manifest_id == value.resource_evidence_manifest_id
            && self.resource_evidence_root_digest == value.resource_evidence_root_digest
            && self.human_adjudication_evidence_manifest_id
                == value.human_adjudication_evidence_manifest_id
            && self.human_adjudication_evidence_root_digest
                == value.human_adjudication_evidence_root_digest
            && self.elapsed_nanoseconds == value.elapsed_nanoseconds
            && self.peak_concurrent_attempts == value.peak_concurrent_attempts
            && self.terminal_status == value.terminal_status
            && self.finalization_status == value.finalization_status
    }
}

const fn terminal_tag(value: GenerationQualificationOperationTerminalStatusV1) -> u8 {
    match value {
        GenerationQualificationOperationTerminalStatusV1::Completed => 0,
        GenerationQualificationOperationTerminalStatusV1::Cancelled => 1,
        GenerationQualificationOperationTerminalStatusV1::DeadlineExceeded => 2,
        GenerationQualificationOperationTerminalStatusV1::Failed => 3,
    }
}

const fn finalization_tag(value: GenerationQualificationOperationFinalizationStatusV1) -> u8 {
    match value {
        GenerationQualificationOperationFinalizationStatusV1::NotRequired => 0,
        GenerationQualificationOperationFinalizationStatusV1::Passed => 1,
        GenerationQualificationOperationFinalizationStatusV1::Failed => 2,
    }
}

#[cfg(test)]
#[path = "operation_receipt/tests.rs"]
mod tests;
