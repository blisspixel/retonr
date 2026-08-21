use std::collections::{BTreeMap, BTreeSet};

use super::{
    HybridScorecardError, HybridScorecardPlan, JudgeCaseOutcome, JudgeCaseResult, JudgeChoice,
    JudgeObservationBatch, JudgePresentation, JudgeTriageSummary, MAX_RUBRIC_CLAUSES_PER_CASE,
};

pub(super) fn normalize_observations(
    plan: &HybridScorecardPlan,
    batch: &JudgeObservationBatch,
) -> Result<JudgeTriageSummary, HybridScorecardError> {
    if batch.observations.len() != plan.cases.len().saturating_mul(2) {
        return Err(HybridScorecardError::InvalidObservations);
    }
    let plans = plan
        .cases
        .iter()
        .map(|case| (case.id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let mut observations = BTreeMap::new();
    for observation in &batch.observations {
        let Some(case) = plans.get(observation.case_id.as_str()) else {
            return Err(HybridScorecardError::InvalidObservations);
        };
        if !valid_clauses(&observation.rubric_clauses, &case.rubric_clauses)
            || observations
                .insert(
                    (observation.case_id.as_str(), observation.presentation),
                    observation,
                )
                .is_some()
        {
            return Err(HybridScorecardError::InvalidObservations);
        }
    }
    let mut summary = JudgeTriageSummary {
        total: plan.cases.len(),
        ..JudgeTriageSummary::default()
    };
    for case in &plan.cases {
        let first = observations
            .get(&(case.id.as_str(), JudgePresentation::CandidateAFirst))
            .ok_or(HybridScorecardError::InvalidObservations)?;
        let swapped = observations
            .get(&(case.id.as_str(), JudgePresentation::CandidateBFirst))
            .ok_or(HybridScorecardError::InvalidObservations)?;
        let outcome = normalize_pair(first.choice, swapped.choice);
        match outcome {
            JudgeCaseOutcome::StableA => summary.stable_a += 1,
            JudgeCaseOutcome::StableB => summary.stable_b += 1,
            JudgeCaseOutcome::StableTie => summary.stable_tie += 1,
            JudgeCaseOutcome::Abstained => summary.abstained += 1,
            JudgeCaseOutcome::OrderSensitive => summary.order_sensitive += 1,
        }
        summary.cases.push(JudgeCaseResult {
            case_id: case.id.clone(),
            outcome,
        });
    }
    Ok(summary)
}

fn valid_clauses(observed: &[String], planned: &[String]) -> bool {
    !observed.is_empty()
        && observed.len() <= MAX_RUBRIC_CLAUSES_PER_CASE
        && observed.windows(2).all(|pair| pair[0] < pair[1])
        && observed
            .iter()
            .collect::<BTreeSet<_>>()
            .is_subset(&planned.iter().collect())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnderlyingChoice {
    A,
    B,
    Tie,
    Abstain,
}

fn normalize_pair(first: JudgeChoice, swapped: JudgeChoice) -> JudgeCaseOutcome {
    let first = underlying(JudgePresentation::CandidateAFirst, first);
    let swapped = underlying(JudgePresentation::CandidateBFirst, swapped);
    if first == UnderlyingChoice::Abstain || swapped == UnderlyingChoice::Abstain {
        JudgeCaseOutcome::Abstained
    } else {
        match (first, swapped) {
            (UnderlyingChoice::A, UnderlyingChoice::A) => JudgeCaseOutcome::StableA,
            (UnderlyingChoice::B, UnderlyingChoice::B) => JudgeCaseOutcome::StableB,
            (UnderlyingChoice::Tie, UnderlyingChoice::Tie) => JudgeCaseOutcome::StableTie,
            _ => JudgeCaseOutcome::OrderSensitive,
        }
    }
}

fn underlying(presentation: JudgePresentation, choice: JudgeChoice) -> UnderlyingChoice {
    match (presentation, choice) {
        (_, JudgeChoice::Tie) => UnderlyingChoice::Tie,
        (_, JudgeChoice::Abstain) => UnderlyingChoice::Abstain,
        (JudgePresentation::CandidateAFirst, JudgeChoice::First)
        | (JudgePresentation::CandidateBFirst, JudgeChoice::Second) => UnderlyingChoice::A,
        (JudgePresentation::CandidateAFirst, JudgeChoice::Second)
        | (JudgePresentation::CandidateBFirst, JudgeChoice::First) => UnderlyingChoice::B,
    }
}
