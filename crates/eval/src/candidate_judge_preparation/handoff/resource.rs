//! Narrow resource-result view retained behind the candidate judge handoff.

use std::fmt;

use rewrite_model::{
    CandidateGenerationReceiptSetV1, GenerationQualificationOperationPolicyV1,
    GenerationRepetitionId, GenerationResourceAttemptResultRecordV1, GenerationSystemId,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::CandidateJudgeRunnerHandoff;
use crate::{
    CandidateJudgePreparationSide, VerifiedCandidateBatchSet, VerifiedCandidateBatchSetError,
};

/// Exact target-side resource results borrowed from one retained judge handoff.
pub(crate) struct CandidateJudgeTargetResourceResults<'a> {
    results: Vec<&'a GenerationResourceAttemptResultRecordV1>,
}

impl<'a> CandidateJudgeTargetResourceResults<'a> {
    pub(crate) fn records(&self) -> &[&'a GenerationResourceAttemptResultRecordV1] {
        &self.results
    }
}

/// Closed relationship rejected while selecting target resource results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateJudgeResourceRelationship {
    /// Candidate sides were not the exact target and baseline pair.
    CandidateSides,
    /// One side named another repetition.
    Repetition,
    /// The target side was not uniformly resource observed.
    TargetResourceMode,
    /// The baseline side carried unexpected resource results.
    BaselineResourceMode,
    /// Target results were missing, extra, or outside semantic suite order.
    ResourceResultClosure,
    /// Candidate receipt sets named another qualification plan or suite.
    OperationScope,
}

/// Content-redacted failure while opening the target resource view.
#[derive(Error)]
pub(crate) enum CandidateJudgeResourceViewError {
    /// Cooperative cancellation was observed.
    #[error("candidate judge resource view was cancelled")]
    Cancelled,
    /// One retained candidate set failed fresh validation.
    #[error("candidate judge resource candidate authority failed for {side:?}")]
    CandidateBatchSet {
        /// Identity-significant candidate side.
        side: CandidateJudgePreparationSide,
        /// Typed retained-authority failure.
        #[source]
        source: VerifiedCandidateBatchSetError,
    },
    /// One exact target, baseline, repetition, mode, or result relationship differed.
    #[error("candidate judge resource relationship does not match: {0:?}")]
    Relationship(CandidateJudgeResourceRelationship),
}

impl fmt::Debug for CandidateJudgeResourceViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CandidateJudgeResourceViewError");
        match self {
            Self::Cancelled => debug.field("kind", &"cancelled"),
            Self::CandidateBatchSet { side, .. } => debug
                .field("kind", &"candidate_batch_set")
                .field("side", side),
            Self::Relationship(relationship) => debug
                .field("kind", &"relationship")
                .field("relationship", relationship),
        };
        debug.finish_non_exhaustive()
    }
}

