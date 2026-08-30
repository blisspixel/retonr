use std::fmt;

use rewrite_inference::{
    LocalJudgeAttemptOutputError, LocalJudgeChoice, StructuredCompletionRequest,
    StructuredCompletionResponse, parse_local_judge_attempt_output,
};
use rewrite_model::{
    CandidateJudgeChoiceV1, CandidateJudgeObservationV1, CandidateJudgePlanV1,
    CandidateJudgePresentationV1, CandidateJudgeRequestAggregateV1, CandidateJudgeScheduleV1,
    GenerationQualificationContractError, OllamaRetainedSessionResponseId,
};
use rewrite_ollama::derive_ollama_retained_session_response_id;
use thiserror::Error;

use super::valid_spans;

/// Exact borrowed inputs for one pure managed-judge output normalization.
///
/// Source and candidate text must come from the runner handoff's synchronous
/// callback. This value does not retain or copy any presented material.
#[derive(Clone, Copy)]
pub(crate) struct CandidateJudgeAttemptNormalizationInput<'input> {
    pub(crate) plan: &'input CandidateJudgePlanV1,
    pub(crate) schedule: &'input CandidateJudgeScheduleV1,
    pub(crate) request_aggregate: &'input CandidateJudgeRequestAggregateV1,
    pub(crate) schedule_cursor: usize,
    pub(crate) request: &'input StructuredCompletionRequest,
    pub(crate) response: &'input StructuredCompletionResponse,
    pub(crate) retained_response_id: &'input OllamaRetainedSessionResponseId,
    pub(crate) case_key: &'input str,
    pub(crate) presentation: CandidateJudgePresentationV1,
    pub(crate) admitted_rubric_clause_ids: &'input [String],
    pub(crate) source: &'input str,
    pub(crate) presented_first: &'input str,
    pub(crate) presented_second: &'input str,
}

/// Content-free failure from exact local-judge output normalization.
#[derive(Error)]
pub(crate) enum CandidateJudgeAttemptNormalizationError {
    #[error("candidate judge attempt output is invalid")]
    InvalidOutput {
        #[source]
        source: LocalJudgeAttemptOutputError,
    },
    #[error("candidate judge schedule relationship is invalid")]
    ScheduleRelationship,
    #[error("candidate judge response relationship is invalid")]
    ResponseRelationship,
    #[error("candidate judge case key is invalid")]
    CaseKeyRelationship,
    #[error("candidate judge rubric-clause relationship is invalid")]
    RubricClauseRelationship,
    #[error("candidate judge source spans are invalid")]
    SourceSpans,
    #[error("candidate judge first-candidate spans are invalid")]
    FirstCandidateSpans,
    #[error("candidate judge second-candidate spans are invalid")]
    SecondCandidateSpans,
    #[error("candidate judge observation contract rejected normalized output")]
    PortableContract {
        #[source]
        source: GenerationQualificationContractError,
    },
}

impl fmt::Debug for CandidateJudgeAttemptNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::InvalidOutput { .. } => "invalid_output",
            Self::ScheduleRelationship => "schedule_relationship",
            Self::ResponseRelationship => "response_relationship",
            Self::CaseKeyRelationship => "case_key_relationship",
            Self::RubricClauseRelationship => "rubric_clause_relationship",
            Self::SourceSpans => "source_spans",
            Self::FirstCandidateSpans => "first_candidate_spans",
            Self::SecondCandidateSpans => "second_candidate_spans",
            Self::PortableContract { .. } => "portable_contract",
        };
        formatter
            .debug_struct("CandidateJudgeAttemptNormalizationError")
            .field("kind", &kind)
            .finish_non_exhaustive()
    }
}

