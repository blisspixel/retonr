use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::super::super::{
    CandidateDeterministicEvaluationId, CandidateDeterministicEvaluationRecordV1,
    CandidateDeterministicEvaluationStatusV1, CandidateGenerationReceiptSetId,
    CandidateGenerationReceiptSetV1, CandidateJudgeJoinId, CandidateJudgeJoinRecordV1,
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationAttemptLedgerManifestId,
    GenerationAttemptLedgerManifestV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationQualificationPlanId, GenerationRepetitionId, GenerationRepetitionRecordV1,
    GenerationSuiteManifestId, GenerationSystemId,
};
use super::super::common::{
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, validate_canonical_json, validate_scope,
};
use super::super::phase_id;
use crate::generation_qualification::codec::{append_digest, append_u32};

/// Repeatability-result identity domain.
pub const GENERATION_REPEATABILITY_RESULT_ID_DOMAIN: &[u8] =
    b"retonr:generation-repeatability-result:v1\0";
/// Maximum JSON bytes accepted for one repeatability result.
pub const MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES: usize = 16_384;
const MAX_GENERATION_REPEATABILITY_RESULT_CANONICAL_BYTES: usize = 4_096;

phase_id!(
    GenerationRepeatabilityResultId,
    "Content identity of one terminal generation repeatability result."
);

/// Closed terminal stage for one preregistered repeatability result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationRepeatabilityTerminalStageV1 {
    /// Candidate generation failed before a complete receipt set existed.
    CandidateGenerationFailed,
    /// Candidate generation passed, but deterministic evaluation failed.
    DeterministicFailed,
    /// Deterministic evaluation passed, but no successful judge join exists.
    JudgeFailed,
    /// Candidate generation, deterministic evaluation, and judge join all completed.
    Passed,
}

/// Exact typed records required to derive one terminal repeatability result.
#[derive(Clone, Copy)]
pub struct GenerationRepeatabilityResultRecordV1Relations<'a> {
    /// Exact target, plan, and suite scope.
    pub scope: GenerationQualificationPhaseScopeV1<'a>,
    /// Exact preregistered repetition.
    pub repetition: &'a GenerationRepetitionRecordV1,
    /// Exact attempt ledger and its recursively checked root.
    pub attempt_ledger: &'a GenerationAttemptLedgerManifestV1,
    /// Exact planned attempts and records that recursively rederive the ledger.
    pub attempt_ledger_relations: GenerationAttemptLedgerManifestV1Relations<'a>,
    /// Closed terminal stage.
    pub terminal_stage: GenerationRepeatabilityTerminalStageV1,
    /// Target receipt set when the stage structurally requires it.
    pub candidate_receipt_set: Option<&'a CandidateGenerationReceiptSetV1>,
    /// Deterministic result when the stage structurally requires it.
    pub deterministic_evaluation: Option<&'a CandidateDeterministicEvaluationRecordV1>,
    /// Exact successful judge join only for the passed stage.
    pub candidate_judge_join: Option<&'a CandidateJudgeJoinRecordV1>,
    /// Domain-separated inert terminal evidence digest.
    pub terminal_evidence_digest: &'a Digest,
}

/// Inert content-free terminal result for one preregistered repetition.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationRepeatabilityResultRecordV1 {
    schema_version: u32,
    generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    repetition_id: GenerationRepetitionId,
    attempt_ledger_manifest_id: GenerationAttemptLedgerManifestId,
    attempt_ledger_root_digest: Digest,
    terminal_stage: GenerationRepeatabilityTerminalStageV1,
    candidate_generation_receipt_set_id: Option<CandidateGenerationReceiptSetId>,
    candidate_deterministic_evaluation_id: Option<CandidateDeterministicEvaluationId>,
    candidate_judge_join_id: Option<CandidateJudgeJoinId>,
    terminal_evidence_digest: Digest,
    #[serde(skip)]
    id: GenerationRepeatabilityResultId,
}

impl GenerationRepeatabilityResultRecordV1 {
    /// Derives one result by recursively checking every present typed record.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless scope, ledger, terminal stage, and
    /// every structurally present typed record form the exact relationship.
    pub fn new(
        relations: GenerationRepeatabilityResultRecordV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        Self::build(relations, None)
    }

