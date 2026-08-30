//! Ordered live authority over every passed repeatability result.

use std::{collections::HashSet, fmt};

use rewrite_model::{
    CandidateJudgeEvidenceClassV1, CandidateJudgeJoinRecordV1, GenerationRepeatabilityResultId,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityTerminalStageV1,
    GenerationSystemId, MAX_GENERATION_QUALIFICATION_PHASE_ITEMS,
};
use rewrite_types::CancellationToken;

use super::managed_schedule_runner::ManagedJudgeScheduleExecutionAuthorityError;
use super::verified_candidate_judge_join::{VerifiedCandidateJudgeJoin, validate_join_view};

mod complete;
mod resource;
mod resource_phase;

pub use complete::{
    CompletePassedRepeatabilityRelations, VerifiedCompletePassedRepeatabilityJoins,
    VerifiedCompletePassedRepeatabilityJoinsError, verify_complete_passed_repeatability_joins,
};
pub use resource_phase::{
    GenerationQualificationResourcePhaseAuthorityError,
    GenerationQualificationResourcePhaseCompilationError,
    GenerationQualificationResourcePhaseCompiler,
    GenerationQualificationResourcePhaseDerivationError,
    VerifiedGenerationQualificationResourcePhase,
};

/// Stable category for an ordered passed-repeatability join verification failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifiedPassedRepeatabilityJoinsErrorKind {
    /// The supplied result or join count exceeds or differs from the exact closure.
    InvalidCount,
    /// A result, target side, identity, order, or portable join relationship differs.
    Relationship,
    /// Operation cancellation was observed while validating the complete set.
    Cancelled,
    /// One or more retained join authorities failed initial or final validation.
    Authority,
}

/// Redacted aggregate failure from validating every retained passed join.
pub struct VerifiedPassedRepeatabilityJoinsError {
    kind: VerifiedPassedRepeatabilityJoinsErrorKind,
    initial_authority_failure_count: usize,
    relationship_failure_count: usize,
    final_authority_failure_count: usize,
}

impl VerifiedPassedRepeatabilityJoinsError {
    /// Returns the stable primary failure category.
    #[must_use]
    pub const fn kind(&self) -> VerifiedPassedRepeatabilityJoinsErrorKind {
        self.kind
    }

    /// Returns the number of joins whose initial retained authority failed.
    #[must_use]
    pub const fn initial_authority_failure_count(&self) -> usize {
        self.initial_authority_failure_count
    }

    /// Returns the number of portable or target-side relationships that failed.
    #[must_use]
    pub const fn relationship_failure_count(&self) -> usize {
        self.relationship_failure_count
    }

    /// Returns the number of mandatory fresh final validations that failed.
    #[must_use]
    pub const fn final_authority_failure_count(&self) -> usize {
        self.final_authority_failure_count
    }
}

impl fmt::Display for VerifiedPassedRepeatabilityJoinsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "passed repeatability join verification failed: {:?}",
            self.kind
        )
    }
}

impl fmt::Debug for VerifiedPassedRepeatabilityJoinsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPassedRepeatabilityJoinsError")
            .field("kind", &self.kind)
            .field(
                "initial_authority_failure_count",
                &self.initial_authority_failure_count,
            )
            .field(
                "relationship_failure_count",
                &self.relationship_failure_count,
            )
            .field(
                "final_authority_failure_count",
                &self.final_authority_failure_count,
            )
            .finish()
    }
}

impl std::error::Error for VerifiedPassedRepeatabilityJoinsError {}

/// Eval-owned nonserializable authority over the exact ordered passed joins.
///
/// This capability grants no qualification, activation, launch, or request
/// authority. It only proves that each `Passed` portable repeatability result was
/// matched to one distinct live opaque join inside its retained authority bracket.
///
/// ```compile_fail
/// use rewrite_eval::VerifiedPassedRepeatabilityJoins;
///
/// fn clone_capability(value: VerifiedPassedRepeatabilityJoins<'_, '_, '_, '_>) {
///     let _forged = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::VerifiedPassedRepeatabilityJoins;
///
/// fn serialize_capability(value: &VerifiedPassedRepeatabilityJoins<'_, '_, '_, '_>) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct VerifiedPassedRepeatabilityJoins<'store, 'records, 'model, 'runtime> {
    target_generation_system_id: GenerationSystemId,
    ordered_result_ids: Vec<GenerationRepeatabilityResultId>,
    joins: Vec<VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>>,
}

impl VerifiedPassedRepeatabilityJoins<'_, '_, '_, '_> {
    /// Returns the exact target generation-system identity.
    #[must_use]
    pub const fn target_generation_system_id(&self) -> &GenerationSystemId {
        &self.target_generation_system_id
    }

    /// Returns the number of all ordered repeatability results bound by this set.
    #[must_use]
    pub const fn result_count(&self) -> usize {
        self.ordered_result_ids.len()
    }

    /// Returns the number of passed results and retained live joins.
    #[must_use]
    pub const fn passed_join_count(&self) -> usize {
        self.joins.len()
    }

