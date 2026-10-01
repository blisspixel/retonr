//! Independently held Prepared scope for an existing retained batch authority.

use rewrite_model::{
    CandidateSelectionPolicyV1, GenerationQualificationPlanV1, GenerationRepetitionRecordV1,
    GenerationSuiteManifestV1, GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};
use rewrite_types::CancellationToken;

use super::{
    VerifiedCandidateBatchSet, VerifiedCandidateBatchSetCore, VerifiedCandidateBatchSetError,
    VerifiedCandidateBatchSetRelationship as Relationship,
};

#[derive(Clone, Copy)]
pub(crate) struct CandidateBatchSetScope<'a> {
    pub(crate) plan: &'a GenerationQualificationPlanV1,
    pub(crate) suite: &'a GenerationSuiteManifestV1,
    pub(crate) repetition: &'a GenerationRepetitionRecordV1,
    pub(crate) system: &'a GenerationSystemRecordV1,
    pub(crate) selection_policy: &'a CandidateSelectionPolicyV1,
    pub(crate) planned_attempts: &'a [PlannedCandidateAttemptV1],
}

impl VerifiedCandidateBatchSet {
    pub(crate) fn validate_prepared_scope(
        &self,
        scope: CandidateBatchSetScope<'_>,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCandidateBatchSetError> {
        self.authority.validate_scope(scope, cancellation)
    }
}

pub(super) fn validate_core_scope<B>(
    core: &VerifiedCandidateBatchSetCore<B>,
    scope: CandidateBatchSetScope<'_>,
) -> Result<(), Relationship> {
    if &core.qualification_plan != scope.plan || &core.suite != scope.suite {
        return Err(Relationship::QualificationClosure);
    }
    if &core.repetition != scope.repetition {
        return Err(Relationship::RepetitionClosure);
    }
    if &core.generation_system != scope.system {
        return Err(Relationship::GenerationSystemClosure);
    }
    if &core.selection_policy != scope.selection_policy {
        return Err(Relationship::SelectionClosure);
    }
    if core.planned_attempts != scope.planned_attempts {
        return Err(Relationship::PlannedAttemptClosure);
    }
    Ok(())
}
