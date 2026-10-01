//! Scoped projection of retained joins for durable complete settlement.

use super::{CompletePassedRepeatabilityRelations, VerifiedCompletePassedRepeatabilityJoins};
use crate::VerifiedCandidateJudgeJoin;
use rewrite_types::CancellationToken;

impl<'store, 'records, 'model, 'runtime>
    VerifiedCompletePassedRepeatabilityJoins<'store, 'records, 'model, 'runtime>
{
    pub(crate) fn with_settlement_joins<T, E>(
        &mut self,
        cancellation: &CancellationToken,
        use_joins: impl FnOnce(
            CompletePassedRepeatabilityRelations<'_>,
            &rewrite_model::GenerationRepeatabilityEvidenceManifestV1,
            &mut [VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>],
        ) -> Result<T, E>,
    ) -> Result<Result<T, E>, ()> {
        self.revalidate(cancellation).map_err(|_| ())?;
        let relations = CompletePassedRepeatabilityRelations {
            operation_policy: &self.operation_policy,
            scope: rewrite_model::GenerationQualificationPhaseScopeV1 {
                generation_system: &self.target_generation_system,
                qualification_plan: &self.qualification_plan,
                suite: &self.suite,
            },
            planned_attempts: &self.planned_attempts,
            attempt_ledger_manifest: &self.attempt_ledger_manifest,
            preregistered_repetitions: &self.preregistered_repetitions,
            ordered_results: &self.ordered_results,
        };
        let result = use_joins(relations, &self.manifest, &mut self.joins.joins);
        self.revalidate(cancellation).map_err(|_| ())?;
        Ok(result)
    }

    pub(crate) fn revalidate_for_mandatory_settlement_finalization(&mut self) -> Result<(), ()> {
        let fresh = CancellationToken::new();
        // Independently visit every retained join even if phase relationships fail.
        let joins = self.joins.revalidate(
            self.target_generation_system.generation_system_id(),
            &self.ordered_results,
            &fresh,
        );
        let complete = self.revalidate(&fresh);
        if joins.is_ok() && complete.is_ok() {
            Ok(())
        } else {
            Err(())
        }
    }
}