    /// Decodes bounded canonical JSON and recursively rechecks all relationships.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, noncanonical,
    /// unsupported, substituted, or status-inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationRepeatabilityResultRecordV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        if bytes.len() > MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES {
            return Err(GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationPhaseEvidenceError::InvalidEncoding)?;
        if wire.schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationPhaseEvidenceError::UnsupportedSchema);
        }
        let value = Self::build(relations, Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    fn build(
        relations: GenerationRepeatabilityResultRecordV1Relations<'_>,
        wire: Option<&Wire>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        validate_relationships(relations)?;
        let mut value = Self {
            schema_version: GENERATION_QUALIFICATION_SCHEMA_VERSION,
            generation_system_id: relations
                .scope
                .generation_system
                .generation_system_id()
                .clone(),
            generation_qualification_plan_id: relations
                .scope
                .qualification_plan
                .qualification_plan_id()
                .clone(),
            suite_manifest_id: relations.scope.suite.suite_manifest_id().clone(),
            repetition_id: relations.repetition.repetition_id().clone(),
            attempt_ledger_manifest_id: relations
                .attempt_ledger
                .attempt_ledger_manifest_id()
                .clone(),
            attempt_ledger_root_digest: relations.attempt_ledger.evidence_root_digest().clone(),
            terminal_stage: relations.terminal_stage,
            candidate_generation_receipt_set_id: relations
                .candidate_receipt_set
                .map(|value| value.receipt_set_id().clone()),
            candidate_deterministic_evaluation_id: relations
                .deterministic_evaluation
                .map(|value| value.deterministic_evaluation_id().clone()),
            candidate_judge_join_id: relations
                .candidate_judge_join
                .map(|value| value.candidate_judge_join_id().clone()),
            terminal_evidence_digest: relations.terminal_evidence_digest.clone(),
            id: GenerationRepeatabilityResultId::from_canonical_bytes(b"uninitialized"),
        };
        let canonical = value.canonical_bytes()?;
        value.id = GenerationRepeatabilityResultId::from_canonical_bytes(&canonical);
        if wire.is_some_and(|expected| !expected.matches(&value)) {
            return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
        }
        Ok(value)
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, GenerationQualificationPhaseEvidenceError> {
        let mut output = GENERATION_REPEATABILITY_RESULT_ID_DOMAIN.to_vec();
        append_u32(&mut output, self.schema_version);
        for digest in [
            self.generation_system_id.digest(),
            self.generation_qualification_plan_id.digest(),
            self.suite_manifest_id.digest(),
            self.repetition_id.digest(),
            self.attempt_ledger_manifest_id.digest(),
            &self.attempt_ledger_root_digest,
        ] {
            append_digest(&mut output, digest);
        }
        output.push(stage_tag(self.terminal_stage));
        append_optional_id(
            &mut output,
            self.candidate_generation_receipt_set_id
                .as_ref()
                .map(CandidateGenerationReceiptSetId::digest),
        );
        append_optional_id(
            &mut output,
            self.candidate_deterministic_evaluation_id
                .as_ref()
                .map(CandidateDeterministicEvaluationId::digest),
        );
        append_optional_id(
            &mut output,
            self.candidate_judge_join_id
                .as_ref()
                .map(CandidateJudgeJoinId::digest),
        );
        append_digest(&mut output, &self.terminal_evidence_digest);
        if output.len() > MAX_GENERATION_REPEATABILITY_RESULT_CANONICAL_BYTES {
            Err(GenerationQualificationPhaseEvidenceError::CanonicalEncodingTooLarge)
        } else {
            Ok(output)
        }
    }

    /// Returns the target generation system.
    #[must_use]
    pub const fn generation_system_id(&self) -> &GenerationSystemId {
        &self.generation_system_id
    }
    /// Returns the exact qualification plan.
    #[must_use]
    pub const fn generation_qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.generation_qualification_plan_id
    }
    /// Returns the exact suite.
    #[must_use]
    pub const fn suite_manifest_id(&self) -> &GenerationSuiteManifestId {
        &self.suite_manifest_id
    }
    /// Returns the preregistered repetition.
    #[must_use]
    pub const fn repetition_id(&self) -> &GenerationRepetitionId {
        &self.repetition_id
    }
    /// Returns the exact attempt-ledger manifest.
    #[must_use]
    pub const fn attempt_ledger_manifest_id(&self) -> &GenerationAttemptLedgerManifestId {
        &self.attempt_ledger_manifest_id
    }
    /// Returns the exact recursively checked attempt-ledger root.
    #[must_use]
    pub const fn attempt_ledger_root_digest(&self) -> &Digest {
        &self.attempt_ledger_root_digest
    }
    /// Returns the closed terminal stage.
    #[must_use]
    pub const fn terminal_stage(&self) -> GenerationRepeatabilityTerminalStageV1 {
        self.terminal_stage
    }
    /// Returns the structurally optional target receipt-set identity.
    #[must_use]
    pub const fn candidate_generation_receipt_set_id(
        &self,
    ) -> Option<&CandidateGenerationReceiptSetId> {
        self.candidate_generation_receipt_set_id.as_ref()
    }
    /// Returns the structurally optional deterministic-evaluation identity.
    #[must_use]
    pub const fn candidate_deterministic_evaluation_id(
        &self,
    ) -> Option<&CandidateDeterministicEvaluationId> {
        self.candidate_deterministic_evaluation_id.as_ref()
    }
    /// Returns the structurally optional candidate-judge join identity.
    #[must_use]
    pub const fn candidate_judge_join_id(&self) -> Option<&CandidateJudgeJoinId> {
        self.candidate_judge_join_id.as_ref()
    }
    /// Returns the inert terminal evidence digest.
    #[must_use]
    pub const fn terminal_evidence_digest(&self) -> &Digest {
        &self.terminal_evidence_digest
    }
    /// Returns the content-derived result identity.
    #[must_use]
    pub const fn repeatability_result_id(&self) -> &GenerationRepeatabilityResultId {
        &self.id
    }
}

