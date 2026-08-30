use rewrite_model::{
    CandidateJudgePlanV1, CandidateJudgePresentationV1, CandidateJudgeRequestAggregateV1,
    CandidateJudgeScheduleEntryV1, CandidateJudgeScheduleV1, StructuredCompletionRequestBindingId,
};
use rewrite_types::CancellationToken;

use super::{
    CandidateJudgePreparationError, CandidateJudgePreparationInput,
    CandidateJudgePreparationRequestFailure, CandidateJudgePreparationSide, ensure_active,
    portable_error,
};
use crate::JudgePresentation;
use crate::local_judge_execution::prompt::{
    LocalJudgeAttemptBuildError, LocalJudgeAttemptLimits, build_local_judge_attempt_request,
};

pub(super) struct BuiltCandidateJudgeAttempt<'input> {
    pub(super) request: rewrite_inference::StructuredCompletionRequest,
    pub(super) case_key: &'input str,
    pub(super) source: &'input str,
    pub(super) candidate_a: &'input str,
    pub(super) candidate_b: &'input str,
    pub(super) rubric_clause_ids: &'input [String],
}

/// Cancellation is checked before every request build. One individual canonical
/// prompt encode and binding digest remains atomic, but the portable plan limits it
/// to the reviewed 4 MiB complete-input ceiling and no raw prompt survives the
/// iteration that derives its identity.
pub(super) fn derive_request_aggregate(
    input: &CandidateJudgePreparationInput<'_, '_>,
    plan: &CandidateJudgePlanV1,
    schedule: &CandidateJudgeScheduleV1,
    eligible_semantic_indices: &[usize],
    cancellation: &CancellationToken,
) -> Result<CandidateJudgeRequestAggregateV1, CandidateJudgePreparationError> {
    ensure_active(cancellation)?;
    let candidate_a = selected_candidates(
        &input.candidate_a,
        CandidateJudgePreparationSide::CandidateA,
        cancellation,
    )?;
    let candidate_b = selected_candidates(
        &input.candidate_b,
        CandidateJudgePreparationSide::CandidateB,
        cancellation,
    )?;
    ensure_active(cancellation)?;
    let attempt_limits = attempt_limits(plan);
    let projection = RequestProjection {
        input,
        schedule,
        eligible_semantic_indices,
        candidate_a,
        candidate_b,
        attempt_limits,
        cancellation,
    };
    let mut callback_failure = None;
    let traversal =
        input
            .case_material
            .with_all_case_source_bytes(cancellation, |semantic_index, source| {
                if callback_failure.is_some() {
                    return Vec::new();
                }
                match projection.derive_case_ids(semantic_index, source) {
                    Ok(ids) => ids,
                    Err(error) => {
                        callback_failure = Some(error);
                        Vec::new()
                    }
                }
            });
    let per_case = match (callback_failure, traversal) {
        (None, Ok(per_case)) => per_case,
        (Some(primary), Ok(_)) => return Err(primary),
        (None, Err(source)) => {
            return Err(CandidateJudgePreparationError::CaseMaterial { source });
        }
        (Some(primary), Err(source)) => {
            return Err(CandidateJudgePreparationError::PrimaryAndFinalValidation {
                primary: Box::new(primary),
                final_validation: Box::new(CandidateJudgePreparationError::CaseMaterial { source }),
            });
        }
    };
    ensure_active(cancellation)?;
    let ids = per_case.into_iter().flatten().collect::<Vec<_>>();
    CandidateJudgeRequestAggregateV1::new(plan, schedule, ids).map_err(portable_error)
}

struct RequestProjection<'input, 'records, 'store> {
    input: &'input CandidateJudgePreparationInput<'records, 'store>,
    schedule: &'input CandidateJudgeScheduleV1,
    eligible_semantic_indices: &'input [usize],
    candidate_a: Vec<&'input rewrite_inference::GenerationCandidate>,
    candidate_b: Vec<&'input rewrite_inference::GenerationCandidate>,
    attempt_limits: LocalJudgeAttemptLimits,
    cancellation: &'input CancellationToken,
}

impl RequestProjection<'_, '_, '_> {
    fn derive_case_ids(
        &self,
        semantic_index: usize,
        source: &[u8],
    ) -> Result<Vec<StructuredCompletionRequestBindingId>, CandidateJudgePreparationError> {
        ensure_active(self.cancellation)?;
        let Ok(position) = self
            .eligible_semantic_indices
            .binary_search(&semantic_index)
        else {
            return Ok(Vec::new());
        };
        let first_schedule_index = position.checked_mul(2).ok_or_else(|| {
            request_error(0, CandidateJudgePreparationRequestFailure::InvalidRequest)
        })?;
        let contract = &self.input.case_material.contracts()[semantic_index];
        let candidate_a = self.candidate_a.get(semantic_index).ok_or_else(|| {
            request_error(
                first_schedule_index,
                CandidateJudgePreparationRequestFailure::InvalidRequest,
            )
        })?;
        let candidate_b = self.candidate_b.get(semantic_index).ok_or_else(|| {
            request_error(
                first_schedule_index,
                CandidateJudgePreparationRequestFailure::InvalidRequest,
            )
        })?;
        let mut ids = Vec::with_capacity(2);
        for relative in 0..2 {
            ensure_active(self.cancellation)?;
            let schedule_index = first_schedule_index + relative;
            let entry = self.schedule.entries().get(schedule_index).ok_or_else(|| {
                request_error(
                    schedule_index,
                    CandidateJudgePreparationRequestFailure::InvalidRequest,
                )
            })?;
            let attempt = build_exact_attempt(
                schedule_index,
                contract,
                source,
                candidate_a,
                candidate_b,
                entry,
                self.input.rubric,
                self.input.plan_relations.judge_system.model_artifact_id(),
                self.attempt_limits,
            )?;
            ids.push(attempt.request.structured_request_binding_id());
        }
        Ok(ids)
    }
}

