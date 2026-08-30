use std::fmt;

use rewrite_model::{
    CandidateDeterministicEvaluationStatusV1, CandidateJudgeChoiceV1,
    CandidateJudgeObservationBatchV1, CandidateJudgeObservationV1, CandidateJudgePresentationV1,
    CandidateJudgeTriageReportRelationshipV1, GenerationQualificationContractError,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::CandidateJudgeRunnerHandoff;
use crate::candidate_judge_preparation::projection::reconstruct_compatibility_suites;
use crate::candidate_judge_preparation::{CandidateJudgePreparationError, ensure_active};
use crate::hybrid_scorecard::deterministic::run_exact_projection_scorecard;
use crate::{
    HYBRID_SCORECARD_SCHEMA_VERSION, HybridScorecardError, HybridScorecardReport, JudgeChoice,
    JudgeObservation, JudgeObservationBatch, JudgePresentation,
};

#[cfg(test)]
mod tests;

/// Exact relationship rejected by the compatibility-triage compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CandidateJudgeTriageCompilationRelationship {
    /// The retained deterministic evaluation was not passing.
    DeterministicEvaluation,
    /// The portable observation batch named another exact run.
    ObservationBatch,
    /// Observation count, schedule position, case, presentation, or order differed.
    ObservationSchedule,
    /// The frozen semantic-to-lexicographic permutation was incomplete or invalid.
    CasePermutation,
    /// The compiled report did not retain the exact compatibility relationship.
    Report,
}

/// Content-redacted failure from exact compatibility triage compilation.
#[derive(Error)]
pub(crate) enum CandidateJudgeTriageCompilationError {
    /// One retained candidate or case-material authority failed validation.
    #[error("candidate judge triage authority validation failed")]
    Preparation(#[source] CandidateJudgePreparationError),
    /// One exact mapping or report relationship differed.
    #[error("candidate judge triage relationship is invalid: {0:?}")]
    Relationship(CandidateJudgeTriageCompilationRelationship),
    /// The compatibility-shaped scorecard rejected the exact projection.
    #[error("candidate judge compatibility scorecard failed")]
    Scorecard(#[source] HybridScorecardError),
    /// The portable triage framing rejected the canonical report.
    #[error("candidate judge triage portable contract failed")]
    Portable(#[source] GenerationQualificationContractError),
    /// Canonical report encoding failed.
    #[error("candidate judge triage canonical encoding failed")]
    Encoding,
    /// Compilation and mandatory final authority validation both failed.
    #[error("candidate judge triage compilation and final validation both failed")]
    PrimaryAndFinalValidation {
        /// Primary compilation failure.
        primary: Box<CandidateJudgeTriageCompilationError>,
        /// Mandatory terminal authority-validation failure.
        final_validation: Box<CandidateJudgePreparationError>,
    },
}

impl fmt::Debug for CandidateJudgeTriageCompilationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("CandidateJudgeTriageCompilationError");
        match self {
            Self::Preparation(_) => debug.field("kind", &"preparation"),
            Self::Relationship(relationship) => debug
                .field("kind", &"relationship")
                .field("relationship", relationship),
            Self::Scorecard(_) => debug.field("kind", &"scorecard"),
            Self::Portable(_) => debug.field("kind", &"portable"),
            Self::Encoding => debug.field("kind", &"encoding"),
            Self::PrimaryAndFinalValidation { .. } => {
                debug.field("kind", &"primary_and_final_validation")
            }
        };
        debug.finish_non_exhaustive()
    }
}

/// Canonical content-free compatibility report and its portable digest framing.
///
/// The report remains probabilistic, triage-only, and non-authoritative. This
/// value contains no source, candidate, prompt, response, rationale, or span text.
pub(crate) struct CompiledCandidateJudgeTriage {
    report: HybridScorecardReport,
    canonical_json: Vec<u8>,
    relationship: CandidateJudgeTriageReportRelationshipV1,
}

impl CompiledCandidateJudgeTriage {
    pub(crate) const fn report(&self) -> &HybridScorecardReport {
        &self.report
    }

    pub(crate) fn canonical_json(&self) -> &[u8] {
        &self.canonical_json
    }

    pub(crate) const fn relationship(&self) -> &CandidateJudgeTriageReportRelationshipV1 {
        &self.relationship
    }
}

impl fmt::Debug for CompiledCandidateJudgeTriage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompiledCandidateJudgeTriage")
            .field("triage_report_digest", self.relationship.digest())
            .field("case_count", &self.report.judge.total)
            .finish_non_exhaustive()
    }
}

