use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::super::{
    CandidateGenerationAttemptOutcomeV1, CandidateGenerationAttemptRecordV1,
    GenerationQualificationPlanId, GenerationRepetitionId, GenerationSuiteManifestId,
    GenerationSystemId, PlannedCandidateAttemptV1,
};
use super::common::{
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
    PhaseManifestFields, canonical_manifest_bytes, fields, root_prefix, validate_canonical_json,
    validate_item_bound,
};
use super::phase_id;
use crate::generation_qualification::codec::append_digest;

/// Attempt-ledger manifest identity domain.
pub const GENERATION_ATTEMPT_LEDGER_MANIFEST_ID_DOMAIN: &[u8] =
    b"retonr:generation-attempt-ledger-manifest:v1\0";
/// Attempt-ledger evidence-root domain.
pub const GENERATION_ATTEMPT_LEDGER_ROOT_DOMAIN: &[u8] =
    b"retonr:generation-attempt-ledger-evidence-root:v1\0";

phase_id!(
    GenerationAttemptLedgerManifestId,
    "Content identity of one generation attempt-ledger phase manifest."
);

/// Exact records and policy required to derive one attempt-ledger manifest.
#[derive(Clone, Copy)]
pub struct GenerationAttemptLedgerManifestV1Relations<'a> {
    /// Exact target, plan, and suite scope.
    pub scope: GenerationQualificationPhaseScopeV1<'a>,
    /// Exact preregistered attempt-ledger policy.
    pub phase_policy_digest: &'a Digest,
    /// Every exact planned attempt in global plan semantic order.
    pub planned_attempts: &'a [PlannedCandidateAttemptV1],
    /// Available attempt records in semantic plan-prefix order.
    pub attempt_records: &'a [CandidateGenerationAttemptRecordV1],
    /// Closed phase outcome.
    pub status: GenerationQualificationPhaseStatusV1,
}

/// Inert content-free manifest of attempted candidate generations.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationAttemptLedgerManifestV1 {
    schema_version: u32,
    generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    phase_policy_digest: Digest,
    evidence_root_digest: Digest,
    evidence_item_count: u32,
    status: GenerationQualificationPhaseStatusV1,
    #[serde(skip)]
    id: GenerationAttemptLedgerManifestId,
}

impl GenerationAttemptLedgerManifestV1 {
    /// Derives one manifest from exact plan-ordered attempt records.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless scope, target projection, status, and
    /// every subordinate identity form the exact bounded relationship.
    pub fn new(
        relations: GenerationAttemptLedgerManifestV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        Self::build(relations, None)
    }

