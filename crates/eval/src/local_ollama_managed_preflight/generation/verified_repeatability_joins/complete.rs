//! Complete passed-repeatability authority over every preregistered repetition.

use std::fmt;

use rewrite_model::{
    GenerationAttemptLedgerManifestV1, GenerationQualificationOperationPolicyV1,
    GenerationQualificationPhaseEvidenceError, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationQualificationPlanV1,
    GenerationRepeatabilityEvidenceManifestV1, GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityTerminalStageV1,
    GenerationRepetitionRecordV1, GenerationResourceAttemptResultRecordV1,
    GenerationSuiteManifestV1, GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{VerifiedPassedRepeatabilityJoins, VerifiedPassedRepeatabilityJoinsError};

/// Exact inert records used to prove a complete passed repeatability closure.
#[derive(Clone, Copy)]
pub struct CompletePassedRepeatabilityRelations<'a> {
    /// Exact preregistered operation policy naming this target and phase policy.
    pub operation_policy: &'a GenerationQualificationOperationPolicyV1,
    /// Exact target, plan, and suite scope.
    pub scope: GenerationQualificationPhaseScopeV1<'a>,
    /// Every planned attempt in frozen plan order.
    pub planned_attempts: &'a [PlannedCandidateAttemptV1],
    /// Exact complete attempt ledger named by every repeatability result.
    pub attempt_ledger_manifest: &'a GenerationAttemptLedgerManifestV1,
    /// Every preregistered repetition in semantic order.
    pub preregistered_repetitions: &'a [GenerationRepetitionRecordV1],
    /// One exact Passed result for every preregistered repetition.
    pub ordered_results: &'a [GenerationRepeatabilityResultRecordV1],
}

/// Nonforgeable authority over the complete all-Passed repeatability closure.
///
/// The authority owns every inert relationship and the live join set. It grants
/// no resource-phase passage, qualification, activation, launch, or traffic.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCompletePassedRepeatabilityJoins;
///
/// fn clone_authority(value: VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>) {
///     let _forged = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedCompletePassedRepeatabilityJoins;
///
/// fn serialize_authority(value: &VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("authority must not serialize");
/// }
/// ```
pub struct VerifiedCompletePassedRepeatabilityJoins<'store, 'records, 'model, 'runtime> {
    operation_policy: GenerationQualificationOperationPolicyV1,
    target_generation_system: GenerationSystemRecordV1,
    qualification_plan: GenerationQualificationPlanV1,
    suite: GenerationSuiteManifestV1,
    planned_attempts: Vec<PlannedCandidateAttemptV1>,
    attempt_ledger_manifest: GenerationAttemptLedgerManifestV1,
    preregistered_repetitions: Vec<GenerationRepetitionRecordV1>,
    ordered_results: Vec<GenerationRepeatabilityResultRecordV1>,
    manifest: GenerationRepeatabilityEvidenceManifestV1,
    joins: VerifiedPassedRepeatabilityJoins<'store, 'records, 'model, 'runtime>,
}

impl VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_> {
    /// Returns the exact complete Passed repeatability manifest.
    #[must_use]
    pub const fn repeatability_manifest(&self) -> &GenerationRepeatabilityEvidenceManifestV1 {
        &self.manifest
    }

    /// Returns the number of preregistered repetitions and retained joins.
    #[must_use]
    pub const fn repetition_count(&self) -> usize {
        self.preregistered_repetitions.len()
    }

