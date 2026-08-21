use std::{collections::BTreeMap, time::Duration};

use rewrite_inference::{
    LocalJudgeAttemptOutput, LocalJudgeAttemptOutputError, LocalJudgeByteSpan, LocalJudgeChoice,
    OperationContext, parse_local_judge_attempt_output,
};
use rewrite_ollama::{
    OllamaModelBinding, OllamaObservedSessionError, OllamaRetainedStreamSession,
    OllamaSessionExecutionReceipt,
};
use thiserror::Error;

use crate::{
    EvaluationSuite, HYBRID_SCORECARD_SCHEMA_VERSION, HybridScorecardError, HybridScorecardPlan,
    HybridScorecardReport, JudgeChoice, JudgeObservation, JudgeObservationBatch, JudgePresentation,
    hybrid_scorecard::run_hybrid_scorecard_hard_gates, hybrid_scorecard_plan_digest,
    run_hybrid_scorecard,
};

mod prompt;
mod receipt;
mod rubric;

use prompt::{PreparedAttempt, prepare_attempts};

pub use prompt::local_judge_prompt_contract_digest;

pub use receipt::{LocalJudgeExecutionEvidenceClass, LocalJudgeExecutionReceipt};
pub use rubric::{
    LOCAL_JUDGE_RUBRIC_SCHEMA_VERSION, LocalJudgeRubric, LocalJudgeRubricClause,
    LocalJudgeRubricError, MAX_LOCAL_JUDGE_RUBRIC_BYTES, MAX_LOCAL_JUDGE_RUBRIC_CLAUSES,
    local_judge_rubric_digest, parse_local_judge_rubric,
};