impl CandidateJudgeRunnerHandoff<'_> {
    /// Compiles the exact schedule observations into compatibility-shaped triage.
    ///
    /// Source and candidate content is reconstructed only inside this method and
    /// is discarded before the content-free result is released. Initial and final
    /// retained-authority validation are mandatory.
    pub(crate) fn compile_compatibility_triage(
        &self,
        observation_batch: &CandidateJudgeObservationBatchV1,
        cancellation: &CancellationToken,
    ) -> Result<CompiledCandidateJudgeTriage, CandidateJudgeTriageCompilationError> {
        self.compile_compatibility_triage_with_post_compilation(
            observation_batch,
            cancellation,
            || {},
        )
    }

    fn compile_compatibility_triage_with_post_compilation(
        &self,
        observation_batch: &CandidateJudgeObservationBatchV1,
        cancellation: &CancellationToken,
        post_compilation: impl FnOnce(),
    ) -> Result<CompiledCandidateJudgeTriage, CandidateJudgeTriageCompilationError> {
        let primary = self.compile_compatibility_triage_inner(observation_batch, cancellation);
        post_compilation();
        let operation_completion = ensure_active(cancellation);
        let final_validation = self.revalidate(&CancellationToken::new());
        let terminal = combine_preparation_results(operation_completion, final_validation);
        match (primary, terminal) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(primary), Ok(())) => Err(primary),
            (Ok(value), Err(final_validation)) => {
                drop(value);
                Err(CandidateJudgeTriageCompilationError::Preparation(
                    final_validation,
                ))
            }
            (Err(primary), Err(final_validation)) => Err(
                CandidateJudgeTriageCompilationError::PrimaryAndFinalValidation {
                    primary: Box::new(primary),
                    final_validation: Box::new(final_validation),
                },
            ),
        }
    }

    fn compile_compatibility_triage_inner(
        &self,
        observation_batch: &CandidateJudgeObservationBatchV1,
        cancellation: &CancellationToken,
    ) -> Result<CompiledCandidateJudgeTriage, CandidateJudgeTriageCompilationError> {
        self.revalidate(cancellation)
            .map_err(CandidateJudgeTriageCompilationError::Preparation)?;
        if self.deterministic_evaluation.status()
            != CandidateDeterministicEvaluationStatusV1::Passed
        {
            return Err(relationship(
                CandidateJudgeTriageCompilationRelationship::DeterministicEvaluation,
            ));
        }
        if observation_batch.candidate_judge_plan_id() != self.judge_plan.candidate_judge_plan_id()
            || observation_batch.candidate_judge_schedule_id()
                != self.judge_schedule.candidate_judge_schedule_id()
            || observation_batch.candidate_judge_request_aggregate_id()
                != self.request_aggregate.request_aggregate_id()
        {
            return Err(relationship(
                CandidateJudgeTriageCompilationRelationship::ObservationBatch,
            ));
        }
        let observations = map_observations(self, observation_batch.observations())?;
        let suites = reconstruct_compatibility_suites(
            &self.candidate_a,
            &self.candidate_b,
            &self.case_material,
            &self.eligible_semantic_indices,
            &self.compatibility_projection,
            cancellation,
        )
        .map_err(CandidateJudgeTriageCompilationError::Preparation)?;
        let batch = JudgeObservationBatch {
            schema_version: HYBRID_SCORECARD_SCHEMA_VERSION,
            plan_id: self.compatibility_projection.plan.plan_id.clone(),
            plan_digest: self.compatibility_projection.plan_digest.clone(),
            observations,
        };
        let report = run_exact_projection_scorecard(
            &self.compatibility_projection.plan,
            &suites.0,
            &suites.1,
            &batch,
        )
        .map_err(CandidateJudgeTriageCompilationError::Scorecard)?;
        drop(suites);
        if report.plan_id != self.compatibility_projection.plan.plan_id
            || report.plan_digest != self.compatibility_projection.plan_digest
            || !report.hard_gates_passed()
            || report.judge.total != self.compatibility_projection.plan.cases.len()
        {
            return Err(relationship(
                CandidateJudgeTriageCompilationRelationship::Report,
            ));
        }
        let canonical_json = serde_json::to_vec(&report)
            .map_err(|_| CandidateJudgeTriageCompilationError::Encoding)?;
        let relationship = CandidateJudgeTriageReportRelationshipV1::new(&canonical_json)
            .map_err(CandidateJudgeTriageCompilationError::Portable)?;
        Ok(CompiledCandidateJudgeTriage {
            report,
            canonical_json,
            relationship,
        })
    }
}

