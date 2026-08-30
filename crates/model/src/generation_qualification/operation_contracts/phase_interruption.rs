//! Canonical inert record for an interrupted qualification phase.

use std::fmt;

use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::{
    MAX_FIXED_CANONICAL_BYTES, MAX_FIXED_RECORD_JSON_BYTES, OPERATION_SCHEMA_VERSION,
    append_digest, append_u32, validate_canonical_json,
};
use super::{
    GENERATION_QUALIFICATION_PHASE_INTERRUPTION_RECORD_ID_DOMAIN,
    GenerationQualificationOperationContractError, GenerationQualificationOperationPolicyId,
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, GenerationQualificationOperationReceiptId,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationPhaseInterruptionRecordId,
};
use crate::generation_qualification::{
    GenerationQualificationPlanId, GenerationSuiteManifestId, GenerationSystemId,
    PlannedCandidateAttemptId,
};

mod validation;

use validation::{phase_policy_digest, validate_relations};

/// Maximum JSON bytes accepted for one phase-interruption record.
pub const MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_JSON_BYTES: usize =
    MAX_FIXED_RECORD_JSON_BYTES;
/// Maximum canonical identity bytes for one phase-interruption record.
pub const MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_CANONICAL_BYTES: usize =
    MAX_FIXED_CANONICAL_BYTES;

/// Closed qualification phase in which an operation was interrupted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationInterruptedPhaseV1 {
    /// Ordered candidate and baseline attempt execution and ledger construction.
    AttemptLedger,
    /// Repetition-level deterministic and managed-judge evidence construction.
    Repeatability,
    /// Strict target resource-observation evidence construction.
    ResourceEvidence,
    /// Human-review evidence construction.
    HumanAdjudication,
}

/// Closed checkpoint at which a qualification phase was interrupted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationPhaseCheckpointV1 {
    /// Before any work in the named phase began.
    BeforePhase,
    /// While acquiring one exact phase evidence item.
    EvidenceAcquisition,
    /// While compiling acquired facts into portable phase evidence.
    EvidenceCompilation,
    /// While compiling the phase manifest itself.
    ManifestCompilation,
    /// During the final fresh authority validation after phase closure.
    FinalAuthorityRevalidation,
    /// During mandatory independent cleanup or package finalization.
    MandatoryFinalization,
}

/// Closed primary reason why a qualification phase was interrupted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationPhaseInterruptionReasonV1 {
    /// Cancellation was observed strictly before the original deadline.
    Cancelled,
    /// The original absolute operation deadline was reached or exceeded.
    DeadlineExceeded,
    /// A fresh exact authority or package relationship changed.
    AuthorityDrift,
    /// A required strict resource observation was absent.
    RequiredObservationMissing,
    /// A required strict resource observation was malformed or inconsistent.
    RequiredObservationInvalid,
    /// Checked resource measurement arithmetic overflowed.
    MeasurementOverflow,
    /// Acquired facts could not be compiled into canonical evidence.
    EvidenceCompilationFailed,
    /// Mandatory independent cleanup failed.
    CleanupFailed,
}

#[derive(Clone, Copy, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum GenerationQualificationPhaseInterruptionRecordKindV1 {
    PhaseInterruption,
}

/// Runner-owned interruption facts that are not derived from exact closures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationQualificationPhaseInterruptionRecordV1Input {
    /// Exact phase in which the primary interruption was observed.
    pub phase: GenerationQualificationInterruptedPhaseV1,
    /// Exact boundary within that phase.
    pub checkpoint: GenerationQualificationPhaseCheckpointV1,
    /// Exact plan attempt when the checkpoint is attempt-specific.
    pub planned_attempt_id: Option<PlannedCandidateAttemptId>,
    /// Primary terminal reason after deadline and cancellation precedence.
    pub reason: GenerationQualificationPhaseInterruptionReasonV1,
}

