use std::{convert::Infallible, fmt};

use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateJudgePlanV1, CandidateJudgePresentationV1,
    CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1, GenerationSystemRecordV1,
};
use rewrite_types::{CancellationToken, Digest};

use super::request::{attempt_limits, build_exact_attempt, selected_candidates};
use super::{
    CandidateJudgePreparationError, CandidateJudgePreparationSide, PreparedCandidateJudgeRun,
    PreparedCompatibilityProjection, revalidate_prepared_authorities,
};
use crate::active_generation_qualification_subject::{
    ActiveGenerationQualificationBinding, ActiveGenerationQualificationSubject,
};
use crate::generation_case_material::VerifiedGenerationCaseMaterial;
use crate::{LocalJudgeRubric, VerifiedCandidateBatchSet};

mod error;
mod join;
mod resource;
mod settlement;
mod traffic;
mod triage;

pub(crate) use error::CandidateJudgeRunnerRequestError;
use error::{request_error_into_preparation, resolve_traversal};
pub(crate) use join::CandidateJudgeJoinCompilationError;
#[cfg(test)]
pub(crate) use join::CandidateJudgeJoinCompilationRelationship;
pub(crate) use resource::CandidateJudgeResourceViewError;
pub(crate) use traffic::CandidateJudgeTrafficRequest;
pub(crate) use triage::{CandidateJudgeTriageCompilationError, CompiledCandidateJudgeTriage};

/// Consuming authority for the exact managed candidate-judge runner.
///
/// The handoff retains both candidate sets and all case material. Its public
/// surface exposes only inert records and content-free identities. Exact source,
/// candidate, and prompt bytes are available only through the crate-private
/// callback used by the managed runner.
///
/// ```compile_fail
/// use rewrite_eval::CandidateJudgeRunnerHandoff;
///
/// fn clone_capability(value: CandidateJudgeRunnerHandoff<'_>) {
///     let _copy = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use rewrite_eval::CandidateJudgeRunnerHandoff;
///
/// fn serialize_capability(value: &CandidateJudgeRunnerHandoff<'_>) {
///     let _bytes = serde_json::to_vec(value).expect("capability must not serialize");
/// }
/// ```
pub struct CandidateJudgeRunnerHandoff<'store> {
    candidate_a: VerifiedCandidateBatchSet,
    candidate_b: VerifiedCandidateBatchSet,
    case_material: VerifiedGenerationCaseMaterial<'store>,
    deterministic_evaluation: CandidateDeterministicEvaluationRecordV1,
    judge_plan: CandidateJudgePlanV1,
    judge_schedule: CandidateJudgeScheduleV1,
    request_aggregate: CandidateJudgeRequestAggregateV1,
    compatibility_projection: PreparedCompatibilityProjection,
    rubric: LocalJudgeRubric,
    judge_system: GenerationSystemRecordV1,
    eligible_semantic_indices: Vec<usize>,
}

