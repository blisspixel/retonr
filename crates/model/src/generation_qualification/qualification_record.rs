//! Compact inert result of one fully revalidated generation qualification.

use std::fmt;

use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    CandidateGenerationAttemptOutcomeV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationLicenseDecisionV1, GenerationQualificationLicenseEvidenceId,
    GenerationQualificationLicenseEvidenceV1Input,
    GenerationQualificationLicenseEvidenceV1Relations,
    GenerationQualificationOperationContractError,
    GenerationQualificationOperationFinalizationStatusV1, GenerationQualificationOperationPolicyId,
    GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, GenerationQualificationOperationReceiptId,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseEvidenceError,
    GenerationQualificationPhaseStatusV1, GenerationQualificationPlatformEvidenceId,
    GenerationQualificationPlatformEvidenceV1Input,
    GenerationQualificationPlatformEvidenceV1Relations, GenerationQualificationPlatformStatusV1,
    GenerationQualificationRequestProjectionEntryV1Input,
    GenerationQualificationRequestProjectionId,
    GenerationQualificationRequestProjectionV1Relations, GenerationRepeatabilityEvidenceManifestV1,
    GenerationRepeatabilityEvidenceManifestV1Relations, GenerationRepeatabilityResultRecordV1,
    GenerationRepeatabilityResultRecordV1Relations, GenerationRepeatabilityTerminalStageV1,
    GenerationResourceEvidenceManifestV1, GenerationResourceEvidenceManifestV1Relations,
    GenerationSystemId,
};

mod codec;
mod validation;

/// Identity domain for one compact generation-qualification record.
pub const GENERATION_QUALIFICATION_RECORD_ID_DOMAIN: &[u8] =
    b"retonr:generation-qualification-record:v1\0";
/// Maximum JSON bytes accepted for one compact qualification record.
pub const MAX_GENERATION_QUALIFICATION_RECORD_JSON_BYTES: usize = 16_384;
/// Maximum canonical identity bytes for one compact qualification record.
pub const MAX_GENERATION_QUALIFICATION_RECORD_CANONICAL_BYTES: usize = 4_096;

/// Content-derived identity of one compact generation-qualification record.
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct GenerationQualificationId(Digest);

impl GenerationQualificationId {
    /// Returns the digest that defines this inert portable identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }

    fn from_canonical_bytes(bytes: &[u8]) -> Self {
        Self(Digest::sha256(bytes))
    }
}

/// Closed structural result of one completed qualification operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationStatusV1 {
    /// Every required portable evidence phase passed.
    Qualified,
    /// A predeclared gate or evidence phase rejected the target.
    Rejected,
}

/// Independently held closure required to construct or decode one record.
///
/// The deliberately explicit bundle prevents serialized fields from becoming
/// their own expected policy, assessment, runner, or phase facts.
#[derive(Clone, Copy)]
pub struct GenerationQualificationRecordV1Relations<'a> {
    /// Exact terminal operation receipt.
    pub operation_receipt: &'a GenerationQualificationOperationReceiptV1,
    /// Direct dependencies named by the terminal receipt.
    pub operation_receipt_relations: GenerationQualificationOperationReceiptV1Relations<'a>,
    /// Independently retained runner-derived terminal facts.
    pub operation_receipt_input: GenerationQualificationOperationReceiptV1Input,
    /// Complete frozen-policy relationship closure.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'a>,
    /// Independently reviewed frozen-policy input.
    pub operation_policy_input: &'a GenerationQualificationOperationPolicyV1Input,
    /// Complete request-projection relationship closure.
    pub request_projection_relations: GenerationQualificationRequestProjectionV1Relations<'a>,
    /// Independently compiled request entries in exact plan order.
    pub request_projection_entry_inputs:
        &'a [GenerationQualificationRequestProjectionEntryV1Input],
    /// Complete platform-evidence relationship closure.
    pub platform_evidence_relations: GenerationQualificationPlatformEvidenceV1Relations<'a>,
    /// Fresh app-derived platform assessment.
    pub platform_evidence_input: GenerationQualificationPlatformEvidenceV1Input,
    /// Complete license-evidence relationship closure.
    pub license_evidence_relations: GenerationQualificationLicenseEvidenceV1Relations<'a>,
    /// Fresh app-derived license assessment.
    pub license_evidence_input: GenerationQualificationLicenseEvidenceV1Input,
    /// Complete target attempt-ledger closure.
    pub attempt_ledger_relations: GenerationAttemptLedgerManifestV1Relations<'a>,
    /// Complete ordered repeatability-manifest closure.
    pub repeatability_manifest_relations: GenerationRepeatabilityEvidenceManifestV1Relations<'a>,
    /// One exact typed closure for every ordered repeatability result.
    pub repeatability_result_relations: &'a [GenerationRepeatabilityResultRecordV1Relations<'a>],
    /// Complete resource-evidence manifest closure.
    pub resource_manifest_relations: GenerationResourceEvidenceManifestV1Relations<'a>,
    /// Complete human-adjudication manifest closure.
    pub human_adjudication_manifest_relations:
        GenerationHumanAdjudicationEvidenceManifestV1Relations<'a>,
}