impl fmt::Debug for GenerationRepeatabilityResultRecordV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationRepeatabilityResultRecordV1")
            .field("repeatability_result_id", &self.id)
            .field("terminal_stage", &self.terminal_stage)
            .finish_non_exhaustive()
    }
}

fn validate_relationships(
    relations: GenerationRepeatabilityResultRecordV1Relations<'_>,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    validate_scope(relations.scope)?;
    if relations.repetition.suite_manifest_id() != relations.scope.suite.suite_manifest_id()
        || relations.attempt_ledger.generation_system_id()
            != relations.scope.generation_system.generation_system_id()
        || relations.attempt_ledger.generation_qualification_plan_id()
            != relations.scope.qualification_plan.qualification_plan_id()
        || relations.attempt_ledger.suite_manifest_id() != relations.scope.suite.suite_manifest_id()
    {
        return Err(GenerationQualificationPhaseEvidenceError::ScopeMismatch);
    }
    let repetition_completed = relations.attempt_ledger.validate_and_repetition_completed(
        relations.attempt_ledger_relations,
        relations.repetition.repetition_id(),
    )?;
    if !typed_records_match(relations) {
        return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
    }
    if stage_matches(relations, repetition_completed) {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseEvidenceError::StatusMismatch)
    }
}

fn typed_records_match(relations: GenerationRepeatabilityResultRecordV1Relations<'_>) -> bool {
    let receipt_matches = relations.candidate_receipt_set.is_none_or(|receipt| {
        receipt.qualification_plan_id()
            == relations.scope.qualification_plan.qualification_plan_id()
            && receipt.suite_manifest_id() == relations.scope.suite.suite_manifest_id()
            && receipt.repetition_id() == relations.repetition.repetition_id()
            && receipt.generation_system_id()
                == relations.scope.generation_system.generation_system_id()
    });
    let deterministic_matches = match (
        relations.candidate_receipt_set,
        relations.deterministic_evaluation,
    ) {
        (Some(receipt), Some(deterministic)) => {
            deterministic.suite_manifest_id() == relations.scope.suite.suite_manifest_id()
                && deterministic.repetition_id() == relations.repetition.repetition_id()
                && target_side_matches(
                    SideBinding {
                        receipt: receipt.receipt_set_id().digest(),
                        system: receipt.generation_system_id().digest(),
                    },
                    SideBinding {
                        receipt: deterministic.candidate_a_receipt_set_id().digest(),
                        system: deterministic.candidate_a_generation_system_id().digest(),
                    },
                    SideBinding {
                        receipt: deterministic.candidate_b_receipt_set_id().digest(),
                        system: deterministic.candidate_b_generation_system_id().digest(),
                    },
                )
        }
        (_, None) => true,
        (None, Some(_)) => false,
    };
    let join_matches = match (
        relations.candidate_receipt_set,
        relations.deterministic_evaluation,
        relations.candidate_judge_join,
    ) {
        (Some(receipt), Some(deterministic), Some(join)) => {
            join.deterministic_evaluation_id() == deterministic.deterministic_evaluation_id()
                && join.candidate_receipt_pair_set_id()
                    == deterministic.candidate_receipt_pair_set_id()
                && join.candidate_a_receipt_set_id() == deterministic.candidate_a_receipt_set_id()
                && join.candidate_b_receipt_set_id() == deterministic.candidate_b_receipt_set_id()
                && target_side_matches(
                    SideBinding {
                        receipt: receipt.receipt_set_id().digest(),
                        system: receipt.generation_system_id().digest(),
                    },
                    SideBinding {
                        receipt: join.candidate_a_receipt_set_id().digest(),
                        system: join.candidate_a_generation_system_id().digest(),
                    },
                    SideBinding {
                        receipt: join.candidate_b_receipt_set_id().digest(),
                        system: join.candidate_b_generation_system_id().digest(),
                    },
                )
        }
        (_, _, None) => true,
        _ => false,
    };
    receipt_matches && deterministic_matches && join_matches
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct SideBinding<'a> {
    receipt: &'a Digest,
    system: &'a Digest,
}

