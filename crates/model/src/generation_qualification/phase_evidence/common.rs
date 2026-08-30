use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use super::super::codec::{append_digest, append_u32, append_u64};
use super::super::{
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationQualificationPlanId,
    GenerationQualificationPlanV1, GenerationSuiteManifestId, GenerationSuiteManifestV1,
    GenerationSystemId, GenerationSystemRecordV1,
};

/// Maximum canonical JSON bytes for one phase manifest.
pub const MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES: usize = 16_384;
/// Maximum subordinate items represented by one phase manifest.
pub const MAX_GENERATION_QUALIFICATION_PHASE_ITEMS: usize = 1_024;
pub(super) const MAX_PHASE_MANIFEST_CANONICAL_BYTES: usize = 1_024;

/// Closed outcome of one generation-qualification evidence phase.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationQualificationPhaseStatusV1 {
    /// The phase completed and met its predeclared policy.
    Passed,
    /// The phase ran but did not meet its predeclared policy.
    Failed,
    /// An earlier failure prevented the phase from running.
    Skipped,
}

/// Exact portable scope shared by every phase manifest.
#[derive(Clone, Copy)]
pub struct GenerationQualificationPhaseScopeV1<'a> {
    /// Exact target generation system.
    pub generation_system: &'a GenerationSystemRecordV1,
    /// Exact preregistered qualification plan.
    pub qualification_plan: &'a GenerationQualificationPlanV1,
    /// Exact suite named by the plan.
    pub suite: &'a GenerationSuiteManifestV1,
}

/// Content-free phase-evidence contract failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GenerationQualificationPhaseEvidenceError {
    /// Encoded JSON exceeds its fixed predecode ceiling.
    #[error("generation qualification phase evidence exceeds its limit")]
    EncodedRecordTooLarge,
    /// JSON is malformed, duplicated, trailing, or contains an unknown field.
    #[error("generation qualification phase evidence encoding is invalid")]
    InvalidEncoding,
    /// JSON differs from the one canonical encoding.
    #[error("generation qualification phase evidence encoding is noncanonical")]
    NonCanonicalEncoding,
    /// The schema version is unsupported.
    #[error("generation qualification phase evidence schema is unsupported")]
    UnsupportedSchema,
    /// The target system, plan, or suite scope differs.
    #[error("generation qualification phase evidence scope does not match")]
    ScopeMismatch,
    /// The item count is empty, excessive, or inconsistent with status.
    #[error("generation qualification phase evidence count is invalid")]
    InvalidCount,
    /// A subordinate identity is duplicated or out of semantic order.
    #[error("generation qualification phase evidence relationship does not match")]
    RelationshipMismatch,
    /// The closed status is inconsistent with the supplied evidence.
    #[error("generation qualification phase evidence status does not match")]
    StatusMismatch,
    /// Canonical identity bytes exceed their fixed ceiling.
    #[error("generation qualification phase evidence identity exceeds its limit")]
    CanonicalEncodingTooLarge,
    /// Checked count conversion overflowed.
    #[error("generation qualification phase evidence count overflowed")]
    CountOverflow,
    /// A required resource observation is internally inconsistent.
    #[error("generation qualification resource observation is invalid")]
    InvalidResourceObservation,
    /// Resource-limit identities are duplicated or out of their closed order.
    #[error("generation qualification resource limit list is invalid")]
    InvalidResourceLimitList,
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct PhaseManifestFields {
    pub(super) schema_version: u32,
    pub(super) generation_system_id: GenerationSystemId,
    pub(super) generation_qualification_plan_id: GenerationQualificationPlanId,
    pub(super) suite_manifest_id: GenerationSuiteManifestId,
    pub(super) phase_policy_digest: Digest,
    pub(super) evidence_root_digest: Digest,
    pub(super) evidence_item_count: u32,
    pub(super) status: GenerationQualificationPhaseStatusV1,
}