fn combine_preparation_results(
    primary: Result<(), CandidateJudgePreparationError>,
    final_validation: Result<(), CandidateJudgePreparationError>,
) -> Result<(), CandidateJudgePreparationError> {
    match (primary, final_validation) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(final_validation)) => Err(final_validation),
        (Err(primary), Err(final_validation)) => {
            Err(CandidateJudgePreparationError::PrimaryAndFinalValidation {
                primary: Box::new(primary),
                final_validation: Box::new(final_validation),
            })
        }
    }
}

fn map_observations(
    handoff: &CandidateJudgeRunnerHandoff<'_>,
    observations: &[CandidateJudgeObservationV1],
) -> Result<Vec<JudgeObservation>, CandidateJudgeTriageCompilationError> {
    let schedule = handoff.judge_schedule.entries();
    let semantic_cases = handoff.judge_plan.cases();
    let compatibility_cases = &handoff.compatibility_projection.plan.cases;
    let permutation = &handoff.compatibility_projection.semantic_to_lexicographic;
    if observations.len() != schedule.len()
        || schedule.len() != semantic_cases.len().saturating_mul(2)
        || permutation.len() != semantic_cases.len()
        || compatibility_cases.len() != semantic_cases.len()
    {
        return Err(relationship(
            CandidateJudgeTriageCompilationRelationship::ObservationSchedule,
        ));
    }
    let mut mapped = std::iter::repeat_with(|| None)
        .take(observations.len())
        .collect::<Vec<_>>();
    for (schedule_index, (observation, entry)) in observations.iter().zip(schedule).enumerate() {
        let semantic_position = schedule_index / 2;
        let pair_position = schedule_index % 2;
        let Some(planned) = semantic_cases.get(semantic_position) else {
            return Err(relationship(
                CandidateJudgeTriageCompilationRelationship::ObservationSchedule,
            ));
        };
        let lexicographic_position = permutation
            .get(semantic_position)
            .copied()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                relationship(CandidateJudgeTriageCompilationRelationship::CasePermutation)
            })?;
        let compatibility_case =
            compatibility_cases
                .get(lexicographic_position)
                .ok_or_else(|| {
                    relationship(CandidateJudgeTriageCompilationRelationship::CasePermutation)
                })?;
        let response_index =
            usize::try_from(observation.candidate_judge_response().schedule_index()).ok();
        if entry.case_id() != planned.case_id()
            || observation.case_id() != entry.case_id()
            || observation.presentation() != entry.presentation()
            || observation.attempt_ordinal() != entry.attempt_ordinal()
            || response_index != Some(schedule_index)
            || observation
                .candidate_judge_response()
                .structured_request_binding_id()
                != &handoff.request_aggregate.structured_request_binding_ids()[schedule_index]
        {
            return Err(relationship(
                CandidateJudgeTriageCompilationRelationship::ObservationSchedule,
            ));
        }
        let slot = lexicographic_position
            .checked_mul(2)
            .and_then(|value| value.checked_add(pair_position))
            .ok_or_else(|| {
                relationship(CandidateJudgeTriageCompilationRelationship::CasePermutation)
            })?;
        let Some(destination) = mapped.get_mut(slot) else {
            return Err(relationship(
                CandidateJudgeTriageCompilationRelationship::CasePermutation,
            ));
        };
        if destination.is_some() {
            return Err(relationship(
                CandidateJudgeTriageCompilationRelationship::CasePermutation,
            ));
        }
        *destination = Some(JudgeObservation {
            case_id: compatibility_case.id.clone(),
            presentation: map_presentation(observation.presentation()),
            choice: map_choice(observation.choice()),
            rubric_clauses: observation.cited_rubric_clause_ids().to_vec(),
        });
    }
    mapped
        .into_iter()
        .map(|value| {
            value.ok_or_else(|| {
                relationship(CandidateJudgeTriageCompilationRelationship::CasePermutation)
            })
        })
        .collect()
}

const fn map_presentation(value: CandidateJudgePresentationV1) -> JudgePresentation {
    match value {
        CandidateJudgePresentationV1::CandidateAFirst => JudgePresentation::CandidateAFirst,
        CandidateJudgePresentationV1::CandidateBFirst => JudgePresentation::CandidateBFirst,
    }
}

const fn map_choice(value: CandidateJudgeChoiceV1) -> JudgeChoice {
    match value {
        CandidateJudgeChoiceV1::First => JudgeChoice::First,
        CandidateJudgeChoiceV1::Second => JudgeChoice::Second,
        CandidateJudgeChoiceV1::Tie => JudgeChoice::Tie,
        CandidateJudgeChoiceV1::Abstain => JudgeChoice::Abstain,
    }
}

const fn relationship(
    value: CandidateJudgeTriageCompilationRelationship,
) -> CandidateJudgeTriageCompilationError {
    CandidateJudgeTriageCompilationError::Relationship(value)
}