    /// Decodes canonical bounded JSON and rederives every relationship.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for oversized, malformed, noncanonical,
    /// unsupported, substituted, reordered, or status-inconsistent input.
    pub fn from_json_bytes(
        bytes: &[u8],
        relations: GenerationAttemptLedgerManifestV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES {
            return Err(GenerationQualificationPhaseEvidenceError::EncodedRecordTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| GenerationQualificationPhaseEvidenceError::InvalidEncoding)?;
        if wire.schema_version != super::super::GENERATION_QUALIFICATION_SCHEMA_VERSION {
            return Err(GenerationQualificationPhaseEvidenceError::UnsupportedSchema);
        }
        let value = Self::build(relations, Some(&wire))?;
        validate_canonical_json(bytes, &value)?;
        Ok(value)
    }

    fn build(
        relations: GenerationAttemptLedgerManifestV1Relations<'_>,
        wire: Option<&Wire>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        validate_attempt_records(relations)?;
        let root = derive_root(relations)?;
        let common = fields(
            relations.scope,
            relations.phase_policy_digest.clone(),
            root,
            relations.attempt_records.len(),
            relations.status,
        )?;
        let canonical =
            canonical_manifest_bytes(GENERATION_ATTEMPT_LEDGER_MANIFEST_ID_DOMAIN, &common)?;
        let value = Self::from_common(
            common,
            GenerationAttemptLedgerManifestId::from_canonical_bytes(&canonical),
        );
        if wire.is_some_and(|expected| !expected.matches(&value)) {
            return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
        }
        Ok(value)
    }

    fn from_common(value: PhaseManifestFields, id: GenerationAttemptLedgerManifestId) -> Self {
        Self {
            schema_version: value.schema_version,
            generation_system_id: value.generation_system_id,
            generation_qualification_plan_id: value.generation_qualification_plan_id,
            suite_manifest_id: value.suite_manifest_id,
            phase_policy_digest: value.phase_policy_digest,
            evidence_root_digest: value.evidence_root_digest,
            evidence_item_count: value.evidence_item_count,
            status: value.status,
            id,
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
    /// Returns the exact phase policy.
    #[must_use]
    pub const fn phase_policy_digest(&self) -> &Digest {
        &self.phase_policy_digest
    }
    /// Returns the rederived content-free evidence root.
    #[must_use]
    pub const fn evidence_root_digest(&self) -> &Digest {
        &self.evidence_root_digest
    }
    /// Returns the exact represented item count.
    #[must_use]
    pub const fn evidence_item_count(&self) -> u32 {
        self.evidence_item_count
    }
    /// Returns the closed phase status.
    #[must_use]
    pub const fn status(&self) -> GenerationQualificationPhaseStatusV1 {
        self.status
    }
    /// Returns the content-derived manifest identity.
    #[must_use]
    pub const fn attempt_ledger_manifest_id(&self) -> &GenerationAttemptLedgerManifestId {
        &self.id
    }

    /// Rebuilds this manifest from the exact planned-attempt and record closure.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless every field, root, identity, target
    /// projection, and closed status rederive this exact manifest.
    pub fn validate_against(
        &self,
        relations: GenerationAttemptLedgerManifestV1Relations<'_>,
    ) -> Result<(), GenerationQualificationPhaseEvidenceError> {
        if &Self::new(relations)? == self {
            Ok(())
        } else {
            Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
        }
    }

    pub(super) fn validate_and_repetition_completed(
        &self,
        relations: GenerationAttemptLedgerManifestV1Relations<'_>,
        repetition_id: &GenerationRepetitionId,
    ) -> Result<bool, GenerationQualificationPhaseEvidenceError> {
        self.validate_against(relations)?;
        let planned = validated_target_attempts(relations)?
            .into_iter()
            .filter(|value| value.repetition_id() == repetition_id)
            .collect::<Vec<_>>();
        if planned.is_empty() {
            return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
        }
        Ok(planned.iter().all(|planned| {
            relations.attempt_records.iter().any(|record| {
                planned_attempt_id(record.outcome()) == planned.planned_attempt_id()
                    && matches!(
                        record.outcome(),
                        CandidateGenerationAttemptOutcomeV1::Completed { .. }
                    )
            })
        }))
    }
}

impl fmt::Debug for GenerationAttemptLedgerManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationAttemptLedgerManifestV1")
            .field("attempt_ledger_manifest_id", &self.id)
            .field("evidence_item_count", &self.evidence_item_count)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

fn validate_attempt_records(
    relations: GenerationAttemptLedgerManifestV1Relations<'_>,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    let target = validated_target_attempts(relations)?;
    if relations.attempt_records.len() > target.len()
        || relations
            .attempt_records
            .iter()
            .zip(&target)
            .any(|(record, expected)| {
                planned_attempt_id(record.outcome()) != expected.planned_attempt_id()
            })
    {
        return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
    }
    let completion = relations
        .attempt_records
        .iter()
        .map(|record| {
            matches!(
                record.outcome(),
                CandidateGenerationAttemptOutcomeV1::Completed { .. }
            )
        })
        .collect::<Vec<_>>();
    if status_matches(relations.status, &completion, target.len()) {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseEvidenceError::StatusMismatch)
    }
}

fn validated_target_attempts(
    relations: GenerationAttemptLedgerManifestV1Relations<'_>,
) -> Result<Vec<&PlannedCandidateAttemptV1>, GenerationQualificationPhaseEvidenceError> {
    validate_item_bound(relations.planned_attempts.len())?;
    validate_item_bound(relations.attempt_records.len())?;
    let plan_ids = relations.scope.qualification_plan.planned_attempt_ids();
    if relations.planned_attempts.len() != plan_ids.len()
        || relations
            .planned_attempts
            .iter()
            .zip(plan_ids)
            .any(|(planned, expected)| planned.planned_attempt_id() != expected)
    {
        return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
    }
    let target = relations
        .planned_attempts
        .iter()
        .filter(|planned| {
            planned.generation_system_id()
                == relations.scope.generation_system.generation_system_id()
        })
        .collect::<Vec<_>>();
    if target.is_empty() {
        return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
    }
    Ok(target)
}

fn status_matches(
    status: GenerationQualificationPhaseStatusV1,
    completed: &[bool],
    target_count: usize,
) -> bool {
    let all_completed = completed.iter().all(|value| *value);
    match status {
        GenerationQualificationPhaseStatusV1::Passed => {
            all_completed && completed.len() == target_count
        }
        GenerationQualificationPhaseStatusV1::Failed => {
            !completed.is_empty() && (!all_completed || completed.len() < target_count)
        }
        GenerationQualificationPhaseStatusV1::Skipped => completed.is_empty(),
    }
}

fn planned_attempt_id(
    outcome: &CandidateGenerationAttemptOutcomeV1,
) -> &super::super::PlannedCandidateAttemptId {
    match outcome {
        CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id, ..
        }
        | CandidateGenerationAttemptOutcomeV1::Failed {
            planned_attempt_id, ..
        } => planned_attempt_id,
    }
}

fn derive_root(
    relations: GenerationAttemptLedgerManifestV1Relations<'_>,
) -> Result<Digest, GenerationQualificationPhaseEvidenceError> {
    let mut bytes = root_prefix(
        GENERATION_ATTEMPT_LEDGER_ROOT_DOMAIN,
        relations.scope,
        relations.phase_policy_digest,
        relations.attempt_records.len(),
    )?;
    for record in relations.attempt_records {
        append_digest(&mut bytes, planned_attempt_id(record.outcome()).digest());
        append_digest(&mut bytes, record.attempt_record_id().digest());
    }
    Ok(Digest::sha256(&bytes))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u32,
    generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    phase_policy_digest: Digest,
    evidence_root_digest: Digest,
    evidence_item_count: u32,
    status: GenerationQualificationPhaseStatusV1,
}

impl Wire {
    fn matches(&self, value: &GenerationAttemptLedgerManifestV1) -> bool {
        self.schema_version == value.schema_version
            && self.generation_system_id == value.generation_system_id
            && self.generation_qualification_plan_id == value.generation_qualification_plan_id
            && self.suite_manifest_id == value.suite_manifest_id
            && self.phase_policy_digest == value.phase_policy_digest
            && self.evidence_root_digest == value.evidence_root_digest
            && self.evidence_item_count == value.evidence_item_count
            && self.status == value.status
    }
}

#[cfg(test)]
mod kernel_tests {
    use super::*;

    #[test]
    fn completed_abort_prefix_is_failed_not_passed() {
        assert!(status_matches(
            GenerationQualificationPhaseStatusV1::Failed,
            &[true],
            2
        ));
        assert!(!status_matches(
            GenerationQualificationPhaseStatusV1::Passed,
            &[true],
            2
        ));
    }

    #[test]
    fn closed_status_matrix_rejects_invalid_combinations() {
        assert!(status_matches(
            GenerationQualificationPhaseStatusV1::Passed,
            &[true, true],
            2
        ));
        assert!(status_matches(
            GenerationQualificationPhaseStatusV1::Failed,
            &[true, false],
            2
        ));
        assert!(status_matches(
            GenerationQualificationPhaseStatusV1::Skipped,
            &[],
            2
        ));
        assert!(!status_matches(
            GenerationQualificationPhaseStatusV1::Failed,
            &[true, true],
            2
        ));
        assert!(!status_matches(
            GenerationQualificationPhaseStatusV1::Skipped,
            &[false],
            2
        ));
    }
}