/// Failure from locked local-judge validation or retained-session execution.
#[derive(Debug, Error)]
pub enum LocalJudgeExecutionError<E> {
    /// The scorecard plan, suites, deterministic gates, or observations failed.
    #[error("local judge scorecard validation failed")]
    Scorecard(#[from] HybridScorecardError),
    /// The canonical rubric failed validation.
    #[error("local judge rubric validation failed")]
    Rubric(#[from] LocalJudgeRubricError),
    /// The frozen policy contains an inconsistent execution constraint.
    #[error("local judge execution policy is invalid")]
    InvalidPolicy,
    /// The operation is cancelled, expired, missing a deadline, or exceeds the plan budget.
    #[error("local judge operation context is invalid")]
    InvalidOperationContext,
    /// The supplied model binding does not match the exact plan.
    #[error("local judge model binding does not match the plan")]
    ModelBindingMismatch,
    /// The exact rubric or a planned clause does not match the plan.
    #[error("local judge rubric does not match the plan")]
    RubricMismatch,
    /// A source, candidate, or complete prompt exceeds its frozen ceiling.
    #[error("local judge input exceeds the frozen execution limit")]
    InputLimitExceeded,
    /// Canonical prompt encoding failed.
    #[error("local judge prompt encoding failed")]
    PromptEncoding,
    /// A constructed structured request violated the frozen contract.
    #[error("local judge structured request is invalid")]
    InvalidRequest,
    /// The retained stream, runtime observations, or one completion failed closed.
    #[error("retained local judge session failed")]
    Session(OllamaObservedSessionError<E>),
    /// A response violated the exact typed output contract.
    #[error("local judge response violated the output contract")]
    InvalidAttemptOutput(LocalJudgeAttemptOutputError),
    /// A response named another case, an unadmitted clause, or an invalid input span.
    #[error("local judge response does not match the presented input")]
    InvalidAttemptRelationship,
    /// Retained-session receipt ordinals or bindings were inconsistent.
    #[error("local judge execution receipt is inconsistent")]
    ReceiptInvariant,
}

/// Terminal outcome from a locked local-judge run.
#[derive(Debug)]
pub enum LocalJudgeExecutionOutcome {
    /// Deterministic gates blocked all judge calls.
    BlockedByHardGate(Box<HybridScorecardReport>),
    /// Every planned two-order attempt completed once.
    Executed(Box<LocalJudgeExecution>),
}

/// Exact observation batch, triage-only scorecard, and limited receipt.
#[derive(Debug)]
pub struct LocalJudgeExecution {
    observations: JudgeObservationBatch,
    report: HybridScorecardReport,
    receipt: LocalJudgeExecutionReceipt,
}

impl LocalJudgeExecution {
    /// Returns the exact plan-bound content-free observations.
    #[must_use]
    pub const fn observations(&self) -> &JudgeObservationBatch {
        &self.observations
    }

    /// Returns the version 1 caller-declared, triage-only scorecard.
    #[must_use]
    pub const fn report(&self) -> &HybridScorecardReport {
        &self.report
    }

    /// Returns the non-serializable retained-transport binding receipt.
    #[must_use]
    pub const fn receipt(&self) -> &LocalJudgeExecutionReceipt {
        &self.receipt
    }

    /// Consumes the result into its content-free components.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        JudgeObservationBatch,
        HybridScorecardReport,
        LocalJudgeExecutionReceipt,
    ) {
        (self.observations, self.report, self.receipt)
    }
}

/// Runs deterministic hard gates and then one locked attempt per case and order.
///
/// The caller must supply a session that has already completed its retained-stream
/// preflight. The function has no connector, retry, pool, or fallback path. The
/// returned scorecard remains caller-declared and triage-only. Its separate
/// receipt proves neither handler execution, model load or use, effective runtime
/// identity, candidate generation, semantic correctness, nor qualification.
///
/// # Errors
///
/// Returns [`LocalJudgeExecutionError`] before judge traffic for invalid plans,
/// rubric, model binding, deadlines, common-subset relationships, or input
/// ceilings. Any attempt or input-specific output failure stops immediately with
/// no retry. Output relationship failures also invalidate the retained session.
pub async fn run_local_judge_execution<F, E>(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
    rubric: &LocalJudgeRubric,
    model: &OllamaModelBinding,
    session: &mut OllamaRetainedStreamSession<F>,
    context: OperationContext<'_>,
) -> Result<LocalJudgeExecutionOutcome, LocalJudgeExecutionError<E>>
where
    F: FnMut(rewrite_ollama::OllamaResponseObservation) -> Result<(), E>,
{
    validate_operation_context(plan, context)?;
    if plan.judge.judge_prompt_contract_digest != local_judge_prompt_contract_digest()
        || plan.judge.judge_output_schema_digest
            != rewrite_inference::local_judge_attempt_output_contract().schema_digest
        || plan.judge.top_p_milli != 1_000
    {
        return Err(LocalJudgeExecutionError::InvalidPolicy);
    }
    let plan_digest = hybrid_scorecard_plan_digest(plan)?;
    let rubric_digest = local_judge_rubric_digest(rubric)?;
    if rubric_digest != plan.rubric_digest {
        return Err(LocalJudgeExecutionError::RubricMismatch);
    }
    if model.reference() != plan.judge.judge_model_reference
        || model.artifact_digest() != &plan.judge.judge_model_digest
        || model.artifact_id().digest() != &plan.judge.judge_model_digest
    {
        return Err(LocalJudgeExecutionError::ModelBindingMismatch);
    }
    let clauses = rubric
        .clauses
        .iter()
        .map(|clause| (clause.id.as_str(), clause))
        .collect::<BTreeMap<_, _>>();
    if plan
        .cases
        .iter()
        .flat_map(|case| &case.rubric_clauses)
        .any(|id| !clauses.contains_key(id.as_str()))
    {
        return Err(LocalJudgeExecutionError::RubricMismatch);
    }
    if let Some(report) = run_hybrid_scorecard_hard_gates(plan, candidate_a, candidate_b)? {
        return Ok(LocalJudgeExecutionOutcome::BlockedByHardGate(Box::new(
            report,
        )));
    }
    let attempts = prepare_attempts::<E>(
        plan,
        candidate_a,
        candidate_b,
        &clauses,
        model,
        &plan_digest,
    )?;
    validate_operation_context(plan, context)?;
    execute_attempts(
        plan,
        candidate_a,
        candidate_b,
        rubric_digest,
        plan_digest,
        attempts,
        session,
        context,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "the private boundary keeps every exact execution binding explicit"
)]
async fn execute_attempts<F, E>(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
    rubric_digest: rewrite_types::Digest,
    plan_digest: rewrite_types::Digest,
    attempts: Vec<PreparedAttempt>,
    session: &mut OllamaRetainedStreamSession<F>,
    context: OperationContext<'_>,
) -> Result<LocalJudgeExecutionOutcome, LocalJudgeExecutionError<E>>
where
    F: FnMut(rewrite_ollama::OllamaResponseObservation) -> Result<(), E>,
{
    let mut observations = Vec::with_capacity(attempts.len());
    let mut receipts = Vec::<OllamaSessionExecutionReceipt>::with_capacity(attempts.len());
    for attempt in attempts {
        let (response, receipt) = session
            .complete_structured(attempt.request, context)
            .await
            .map_err(LocalJudgeExecutionError::Session)?;
        let output = match parse_local_judge_attempt_output(response.output_json()) {
            Ok(output) => output,
            Err(error) => {
                session.invalidate();
                return Err(LocalJudgeExecutionError::InvalidAttemptOutput(error));
            }
        };
        let planned = &plan.cases[attempt.case_index];
        let case_a = &candidate_a.cases[attempt.case_index];
        let case_b = &candidate_b.cases[attempt.case_index];
        if !valid_attempt_relationship(
            &output,
            planned,
            case_a.source.as_str(),
            case_a.candidate.as_str(),
            case_b.candidate.as_str(),
            attempt.presentation,
        ) {
            session.invalidate();
            return Err(LocalJudgeExecutionError::InvalidAttemptRelationship);
        }
        observations.push(JudgeObservation {
            case_id: output.case_id,
            presentation: attempt.presentation,
            choice: map_choice(output.choice),
            rubric_clauses: output.rubric_clauses,
        });
        receipts.push(receipt);
    }
    session.invalidate();
    let batch = JudgeObservationBatch {
        schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: plan.plan_id.clone(),
        plan_digest: plan_digest.clone(),
        observations,
    };
    let report = run_hybrid_scorecard(plan, candidate_a, candidate_b, &batch)?;
    let receipt = LocalJudgeExecutionReceipt::new(plan_digest, rubric_digest, &batch, &receipts)?;
    Ok(LocalJudgeExecutionOutcome::Executed(Box::new(
        LocalJudgeExecution {
            observations: batch,
            report,
            receipt,
        },
    )))
}

fn validate_operation_context<E>(
    plan: &HybridScorecardPlan,
    context: OperationContext<'_>,
) -> Result<(), LocalJudgeExecutionError<E>> {
    let Some(deadline) = context.deadline() else {
        return Err(LocalJudgeExecutionError::InvalidOperationContext);
    };
    let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
        return Err(LocalJudgeExecutionError::InvalidOperationContext);
    };
    if context.is_cancelled()
        || remaining.is_zero()
        || remaining > Duration::from_millis(u64::from(plan.judge.maximum_elapsed_millis))
    {
        return Err(LocalJudgeExecutionError::InvalidOperationContext);
    }
    Ok(())
}