impl CandidateJudgeResourceViewError {
    pub(crate) const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

impl CandidateJudgeRunnerHandoff<'_> {
    /// Revalidates the exact target, baseline, repetition, plan, and suite scope.
    pub(crate) fn revalidate_operation_scope(
        &self,
        operation_policy: &GenerationQualificationOperationPolicyV1,
        repetition_id: &GenerationRepetitionId,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgeResourceViewError> {
        self.target_resource_results(
            operation_policy.target_generation_system_id(),
            operation_policy.baseline_generation_system_id(),
            repetition_id,
            cancellation,
        )?;
        let candidate_a = side_view(
            CandidateJudgePreparationSide::CandidateA,
            &self.candidate_a,
            cancellation,
        )?;
        let candidate_b = side_view(
            CandidateJudgePreparationSide::CandidateB,
            &self.candidate_b,
            cancellation,
        )?;
        if operation_scope_matches(
            [candidate_a.receipt_set, candidate_b.receipt_set]
                .into_iter()
                .map(|receipt_set| {
                    receipt_set.qualification_plan_id()
                        == operation_policy.generation_qualification_plan_id()
                        && receipt_set.suite_manifest_id() == operation_policy.suite_manifest_id()
                }),
        ) {
            ensure_active(cancellation)
        } else {
            Err(CandidateJudgeResourceViewError::Relationship(
                CandidateJudgeResourceRelationship::OperationScope,
            ))
        }
    }

    /// Opens the exact resource-observed target set without exposing live observations.
    pub(crate) fn target_resource_results<'a>(
        &'a self,
        target_generation_system_id: &GenerationSystemId,
        baseline_generation_system_id: &GenerationSystemId,
        repetition_id: &GenerationRepetitionId,
        cancellation: &CancellationToken,
    ) -> Result<CandidateJudgeTargetResourceResults<'a>, CandidateJudgeResourceViewError> {
        ensure_active(cancellation)?;
        let candidate_a = side_view(
            CandidateJudgePreparationSide::CandidateA,
            &self.candidate_a,
            cancellation,
        )?;
        let candidate_b = side_view(
            CandidateJudgePreparationSide::CandidateB,
            &self.candidate_b,
            cancellation,
        )?;
        let roles = select_roles(
            role(
                candidate_a.receipt_set.generation_system_id(),
                target_generation_system_id,
                baseline_generation_system_id,
            ),
            role(
                candidate_b.receipt_set.generation_system_id(),
                target_generation_system_id,
                baseline_generation_system_id,
            ),
        )?;
        let (target, baseline) = match roles {
            TargetSide::CandidateA => (&candidate_a, &candidate_b),
            TargetSide::CandidateB => (&candidate_b, &candidate_a),
        };
        validate_repetition(target, baseline, repetition_id)?;
        let target_results =
            target
                .results
                .as_ref()
                .ok_or(CandidateJudgeResourceViewError::Relationship(
                    CandidateJudgeResourceRelationship::TargetResourceMode,
                ))?;
        if baseline.results.is_some() {
            return Err(CandidateJudgeResourceViewError::Relationship(
                CandidateJudgeResourceRelationship::BaselineResourceMode,
            ));
        }
        validate_result_closure(target.receipt_set, target_results)?;
        ensure_active(cancellation)?;
        Ok(CandidateJudgeTargetResourceResults {
            results: target_results.clone(),
        })
    }
}

struct CandidateSideView<'a> {
    receipt_set: &'a CandidateGenerationReceiptSetV1,
    results: Option<Vec<&'a GenerationResourceAttemptResultRecordV1>>,
}

