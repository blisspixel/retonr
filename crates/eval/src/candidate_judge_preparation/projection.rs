use std::fmt;

use rewrite_inference::GenerationCandidate;
use rewrite_model::CandidateJudgePlanV1;
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};

use super::request::selected_candidates;
use super::{
    CandidateJudgePreparationError, CandidateJudgePreparationInput,
    CandidateJudgePreparationRelationship, CandidateJudgePreparationSide, ensure_active,
};
use crate::candidate_deterministic_compiler::{
    derive_case_key_permutation, project_case_pair, validate_projection_subset_bound,
};
use crate::generation_case_material::{
    VerifiedGenerationCaseMaterial, VerifiedGenerationCaseMaterialTraversalError,
};
use crate::hybrid_scorecard::deterministic;
use crate::{
    EVALUATION_SCHEMA_VERSION, EvaluationCase, EvaluationSuite, HybridScorecardCasePlan,
    HybridScorecardPlan, JudgeAuthority, JudgeExecution, JudgeOrderPolicy, LocalJudgePolicy,
    VerifiedCandidateBatchSet,
};

const CANDIDATE_DIGEST_CHUNK_BYTES: usize = 64 * 1024;

pub(super) struct PreparedCompatibilityProjection {
    pub(super) plan: HybridScorecardPlan,
    pub(super) plan_digest: Digest,
    pub(super) semantic_to_lexicographic: Vec<u32>,
}

impl fmt::Debug for PreparedCompatibilityProjection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedCompatibilityProjection")
            .field("plan_digest", &self.plan_digest)
            .field("case_count", &self.plan.cases.len())
            .field(
                "semantic_to_lexicographic_count",
                &self.semantic_to_lexicographic.len(),
            )
            .finish_non_exhaustive()
    }
}

struct ProjectedCase {
    eligible_position: usize,
    semantic_index: usize,
    candidate_a: EvaluationCase,
    candidate_b: EvaluationCase,
}