    /// Freshly revalidates the complete ordered result and live-join closure.
    ///
    /// Every held join runs its own initial and mandatory fresh final authority
    /// validation. One failure does not prevent the remaining joins from reaching
    /// their final validation.
    ///
    /// # Errors
    ///
    /// Returns a redacted aggregate error for cancellation, missing, extra,
    /// reordered, duplicated, foreign, stale, or failed authority input.
    pub fn revalidate(
        &mut self,
        target_generation_system_id: &GenerationSystemId,
        ordered_results: &[GenerationRepeatabilityResultRecordV1],
        cancellation: &CancellationToken,
    ) -> Result<(), VerifiedPassedRepeatabilityJoinsError> {
        let stored_matches = self.target_generation_system_id == *target_generation_system_id
            && self.ordered_result_ids.len() == ordered_results.len()
            && self
                .ordered_result_ids
                .iter()
                .zip(ordered_results)
                .all(|(stored, current)| stored == current.repeatability_result_id());
        validate_join_set(
            target_generation_system_id,
            ordered_results,
            &mut self.joins,
            cancellation,
            !stored_matches,
        )
    }
}

impl fmt::Debug for VerifiedPassedRepeatabilityJoins<'_, '_, '_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPassedRepeatabilityJoins")
            .field("result_count", &self.ordered_result_ids.len())
            .field("passed_join_count", &self.joins.len())
            .finish_non_exhaustive()
    }
}

/// Consumes and verifies one ordered live join for every passed result.
///
/// All supplied joins receive their mandatory independent final validation even
/// when another join or relationship fails. The returned capability is only a
/// non-qualifying intermediate authority.
///
/// # Errors
///
/// Returns a redacted aggregate error for cancellation, missing, extra,
/// reordered, duplicated, foreign, stale, or failed authority input.
pub fn verify_passed_repeatability_joins<'store, 'records, 'model, 'runtime>(
    target_generation_system_id: &GenerationSystemId,
    ordered_results: &[GenerationRepeatabilityResultRecordV1],
    mut joins: Vec<VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>>,
    cancellation: &CancellationToken,
) -> Result<
    VerifiedPassedRepeatabilityJoins<'store, 'records, 'model, 'runtime>,
    VerifiedPassedRepeatabilityJoinsError,
> {
    validate_join_set(
        target_generation_system_id,
        ordered_results,
        &mut joins,
        cancellation,
        false,
    )?;
    Ok(VerifiedPassedRepeatabilityJoins {
        target_generation_system_id: target_generation_system_id.clone(),
        ordered_result_ids: ordered_results
            .iter()
            .map(|result| result.repeatability_result_id().clone())
            .collect(),
        joins,
    })
}

#[derive(Clone, Copy, Debug)]
struct JoinBindingError;

#[derive(Default)]
struct ValidationFailures {
    invalid_count: bool,
    static_relationship: bool,
    initial_authority: usize,
    relationship: usize,
    final_authority: usize,
}

fn validate_join_set(
    target_generation_system_id: &GenerationSystemId,
    ordered_results: &[GenerationRepeatabilityResultRecordV1],
    joins: &mut [VerifiedCandidateJudgeJoin<'_, '_, '_, '_>],
    cancellation: &CancellationToken,
    force_relationship_failure: bool,
) -> Result<(), VerifiedPassedRepeatabilityJoinsError> {
    let passed_results = ordered_results
        .iter()
        .filter(|result| result.terminal_stage() == GenerationRepeatabilityTerminalStageV1::Passed)
        .collect::<Vec<_>>();
    let mut failures = ValidationFailures {
        invalid_count: ordered_results.len() > MAX_GENERATION_QUALIFICATION_PHASE_ITEMS
            || joins.len() > MAX_GENERATION_QUALIFICATION_PHASE_ITEMS
            || joins.len() != passed_results.len(),
        static_relationship: force_relationship_failure
            || !valid_result_closure(target_generation_system_id, ordered_results),
        ..ValidationFailures::default()
    };
    let force_callback_failure = failures.invalid_count || failures.static_relationship;
    for (index, join) in joins.iter_mut().enumerate() {
        let expected = passed_results.get(index).copied();
        let bracket = join.with_revalidated_authorities(cancellation, |view| {
            validate_join_view(&view, cancellation).map_err(|_error| JoinBindingError)?;
            if force_callback_failure
                || expected.is_none_or(|result| {
                    !join_matches_passed_result(result, target_generation_system_id, view.record)
                })
            {
                Err(JoinBindingError)
            } else {
                Ok(())
            }
        });
        failures.record(bracket);
    }
    failures.finish(cancellation.is_cancelled())
}

fn valid_result_closure(
    target_generation_system_id: &GenerationSystemId,
    ordered_results: &[GenerationRepeatabilityResultRecordV1],
) -> bool {
    let mut seen = HashSet::with_capacity(ordered_results.len());
    ordered_results.iter().all(|result| {
        result.generation_system_id() == target_generation_system_id
            && seen.insert(result.repeatability_result_id().digest().as_str())
            && valid_result_shape(result)
    })
}

