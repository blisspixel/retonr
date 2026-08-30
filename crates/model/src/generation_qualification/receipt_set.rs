use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::codec::{append_count, append_digest, append_u32, validate_canonical_json};
use super::{
    CANDIDATE_GENERATION_RECEIPT_SET_ID_DOMAIN, CandidateEvidenceId,
    CandidateGenerationAttemptOutcomeV1, CandidateGenerationAttemptRecordId,
    CandidateGenerationAttemptRecordV1, CandidateGenerationReceiptId,
    CandidateGenerationReceiptSetId, CandidateGenerationReceiptV1, CandidateSelectionPolicyId,
    CandidateSelectionPolicyV1, GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationCaseId,
    GenerationQualificationContractError, GenerationQualificationPlanId,
    GenerationQualificationPlanV1, GenerationRepetitionId, GenerationRepetitionRecordV1,
    GenerationSuiteManifestId, GenerationSuiteManifestV1, GenerationSystemId,
    GenerationSystemRecordV1, PlannedCandidateAttemptId, PlannedCandidateAttemptV1,
};

/// Maximum JSON bytes accepted for one candidate-generation receipt set.
pub const MAX_CANDIDATE_GENERATION_RECEIPT_SET_JSON_BYTES: usize = 4 * 1_024 * 1_024;
const MAX_CANDIDATE_GENERATION_RECEIPT_SET_CANONICAL_BYTES: usize = 128 * 1_024;

/// One selected completed attempt in semantic suite order.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationReceiptSetEntryV1 {
    case_id: GenerationCaseId,
    planned_attempt_id: PlannedCandidateAttemptId,
    attempt_record_id: CandidateGenerationAttemptRecordId,
    receipt_id: CandidateGenerationReceiptId,
    selected_ordinal: u8,
    selected_candidate_evidence_id: CandidateEvidenceId,
}

impl CandidateGenerationReceiptSetEntryV1 {
    /// Returns the exact case identity.
    #[must_use]
    pub const fn case_id(&self) -> &GenerationCaseId {
        &self.case_id
    }
    /// Returns the exact planned-attempt identity.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.planned_attempt_id
    }
    /// Returns the exact completed attempt-record identity.
    #[must_use]
    pub const fn attempt_record_id(&self) -> &CandidateGenerationAttemptRecordId {
        &self.attempt_record_id
    }
    /// Returns the exact receipt identity.
    #[must_use]
    pub const fn receipt_id(&self) -> &CandidateGenerationReceiptId {
        &self.receipt_id
    }
    /// Returns the preselected candidate ordinal.
    #[must_use]
    pub const fn selected_ordinal(&self) -> u8 {
        self.selected_ordinal
    }
    /// Returns the selected retained candidate identity.
    #[must_use]
    pub const fn selected_candidate_evidence_id(&self) -> &CandidateEvidenceId {
        &self.selected_candidate_evidence_id
    }
}

/// Exact records required to derive or decode one receipt set.
#[derive(Clone, Copy)]
pub struct CandidateGenerationReceiptSetV1Relations<'a> {
    /// Exact qualification plan.
    pub qualification_plan: &'a GenerationQualificationPlanV1,
    /// Exact suite in semantic order.
    pub suite: &'a GenerationSuiteManifestV1,
    /// Exact repetition selected for this set.
    pub repetition: &'a GenerationRepetitionRecordV1,
    /// Exact generation system selected for this set.
    pub generation_system: &'a GenerationSystemRecordV1,
    /// Exact pre-output selection policy.
    pub selection_policy: &'a CandidateSelectionPolicyV1,
    /// Every planned attempt in exact plan order.
    pub planned_attempts: &'a [PlannedCandidateAttemptV1],
    /// Completed attempt records in semantic suite order.
    pub attempt_records: &'a [CandidateGenerationAttemptRecordV1],
    /// Final receipts in semantic suite order.
    pub receipts: &'a [CandidateGenerationReceiptV1],
}

/// Inert portable selected-candidate closure for one system and repetition.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateGenerationReceiptSetV1 {
    schema_version: u32,
    qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    repetition_id: GenerationRepetitionId,
    generation_system_id: GenerationSystemId,
    selection_policy_id: CandidateSelectionPolicyId,
    entries: Vec<CandidateGenerationReceiptSetEntryV1>,
    entry_count: u32,
    #[serde(skip)]
    id: CandidateGenerationReceiptSetId,
}

impl CandidateGenerationReceiptSetV1 {
    /// Derives one complete receipt set from freshly reloaded exact records.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for a missing, extra,
    /// duplicate, reordered, failed, foreign, or incorrectly selected attempt.
    pub fn new(
        relations: CandidateGenerationReceiptSetV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        let entries = derive_entries(relations)?;
        Self::from_parts(
            GENERATION_QUALIFICATION_SCHEMA_VERSION,
            relations,
            entries,
            u32::try_from(relations.suite.case_ids().len())
                .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?,
        )
    }