#[expect(
    clippy::similar_names,
    reason = "candidate A and B suites are identity-significant ordered inputs"
)]
pub(super) fn compile_compatibility_projection(
    input: &CandidateJudgePreparationInput<'_, '_>,
    judge_plan: &CandidateJudgePlanV1,
    eligible_semantic_indices: &[usize],
    cancellation: &CancellationToken,
) -> Result<PreparedCompatibilityProjection, CandidateJudgePreparationError> {
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
    validate_projection_subset_bound(
        &input.case_material,
        &candidate_a,
        &candidate_b,
        eligible_semantic_indices,
    )
    .map_err(|_| compatibility_error())?;

    let projected = project_cases(
        &input.case_material,
        eligible_semantic_indices,
        &candidate_a,
        &candidate_b,
        cancellation,
    )?;
    ensure_active(cancellation)?;
    if projected.len() != eligible_semantic_indices.len() {
        return Err(compatibility_error());
    }
    let permutation_entries = projected
        .iter()
        .map(|case| {
            (
                case.eligible_position,
                case.candidate_a.id.as_str(),
                case.candidate_b.id.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let semantic_to_lexicographic =
        derive_case_key_permutation(&permutation_entries).map_err(|_| compatibility_error())?;

    let mut projected = projected;
    projected.sort_unstable_by(|left, right| left.candidate_a.id.cmp(&right.candidate_a.id));
    let compatibility_cases = compatibility_cases(input, judge_plan, &projected, cancellation)?;
    let (candidate_a_suite, candidate_b_suite) = projected_suites(projected);
    ensure_active(cancellation)?;
    let corpus_digest = deterministic::suite_pair_digest(&candidate_a_suite, &candidate_b_suite)
        .map_err(|_| compatibility_error())?;
    let plan = compatibility_plan(input, judge_plan, corpus_digest, compatibility_cases)?;
    ensure_active(cancellation)?;
    let Some(plan_digest) = deterministic::exact_projection_plan_digest_if_hard_gates_pass(
        &plan,
        &candidate_a_suite,
        &candidate_b_suite,
    )
    .map_err(|_| compatibility_error())?
    else {
        return Err(compatibility_error());
    };
    ensure_active(cancellation)?;
    Ok(PreparedCompatibilityProjection {
        plan,
        plan_digest,
        semantic_to_lexicographic,
    })
}

#[expect(
    clippy::similar_names,
    reason = "candidate A and B copies are identity-significant ordered inputs"
)]
fn project_cases(
    case_material: &VerifiedGenerationCaseMaterial<'_>,
    eligible_semantic_indices: &[usize],
    candidate_a: &[&GenerationCandidate],
    candidate_b: &[&GenerationCandidate],
    cancellation: &CancellationToken,
) -> Result<Vec<ProjectedCase>, CandidateJudgePreparationError> {
    case_material
        .try_with_all_case_source_bytes(cancellation, |semantic_index, source| {
            ensure_active(cancellation)?;
            let Ok(eligible_position) = eligible_semantic_indices.binary_search(&semantic_index)
            else {
                return Ok(None);
            };
            let contract = case_material
                .contracts()
                .get(semantic_index)
                .ok_or_else(compatibility_error)?;
            let candidate_a = candidate_a
                .get(semantic_index)
                .ok_or_else(compatibility_error)?;
            let candidate_b = candidate_b
                .get(semantic_index)
                .ok_or_else(compatibility_error)?;
            ensure_active(cancellation)?;
            let candidate_a_text = candidate_a.text.clone();
            ensure_active(cancellation)?;
            let candidate_b_text = candidate_b.text.clone();
            ensure_active(cancellation)?;
            let (candidate_a, candidate_b) =
                project_case_pair(contract, source, candidate_a_text, candidate_b_text)
                    .map_err(|_| compatibility_error())?;
            Ok(Some(ProjectedCase {
                eligible_position,
                semantic_index,
                candidate_a,
                candidate_b,
            }))
        })
        .map_err(map_traversal_error)
        .map(|cases| cases.into_iter().flatten().collect())
}

/// Reconstructs the exact eligible compatibility suites from retained authority.
///
/// The suites contain source and candidate text and must remain scoped to the
/// crate-private triage compiler. This function rederives the frozen permutation,
/// corpus, plan relationship, and passing eligible-subset hard gates before
/// releasing them.
pub(super) fn reconstruct_compatibility_suites(
    candidate_a: &VerifiedCandidateBatchSet,
    candidate_b: &VerifiedCandidateBatchSet,
    case_material: &VerifiedGenerationCaseMaterial<'_>,
    eligible_semantic_indices: &[usize],
    expected: &PreparedCompatibilityProjection,
    cancellation: &CancellationToken,
) -> Result<(EvaluationSuite, EvaluationSuite), CandidateJudgePreparationError> {
    ensure_active(cancellation)?;
    let candidate_a = selected_candidates(
        candidate_a,
        CandidateJudgePreparationSide::CandidateA,
        cancellation,
    )?;
    let candidate_b = selected_candidates(
        candidate_b,
        CandidateJudgePreparationSide::CandidateB,
        cancellation,
    )?;
    validate_projection_subset_bound(
        case_material,
        &candidate_a,
        &candidate_b,
        eligible_semantic_indices,
    )
    .map_err(|_| compatibility_error())?;
    let mut projected = project_cases(
        case_material,
        eligible_semantic_indices,
        &candidate_a,
        &candidate_b,
        cancellation,
    )?;
    if projected.len() != eligible_semantic_indices.len() {
        return Err(compatibility_error());
    }
    let permutation_entries = projected
        .iter()
        .map(|case| {
            (
                case.eligible_position,
                case.candidate_a.id.as_str(),
                case.candidate_b.id.as_str(),
            )
        })
        .collect::<Vec<_>>();
    let permutation =
        derive_case_key_permutation(&permutation_entries).map_err(|_| compatibility_error())?;
    if permutation != expected.semantic_to_lexicographic {
        return Err(compatibility_error());
    }
    projected.sort_unstable_by(|left, right| left.candidate_a.id.cmp(&right.candidate_a.id));
    let suites = projected_suites(projected);
    ensure_active(cancellation)?;
    let Some(plan_digest) = deterministic::exact_projection_plan_digest_if_hard_gates_pass(
        &expected.plan,
        &suites.0,
        &suites.1,
    )
    .map_err(|_| compatibility_error())?
    else {
        return Err(compatibility_error());
    };
    if plan_digest != expected.plan_digest {
        return Err(compatibility_error());
    }
    ensure_active(cancellation)?;
    Ok(suites)
}

fn compatibility_cases(
    input: &CandidateJudgePreparationInput<'_, '_>,
    judge_plan: &CandidateJudgePlanV1,
    projected: &[ProjectedCase],
    cancellation: &CancellationToken,
) -> Result<Vec<HybridScorecardCasePlan>, CandidateJudgePreparationError> {
    projected
        .iter()
        .map(|case| {
            ensure_active(cancellation)?;
            let manifest = input
                .case_material
                .cases()
                .get(case.semantic_index)
                .ok_or_else(compatibility_error)?;
            let planned = judge_plan
                .cases()
                .get(case.eligible_position)
                .ok_or_else(compatibility_error)?;
            if planned.case_id() != manifest.case_id() || case.candidate_a.id != case.candidate_b.id
            {
                return Err(compatibility_error());
            }
            Ok(HybridScorecardCasePlan {
                id: case.candidate_a.id.clone(),
                cluster_id: manifest.cluster_id().digest().as_str().to_owned(),
                source_digest: manifest.source_digest().clone(),
                candidate_a_digest: candidate_digest(
                    case.candidate_a.candidate.as_bytes(),
                    cancellation,
                )?,
                candidate_b_digest: candidate_digest(
                    case.candidate_b.candidate.as_bytes(),
                    cancellation,
                )?,
                candidate_a_system_digest: judge_plan
                    .candidate_a_generation_system_id()
                    .digest()
                    .clone(),
                candidate_b_system_digest: judge_plan
                    .candidate_b_generation_system_id()
                    .digest()
                    .clone(),
                rubric_clauses: planned.rubric_clause_ids().to_vec(),
            })
        })
        .collect()
}

fn candidate_digest(
    bytes: &[u8],
    cancellation: &CancellationToken,
) -> Result<Digest, CandidateJudgePreparationError> {
    let mut hasher = Sha256::new();
    for chunk in bytes.chunks(CANDIDATE_DIGEST_CHUNK_BYTES) {
        ensure_active(cancellation)?;
        hasher.update(chunk);
    }
    ensure_active(cancellation)?;
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize())).map_err(|_| compatibility_error())
}