fn valid_attempt_relationship(
    output: &LocalJudgeAttemptOutput,
    planned: &crate::HybridScorecardCasePlan,
    source: &str,
    candidate_a: &str,
    candidate_b: &str,
    presentation: JudgePresentation,
) -> bool {
    if output.case_id != planned.id
        || output
            .rubric_clauses
            .iter()
            .any(|clause| planned.rubric_clauses.binary_search(clause).is_err())
        || !valid_spans(&output.source_spans, source)
    {
        return false;
    }
    let (first, second) = match presentation {
        JudgePresentation::CandidateAFirst => (candidate_a, candidate_b),
        JudgePresentation::CandidateBFirst => (candidate_b, candidate_a),
    };
    valid_spans(&output.first_candidate_spans, first)
        && valid_spans(&output.second_candidate_spans, second)
}

fn valid_spans(spans: &[LocalJudgeByteSpan], input: &str) -> bool {
    spans.iter().all(|span| {
        let Ok(start) = usize::try_from(span.start) else {
            return false;
        };
        let Ok(end) = usize::try_from(span.end) else {
            return false;
        };
        end <= input.len() && input.is_char_boundary(start) && input.is_char_boundary(end)
    })
}

const fn map_choice(choice: LocalJudgeChoice) -> JudgeChoice {
    match choice {
        LocalJudgeChoice::First => JudgeChoice::First,
        LocalJudgeChoice::Second => JudgeChoice::Second,
        LocalJudgeChoice::Tie => JudgeChoice::Tie,
        LocalJudgeChoice::Abstain => JudgeChoice::Abstain,
    }
}

#[cfg(test)]
mod tests;