/// Compact inert qualification result for one target and one exact baseline.
///
/// The record contains identities only. It grants no activation, launch,
/// generation, qualification-verification, or live-use authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationRecordV1 {
    schema_version: u32,
    target_generation_system_id: GenerationSystemId,
    baseline_generation_system_id: GenerationSystemId,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    request_projection_id: GenerationQualificationRequestProjectionId,
    platform_evidence_id: GenerationQualificationPlatformEvidenceId,
    license_evidence_id: GenerationQualificationLicenseEvidenceId,
    operation_receipt_id: GenerationQualificationOperationReceiptId,
    status: GenerationQualificationStatusV1,
    #[serde(skip)]
    id: GenerationQualificationId,
}

impl GenerationQualificationRecordV1 {
    /// Derives one compact record after recursively checking every portable fact.
    ///
    /// This public constructor produces inert data only. The eval-owned verifier
    /// must additionally consume current opaque authorities before retaining the
    /// result as verified qualification.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for stale, incomplete, substituted,
    /// nonterminal, operationally aborted, or policy-inconsistent evidence.
    pub fn new(
        relations: &GenerationQualificationRecordV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationRecordV1Error> {
        Self::build(relations, None)
    }

    /// Revalidates this record against fresh independently held evidence.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless every dependency and derived field
    /// reconstructs this exact record.
    pub fn validate_against(
        &self,
        relations: &GenerationQualificationRecordV1Relations<'_>,
    ) -> Result<(), GenerationQualificationRecordV1Error> {
        if &Self::new(relations)? == self {
            Ok(())
        } else {
            Err(GenerationQualificationRecordV1Error::RelationshipMismatch)
        }
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact target generation system.
    #[must_use]
    pub const fn target_generation_system_id(&self) -> &GenerationSystemId {
        &self.target_generation_system_id
    }
    /// Returns the sole exact baseline generation system.
    #[must_use]
    pub const fn baseline_generation_system_id(&self) -> &GenerationSystemId {
        &self.baseline_generation_system_id
    }
    /// Returns the frozen operation-policy identity.
    #[must_use]
    pub const fn operation_policy_id(&self) -> &GenerationQualificationOperationPolicyId {
        &self.operation_policy_id
    }
    /// Returns the complete request-projection identity.
    #[must_use]
    pub const fn request_projection_id(&self) -> &GenerationQualificationRequestProjectionId {
        &self.request_projection_id
    }
    /// Returns the exact platform-evidence identity.
    #[must_use]
    pub const fn platform_evidence_id(&self) -> &GenerationQualificationPlatformEvidenceId {
        &self.platform_evidence_id
    }
    /// Returns the exact license-evidence identity.
    #[must_use]
    pub const fn license_evidence_id(&self) -> &GenerationQualificationLicenseEvidenceId {
        &self.license_evidence_id
    }
    /// Returns the exact terminal operation-receipt identity.
    #[must_use]
    pub const fn operation_receipt_id(&self) -> &GenerationQualificationOperationReceiptId {
        &self.operation_receipt_id
    }
    /// Returns the internally derived structural qualification result.
    #[must_use]
    pub const fn status(&self) -> GenerationQualificationStatusV1 {
        self.status
    }
    /// Returns the content-derived compact qualification identity.
    #[must_use]
    pub const fn generation_qualification_id(&self) -> &GenerationQualificationId {
        &self.id
    }
}

impl fmt::Debug for GenerationQualificationRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationRecordV1")
            .field("generation_qualification_id", &self.id)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

/// Content-free failure while deriving a compact qualification record.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationRecordV1Error {
    /// Encoded JSON exceeds its fixed predecode ceiling.
    #[error("generation qualification record exceeds its limit")]
    EncodedRecordTooLarge,
    /// JSON is malformed, duplicated, trailing, or contains an unknown field.
    #[error("generation qualification record encoding is invalid")]
    InvalidEncoding,
    /// JSON differs from the one canonical encoding.
    #[error("generation qualification record encoding is noncanonical")]
    NonCanonicalEncoding,
    /// The compact record schema is unsupported.
    #[error("generation qualification record schema is unsupported")]
    UnsupportedSchema,
    /// Canonical identity bytes exceed their fixed ceiling.
    #[error("generation qualification record identity exceeds its limit")]
    CanonicalEncodingTooLarge,
    /// Preregistered operation evidence failed recursive validation.
    #[error("generation qualification operation evidence is invalid")]
    InvalidOperationEvidence(#[source] GenerationQualificationOperationContractError),
    /// Phase evidence failed recursive validation.
    #[error("generation qualification phase evidence is invalid")]
    InvalidPhaseEvidence(#[source] GenerationQualificationPhaseEvidenceError),
    /// A serialized or supplied relationship was substituted.
    #[error("generation qualification record relationship does not match")]
    RelationshipMismatch,
    /// The receipt represents an operational outcome that cannot yield a record.
    #[error("generation qualification operation cannot produce a qualification record")]
    IneligibleOperation,
    /// Completed evidence does not end at an actual policy-directed decision.
    #[error("generation qualification decision closure is invalid")]
    InvalidDecisionClosure,
}

#[cfg(test)]
#[path = "qualification_record/tests.rs"]
mod tests;