pub(super) fn attempt_limits(plan: &CandidateJudgePlanV1) -> LocalJudgeAttemptLimits {
    let limits = plan.limits();
    LocalJudgeAttemptLimits {
        maximum_source_bytes: u64::from(limits.maximum_source_bytes()),
        maximum_candidate_bytes: u64::from(limits.maximum_candidate_bytes()),
        maximum_input_bytes: u64::from(limits.maximum_complete_input_bytes()),
        context_token_limit: limits.maximum_context_tokens(),
        output_token_limit: limits.maximum_output_tokens(),
        maximum_response_bytes: u64::from(limits.maximum_response_bytes()),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the exact request join keeps every identity-significant input explicit"
)]
pub(super) fn build_exact_attempt<'input>(
    schedule_index: usize,
    contract: &'input crate::GenerationDeterministicCaseContractV1,
    source: &'input [u8],
    candidate_a: &'input rewrite_inference::GenerationCandidate,
    candidate_b: &'input rewrite_inference::GenerationCandidate,
    entry: &CandidateJudgeScheduleEntryV1,
    rubric: &'input crate::LocalJudgeRubric,
    judge_model_artifact_id: &rewrite_model::ArtifactId,
    limits: LocalJudgeAttemptLimits,
) -> Result<BuiltCandidateJudgeAttempt<'input>, CandidateJudgePreparationError> {
    let source = std::str::from_utf8(source).map_err(|_| {
        request_error(
            schedule_index,
            CandidateJudgePreparationRequestFailure::SourceNotUtf8,
        )
    })?;
    let clauses = contract
        .rubric_clause_ids()
        .iter()
        .map(|id| rubric_clause(rubric, id, schedule_index))
        .collect::<Result<Vec<_>, _>>()?;
    let request = build_local_judge_attempt_request(
        contract.case_key(),
        source,
        &candidate_a.text,
        &candidate_b.text,
        presentation(entry.presentation()),
        &clauses,
        judge_model_artifact_id,
        judge_model_artifact_id.digest(),
        entry.seed(),
        limits,
    )
    .map_err(|error| request_error(schedule_index, error.into()))?;
    Ok(BuiltCandidateJudgeAttempt {
        request,
        case_key: contract.case_key(),
        source,
        candidate_a: &candidate_a.text,
        candidate_b: &candidate_b.text,
        rubric_clause_ids: contract.rubric_clause_ids(),
    })
}

fn rubric_clause<'rubric>(
    rubric: &'rubric crate::LocalJudgeRubric,
    id: &str,
    schedule_index: usize,
) -> Result<&'rubric crate::LocalJudgeRubricClause, CandidateJudgePreparationError> {
    rubric
        .clauses
        .binary_search_by(|clause| clause.id.as_str().cmp(id))
        .map(|index| &rubric.clauses[index])
        .map_err(|_| {
            request_error(
                schedule_index,
                CandidateJudgePreparationRequestFailure::RubricMismatch,
            )
        })
}

pub(super) fn selected_candidates<'a>(
    authority: &'a crate::VerifiedCandidateBatchSet,
    side: CandidateJudgePreparationSide,
    cancellation: &CancellationToken,
) -> Result<Vec<&'a rewrite_inference::GenerationCandidate>, CandidateJudgePreparationError> {
    authority
        .selected_candidates(cancellation)
        .map_err(|source| CandidateJudgePreparationError::CandidateBatchSet { side, source })
}

const fn presentation(value: CandidateJudgePresentationV1) -> JudgePresentation {
    match value {
        CandidateJudgePresentationV1::CandidateAFirst => JudgePresentation::CandidateAFirst,
        CandidateJudgePresentationV1::CandidateBFirst => JudgePresentation::CandidateBFirst,
    }
}

fn request_error(
    schedule_index: usize,
    failure: CandidateJudgePreparationRequestFailure,
) -> CandidateJudgePreparationError {
    CandidateJudgePreparationError::Request {
        schedule_index,
        failure,
    }
}

impl From<LocalJudgeAttemptBuildError> for CandidateJudgePreparationRequestFailure {
    fn from(value: LocalJudgeAttemptBuildError) -> Self {
        match value {
            LocalJudgeAttemptBuildError::RubricMismatch => Self::RubricMismatch,
            LocalJudgeAttemptBuildError::InputLimitExceeded => Self::InputLimitExceeded,
            LocalJudgeAttemptBuildError::PromptEncoding => Self::PromptEncoding,
            LocalJudgeAttemptBuildError::InvalidRequest => Self::InvalidRequest,
        }
    }
}

#[cfg(test)]
#[path = "request/private_tests.rs"]
mod tests;