    fn from_parts(
        schema_version: u32,
        relations: CandidateGenerationReceiptSetV1Relations<'_>,
        entries: Vec<CandidateGenerationReceiptSetEntryV1>,
        entry_count: u32,
    ) -> Result<Self, GenerationQualificationContractError> {
        if schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                schema_version,
            ));
        }
        let expected = derive_entries(relations)?;
        if entries != expected
            || entry_count
                != u32::try_from(expected.len())
                    .map_err(|_| GenerationQualificationContractError::EncodingOverflow)?
        {
            return Err(GenerationQualificationContractError::ReceiptSetRelationshipMismatch);
        }
        let qualification_plan_id = relations.qualification_plan.qualification_plan_id().clone();
        let suite_manifest_id = relations.suite.suite_manifest_id().clone();
        let repetition_id = relations.repetition.repetition_id().clone();
        let generation_system_id = relations.generation_system.generation_system_id().clone();
        let selection_policy_id = relations.selection_policy.selection_policy_id().clone();
        let canonical = canonical_bytes(
            schema_version,
            &qualification_plan_id,
            &suite_manifest_id,
            &repetition_id,
            &generation_system_id,
            &selection_policy_id,
            &entries,
            entry_count,
        )?;
        if canonical.len() > MAX_CANDIDATE_GENERATION_RECEIPT_SET_CANONICAL_BYTES {
            return Err(GenerationQualificationContractError::CanonicalEncodingTooLarge);
        }
        Ok(Self {
            schema_version,
            qualification_plan_id,
            suite_manifest_id,
            repetition_id,
            generation_system_id,
            selection_policy_id,
            entries,
            entry_count,
            id: CandidateGenerationReceiptSetId(rewrite_types::Digest::sha256(&canonical)),
        })
    }

    /// Parses bounded canonical JSON and rederives every entry from exact records.
    ///
    /// # Errors
    ///
    /// Returns [`GenerationQualificationContractError`] for oversized, malformed,
    /// unknown, duplicate, trailing, future-schema, noncanonical, or stale input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: CandidateGenerationReceiptSetV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationContractError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            qualification_plan_id: GenerationQualificationPlanId,
            suite_manifest_id: GenerationSuiteManifestId,
            repetition_id: GenerationRepetitionId,
            generation_system_id: GenerationSystemId,
            selection_policy_id: CandidateSelectionPolicyId,
            entries: Vec<CandidateGenerationReceiptSetEntryV1>,
            entry_count: u32,
        }

        if bytes.len() > MAX_CANDIDATE_GENERATION_RECEIPT_SET_JSON_BYTES {
            return Err(GenerationQualificationContractError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationContractError::InvalidEncoding)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationContractError::UnsupportedSchema(
                wire.schema_version,
            ));
        }
        if &wire.qualification_plan_id != relations.qualification_plan.qualification_plan_id()
            || &wire.suite_manifest_id != relations.suite.suite_manifest_id()
            || &wire.repetition_id != relations.repetition.repetition_id()
            || &wire.generation_system_id != relations.generation_system.generation_system_id()
            || &wire.selection_policy_id != relations.selection_policy.selection_policy_id()
        {
            return Err(GenerationQualificationContractError::ReceiptSetRelationshipMismatch);
        }
        let record = Self::from_parts(
            wire.schema_version,
            relations,
            wire.entries,
            wire.entry_count,
        )?;
        validate_canonical_json(bytes, &record)?;
        Ok(record)
    }

    /// Returns the schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact qualification-plan identity.
    #[must_use]
    pub const fn qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.qualification_plan_id
    }
    /// Returns the exact suite identity.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the exact repetition identity.
    #[must_use]
    pub const fn repetition_id(&self) -> &GenerationRepetitionId {
        &self.repetition_id
    }
    /// Returns the exact generation-system identity.
    #[must_use]
    pub const fn generation_system_id(&self) -> &GenerationSystemId {
        &self.generation_system_id
    }
    /// Returns the exact pre-output selection-policy identity.
    #[must_use]
    pub const fn selection_policy_id(&self) -> &CandidateSelectionPolicyId {
        &self.selection_policy_id
    }
    /// Returns selected entries in semantic suite order.
    #[must_use]
    pub fn entries(&self) -> &[CandidateGenerationReceiptSetEntryV1] {
        &self.entries
    }
    /// Returns the array-derived entry count.
    #[must_use]
    pub const fn entry_count(&self) -> u32 {
        self.entry_count
    }
    /// Returns the content-derived receipt-set identity.
    #[must_use]
    pub const fn receipt_set_id(&self) -> &CandidateGenerationReceiptSetId {
        &self.id
    }
}

impl fmt::Debug for CandidateGenerationReceiptSetV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationReceiptSetV1")
            .field("schema_version", &self.schema_version)
            .field("receipt_set_id", &self.id)
            .field("entry_count", &self.entry_count)
            .finish_non_exhaustive()
    }
}

