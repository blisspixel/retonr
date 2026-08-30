use std::{collections::HashSet, fmt};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use super::super::{
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationPlanId,
    GenerationRepetitionRecordV1, GenerationSuiteManifestId, GenerationSystemId,
    PlannedCandidateAttemptV1,
};
use super::common::{
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
    PhaseManifestFields, canonical_manifest_bytes, fields, root_prefix, validate_canonical_json,
    validate_item_bound,
};
use super::phase_id;
use crate::generation_qualification::codec::append_digest;

mod result;

pub use result::{
    GENERATION_REPEATABILITY_RESULT_ID_DOMAIN, GenerationRepeatabilityResultId,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityResultRecordV1Relations,
    GenerationRepeatabilityTerminalStageV1, MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES,
};

/// Repeatability-evidence manifest identity domain.
pub const GENERATION_REPEATABILITY_EVIDENCE_MANIFEST_ID_DOMAIN: &[u8] =
    b"retonr:generation-repeatability-evidence-manifest:v1\0";
/// Repeatability-evidence root domain.
pub const GENERATION_REPEATABILITY_EVIDENCE_ROOT_DOMAIN: &[u8] =
    b"retonr:generation-repeatability-evidence-root:v1\0";

phase_id!(
    GenerationRepeatabilityEvidenceManifestId,
    "Content identity of one generation repeatability-evidence phase manifest."
);

/// Exact records and policy required to derive one repeatability manifest.
#[derive(Clone, Copy)]
pub struct GenerationRepeatabilityEvidenceManifestV1Relations<'a> {
    /// Exact target, plan, and suite scope.
    pub scope: GenerationQualificationPhaseScopeV1<'a>,
    /// Exact preregistered repeatability policy.
    pub phase_policy_digest: &'a Digest,
    /// Every exact planned attempt in global plan semantic order.
    pub planned_attempts: &'a [PlannedCandidateAttemptV1],
    /// Exact repetitions in preregistered semantic order.
    pub preregistered_repetitions: &'a [GenerationRepetitionRecordV1],
    /// Available results in the same semantic-order prefix.
    pub results: &'a [GenerationRepeatabilityResultRecordV1],
    /// Closed phase outcome.
    pub status: GenerationQualificationPhaseStatusV1,
}

/// Inert content-free manifest of preregistered repeatability results.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationRepeatabilityEvidenceManifestV1 {
    schema_version: u32,
    generation_system_id: GenerationSystemId,
    generation_qualification_plan_id: GenerationQualificationPlanId,
    suite_manifest_id: GenerationSuiteManifestId,
    phase_policy_digest: Digest,
    evidence_root_digest: Digest,
    evidence_item_count: u32,
    status: GenerationQualificationPhaseStatusV1,
    #[serde(skip)]
    id: GenerationRepeatabilityEvidenceManifestId,
}

impl GenerationRepeatabilityEvidenceManifestV1 {
    /// Derives one manifest from exact preregistered-order result records.
    ///
    /// # Errors
    ///
    /// Returns a content-free error unless the plan projection, repetitions,
    /// results, status, and root form the exact bounded relationship.
    pub fn new(
        relations: GenerationRepeatabilityEvidenceManifestV1Relations<'_>,
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
        relations: GenerationRepeatabilityEvidenceManifestV1Relations<'_>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        if bytes.len() > MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES {
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
        relations: GenerationRepeatabilityEvidenceManifestV1Relations<'_>,
        wire: Option<&Wire>,
    ) -> Result<Self, GenerationQualificationPhaseEvidenceError> {
        validate_results(relations)?;
        let root = derive_root(relations)?;
        let common = fields(
            relations.scope,
            relations.phase_policy_digest.clone(),
            root,
            relations.results.len(),
            relations.status,
        )?;
        let canonical = canonical_manifest_bytes(
            GENERATION_REPEATABILITY_EVIDENCE_MANIFEST_ID_DOMAIN,
            &common,
        )?;
        let value = Self::from_common(
            common,
            GenerationRepeatabilityEvidenceManifestId::from_canonical_bytes(&canonical),
        );
        if wire.is_some_and(|expected| !expected.matches(&value)) {
            return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
        }
        Ok(value)
    }