/// Parses and normalizes one exact schedule-bound local-judge response.
///
/// The returned observation is content-free. Input text and spans are consumed
/// only while this call remains inside the runner handoff's material callback.
///
/// # Errors
///
/// Returns [`CandidateJudgeAttemptNormalizationError`] for a substituted plan,
/// schedule, request, response, case key, rubric clause, input span, or portable
/// observation relationship.
pub(crate) fn normalize_candidate_judge_attempt(
    input: &CandidateJudgeAttemptNormalizationInput<'_>,
) -> Result<CandidateJudgeObservationV1, CandidateJudgeAttemptNormalizationError> {
    let entry = input
        .schedule
        .entries()
        .get(input.schedule_cursor)
        .ok_or(CandidateJudgeAttemptNormalizationError::ScheduleRelationship)?;
    let request_id = input
        .request_aggregate
        .structured_request_binding_ids()
        .get(input.schedule_cursor)
        .ok_or(CandidateJudgeAttemptNormalizationError::ScheduleRelationship)?;
    let planned_case = input
        .plan
        .cases()
        .iter()
        .find(|case| case.case_id() == entry.case_id())
        .ok_or(CandidateJudgeAttemptNormalizationError::ScheduleRelationship)?;
    if input.schedule.candidate_judge_plan_id() != input.plan.candidate_judge_plan_id()
        || input.request_aggregate.candidate_judge_plan_id() != input.plan.candidate_judge_plan_id()
        || input.request_aggregate.candidate_judge_schedule_id()
            != input.schedule.candidate_judge_schedule_id()
        || input.request_aggregate.entry_count() != input.schedule.entry_count()
        || entry.presentation() != input.presentation
        || request_id != &input.request.structured_request_binding_id()
    {
        return Err(CandidateJudgeAttemptNormalizationError::ScheduleRelationship);
    }

    if input.response.request_binding_digest() != &input.request.binding_digest()
        || &derive_ollama_retained_session_response_id(input.response) != input.retained_response_id
    {
        return Err(CandidateJudgeAttemptNormalizationError::ResponseRelationship);
    }
    let output = parse_local_judge_attempt_output(input.response.output_json())
        .map_err(|source| CandidateJudgeAttemptNormalizationError::InvalidOutput { source })?;
    if output.case_id != input.case_key {
        return Err(CandidateJudgeAttemptNormalizationError::CaseKeyRelationship);
    }
    if input.admitted_rubric_clause_ids != planned_case.rubric_clause_ids()
        || input.admitted_rubric_clause_ids.is_empty()
        || input
            .admitted_rubric_clause_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || output.rubric_clauses.iter().any(|clause| {
            input
                .admitted_rubric_clause_ids
                .binary_search(clause)
                .is_err()
        })
    {
        return Err(CandidateJudgeAttemptNormalizationError::RubricClauseRelationship);
    }
    if !valid_spans(&output.source_spans, input.source) {
        return Err(CandidateJudgeAttemptNormalizationError::SourceSpans);
    }
    if !valid_spans(&output.first_candidate_spans, input.presented_first) {
        return Err(CandidateJudgeAttemptNormalizationError::FirstCandidateSpans);
    }
    if !valid_spans(&output.second_candidate_spans, input.presented_second) {
        return Err(CandidateJudgeAttemptNormalizationError::SecondCandidateSpans);
    }

    CandidateJudgeObservationV1::new(
        input.plan,
        input.schedule,
        input.request_aggregate,
        input.schedule_cursor,
        input.retained_response_id.clone(),
        map_choice(output.choice),
        output.rubric_clauses,
    )
    .map_err(|source| CandidateJudgeAttemptNormalizationError::PortableContract { source })
}

const fn map_choice(choice: LocalJudgeChoice) -> CandidateJudgeChoiceV1 {
    match choice {
        LocalJudgeChoice::First => CandidateJudgeChoiceV1::First,
        LocalJudgeChoice::Second => CandidateJudgeChoiceV1::Second,
        LocalJudgeChoice::Tie => CandidateJudgeChoiceV1::Tie,
        LocalJudgeChoice::Abstain => CandidateJudgeChoiceV1::Abstain,
    }
}

#[cfg(test)]
#[path = "normalization/tests.rs"]
mod tests;