    /// Freshly revalidates the complete inert closure and every retained join.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for cancellation, any incomplete, reordered,
    /// substituted, non-Passed result closure, or any failed live join authority.
    pub fn revalidate(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedCompletePassedRepeatabilityJoinsError> {
        check_active(cancellation)?;
        validate_operation_scope(&self.operation_policy, self.relations().scope)?;
        let expected = derive_manifest(self.relations())?;
        if expected != self.manifest {
            return Err(VerifiedCompletePassedRepeatabilityJoinsError::Relationship);
        }
        self.joins
            .revalidate(
                self.target_generation_system.generation_system_id(),
                &self.ordered_results,
                cancellation,
            )
            .map_err(VerifiedCompletePassedRepeatabilityJoinsError::Joins)?;
        check_active(cancellation)
    }

    pub(super) fn collect_resource_results(
        &mut self,
        baseline_generation_system_id: &rewrite_model::GenerationSystemId,
        cancellation: &CancellationToken,
    ) -> Result<
        Vec<GenerationResourceAttemptResultRecordV1>,
        VerifiedCompletePassedRepeatabilityJoinsError,
    > {
        self.revalidate(cancellation)?;
        let results = self
            .joins
            .collect_target_resource_results(
                self.target_generation_system.generation_system_id(),
                baseline_generation_system_id,
                &self.ordered_results,
                cancellation,
            )
            .map_err(VerifiedCompletePassedRepeatabilityJoinsError::Joins)?;
        check_active(cancellation)?;
        Ok(results)
    }

    pub(super) const fn target_generation_system(&self) -> &GenerationSystemRecordV1 {
        &self.target_generation_system
    }

    pub(super) const fn operation_policy(&self) -> &GenerationQualificationOperationPolicyV1 {
        &self.operation_policy
    }

    pub(super) const fn qualification_plan(&self) -> &GenerationQualificationPlanV1 {
        &self.qualification_plan
    }

    pub(super) const fn suite(&self) -> &GenerationSuiteManifestV1 {
        &self.suite
    }

    pub(super) fn planned_attempts(&self) -> &[PlannedCandidateAttemptV1] {
        &self.planned_attempts
    }

    pub(super) fn preregistered_repetitions(&self) -> &[GenerationRepetitionRecordV1] {
        &self.preregistered_repetitions
    }

    fn relations(&self) -> CompletePassedRepeatabilityRelations<'_> {
        CompletePassedRepeatabilityRelations {
            operation_policy: &self.operation_policy,
            scope: GenerationQualificationPhaseScopeV1 {
                generation_system: &self.target_generation_system,
                qualification_plan: &self.qualification_plan,
                suite: &self.suite,
            },
            planned_attempts: &self.planned_attempts,
            attempt_ledger_manifest: &self.attempt_ledger_manifest,
            preregistered_repetitions: &self.preregistered_repetitions,
            ordered_results: &self.ordered_results,
        }
    }
}

impl fmt::Debug for VerifiedCompletePassedRepeatabilityJoins<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedCompletePassedRepeatabilityJoins")
            .field("repetition_count", &self.preregistered_repetitions.len())
            .field(
                "repeatability_manifest_id",
                self.manifest.repeatability_evidence_manifest_id(),
            )
            .finish_non_exhaustive()
    }
}