/// Exact recursive closures needed to construct or decode an interruption record.
#[derive(Clone, Copy)]
pub struct GenerationQualificationPhaseInterruptionRecordV1Relations<'a> {
    /// Exact preregistered operation policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Complete policy dependency closure used for fresh recursive validation.
    pub operation_policy_relations: GenerationQualificationOperationPolicyV1Relations<'a>,
    /// Independently retained exact operation-policy input.
    pub operation_policy_input: &'a GenerationQualificationOperationPolicyV1Input,
    /// Exact noncompleted terminal operation receipt.
    pub operation_receipt: &'a GenerationQualificationOperationReceiptV1,
    /// Complete receipt dependency closure used for fresh recursive validation.
    pub operation_receipt_relations: GenerationQualificationOperationReceiptV1Relations<'a>,
    /// Independently retained runner-derived terminal facts.
    pub operation_receipt_input: GenerationQualificationOperationReceiptV1Input,
}

/// Canonical inert companion evidence for one interrupted qualification phase.
///
/// This record grants no execution, persistence, activation, qualification, or
/// live-use authority. It is not a phase-manifest item and must never be used as
/// one. Its only meaning comes from recursive validation against the exact
/// operation policy, terminal receipt, and complete frozen plan order.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationQualificationPhaseInterruptionRecordV1 {
    schema_version: u32,
    record_kind: GenerationQualificationPhaseInterruptionRecordKindV1,
    operation_receipt_id: GenerationQualificationOperationReceiptId,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    target_generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    phase_policy_digest: Digest,
    phase: GenerationQualificationInterruptedPhaseV1,
    checkpoint: GenerationQualificationPhaseCheckpointV1,
    planned_attempt_id: Option<PlannedCandidateAttemptId>,
    reason: GenerationQualificationPhaseInterruptionReasonV1,
    #[serde(skip)]
    id: GenerationQualificationPhaseInterruptionRecordId,
}

impl GenerationQualificationPhaseInterruptionRecordV1 {
    /// Constructs a phase interruption from exact recursive terminal closures.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for a substituted closure, completed
    /// receipt, foreign attempt, or incompatible phase, checkpoint, reason,
    /// manifest progression, concurrency, or finalization fact.
    pub fn new(
        relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
        input: GenerationQualificationPhaseInterruptionRecordV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        Self::build(relations, input, None)
    }