impl fmt::Debug for PhaseManifestFields {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PhaseManifestFields")
            .field("evidence_item_count", &self.evidence_item_count)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

pub(super) fn fields(
    scope: GenerationQualificationPhaseScopeV1<'_>,
    phase_policy_digest: Digest,
    evidence_root_digest: Digest,
    evidence_item_count: usize,
    status: GenerationQualificationPhaseStatusV1,
) -> Result<PhaseManifestFields, GenerationQualificationPhaseEvidenceError> {
    validate_scope(scope)?;
    validate_count_and_status(evidence_item_count, status)?;
    Ok(PhaseManifestFields {
        schema_version: GENERATION_QUALIFICATION_SCHEMA_VERSION,
        generation_system_id: scope.generation_system.generation_system_id().clone(),
        generation_qualification_plan_id: scope.qualification_plan.qualification_plan_id().clone(),
        suite_manifest_id: scope.suite.suite_manifest_id().clone(),
        phase_policy_digest,
        evidence_root_digest,
        evidence_item_count: u32::try_from(evidence_item_count)
            .map_err(|_| GenerationQualificationPhaseEvidenceError::CountOverflow)?,
        status,
    })
}

pub(super) fn validate_scope(
    scope: GenerationQualificationPhaseScopeV1<'_>,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    let system_id = scope.generation_system.generation_system_id();
    if scope.qualification_plan.suite_manifest_id() != scope.suite.suite_manifest_id()
        || scope
            .qualification_plan
            .generation_system_ids()
            .iter()
            .filter(|value| *value == system_id)
            .count()
            != 1
    {
        return Err(GenerationQualificationPhaseEvidenceError::ScopeMismatch);
    }
    Ok(())
}

pub(super) fn validate_count_and_status(
    count: usize,
    status: GenerationQualificationPhaseStatusV1,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    let valid = count <= MAX_GENERATION_QUALIFICATION_PHASE_ITEMS
        && match status {
            GenerationQualificationPhaseStatusV1::Passed
            | GenerationQualificationPhaseStatusV1::Failed => count > 0,
            GenerationQualificationPhaseStatusV1::Skipped => count == 0,
        };
    if valid {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseEvidenceError::InvalidCount)
    }
}

pub(super) fn validate_item_bound(
    count: usize,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    if count <= MAX_GENERATION_QUALIFICATION_PHASE_ITEMS {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseEvidenceError::InvalidCount)
    }
}

pub(super) fn root_prefix(
    domain: &[u8],
    scope: GenerationQualificationPhaseScopeV1<'_>,
    phase_policy_digest: &Digest,
    item_count: usize,
) -> Result<Vec<u8>, GenerationQualificationPhaseEvidenceError> {
    validate_scope(scope)?;
    let mut output = domain.to_vec();
    append_u32(&mut output, GENERATION_QUALIFICATION_SCHEMA_VERSION);
    append_digest(
        &mut output,
        scope.generation_system.generation_system_id().digest(),
    );
    append_digest(
        &mut output,
        scope.qualification_plan.qualification_plan_id().digest(),
    );
    append_digest(&mut output, scope.suite.suite_manifest_id().digest());
    append_digest(&mut output, phase_policy_digest);
    append_u64(
        &mut output,
        u64::try_from(item_count)
            .map_err(|_| GenerationQualificationPhaseEvidenceError::CountOverflow)?,
    );
    Ok(output)
}

pub(super) fn canonical_manifest_bytes(
    domain: &[u8],
    fields: &PhaseManifestFields,
) -> Result<Vec<u8>, GenerationQualificationPhaseEvidenceError> {
    let mut output = domain.to_vec();
    append_u32(&mut output, fields.schema_version);
    for digest in [
        fields.generation_system_id.digest(),
        fields.generation_qualification_plan_id.digest(),
        fields.suite_manifest_id.digest(),
        &fields.phase_policy_digest,
        &fields.evidence_root_digest,
    ] {
        append_digest(&mut output, digest);
    }
    append_u32(&mut output, fields.evidence_item_count);
    output.push(status_tag(fields.status));
    if output.len() > MAX_PHASE_MANIFEST_CANONICAL_BYTES {
        Err(GenerationQualificationPhaseEvidenceError::CanonicalEncodingTooLarge)
    } else {
        Ok(output)
    }
}

pub(super) const fn status_tag(value: GenerationQualificationPhaseStatusV1) -> u8 {
    match value {
        GenerationQualificationPhaseStatusV1::Passed => 0,
        GenerationQualificationPhaseStatusV1::Failed => 1,
        GenerationQualificationPhaseStatusV1::Skipped => 2,
    }
}

pub(super) fn validate_canonical_json<T: Serialize>(
    bytes: &[u8],
    value: &T,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    let canonical = serde_json::to_vec(value)
        .map_err(|_| GenerationQualificationPhaseEvidenceError::InvalidEncoding)?;
    if canonical == bytes {
        Ok(())
    } else {
        Err(GenerationQualificationPhaseEvidenceError::NonCanonicalEncoding)
    }
}