fn projected_suites(projected: Vec<ProjectedCase>) -> (EvaluationSuite, EvaluationSuite) {
    let mut candidate_a = Vec::with_capacity(projected.len());
    let mut candidate_b = Vec::with_capacity(projected.len());
    for projected in projected {
        candidate_a.push(projected.candidate_a);
        candidate_b.push(projected.candidate_b);
    }
    (
        EvaluationSuite {
            schema_version: EVALUATION_SCHEMA_VERSION,
            cases: candidate_a,
        },
        EvaluationSuite {
            schema_version: EVALUATION_SCHEMA_VERSION,
            cases: candidate_b,
        },
    )
}

fn compatibility_plan(
    input: &CandidateJudgePreparationInput<'_, '_>,
    judge_plan: &CandidateJudgePlanV1,
    corpus_digest: Digest,
    cases: Vec<HybridScorecardCasePlan>,
) -> Result<HybridScorecardPlan, CandidateJudgePreparationError> {
    let limits = judge_plan.limits();
    let max_judge_cases =
        u16::try_from(limits.maximum_judge_cases()).map_err(|_| compatibility_error())?;
    let attempts_per_order =
        u8::try_from(judge_plan.attempts_per_order()).map_err(|_| compatibility_error())?;
    let model_digest = input
        .plan_relations
        .judge_system
        .model_artifact_id()
        .digest()
        .clone();
    let plan = HybridScorecardPlan {
        schema_version: crate::HYBRID_SCORECARD_SCHEMA_VERSION,
        plan_id: judge_plan
            .candidate_judge_plan_id()
            .digest()
            .as_str()
            .to_owned(),
        corpus_digest,
        rubric_digest: judge_plan.rubric_digest().clone(),
        deterministic_policy_digest: deterministic::policy_digest(),
        judge: LocalJudgePolicy {
            execution: JudgeExecution::LocalIsolated,
            authority: JudgeAuthority::TriageOnly,
            order_policy: JudgeOrderPolicy::BothOrders,
            judge_system_digest: judge_plan.judge_generation_system_id().digest().clone(),
            judge_model_reference: model_digest.as_str().to_owned(),
            judge_model_digest: model_digest,
            judge_prompt_contract_digest: judge_plan.prompt_contract_digest().clone(),
            judge_output_schema_digest: judge_plan.output_schema_digest().clone(),
            presentation_seed: judge_plan.presentation_seed(),
            temperature_milli: 0,
            top_p_milli: 1_000,
            attempts_per_order,
            max_judge_cases,
            max_source_bytes: limits.maximum_source_bytes(),
            max_candidate_bytes: limits.maximum_candidate_bytes(),
            max_input_bytes: limits.maximum_complete_input_bytes(),
            context_token_limit: limits.maximum_context_tokens(),
            output_token_limit: limits.maximum_output_tokens(),
            max_response_bytes: limits.maximum_response_bytes(),
            maximum_elapsed_millis: limits.maximum_elapsed_milliseconds(),
        },
        cases,
    };
    if plan.cases.iter().any(|case| {
        case.candidate_a_system_digest != *judge_plan.candidate_a_generation_system_id().digest()
            || case.candidate_b_system_digest
                != *judge_plan.candidate_b_generation_system_id().digest()
    }) {
        return Err(compatibility_error());
    }
    Ok(plan)
}

