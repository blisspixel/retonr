//! Read-only schema-10 admission classes for one qualification plan.
//!
//! The classification compares indexed attempt metadata with one caller-selected
//! evidence-root identity. It does not decode canonical record bytes, mutate
//! rows, or grant activation authority.

use rewrite_model::{
    CandidateGenerationEvidenceBundleId, GenerationQualificationPlanId,
    MAX_PLANNED_GENERATION_ATTEMPTS, PlannedCandidateAttemptId,
};

use super::ArtifactStateStore;
use super::candidate_generation_evidence_storage::CandidateGenerationEvidenceStorageRootId;
use crate::{StoreError, StoreResult};

/// Indexed admission class for one planned candidate attempt.
///
/// `CheckpointOnly`, `TerminalFailed`, and `TerminalCompleted` are trustworthy
/// classes that still cannot activate. Only `NotStarted` is pristine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateGenerationAttemptAdmissionClassV1 {
    /// No precursor, terminal record, or completion cohort exists.
    NotStarted,
    /// A precursor checkpoint exists and no terminal cohort exists.
    CheckpointOnly,
    /// One failed terminal record exists without a completion cohort.
    TerminalFailed,
    /// One completed cohort exists and its storage root is the caller root.
    TerminalCompleted,
    /// Indexed rows are internally consistent but not one legal terminal shape.
    Ambiguous,
    /// Indexed identity, outcome, or uniqueness is not trustworthy.
    Corrupt,
}

/// Inert admission observation for one plan-order attempt.
///
/// The value carries no canonical bytes and grants no execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateGenerationAttemptAdmissionV1 {
    planned_attempt_id: PlannedCandidateAttemptId,
    class: CandidateGenerationAttemptAdmissionClassV1,
    evidence_bundle_id: Option<CandidateGenerationEvidenceBundleId>,
}

impl CandidateGenerationAttemptAdmissionV1 {
    /// Builds one observation whose bundle id agrees with its class.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::CorruptRecord`] unless `TerminalCompleted` has a
    /// bundle id and every other class has none.
    pub fn from_observation(
        planned_attempt_id: PlannedCandidateAttemptId,
        class: CandidateGenerationAttemptAdmissionClassV1,
        evidence_bundle_id: Option<CandidateGenerationEvidenceBundleId>,
    ) -> StoreResult<Self> {
        let completed = matches!(
            class,
            CandidateGenerationAttemptAdmissionClassV1::TerminalCompleted
        );
        if completed == evidence_bundle_id.is_some() {
            Ok(Self {
                planned_attempt_id,
                class,
                evidence_bundle_id,
            })
        } else {
            Err(StoreError::CorruptRecord)
        }
    }

    /// Returns the planned attempt this observation classifies.
    #[must_use]
    pub const fn planned_attempt_id(&self) -> &PlannedCandidateAttemptId {
        &self.planned_attempt_id
    }

    /// Returns the indexed admission class.
    #[must_use]
    pub const fn class(&self) -> CandidateGenerationAttemptAdmissionClassV1 {
        self.class
    }

    /// Returns the evidence bundle id for a completed cohort on the caller root.
    #[must_use]
    pub const fn evidence_bundle_id(&self) -> Option<&CandidateGenerationEvidenceBundleId> {
        self.evidence_bundle_id.as_ref()
    }
}

struct AttemptFacts {
    precursor_id: Option<String>,
    outcome: Option<String>,
    attempt_precursor_id: Option<String>,
    attempt_receipt_id: Option<String>,
    managed_count: i64,
    cleanup_count: i64,
    bundle_count: i64,
    storage_count: i64,
    readback_count: i64,
    receipt_count: i64,
}

impl ArtifactStateStore {
    /// Classifies every caller-ordered attempt from indexed schema-10 metadata.
    ///
    /// The read uses one deferred transaction and commits it without writing.
    /// Plan-order disagreement, an empty caller slice, or an attempt id outside
    /// the stored plan fails the whole call. A caller slice above the plan
    /// ceiling fails before the database is read.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::RecordTooLarge`] when the caller slice exceeds
    /// [`MAX_PLANNED_GENERATION_ATTEMPTS`]. Returns [`StoreError::CorruptRecord`]
    /// when the stored plan order disagrees with the caller or an unscoped
    /// completion row has no precursor. Per-attempt ambiguity and corruption are
    /// returned in the vector instead of failing the other attempts.
    pub fn candidate_generation_attempt_admission_v1(
        &self,
        plan_id: &GenerationQualificationPlanId,
        planned_attempt_ids: &[PlannedCandidateAttemptId],
        storage_root_id: &CandidateGenerationEvidenceStorageRootId,
    ) -> StoreResult<Vec<CandidateGenerationAttemptAdmissionV1>> {
        if planned_attempt_ids.is_empty() {
            return Err(StoreError::CorruptRecord);
        }
        if planned_attempt_ids.len() > MAX_PLANNED_GENERATION_ATTEMPTS {
            return Err(StoreError::RecordTooLarge);
        }
        let transaction = self.connection.unchecked_transaction()?;
        let admissions =
            classify::read_admissions(&transaction, plan_id, planned_attempt_ids, storage_root_id)?;
        transaction.commit()?;
        Ok(admissions)
    }
}

mod classify;

#[cfg(test)]
#[path = "candidate_generation_attempt_admission/tests.rs"]
mod tests;