impl CandidateJudgeRunnerHandoff<'_> {
    pub(crate) fn active_binding(&self) -> Option<&ActiveGenerationQualificationBinding> {
        self.candidate_a.active_binding()
    }

    pub(crate) fn matches_active_subject(
        &self,
        subject: &ActiveGenerationQualificationSubject,
    ) -> bool {
        self.candidate_a.matches_active_subject(subject)
            && self.candidate_b.matches_active_subject(subject)
    }

    /// Returns the passed deterministic gate record.
    #[must_use]
    pub const fn deterministic_evaluation(&self) -> &CandidateDeterministicEvaluationRecordV1 {
        &self.deterministic_evaluation
    }

    /// Returns the exact reloaded judge plan.
    #[must_use]
    pub const fn judge_plan(&self) -> &CandidateJudgePlanV1 {
        &self.judge_plan
    }

    /// Returns the exact schedule in execution order.
    #[must_use]
    pub const fn judge_schedule(&self) -> &CandidateJudgeScheduleV1 {
        &self.judge_schedule
    }

    /// Returns all first-pass request identities in exact schedule order.
    #[must_use]
    pub const fn request_aggregate(&self) -> &CandidateJudgeRequestAggregateV1 {
        &self.request_aggregate
    }

    /// Returns the exact judge generation-system record.
    #[must_use]
    pub const fn judge_system(&self) -> &GenerationSystemRecordV1 {
        &self.judge_system
    }

    /// Returns the frozen compatibility plan digest.
    #[must_use]
    pub const fn compatibility_plan_digest(&self) -> &Digest {
        &self.compatibility_projection.plan_digest
    }

    /// Returns the semantic-order to compatibility-case-order permutation.
    #[must_use]
    pub fn compatibility_case_permutation(&self) -> &[u32] {
        &self.compatibility_projection.semantic_to_lexicographic
    }

    /// Revalidates every retained authority and fixed judge-policy relationship.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateJudgePreparationError`] for cancellation, authority
    /// drift, rubric drift, or judge-policy substitution.
    pub fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError> {
        revalidate_prepared_authorities(
            &self.candidate_a,
            &self.candidate_b,
            &self.case_material,
            &self.judge_plan,
            &self.rubric,
            &self.judge_system,
            cancellation,
        )?;
        validate_active_subject_closure(&self.candidate_a, &self.candidate_b)
    }

    /// Rebuilds and verifies one exact schedule-order request without exposing it.
    ///
    /// This is a content-free diagnostic boundary. The managed runner uses the
    /// crate-private callback form to consume the same verified request and its
    /// schedule-bound normalization material.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateJudgePreparationError`] for an invalid cursor,
    /// cancellation, authority drift, request substitution, or final-validation
    /// failure.
    pub fn revalidate_request_at(
        &self,
        schedule_cursor: usize,
        cancellation: &CancellationToken,
    ) -> Result<(), CandidateJudgePreparationError> {
        let traffic = self
            .prepare_traffic_request_at(schedule_cursor, cancellation)
            .map_err(request_error_into_preparation)?;
        let observed_cursor = traffic.schedule_cursor();
        let binding_id = traffic.request_binding_id();
        let request = traffic.into_structured_request();
        if observed_cursor != schedule_cursor
            || request.structured_request_binding_id() != binding_id
        {
            return Err(CandidateJudgePreparationError::Relationship(
                super::CandidateJudgePreparationRelationship::ScheduleClosure,
            ));
        }
        Ok(())
    }

    pub(crate) fn prepare_traffic_request_at(
        &self,
        schedule_cursor: usize,
        cancellation: &CancellationToken,
    ) -> Result<CandidateJudgeTrafficRequest, CandidateJudgeRunnerRequestError<Infallible>> {
        let request = self.with_attempt_at(schedule_cursor, cancellation, |attempt| {
            Ok(attempt.request().clone())
        })?;
        Ok(CandidateJudgeTrafficRequest::new(schedule_cursor, request))
    }

    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "reserved for the managed judge runner")
    )]
    pub(crate) fn with_request_at<E, F>(
        &self,
        schedule_cursor: usize,
        cancellation: &CancellationToken,
        use_attempt: F,
    ) -> Result<(), CandidateJudgeRunnerRequestError<E>>
    where
        F: for<'attempt> FnOnce(CandidateJudgeAttemptMaterial<'attempt>) -> Result<(), E>,
    {
        self.with_attempt_at(schedule_cursor, cancellation, use_attempt)
    }

    /// Applies one scoped callback and releases its owned result only after the
    /// mandatory final authority validation succeeds. A result is dropped before
    /// any final-validation error is returned.
    pub(crate) fn with_attempt_at<T, E, F>(
        &self,
        schedule_cursor: usize,
        cancellation: &CancellationToken,
        use_attempt: F,
    ) -> Result<T, CandidateJudgeRunnerRequestError<E>>
    where
        F: for<'attempt> FnOnce(CandidateJudgeAttemptMaterial<'attempt>) -> Result<T, E>,
    {
        let primary = self.with_attempt_at_inner(schedule_cursor, cancellation, use_attempt);
        let final_validation = self.revalidate(cancellation);
        match (primary, final_validation) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(primary), Ok(())) => Err(primary),
            (Ok(value), Err(source)) => {
                drop(value);
                Err(CandidateJudgeRunnerRequestError::Preparation { source })
            }
            (Err(primary), Err(final_validation)) => Err(
                CandidateJudgeRunnerRequestError::PrimaryAndFinalValidation {
                    primary: Box::new(primary),
                    final_validation: Box::new(final_validation),
                },
            ),
        }
    }

    fn with_attempt_at_inner<T, E, F>(
        &self,
        schedule_cursor: usize,
        cancellation: &CancellationToken,
        use_attempt: F,
    ) -> Result<T, CandidateJudgeRunnerRequestError<E>>
    where
        F: for<'attempt> FnOnce(CandidateJudgeAttemptMaterial<'attempt>) -> Result<T, E>,
    {
        self.revalidate(cancellation)
            .map_err(|source| CandidateJudgeRunnerRequestError::Preparation { source })?;
        let relationship = self.attempt_relationship(schedule_cursor)?;
        let candidate_a = selected_candidates(
            &self.candidate_a,
            CandidateJudgePreparationSide::CandidateA,
            cancellation,
        )
        .map_err(|source| CandidateJudgeRunnerRequestError::Preparation { source })?;
        let candidate_b = selected_candidates(
            &self.candidate_b,
            CandidateJudgePreparationSide::CandidateB,
            cancellation,
        )
        .map_err(|source| CandidateJudgeRunnerRequestError::Preparation { source })?;
        let candidate_a = candidate_a
            .get(relationship.semantic_index)
            .ok_or(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor })?;
        let candidate_b = candidate_b
            .get(relationship.semantic_index)
            .ok_or(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor })?;
        let mut callback = Some(use_attempt);
        let traversal = self.case_material.try_with_all_case_source_bytes(
            cancellation,
            |semantic_index, source| {
                if semantic_index != relationship.semantic_index {
                    return Ok(());
                }
                let attempt = build_exact_attempt(
                    schedule_cursor,
                    relationship.contract,
                    source,
                    candidate_a,
                    candidate_b,
                    relationship.entry,
                    &self.rubric,
                    self.judge_system.model_artifact_id(),
                    attempt_limits(&self.judge_plan),
                )
                .map_err(AttemptVisitStop::Preparation)?;
                if &attempt.request.structured_request_binding_id() != relationship.request_id {
                    return Err(AttemptVisitStop::RequestBindingMismatch);
                }
                let (presented_first, presented_second) = match relationship.entry.presentation() {
                    CandidateJudgePresentationV1::CandidateAFirst => {
                        (attempt.candidate_a, attempt.candidate_b)
                    }
                    CandidateJudgePresentationV1::CandidateBFirst => {
                        (attempt.candidate_b, attempt.candidate_a)
                    }
                };
                let view = CandidateJudgeAttemptMaterial {
                    schedule_cursor,
                    presentation: relationship.entry.presentation(),
                    request: &attempt.request,
                    case_key: attempt.case_key,
                    source: attempt.source,
                    candidate_a: attempt.candidate_a,
                    candidate_b: attempt.candidate_b,
                    presented_first,
                    presented_second,
                    rubric_clause_ids: attempt.rubric_clause_ids,
                };
                if view.schedule_cursor() != schedule_cursor
                    || view.presentation() != relationship.entry.presentation()
                    || view.request().structured_request_binding_id() != *relationship.request_id
                    || view.case_key() != relationship.contract.case_key()
                    || view.source() != attempt.source
                    || view.candidate_a() != attempt.candidate_a
                    || view.candidate_b() != attempt.candidate_b
                    || view.rubric_clause_ids() != relationship.contract.rubric_clause_ids()
                    || match view.presentation() {
                        CandidateJudgePresentationV1::CandidateAFirst => {
                            view.presented_first() != view.candidate_a()
                                || view.presented_second() != view.candidate_b()
                        }
                        CandidateJudgePresentationV1::CandidateBFirst => {
                            view.presented_first() != view.candidate_b()
                                || view.presented_second() != view.candidate_a()
                        }
                    }
                {
                    return Err(AttemptVisitStop::Preparation(
                        CandidateJudgePreparationError::Relationship(
                            super::CandidateJudgePreparationRelationship::ScheduleClosure,
                        ),
                    ));
                }
                let result = callback
                    .take()
                    .expect("exact request callback is invoked at most once")(
                    view
                );
                Err(match result {
                    Ok(value) => AttemptVisitStop::Completed(value),
                    Err(source) => AttemptVisitStop::Callback(source),
                })
            },
        );
        resolve_traversal(schedule_cursor, traversal)
    }

    fn attempt_relationship<E>(
        &self,
        schedule_cursor: usize,
    ) -> Result<AttemptRelationship<'_>, CandidateJudgeRunnerRequestError<E>> {
        let entry = self
            .judge_schedule
            .entries()
            .get(schedule_cursor)
            .ok_or(CandidateJudgeRunnerRequestError::CursorOutOfRange { schedule_cursor })?;
        let request_id = self
            .request_aggregate
            .structured_request_binding_ids()
            .get(schedule_cursor)
            .ok_or(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor })?;
        let eligible_position = schedule_cursor / 2;
        let semantic_index = *self
            .eligible_semantic_indices
            .get(eligible_position)
            .ok_or(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor })?;
        let planned_case = self
            .judge_plan
            .cases()
            .get(eligible_position)
            .ok_or(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor })?;
        let manifest = self
            .case_material
            .cases()
            .get(semantic_index)
            .ok_or(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor })?;
        let contract = self
            .case_material
            .contracts()
            .get(semantic_index)
            .ok_or(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor })?;
        if entry.case_id() != planned_case.case_id()
            || entry.case_id() != manifest.case_id()
            || planned_case.rubric_clause_ids() != contract.rubric_clause_ids()
        {
            return Err(CandidateJudgeRunnerRequestError::AttemptClosure { schedule_cursor });
        }
        Ok(AttemptRelationship {
            semantic_index,
            entry,
            contract,
            request_id,
        })
    }
}