    fn from_common(
        value: PhaseManifestFields,
        id: GenerationRepeatabilityEvidenceManifestId,
    ) -> Self {
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
    pub const fn repeatability_evidence_manifest_id(
        &self,
    ) -> &GenerationRepeatabilityEvidenceManifestId {
        &self.id
    }
}

impl fmt::Debug for GenerationRepeatabilityEvidenceManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GenerationRepeatabilityEvidenceManifestV1")
            .field("repeatability_evidence_manifest_id", &self.id)
            .field("evidence_item_count", &self.evidence_item_count)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

fn validate_results(
    relations: GenerationRepeatabilityEvidenceManifestV1Relations<'_>,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    validate_item_bound(relations.planned_attempts.len())?;
    validate_item_bound(relations.preregistered_repetitions.len())?;
    validate_item_bound(relations.results.len())?;
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
    let target_repetition_ids = relations
        .planned_attempts
        .iter()
        .filter(|planned| {
            planned.generation_system_id()
                == relations.scope.generation_system.generation_system_id()
        })
        .map(PlannedCandidateAttemptV1::repetition_id)
        .collect::<HashSet<_>>();
    let supplied_repetition_ids = relations
        .preregistered_repetitions
        .iter()
        .map(GenerationRepetitionRecordV1::repetition_id)
        .collect::<HashSet<_>>();
    if target_repetition_ids.is_empty()
        || target_repetition_ids != supplied_repetition_ids
        || relations
            .preregistered_repetitions
            .iter()
            .enumerate()
            .any(|(index, repetition)| {
                usize::try_from(repetition.repetition_ordinal()) != Ok(index)
            })
        || relations.results.len() > relations.preregistered_repetitions.len()
        || relations
            .preregistered_repetitions
            .iter()
            .any(|repetition| {
                repetition.suite_manifest_id() != relations.scope.suite.suite_manifest_id()
            })
        || relations
            .results
            .iter()
            .zip(relations.preregistered_repetitions)
            .any(|(result, repetition)| {
                result.generation_system_id()
                    != relations.scope.generation_system.generation_system_id()
                    || result.generation_qualification_plan_id()
                        != relations.scope.qualification_plan.qualification_plan_id()
                    || result.suite_manifest_id() != relations.scope.suite.suite_manifest_id()
                    || result.repetition_id() != repetition.repetition_id()
            })
    {
        return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
    }
    let terminal_stages = relations
        .results
        .iter()
        .map(GenerationRepeatabilityResultRecordV1::terminal_stage)
        .collect::<Vec<_>>();
    if repeatability_status_matches(
        relations.status,
        &terminal_stages,
        relations.preregistered_repetitions.len(),
    ) {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseEvidenceError::StatusMismatch)
    }
}

fn repeatability_status_matches(
    status: GenerationQualificationPhaseStatusV1,
    terminal_stages: &[GenerationRepeatabilityTerminalStageV1],
    preregistered_count: usize,
) -> bool {
    let all_passed = terminal_stages
        .iter()
        .all(|stage| *stage == GenerationRepeatabilityTerminalStageV1::Passed);
    match status {
        GenerationQualificationPhaseStatusV1::Passed => {
            all_passed && terminal_stages.len() == preregistered_count
        }
        GenerationQualificationPhaseStatusV1::Failed => {
            !terminal_stages.is_empty()
                && (!all_passed || terminal_stages.len() < preregistered_count)
        }
        GenerationQualificationPhaseStatusV1::Skipped => terminal_stages.is_empty(),
    }
}

fn derive_root(
    relations: GenerationRepeatabilityEvidenceManifestV1Relations<'_>,
) -> Result<Digest, GenerationQualificationPhaseEvidenceError> {
    let mut bytes = root_prefix(
        GENERATION_REPEATABILITY_EVIDENCE_ROOT_DOMAIN,
        relations.scope,
        relations.phase_policy_digest,
        relations.results.len(),
    )?;
    for (result, repetition) in relations
        .results
        .iter()
        .zip(relations.preregistered_repetitions)
    {
        append_digest(&mut bytes, repetition.repetition_id().digest());
        append_digest(&mut bytes, result.repeatability_result_id().digest());
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
    fn matches(&self, value: &GenerationRepeatabilityEvidenceManifestV1) -> bool {
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
    fn mixed_per_repetition_outcome_is_overall_failed() {
        assert!(repeatability_status_matches(
            GenerationQualificationPhaseStatusV1::Failed,
            &[
                GenerationRepeatabilityTerminalStageV1::Passed,
                GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
            ],
            2,
        ));
        assert!(!repeatability_status_matches(
            GenerationQualificationPhaseStatusV1::Passed,
            &[
                GenerationRepeatabilityTerminalStageV1::Passed,
                GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
            ],
            2,
        ));
    }

    #[test]
    fn all_passed_abort_prefix_is_failed_without_synthetic_result() {
        assert!(repeatability_status_matches(
            GenerationQualificationPhaseStatusV1::Failed,
            &[GenerationRepeatabilityTerminalStageV1::Passed],
            2,
        ));
        assert!(!repeatability_status_matches(
            GenerationQualificationPhaseStatusV1::Passed,
            &[GenerationRepeatabilityTerminalStageV1::Passed],
            2,
        ));
        assert!(repeatability_status_matches(
            GenerationQualificationPhaseStatusV1::Skipped,
            &[],
            2,
        ));
    }
}