/// Content-free complete passed-repeatability verification failure.
#[derive(Debug, Error)]
pub enum VerifiedCompletePassedRepeatabilityJoinsError {
    /// Cooperative cancellation was observed.
    #[error("complete passed repeatability verification was cancelled")]
    Cancelled,
    /// The inert repeatability closure was incomplete or inconsistent.
    #[error("complete passed repeatability relationship does not match")]
    Relationship,
    /// The model phase-evidence contract rejected the exact closure.
    #[error("complete passed repeatability manifest construction failed")]
    PhaseEvidence(#[source] GenerationQualificationPhaseEvidenceError),
    /// One or more retained live joins failed fresh verification.
    #[error("complete passed repeatability join authority failed")]
    Joins(#[source] VerifiedPassedRepeatabilityJoinsError),
}

/// Consumes an ordered join set and proves every preregistered repetition Passed.
///
/// # Errors
///
/// Returns a content-free error for cancellation, an arbitrary subset or prefix,
/// any missing, extra, reordered, non-Passed result, or failed live join authority.
pub fn verify_complete_passed_repeatability_joins<'store, 'records, 'model, 'runtime>(
    relations: CompletePassedRepeatabilityRelations<'_>,
    mut joins: VerifiedPassedRepeatabilityJoins<'store, 'records, 'model, 'runtime>,
    cancellation: &CancellationToken,
) -> Result<
    VerifiedCompletePassedRepeatabilityJoins<'store, 'records, 'model, 'runtime>,
    VerifiedCompletePassedRepeatabilityJoinsError,
> {
    check_active(cancellation)?;
    validate_operation_scope(relations.operation_policy, relations.scope)?;
    let manifest = derive_manifest(relations)?;
    joins
        .revalidate(
            relations.scope.generation_system.generation_system_id(),
            relations.ordered_results,
            cancellation,
        )
        .map_err(VerifiedCompletePassedRepeatabilityJoinsError::Joins)?;
    check_active(cancellation)?;
    Ok(VerifiedCompletePassedRepeatabilityJoins {
        operation_policy: relations.operation_policy.clone(),
        target_generation_system: relations.scope.generation_system.clone(),
        qualification_plan: relations.scope.qualification_plan.clone(),
        suite: relations.scope.suite.clone(),
        planned_attempts: relations.planned_attempts.to_vec(),
        attempt_ledger_manifest: relations.attempt_ledger_manifest.clone(),
        preregistered_repetitions: relations.preregistered_repetitions.to_vec(),
        ordered_results: relations.ordered_results.to_vec(),
        manifest,
        joins,
    })
}

fn derive_manifest(
    relations: CompletePassedRepeatabilityRelations<'_>,
) -> Result<GenerationRepeatabilityEvidenceManifestV1, VerifiedCompletePassedRepeatabilityJoinsError>
{
    if !attempt_ledger_matches(relations)
        || !complete_result_closure_matches(
            relations.preregistered_repetitions.len(),
            relations.ordered_results.len(),
            relations
                .ordered_results
                .iter()
                .zip(relations.preregistered_repetitions)
                .map(|(result, repetition)| {
                    result.terminal_stage() == GenerationRepeatabilityTerminalStageV1::Passed
                        && result.repetition_id() == repetition.repetition_id()
                }),
        )
    {
        return Err(VerifiedCompletePassedRepeatabilityJoinsError::Relationship);
    }
    GenerationRepeatabilityEvidenceManifestV1::new(
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope: relations.scope,
            phase_policy_digest: relations.operation_policy.repeatability_policy_digest(),
            planned_attempts: relations.planned_attempts,
            preregistered_repetitions: relations.preregistered_repetitions,
            results: relations.ordered_results,
            status: GenerationQualificationPhaseStatusV1::Passed,
        },
    )
    .map_err(VerifiedCompletePassedRepeatabilityJoinsError::PhaseEvidence)
}

fn attempt_ledger_matches(relations: CompletePassedRepeatabilityRelations<'_>) -> bool {
    let ledger = relations.attempt_ledger_manifest;
    ledger.generation_system_id() == relations.scope.generation_system.generation_system_id()
        && ledger.generation_qualification_plan_id()
            == relations.scope.qualification_plan.qualification_plan_id()
        && ledger.suite_manifest_id() == relations.scope.suite.suite_manifest_id()
        && ledger.phase_policy_digest() == relations.operation_policy.attempt_ledger_policy_digest()
        && ledger.status() == GenerationQualificationPhaseStatusV1::Passed
        && relations.ordered_results.iter().all(|result| {
            result.attempt_ledger_manifest_id() == ledger.attempt_ledger_manifest_id()
                && result.attempt_ledger_root_digest() == ledger.evidence_root_digest()
        })
}

fn validate_operation_scope(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    scope: GenerationQualificationPhaseScopeV1<'_>,
) -> Result<(), VerifiedCompletePassedRepeatabilityJoinsError> {
    if operation_policy.target_generation_system_id()
        == scope.generation_system.generation_system_id()
        && operation_policy.baseline_generation_system_id()
            != operation_policy.target_generation_system_id()
        && operation_policy.generation_qualification_plan_id()
            == scope.qualification_plan.qualification_plan_id()
        && operation_policy.suite_manifest_id() == scope.suite.suite_manifest_id()
    {
        Ok(())
    } else {
        Err(VerifiedCompletePassedRepeatabilityJoinsError::Relationship)
    }
}

fn complete_result_closure_matches(
    expected_count: usize,
    observed_count: usize,
    relationships: impl IntoIterator<Item = bool>,
) -> bool {
    expected_count == observed_count && relationships.into_iter().all(|matches| matches)
}

fn check_active(
    cancellation: &CancellationToken,
) -> Result<(), VerifiedCompletePassedRepeatabilityJoinsError> {
    if cancellation.is_cancelled() {
        Err(VerifiedCompletePassedRepeatabilityJoinsError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "complete/tests.rs"]
mod tests;