impl fmt::Debug for CandidateJudgeRunnerHandoff<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CandidateJudgeRunnerHandoff")
            .field(
                "deterministic_evaluation_id",
                self.deterministic_evaluation.deterministic_evaluation_id(),
            )
            .field("judge_plan_id", self.judge_plan.candidate_judge_plan_id())
            .field(
                "judge_schedule_id",
                self.judge_schedule.candidate_judge_schedule_id(),
            )
            .field(
                "request_aggregate_id",
                self.request_aggregate.request_aggregate_id(),
            )
            .field(
                "compatibility_plan_digest",
                &self.compatibility_projection.plan_digest,
            )
            .finish_non_exhaustive()
    }
}

impl<'store> PreparedCandidateJudgeRun<'store> {
    /// Revalidates and consumes this prepared authority into the managed-runner handoff.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateJudgePreparationError`] without releasing a runner
    /// authority if cancellation or retained authority drift is observed.
    pub fn into_runner_handoff(
        self,
        cancellation: &CancellationToken,
    ) -> Result<CandidateJudgeRunnerHandoff<'store>, CandidateJudgePreparationError> {
        self.revalidate(cancellation)?;
        validate_active_subject_closure(&self.candidate_a, &self.candidate_b)?;
        Ok(CandidateJudgeRunnerHandoff {
            candidate_a: self.candidate_a,
            candidate_b: self.candidate_b,
            case_material: self.case_material,
            deterministic_evaluation: self.deterministic_evaluation,
            judge_plan: self.judge_plan,
            judge_schedule: self.judge_schedule,
            request_aggregate: self.request_aggregate,
            compatibility_projection: self.compatibility_projection,
            rubric: self.rubric,
            judge_system: self.judge_system,
            eligible_semantic_indices: self.eligible_semantic_indices,
        })
    }
}