fn target_side_matches(
    target: SideBinding<'_>,
    left_side: SideBinding<'_>,
    right_side: SideBinding<'_>,
) -> bool {
    target == left_side || target == right_side
}

fn stage_matches(
    relations: GenerationRepeatabilityResultRecordV1Relations<'_>,
    repetition_completed: bool,
) -> bool {
    stage_state_matches(
        relations.terminal_stage,
        StageState {
            ledger_status: relations.attempt_ledger.status(),
            repetition_completed,
            receipt_present: relations.candidate_receipt_set.is_some(),
            deterministic_status: relations
                .deterministic_evaluation
                .map(CandidateDeterministicEvaluationRecordV1::status),
            join_present: relations.candidate_judge_join.is_some(),
        },
    )
}

#[derive(Clone, Copy)]
struct StageState {
    ledger_status: GenerationQualificationPhaseStatusV1,
    repetition_completed: bool,
    receipt_present: bool,
    deterministic_status: Option<CandidateDeterministicEvaluationStatusV1>,
    join_present: bool,
}

fn stage_state_matches(
    terminal_stage: GenerationRepeatabilityTerminalStageV1,
    evidence_state: StageState,
) -> bool {
    match terminal_stage {
        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed => {
            evidence_state.ledger_status == GenerationQualificationPhaseStatusV1::Failed
                && !evidence_state.repetition_completed
                && !evidence_state.receipt_present
                && evidence_state.deterministic_status.is_none()
                && !evidence_state.join_present
        }
        GenerationRepeatabilityTerminalStageV1::DeterministicFailed => {
            evidence_state.ledger_status != GenerationQualificationPhaseStatusV1::Skipped
                && evidence_state.repetition_completed
                && evidence_state.receipt_present
                && evidence_state.deterministic_status
                    == Some(CandidateDeterministicEvaluationStatusV1::Failed)
                && !evidence_state.join_present
        }
        GenerationRepeatabilityTerminalStageV1::JudgeFailed => {
            evidence_state.ledger_status != GenerationQualificationPhaseStatusV1::Skipped
                && evidence_state.repetition_completed
                && evidence_state.receipt_present
                && evidence_state.deterministic_status
                    == Some(CandidateDeterministicEvaluationStatusV1::Passed)
                && !evidence_state.join_present
        }
        GenerationRepeatabilityTerminalStageV1::Passed => {
            evidence_state.ledger_status != GenerationQualificationPhaseStatusV1::Skipped
                && evidence_state.repetition_completed
                && evidence_state.receipt_present
                && evidence_state.deterministic_status
                    == Some(CandidateDeterministicEvaluationStatusV1::Passed)
                && evidence_state.join_present
        }
    }
}

fn append_optional_id(output: &mut Vec<u8>, value: Option<&Digest>) {
    if let Some(value) = value {
        output.push(1);
        append_digest(output, value);
    } else {
        output.push(0);
    }
}

const fn stage_tag(value: GenerationRepeatabilityTerminalStageV1) -> u8 {
    match value {
        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed => 0,
        GenerationRepeatabilityTerminalStageV1::DeterministicFailed => 1,
        GenerationRepeatabilityTerminalStageV1::JudgeFailed => 2,
        GenerationRepeatabilityTerminalStageV1::Passed => 3,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    repetition_id: GenerationRepetitionId,
    attempt_ledger_manifest_id: GenerationAttemptLedgerManifestId,
    attempt_ledger_root_digest: Digest,
    terminal_stage: GenerationRepeatabilityTerminalStageV1,
    candidate_generation_receipt_set_id: Option<CandidateGenerationReceiptSetId>,
    candidate_deterministic_evaluation_id: Option<CandidateDeterministicEvaluationId>,
    candidate_judge_join_id: Option<CandidateJudgeJoinId>,
    terminal_evidence_digest: Digest,
}

impl Wire {
    fn matches(&self, value: &GenerationRepeatabilityResultRecordV1) -> bool {
        self.schema_version == value.schema_version
            && self.generation_system_id == value.generation_system_id
            && self.generation_qualification_plan_id == value.generation_qualification_plan_id
            && self.suite_manifest_id == value.suite_manifest_id
            && self.repetition_id == value.repetition_id
            && self.attempt_ledger_manifest_id == value.attempt_ledger_manifest_id
            && self.attempt_ledger_root_digest == value.attempt_ledger_root_digest
            && self.terminal_stage == value.terminal_stage
            && self.candidate_generation_receipt_set_id == value.candidate_generation_receipt_set_id
            && self.candidate_deterministic_evaluation_id
                == value.candidate_deterministic_evaluation_id
            && self.candidate_judge_join_id == value.candidate_judge_join_id
            && self.terminal_evidence_digest == value.terminal_evidence_digest
    }
}

#[cfg(test)]
mod tests;