    /// Decodes bounded canonical JSON against independent recursive closures.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, duplicated,
    /// trailing, noncanonical, unsupported, substituted, or incompatible input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
        expected_input: &GenerationQualificationPhaseInterruptionRecordV1Input,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_JSON_BYTES {
            return Err(GenerationQualificationOperationContractError::EncodedRecordTooLarge);
        }
        let wire: PhaseInterruptionWire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationOperationContractError::InvalidEncoding)?;
        if wire.schema_version != OPERATION_SCHEMA_VERSION {
            return Err(GenerationQualificationOperationContractError::UnsupportedSchema);
        }
        if !wire.matches_input(expected_input) {
            return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
        }
        let value = Self::build(relations, expected_input.clone(), Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    /// Revalidates this record against fresh exact recursive closures.
    ///
    /// # Errors
    ///
    /// Returns a content-free error if any dependency or interruption fact differs.
    pub fn validate_against(
        &self,
        relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
        input: GenerationQualificationPhaseInterruptionRecordV1Input,
    ) -> Result<(), GenerationQualificationOperationContractError> {
        let expected = Self::new(relations, input)?;
        if self == &expected {
            Ok(())
        } else {
            Err(GenerationQualificationOperationContractError::RelationshipMismatch)
        }
    }

    fn build(
        relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
        input: GenerationQualificationPhaseInterruptionRecordV1Input,
        wire: Option<&PhaseInterruptionWire>,
    ) -> Result<Self, GenerationQualificationOperationContractError> {
        validate_relations(relations, &input)?;
        let phase_policy_digest = phase_policy_digest(relations.operation_policy, input.phase);
        let mut value = Self {
            schema_version: OPERATION_SCHEMA_VERSION,
            record_kind: GenerationQualificationPhaseInterruptionRecordKindV1::PhaseInterruption,
            operation_receipt_id: relations.operation_receipt.operation_receipt_id().clone(),
            operation_policy_id: relations.operation_policy.operation_policy_id().clone(),
            target_generation_system_id: relations
                .operation_policy
                .target_generation_system_id()
                .clone(),
            generation_qualification_plan_id: relations
                .operation_policy
                .generation_qualification_plan_id()
                .clone(),
            suite_manifest_id: relations.operation_policy.suite_manifest_id().clone(),
            phase_policy_digest: phase_policy_digest.clone(),
            phase: input.phase,
            checkpoint: input.checkpoint,
            planned_attempt_id: input.planned_attempt_id,
            reason: input.reason,
            id: GenerationQualificationPhaseInterruptionRecordId::from_canonical_bytes(
                b"uninitialized generation qualification phase interruption",
            ),
        };
        if wire.is_some_and(|wire| !wire.matches(&value)) {
            return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
        }
        let canonical = value.canonical_bytes();
        if canonical.len() > MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_CANONICAL_BYTES {
            return Err(GenerationQualificationOperationContractError::CanonicalEncodingTooLarge);
        }
        value.id =
            GenerationQualificationPhaseInterruptionRecordId::from_canonical_bytes(&canonical);
        Ok(value)
    }

    /// Returns the portable schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the content-derived interruption-record identity.
    #[must_use]
    pub const fn phase_interruption_record_id(
        &self,
    ) -> &GenerationQualificationPhaseInterruptionRecordId {
        &self.id
    }
    /// Returns the exact terminal operation receipt.
    #[must_use]
    pub const fn operation_receipt_id(&self) -> &GenerationQualificationOperationReceiptId {
        &self.operation_receipt_id
    }
    /// Returns the exact preregistered operation policy.
    #[must_use]
    pub const fn operation_policy_id(&self) -> &GenerationQualificationOperationPolicyId {
        &self.operation_policy_id
    }
    /// Returns the exact target generation system.
    #[must_use]
    pub const fn target_generation_system_id(&self) -> &GenerationSystemId {
        &self.target_generation_system_id
    }
    /// Returns the exact frozen qualification plan.
    #[must_use]
    pub const fn generation_qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.generation_qualification_plan_id
    }
    /// Returns the exact frozen suite.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the phase-policy digest derived from the exact operation policy.
    #[must_use]
    pub const fn phase_policy_digest(&self) -> &Digest {
        &self.phase_policy_digest
    }
    /// Returns the interrupted phase.
    #[must_use]
    pub const fn phase(&self) -> GenerationQualificationInterruptedPhaseV1 {
        self.phase
    }
    /// Returns the exact phase checkpoint.
    #[must_use]
    pub const fn checkpoint(&self) -> GenerationQualificationPhaseCheckpointV1 {
        self.checkpoint
    }
    /// Returns the exact attempt when the checkpoint is attempt-specific.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> Option<&PlannedCandidateAttemptId> {
        self.planned_attempt_id.as_ref()
    }
    /// Returns the closed primary interruption reason.
    #[must_use]
    pub const fn reason(&self) -> GenerationQualificationPhaseInterruptionReasonV1 {
        self.reason
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = GENERATION_QUALIFICATION_PHASE_INTERRUPTION_RECORD_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        output.push(0); // GenerationQualificationPhaseInterruptionRecordKindV1::PhaseInterruption
        for digest in [
            self.operation_receipt_id.digest(),
            self.operation_policy_id.digest(),
            self.target_generation_system_id.digest(),
            self.generation_qualification_plan_id.digest(),
            self.suite_manifest_id.digest(),
            &self.phase_policy_digest,
        ] {
            append_digest(&mut output, digest);
        }
        output.push(phase_tag(self.phase));
        output.push(checkpoint_tag(self.checkpoint));
        match &self.planned_attempt_id {
            None => output.push(0),
            Some(id) => {
                output.push(1);
                append_digest(&mut output, id.digest());
            }
        }
        output.push(reason_tag(self.reason));
        output
    }
}