fn derive_entries(
    relations: CandidateGenerationReceiptSetV1Relations<'_>,
) -> Result<Vec<CandidateGenerationReceiptSetEntryV1>, GenerationQualificationContractError> {
    if relations.qualification_plan.suite_manifest_id() != relations.suite.suite_manifest_id()
        || relations.repetition.suite_manifest_id() != relations.suite.suite_manifest_id()
        || !relations
            .qualification_plan
            .generation_system_ids()
            .contains(relations.generation_system.generation_system_id())
        || relations.attempt_records.len() != relations.suite.case_ids().len()
        || relations.receipts.len() != relations.suite.case_ids().len()
    {
        return Err(GenerationQualificationContractError::ReceiptSetRelationshipMismatch);
    }
    relations
        .selection_policy
        .validate_against_plan(relations.qualification_plan, relations.planned_attempts)
        .map_err(|_| GenerationQualificationContractError::ReceiptSetRelationshipMismatch)?;

    let mut output = Vec::with_capacity(relations.suite.case_ids().len());
    for (case_index, case_id) in relations.suite.case_ids().iter().enumerate() {
        let matching = relations
            .planned_attempts
            .iter()
            .filter(|attempt| {
                attempt.case_id() == case_id
                    && attempt.repetition_id() == relations.repetition.repetition_id()
                    && attempt.generation_system_id()
                        == relations.generation_system.generation_system_id()
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(GenerationQualificationContractError::ReceiptSetRelationshipMismatch);
        }
        let planned = matching[0];
        let attempt_record = &relations.attempt_records[case_index];
        let receipt = &relations.receipts[case_index];
        let (completed_planned_id, completed_receipt_id) = match attempt_record.outcome() {
            CandidateGenerationAttemptOutcomeV1::Completed {
                planned_attempt_id,
                receipt_id,
                ..
            } => (planned_attempt_id, receipt_id),
            CandidateGenerationAttemptOutcomeV1::Failed { .. } => {
                return Err(GenerationQualificationContractError::ReceiptSetRelationshipMismatch);
            }
        };
        let selection = &relations.selection_policy.entries()[case_index];
        let selected_candidate = receipt
            .candidate_entries()
            .get(usize::from(selection.selected_ordinal()))
            .filter(|candidate| candidate.ordinal() == selection.selected_ordinal())
            .ok_or(GenerationQualificationContractError::ReceiptSetRelationshipMismatch)?;
        if selection.case_id() != case_id
            || planned.planned_attempt_id() != completed_planned_id
            || planned.planned_attempt_id() != receipt.planned_attempt_id()
            || receipt.receipt_id() != completed_receipt_id
            || receipt.qualification_plan_id()
                != relations.qualification_plan.qualification_plan_id()
            || receipt.suite_manifest_id() != relations.suite.suite_manifest_id()
            || receipt.case_id() != case_id
            || receipt.repetition_id() != relations.repetition.repetition_id()
            || receipt.generation_system_id() != relations.generation_system.generation_system_id()
        {
            return Err(GenerationQualificationContractError::ReceiptSetRelationshipMismatch);
        }
        output.push(CandidateGenerationReceiptSetEntryV1 {
            case_id: case_id.clone(),
            planned_attempt_id: planned.planned_attempt_id().clone(),
            attempt_record_id: attempt_record.attempt_record_id().clone(),
            receipt_id: receipt.receipt_id().clone(),
            selected_ordinal: selection.selected_ordinal(),
            selected_candidate_evidence_id: selected_candidate.candidate_evidence_id().clone(),
        });
    }
    Ok(output)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the frozen identity keeps every top-level relationship explicit"
)]
fn canonical_bytes(
    schema_version: u32,
    qualification_plan_id: &GenerationQualificationPlanId,
    suite_manifest_id: &GenerationSuiteManifestId,
    repetition_id: &GenerationRepetitionId,
    generation_system_id: &GenerationSystemId,
    selection_policy_id: &CandidateSelectionPolicyId,
    entries: &[CandidateGenerationReceiptSetEntryV1],
    entry_count: u32,
) -> Result<Vec<u8>, GenerationQualificationContractError> {
    let mut output = CANDIDATE_GENERATION_RECEIPT_SET_ID_DOMAIN.to_vec();
    append_u32(&mut output, schema_version);
    for id in [
        qualification_plan_id.digest(),
        suite_manifest_id.digest(),
        repetition_id.digest(),
        generation_system_id.digest(),
        selection_policy_id.digest(),
    ] {
        append_digest(&mut output, id);
    }
    append_count(&mut output, entries.len())?;
    for entry in entries {
        for id in [
            entry.case_id().digest(),
            entry.planned_attempt_id().digest(),
            entry.attempt_record_id().digest(),
            entry.receipt_id().digest(),
        ] {
            append_digest(&mut output, id);
        }
        output.push(entry.selected_ordinal());
        append_digest(&mut output, entry.selected_candidate_evidence_id().digest());
    }
    append_u32(&mut output, entry_count);
    Ok(output)
}
