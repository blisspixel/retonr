use rewrite_types::Digest;

use super::{plan_digest, run_deterministic_gates};
use crate::EvaluationSuite;
use crate::hybrid_scorecard::{
    HybridScorecardError, HybridScorecardPlan, HybridScorecardReport, JudgeObservationBatch,
    JudgeObservationEvidenceClass, JudgeTriageSummary, ReleaseReviewDisposition,
    normalize_observations, report, validate_exact_projection_plan, validate_observation_batch,
};

/// Validates the exact-path compatibility projection and returns its legacy plan
/// digest only when every eligible-subset deterministic hard gate passes.
///
/// Candidate equality is admitted only by this crate-private path. Public
/// compatibility V1 plan validation remains unchanged.
pub(crate) fn exact_projection_plan_digest_if_hard_gates_pass(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
) -> Result<Option<Digest>, HybridScorecardError> {
    validate_exact_projection_plan(plan)?;
    let receipt = run_deterministic_gates(plan, candidate_a, candidate_b)?;
    if receipt.success {
        plan_digest(plan).map(Some)
    } else {
        Ok(None)
    }
}

/// Builds the compatibility-shaped triage report for one exact eligible subset.
///
/// Candidate equality is admitted only through this crate-private path. The
/// report keeps the unchanged compatibility evidence class and remains
/// probabilistic, triage-only, and non-authoritative.
pub(crate) fn run_exact_projection_scorecard(
    plan: &HybridScorecardPlan,
    candidate_a: &EvaluationSuite,
    candidate_b: &EvaluationSuite,
    batch: &JudgeObservationBatch,
) -> Result<HybridScorecardReport, HybridScorecardError> {
    validate_exact_projection_plan(plan)?;
    validate_observation_batch(batch)?;
    let exact_plan_digest = plan_digest(plan)?;
    if batch.plan_id != plan.plan_id || batch.plan_digest != exact_plan_digest {
        return Err(HybridScorecardError::PlanMismatch);
    }
    let deterministic = run_deterministic_gates(plan, candidate_a, candidate_b)?;
    if !deterministic.success {
        if !batch.observations.is_empty() {
            return Err(HybridScorecardError::JudgeAfterHardGateFailure);
        }
        return Ok(report(
            plan,
            &deterministic,
            exact_plan_digest,
            None,
            None,
            JudgeTriageSummary::default(),
            ReleaseReviewDisposition::BlockedByHardGate,
        ));
    }
    let judge = normalize_observations(plan, batch)?;
    let observation_batch_digest = super::observation_batch_digest(batch)?;
    Ok(report(
        plan,
        &deterministic,
        exact_plan_digest,
        Some(observation_batch_digest),
        Some(JudgeObservationEvidenceClass::CallerDeclared),
        judge,
        ReleaseReviewDisposition::RequiresHumanAdjudication,
    ))
}
