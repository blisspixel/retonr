//! Store-local durable reference contract for generation evidence bundles.

use std::fmt;

use rewrite_model::{
    ArtifactSetRelativePath, CandidateGenerationEvidenceBundleId, GenerationQualificationPlanId,
    PlannedCandidateAttemptId,
};
use thiserror::Error;

/// Maximum stored tree-entry ceiling accepted for bundle reacquisition.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_TREE_ENTRIES: u32 = 8_193;
/// Maximum stored tree-depth ceiling accepted for bundle reacquisition.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_TREE_DEPTH: u16 = 256;
/// Maximum stored aggregate-byte ceiling accepted for bundle reacquisition.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_AGGREGATE_BYTES: u64 = 256 * 1_024 * 1_024;

/// Invalid store-local generation-evidence reference or limit.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CandidateGenerationEvidenceStorageContractError {
    /// The root identity was not one lowercase SHA-256 value.
    #[error("candidate generation evidence storage root identity is invalid")]
    InvalidRootId,
    /// A reacquisition ceiling was zero or exceeded its fixed adapter bound.
    #[error("candidate generation evidence storage limit is invalid")]
    InvalidLimit,
    /// The relative reference was noncanonical or disagreed with its typed IDs.
    #[error("candidate generation evidence storage reference is invalid")]
    InvalidReference,
}

/// Stable local identity of one application-owned evidence storage root.
///
/// The identity is not a host path and does not participate in portable evidence
/// identities. Moving the complete root preserves this value.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CandidateGenerationEvidenceStorageRootId(String);

impl CandidateGenerationEvidenceStorageRootId {
    /// Parses one exact lowercase SHA-256 root identity.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceStorageContractError::InvalidRootId`]
    /// for any other byte sequence.
    pub fn new(
        value: impl Into<String>,
    ) -> Result<Self, CandidateGenerationEvidenceStorageContractError> {
        let value = value.into();
        if is_digest(&value) {
            Ok(Self(value))
        } else {
            Err(CandidateGenerationEvidenceStorageContractError::InvalidRootId)
        }
    }

    /// Returns the lowercase root identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CandidateGenerationEvidenceStorageRootId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CandidateGenerationEvidenceStorageRootId")
            .field(&self.0)
            .finish()
    }
}

/// Fixed read-side limits persisted with one durable bundle reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[expect(
    clippy::struct_field_names,
    reason = "each persisted reacquisition ceiling remains explicit"
)]
pub struct CandidateGenerationEvidenceStorageV1Limits {
    maximum_tree_entries: u32,
    maximum_tree_depth: u16,
    maximum_aggregate_bytes: u64,
}

impl CandidateGenerationEvidenceStorageV1Limits {
    /// Creates bounded, nonzero bundle-reacquisition limits.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceStorageContractError::InvalidLimit`]
    /// when any value is zero or exceeds its adapter ceiling.
    pub const fn new(
        maximum_tree_entries: u32,
        maximum_tree_depth: u16,
        maximum_aggregate_bytes: u64,
    ) -> Result<Self, CandidateGenerationEvidenceStorageContractError> {
        if maximum_tree_entries == 0
            || maximum_tree_entries > MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_TREE_ENTRIES
            || maximum_tree_depth == 0
            || maximum_tree_depth > MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_TREE_DEPTH
            || maximum_aggregate_bytes == 0
            || maximum_aggregate_bytes > MAX_CANDIDATE_GENERATION_EVIDENCE_STORAGE_AGGREGATE_BYTES
        {
            Err(CandidateGenerationEvidenceStorageContractError::InvalidLimit)
        } else {
            Ok(Self {
                maximum_tree_entries,
                maximum_tree_depth,
                maximum_aggregate_bytes,
            })
        }
    }

    /// Returns the maximum number of entries read during reacquisition.
    #[must_use]
    pub const fn maximum_tree_entries(self) -> u32 {
        self.maximum_tree_entries
    }

    /// Returns the maximum relative directory depth read during reacquisition.
    #[must_use]
    pub const fn maximum_tree_depth(self) -> u16 {
        self.maximum_tree_depth
    }

    /// Returns the maximum aggregate bytes read during reacquisition.
    #[must_use]
    pub const fn maximum_aggregate_bytes(self) -> u64 {
        self.maximum_aggregate_bytes
    }
}