fn validate_active_subject_closure(
    candidate_a: &VerifiedCandidateBatchSet,
    candidate_b: &VerifiedCandidateBatchSet,
) -> Result<(), CandidateJudgePreparationError> {
    match (candidate_a.active_binding(), candidate_b.active_binding()) {
        (None, None) => Ok(()),
        (Some(left), Some(right)) if left.same_subject(right) => Ok(()),
        _ => Err(CandidateJudgePreparationError::Relationship(
            super::CandidateJudgePreparationRelationship::ActiveSubjectClosure,
        )),
    }
}

pub(crate) struct CandidateJudgeAttemptMaterial<'attempt> {
    schedule_cursor: usize,
    presentation: CandidateJudgePresentationV1,
    request: &'attempt StructuredCompletionRequest,
    case_key: &'attempt str,
    source: &'attempt str,
    candidate_a: &'attempt str,
    candidate_b: &'attempt str,
    presented_first: &'attempt str,
    presented_second: &'attempt str,
    rubric_clause_ids: &'attempt [String],
}

impl<'attempt> CandidateJudgeAttemptMaterial<'attempt> {
    pub(crate) const fn schedule_cursor(&self) -> usize {
        self.schedule_cursor
    }

    pub(crate) const fn presentation(&self) -> CandidateJudgePresentationV1 {
        self.presentation
    }

    pub(crate) const fn request(&self) -> &'attempt StructuredCompletionRequest {
        self.request
    }

    pub(crate) const fn case_key(&self) -> &'attempt str {
        self.case_key
    }

    pub(crate) const fn source(&self) -> &'attempt str {
        self.source
    }

    pub(crate) const fn candidate_a(&self) -> &'attempt str {
        self.candidate_a
    }

    pub(crate) const fn candidate_b(&self) -> &'attempt str {
        self.candidate_b
    }

    pub(crate) const fn presented_first(&self) -> &'attempt str {
        self.presented_first
    }

    pub(crate) const fn presented_second(&self) -> &'attempt str {
        self.presented_second
    }

    pub(crate) const fn rubric_clause_ids(&self) -> &'attempt [String] {
        self.rubric_clause_ids
    }
}

struct AttemptRelationship<'attempt> {
    semantic_index: usize,
    entry: &'attempt rewrite_model::CandidateJudgeScheduleEntryV1,
    contract: &'attempt crate::GenerationDeterministicCaseContractV1,
    request_id: &'attempt rewrite_model::StructuredCompletionRequestBindingId,
}

enum AttemptVisitStop<T, E> {
    Completed(T),
    Preparation(CandidateJudgePreparationError),
    RequestBindingMismatch,
    Callback(E),
}

#[cfg(test)]
#[path = "handoff/tests.rs"]
mod tests;