fn side_view<'a>(
    side: CandidateJudgePreparationSide,
    set: &'a VerifiedCandidateBatchSet,
    cancellation: &CancellationToken,
) -> Result<CandidateSideView<'a>, CandidateJudgeResourceViewError> {
    let receipt_set = set
        .receipt_set(cancellation)
        .map_err(|source| CandidateJudgeResourceViewError::CandidateBatchSet { side, source })?;
    let results = set
        .resource_results(cancellation)
        .map_err(|source| CandidateJudgeResourceViewError::CandidateBatchSet { side, source })?;
    Ok(CandidateSideView {
        receipt_set,
        results,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SideRole {
    Target,
    Baseline,
    Foreign,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetSide {
    CandidateA,
    CandidateB,
}

fn role(
    observed: &GenerationSystemId,
    target: &GenerationSystemId,
    baseline: &GenerationSystemId,
) -> SideRole {
    if observed == target {
        SideRole::Target
    } else if observed == baseline {
        SideRole::Baseline
    } else {
        SideRole::Foreign
    }
}

fn select_roles(
    candidate_a: SideRole,
    candidate_b: SideRole,
) -> Result<TargetSide, CandidateJudgeResourceViewError> {
    match (candidate_a, candidate_b) {
        (SideRole::Target, SideRole::Baseline) => Ok(TargetSide::CandidateA),
        (SideRole::Baseline, SideRole::Target) => Ok(TargetSide::CandidateB),
        _ => Err(CandidateJudgeResourceViewError::Relationship(
            CandidateJudgeResourceRelationship::CandidateSides,
        )),
    }
}

fn validate_repetition(
    target: &CandidateSideView<'_>,
    baseline: &CandidateSideView<'_>,
    expected: &GenerationRepetitionId,
) -> Result<(), CandidateJudgeResourceViewError> {
    if target.receipt_set.repetition_id() == expected
        && baseline.receipt_set.repetition_id() == expected
    {
        Ok(())
    } else {
        Err(CandidateJudgeResourceViewError::Relationship(
            CandidateJudgeResourceRelationship::Repetition,
        ))
    }
}

fn validate_result_closure(
    receipt_set: &CandidateGenerationReceiptSetV1,
    results: &[&GenerationResourceAttemptResultRecordV1],
) -> Result<(), CandidateJudgeResourceViewError> {
    let matches = result_closure_matches(
        results.len(),
        receipt_set.entries().len(),
        results
            .iter()
            .zip(receipt_set.entries())
            .map(|(result, receipt)| {
                result.generation_system_id() == receipt_set.generation_system_id()
                    && result.generation_qualification_plan_id()
                        == receipt_set.qualification_plan_id()
                    && result.suite_manifest_id() == receipt_set.suite_manifest_id()
                    && result.repetition_id() == receipt_set.repetition_id()
                    && result.case_id() == receipt.case_id()
                    && result.planned_attempt_id() == receipt.planned_attempt_id()
                    && result.attempt_record_id() == receipt.attempt_record_id()
                    && result.candidate_generation_receipt_id() == receipt.receipt_id()
            }),
    );
    if matches {
        Ok(())
    } else {
        Err(CandidateJudgeResourceViewError::Relationship(
            CandidateJudgeResourceRelationship::ResourceResultClosure,
        ))
    }
}

fn result_closure_matches(
    result_count: usize,
    receipt_count: usize,
    relationships: impl IntoIterator<Item = bool>,
) -> bool {
    result_count == receipt_count && relationships.into_iter().all(|matches| matches)
}

fn operation_scope_matches(relationships: impl IntoIterator<Item = bool>) -> bool {
    relationships.into_iter().all(|matches| matches)
}

fn ensure_active(cancellation: &CancellationToken) -> Result<(), CandidateJudgeResourceViewError> {
    if cancellation.is_cancelled() {
        Err(CandidateJudgeResourceViewError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CandidateJudgeResourceRelationship as Relationship, CandidateJudgeResourceViewError,
        SideRole, TargetSide, operation_scope_matches, result_closure_matches, select_roles,
    };

    #[test]
    fn exact_target_side_is_independent_of_candidate_position() {
        assert!(matches!(
            select_roles(SideRole::Target, SideRole::Baseline),
            Ok(TargetSide::CandidateA)
        ));
        assert!(matches!(
            select_roles(SideRole::Baseline, SideRole::Target),
            Ok(TargetSide::CandidateB)
        ));
    }

    #[test]
    fn duplicate_missing_and_foreign_roles_fail_closed() {
        for roles in [
            (SideRole::Target, SideRole::Target),
            (SideRole::Baseline, SideRole::Baseline),
            (SideRole::Target, SideRole::Foreign),
            (SideRole::Foreign, SideRole::Baseline),
        ] {
            assert!(matches!(
                select_roles(roles.0, roles.1),
                Err(CandidateJudgeResourceViewError::Relationship(
                    Relationship::CandidateSides
                ))
            ));
        }
    }

    #[test]
    fn resource_closure_rejects_missing_extra_reordered_and_stale_results() {
        assert!(result_closure_matches(2, 2, [true, true]));
        assert!(!result_closure_matches(1, 2, [true]));
        assert!(!result_closure_matches(3, 2, [true, true, true]));
        assert!(!result_closure_matches(2, 2, [false, false]));
        assert!(!result_closure_matches(2, 2, [true, false]));
    }

    #[test]
    fn both_candidate_sides_must_match_the_operation_plan_and_suite() {
        assert!(operation_scope_matches([true, true]));
        assert!(!operation_scope_matches([false, true]));
        assert!(!operation_scope_matches([true, false]));
    }
}