impl fmt::Debug for GenerationQualificationPhaseInterruptionRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationQualificationPhaseInterruptionRecordV1")
            .field("phase_interruption_record_id", &self.id)
            .field("phase", &self.phase)
            .field("checkpoint", &self.checkpoint)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PhaseInterruptionWire {
    schema_version: u32,
    record_kind: GenerationQualificationPhaseInterruptionRecordKindV1,
    operation_receipt_id: GenerationQualificationOperationReceiptId,
    operation_policy_id: GenerationQualificationOperationPolicyId,
    target_generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    phase_policy_digest: Digest,
    phase: GenerationQualificationInterruptedPhaseV1,
    checkpoint: GenerationQualificationPhaseCheckpointV1,
    #[serde(deserialize_with = "deserialize_planned_attempt_id")]
    planned_attempt_id: Option<PlannedCandidateAttemptId>,
    reason: GenerationQualificationPhaseInterruptionReasonV1,
}

impl PhaseInterruptionWire {
    fn matches_input(&self, input: &GenerationQualificationPhaseInterruptionRecordV1Input) -> bool {
        self.phase == input.phase
            && self.checkpoint == input.checkpoint
            && self.planned_attempt_id == input.planned_attempt_id
            && self.reason == input.reason
    }

    fn matches(&self, value: &GenerationQualificationPhaseInterruptionRecordV1) -> bool {
        self.schema_version == value.schema_version
            && self.record_kind == value.record_kind
            && self.operation_receipt_id == value.operation_receipt_id
            && self.operation_policy_id == value.operation_policy_id
            && self.target_generation_system_id == value.target_generation_system_id
            && self.generation_qualification_plan_id == value.generation_qualification_plan_id
            && self.suite_manifest_id == value.suite_manifest_id
            && self.phase_policy_digest == value.phase_policy_digest
            && self.phase == value.phase
            && self.checkpoint == value.checkpoint
            && self.planned_attempt_id == value.planned_attempt_id
            && self.reason == value.reason
    }
}

fn deserialize_planned_attempt_id<'de, D>(
    deserializer: D,
) -> Result<Option<PlannedCandidateAttemptId>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::deserialize(deserializer)
}

const fn phase_tag(value: GenerationQualificationInterruptedPhaseV1) -> u8 {
    match value {
        GenerationQualificationInterruptedPhaseV1::AttemptLedger => 0,
        GenerationQualificationInterruptedPhaseV1::Repeatability => 1,
        GenerationQualificationInterruptedPhaseV1::ResourceEvidence => 2,
        GenerationQualificationInterruptedPhaseV1::HumanAdjudication => 3,
    }
}

const fn checkpoint_tag(value: GenerationQualificationPhaseCheckpointV1) -> u8 {
    match value {
        GenerationQualificationPhaseCheckpointV1::BeforePhase => 0,
        GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition => 1,
        GenerationQualificationPhaseCheckpointV1::EvidenceCompilation => 2,
        GenerationQualificationPhaseCheckpointV1::ManifestCompilation => 3,
        GenerationQualificationPhaseCheckpointV1::FinalAuthorityRevalidation => 4,
        GenerationQualificationPhaseCheckpointV1::MandatoryFinalization => 5,
    }
}

const fn reason_tag(value: GenerationQualificationPhaseInterruptionReasonV1) -> u8 {
    match value {
        GenerationQualificationPhaseInterruptionReasonV1::Cancelled => 0,
        GenerationQualificationPhaseInterruptionReasonV1::DeadlineExceeded => 1,
        GenerationQualificationPhaseInterruptionReasonV1::AuthorityDrift => 2,
        GenerationQualificationPhaseInterruptionReasonV1::RequiredObservationMissing => 3,
        GenerationQualificationPhaseInterruptionReasonV1::RequiredObservationInvalid => 4,
        GenerationQualificationPhaseInterruptionReasonV1::MeasurementOverflow => 5,
        GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed => 6,
        GenerationQualificationPhaseInterruptionReasonV1::CleanupFailed => 7,
    }
}

#[cfg(test)]
#[path = "phase_interruption/tests.rs"]
mod tests;