fn map_traversal_error(
    error: VerifiedGenerationCaseMaterialTraversalError<CandidateJudgePreparationError>,
) -> CandidateJudgePreparationError {
    match error {
        VerifiedGenerationCaseMaterialTraversalError::Material { source } => {
            CandidateJudgePreparationError::CaseMaterial { source }
        }
        VerifiedGenerationCaseMaterialTraversalError::MaterialAndFinalValidation {
            primary,
            final_validation,
        } => combine(
            CandidateJudgePreparationError::CaseMaterial { source: primary },
            CandidateJudgePreparationError::CaseMaterial {
                source: *final_validation,
            },
        ),
        VerifiedGenerationCaseMaterialTraversalError::Callback { source, .. } => source,
        VerifiedGenerationCaseMaterialTraversalError::CallbackAndFinalValidation {
            source,
            final_validation,
            ..
        } => combine(
            source,
            CandidateJudgePreparationError::CaseMaterial {
                source: *final_validation,
            },
        ),
        VerifiedGenerationCaseMaterialTraversalError::CallbackAndSourceValidation {
            source,
            source_validation,
            final_validation,
            ..
        } => {
            let combined = combine(
                source,
                CandidateJudgePreparationError::CaseMaterial {
                    source: *source_validation,
                },
            );
            match final_validation {
                Some(final_validation) => combine(
                    combined,
                    CandidateJudgePreparationError::CaseMaterial {
                        source: *final_validation,
                    },
                ),
                None => combined,
            }
        }
    }
}

fn combine(
    primary: CandidateJudgePreparationError,
    final_validation: CandidateJudgePreparationError,
) -> CandidateJudgePreparationError {
    CandidateJudgePreparationError::PrimaryAndFinalValidation {
        primary: Box::new(primary),
        final_validation: Box::new(final_validation),
    }
}

fn compatibility_error() -> CandidateJudgePreparationError {
    CandidateJudgePreparationError::Relationship(
        CandidateJudgePreparationRelationship::CompatibilityProjection,
    )
}

#[cfg(test)]
mod tests;