fn valid_result_shape(result: &GenerationRepeatabilityResultRecordV1) -> bool {
    let receipt = result.candidate_generation_receipt_set_id().is_some();
    let deterministic = result.candidate_deterministic_evaluation_id().is_some();
    let join = result.candidate_judge_join_id().is_some();
    match result.terminal_stage() {
        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed => {
            !receipt && !deterministic && !join
        }
        GenerationRepeatabilityTerminalStageV1::DeterministicFailed
        | GenerationRepeatabilityTerminalStageV1::JudgeFailed => receipt && deterministic && !join,
        GenerationRepeatabilityTerminalStageV1::Passed => receipt && deterministic && join,
    }
}

fn join_matches_passed_result(
    result: &GenerationRepeatabilityResultRecordV1,
    target_generation_system_id: &GenerationSystemId,
    join: &CandidateJudgeJoinRecordV1,
) -> bool {
    let target_receipt = match (
        join.candidate_a_generation_system_id() == target_generation_system_id,
        join.candidate_b_generation_system_id() == target_generation_system_id,
    ) {
        (true, false) => Some(join.candidate_a_receipt_set_id()),
        (false, true) => Some(join.candidate_b_receipt_set_id()),
        (true, true) | (false, false) => None,
    };
    BindingFacts {
        result: RelationMatch::from_bool(
            result.terminal_stage() == GenerationRepeatabilityTerminalStageV1::Passed,
        ),
        target_receipt: RelationMatch::from_bool(
            target_receipt == result.candidate_generation_receipt_set_id(),
        ),
        deterministic: RelationMatch::from_bool(
            Some(join.deterministic_evaluation_id())
                == result.candidate_deterministic_evaluation_id(),
        ),
        join: RelationMatch::from_bool(
            Some(join.candidate_judge_join_id()) == result.candidate_judge_join_id(),
        ),
        evidence_class: RelationMatch::from_bool(
            join.evidence_class() == CandidateJudgeEvidenceClassV1::ManagedLocalJudgeTriage,
        ),
        claims: RelationMatch::from_bool(
            !join.candidate_semantics_proven()
                && !join.judge_correctness_proven()
                && !join.qualified(),
        ),
    }
    .is_valid()
}

#[derive(Clone, Copy)]
struct BindingFacts {
    result: RelationMatch,
    target_receipt: RelationMatch,
    deterministic: RelationMatch,
    join: RelationMatch,
    evidence_class: RelationMatch,
    claims: RelationMatch,
}

impl BindingFacts {
    const fn is_valid(self) -> bool {
        self.result.is_exact()
            && self.target_receipt.is_exact()
            && self.deterministic.is_exact()
            && self.join.is_exact()
            && self.evidence_class.is_exact()
            && self.claims.is_exact()
    }
}

#[derive(Clone, Copy)]
enum RelationMatch {
    Exact,
    Mismatch,
}

impl RelationMatch {
    const fn from_bool(matches: bool) -> Self {
        if matches { Self::Exact } else { Self::Mismatch }
    }

    const fn is_exact(self) -> bool {
        matches!(self, Self::Exact)
    }
}

impl ValidationFailures {
    fn record(
        &mut self,
        result: Result<(), ManagedJudgeScheduleExecutionAuthorityError<JoinBindingError>>,
    ) {
        match result {
            Ok(()) => {}
            Err(ManagedJudgeScheduleExecutionAuthorityError::Initial(_failures)) => {
                self.initial_authority += 1;
            }
            Err(ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal { .. }) => {
                self.initial_authority += 1;
                self.final_authority += 1;
            }
            Err(ManagedJudgeScheduleExecutionAuthorityError::Callback(_error)) => {
                self.relationship += 1;
            }
            Err(ManagedJudgeScheduleExecutionAuthorityError::Final(_failures)) => {
                self.final_authority += 1;
            }
            Err(ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal { .. }) => {
                self.relationship += 1;
                self.final_authority += 1;
            }
        }
    }

    fn finish(self, cancelled: bool) -> Result<(), VerifiedPassedRepeatabilityJoinsError> {
        let kind = if self.invalid_count {
            Some(VerifiedPassedRepeatabilityJoinsErrorKind::InvalidCount)
        } else if self.static_relationship || self.relationship > 0 {
            Some(VerifiedPassedRepeatabilityJoinsErrorKind::Relationship)
        } else if cancelled {
            Some(VerifiedPassedRepeatabilityJoinsErrorKind::Cancelled)
        } else if self.initial_authority > 0 || self.final_authority > 0 {
            Some(VerifiedPassedRepeatabilityJoinsErrorKind::Authority)
        } else {
            None
        };
        if let Some(kind) = kind {
            Err(VerifiedPassedRepeatabilityJoinsError {
                kind,
                initial_authority_failure_count: self.initial_authority,
                relationship_failure_count: self.relationship
                    + usize::from(self.static_relationship),
                final_authority_failure_count: self.final_authority,
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "verified_repeatability_joins/tests.rs"]
mod tests;