/// Root-bound canonical relative reference for one immutable evidence bundle.
#[derive(Clone, Eq, PartialEq)]
pub struct CandidateGenerationEvidenceBundleStorageV1 {
    storage_root_id: CandidateGenerationEvidenceStorageRootId,
    qualification_plan_id: GenerationQualificationPlanId,
    planned_attempt_id: PlannedCandidateAttemptId,
    evidence_bundle_id: CandidateGenerationEvidenceBundleId,
    relative_reference: ArtifactSetRelativePath,
    limits: CandidateGenerationEvidenceStorageV1Limits,
}

impl CandidateGenerationEvidenceBundleStorageV1 {
    /// Compiles the sole canonical reference from typed identities.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceStorageContractError::InvalidReference`]
    /// only if the fixed layout cannot form a valid portable relative path.
    pub fn new(
        storage_root_id: CandidateGenerationEvidenceStorageRootId,
        qualification_plan_id: GenerationQualificationPlanId,
        planned_attempt_id: PlannedCandidateAttemptId,
        evidence_bundle_id: CandidateGenerationEvidenceBundleId,
        limits: CandidateGenerationEvidenceStorageV1Limits,
    ) -> Result<Self, CandidateGenerationEvidenceStorageContractError> {
        let relative_reference = compile_reference(
            &qualification_plan_id,
            &planned_attempt_id,
            &evidence_bundle_id,
        )?;
        Ok(Self {
            storage_root_id,
            qualification_plan_id,
            planned_attempt_id,
            evidence_bundle_id,
            relative_reference,
            limits,
        })
    }

    /// Reloads and independently recompiles one stored relative reference.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceStorageContractError::InvalidReference`]
    /// unless `relative_reference` is byte-for-byte the fixed typed-ID layout.
    pub fn from_stored_reference(
        storage_root_id: CandidateGenerationEvidenceStorageRootId,
        qualification_plan_id: GenerationQualificationPlanId,
        planned_attempt_id: PlannedCandidateAttemptId,
        evidence_bundle_id: CandidateGenerationEvidenceBundleId,
        relative_reference: ArtifactSetRelativePath,
        limits: CandidateGenerationEvidenceStorageV1Limits,
    ) -> Result<Self, CandidateGenerationEvidenceStorageContractError> {
        let expected = compile_reference(
            &qualification_plan_id,
            &planned_attempt_id,
            &evidence_bundle_id,
        )?;
        if relative_reference != expected {
            return Err(CandidateGenerationEvidenceStorageContractError::InvalidReference);
        }
        Ok(Self {
            storage_root_id,
            qualification_plan_id,
            planned_attempt_id,
            evidence_bundle_id,
            relative_reference,
            limits,
        })
    }

    /// Returns the stable local storage-root identity.
    #[must_use]
    pub const fn storage_root_id(&self) -> &CandidateGenerationEvidenceStorageRootId {
        &self.storage_root_id
    }

    /// Returns the exact qualification plan identity.
    #[must_use]
    pub const fn qualification_plan_id(&self) -> &GenerationQualificationPlanId {
        &self.qualification_plan_id
    }

    /// Returns the exact planned-attempt identity.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.planned_attempt_id
    }

    /// Returns the exact portable evidence-bundle identity.
    #[must_use]
    pub const fn evidence_bundle_id(&self) -> &CandidateGenerationEvidenceBundleId {
        &self.evidence_bundle_id
    }

    /// Returns the canonical reference relative to the retained storage root.
    #[must_use]
    pub const fn relative_reference(&self) -> &ArtifactSetRelativePath {
        &self.relative_reference
    }

    /// Returns the fixed read-side reacquisition limits.
    #[must_use]
    pub const fn limits(&self) -> CandidateGenerationEvidenceStorageV1Limits {
        self.limits
    }
}

impl fmt::Debug for CandidateGenerationEvidenceBundleStorageV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateGenerationEvidenceBundleStorageV1")
            .field("storage_root_id", &self.storage_root_id)
            .field("evidence_bundle_id", &self.evidence_bundle_id)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

fn compile_reference(
    plan_id: &GenerationQualificationPlanId,
    attempt_id: &PlannedCandidateAttemptId,
    bundle_id: &CandidateGenerationEvidenceBundleId,
) -> Result<ArtifactSetRelativePath, CandidateGenerationEvidenceStorageContractError> {
    ArtifactSetRelativePath::new(format!(
        "bundles/v1/{}/{}/{}",
        plan_id.digest().as_str(),
        attempt_id.digest().as_str(),
        bundle_id.digest().as_str(),
    ))
    .map_err(|_| CandidateGenerationEvidenceStorageContractError::InvalidReference)
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[path = "candidate_generation_evidence_storage/tests.rs"]
mod tests;
