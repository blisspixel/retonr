use rewrite_inference::local_judge_attempt_output_contract;
use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateGenerationReceiptSetV1, CandidateJudgePlanV1,
};
use rewrite_types::CancellationToken;
use rewrite_types::RewriteStatus;

use super::{
    CandidateJudgePreparationError, CandidateJudgePreparationInput,
    CandidateJudgePreparationRelationship, CandidateJudgePreparationSide, ensure_active,
    portable_error,
};
use crate::{
    ExpectedOutput, ReferenceJudgment, local_judge_prompt_contract_digest,
    local_judge_rubric_digest,
};

pub(super) struct ValidatedPreOutputClosure {
    pub(super) judge_plan: CandidateJudgePlanV1,
    pub(super) eligible_semantic_indices: Vec<usize>,
}

pub(super) fn validate_pre_output_closure(
    input: &CandidateJudgePreparationInput<'_, '_>,
    deterministic: &CandidateDeterministicEvaluationRecordV1,
    cancellation: &CancellationToken,
) -> Result<ValidatedPreOutputClosure, CandidateJudgePreparationError> {
    ensure_active(cancellation)?;
    let plan_bytes = serde_json::to_vec(input.judge_plan).map_err(|_| {
        CandidateJudgePreparationError::Relationship(
            CandidateJudgePreparationRelationship::JudgePlan,
        )
    })?;
    let judge_plan = CandidateJudgePlanV1::from_json_bytes(&plan_bytes, input.plan_relations)
        .map_err(portable_error)?;
    if &judge_plan != input.judge_plan {
        return Err(relationship(
            CandidateJudgePreparationRelationship::JudgePlan,
        ));
    }

    let receipt_a = input
        .candidate_a
        .receipt_set(cancellation)
        .map_err(|source| CandidateJudgePreparationError::CandidateBatchSet {
            side: CandidateJudgePreparationSide::CandidateA,
            source,
        })?;
    let receipt_b = input
        .candidate_b
        .receipt_set(cancellation)
        .map_err(|source| CandidateJudgePreparationError::CandidateBatchSet {
            side: CandidateJudgePreparationSide::CandidateB,
            source,
        })?;
    validate_candidate_plan_closure(&judge_plan, receipt_a, receipt_b, deterministic)?;

    if judge_plan.case_material_set_digest() != input.case_material.case_material_set_digest()
        || judge_plan.suite_manifest_id() != input.case_material.suite().suite_manifest_id()
        || deterministic.case_material_set_digest()
            != input.case_material.case_material_set_digest()
    {
        return Err(relationship(
            CandidateJudgePreparationRelationship::CaseMaterialClosure,
        ));
    }
    let rubric_digest =
        local_judge_rubric_digest(input.rubric).map_err(CandidateJudgePreparationError::Rubric)?;
    if judge_plan.rubric_digest() != &rubric_digest
        || judge_plan.prompt_contract_digest() != &local_judge_prompt_contract_digest()
        || judge_plan.output_schema_digest() != &local_judge_attempt_output_contract().schema_digest
    {
        return Err(relationship(
            CandidateJudgePreparationRelationship::JudgePolicyClosure,
        ));
    }

    let eligible_semantic_indices = exact_eligible_indices(input, &judge_plan)?;
    Ok(ValidatedPreOutputClosure {
        judge_plan,
        eligible_semantic_indices,
    })
}

fn validate_candidate_plan_closure(
    plan: &CandidateJudgePlanV1,
    candidate_a: &CandidateGenerationReceiptSetV1,
    candidate_b: &CandidateGenerationReceiptSetV1,
    deterministic: &CandidateDeterministicEvaluationRecordV1,
) -> Result<(), CandidateJudgePreparationError> {
    let common = |receipt: &CandidateGenerationReceiptSetV1| {
        plan.qualification_plan_id() == receipt.qualification_plan_id()
            && plan.suite_manifest_id() == receipt.suite_manifest_id()
            && plan.repetition_id() == receipt.repetition_id()
            && plan.selection_policy_id() == receipt.selection_policy_id()
    };
    if !common(candidate_a)
        || !common(candidate_b)
        || plan.candidate_a_generation_system_id() != candidate_a.generation_system_id()
        || plan.candidate_b_generation_system_id() != candidate_b.generation_system_id()
        || plan.suite_manifest_id() != deterministic.suite_manifest_id()
        || plan.repetition_id() != deterministic.repetition_id()
        || plan.candidate_a_generation_system_id()
            != deterministic.candidate_a_generation_system_id()
        || plan.candidate_b_generation_system_id()
            != deterministic.candidate_b_generation_system_id()
    {
        return Err(relationship(
            CandidateJudgePreparationRelationship::CandidatePlanClosure,
        ));
    }
    Ok(())
}

fn exact_eligible_indices(
    input: &CandidateJudgePreparationInput<'_, '_>,
    plan: &CandidateJudgePlanV1,
) -> Result<Vec<usize>, CandidateJudgePreparationError> {
    let eligible = input
        .case_material
        .contracts()
        .iter()
        .enumerate()
        .filter_map(|(index, contract)| {
            (contract.reference_judgment() == ReferenceJudgment::Acceptable
                && contract.expected_status() == RewriteStatus::Rewritten
                && contract.expected_reason().is_none()
                && contract.expected_output() == ExpectedOutput::Candidate)
                .then_some(index)
        })
        .collect::<Vec<_>>();
    if eligible.is_empty() || eligible.len() != plan.cases().len() {
        return Err(relationship(
            CandidateJudgePreparationRelationship::EligibleCaseClosure,
        ));
    }
    for (plan_case, semantic_index) in plan.cases().iter().zip(&eligible) {
        let manifest = &input.case_material.cases()[*semantic_index];
        let contract = &input.case_material.contracts()[*semantic_index];
        if plan_case.case_id() != manifest.case_id() {
            return Err(relationship(
                CandidateJudgePreparationRelationship::EligibleCaseClosure,
            ));
        }
        if plan_case.rubric_clause_ids() != contract.rubric_clause_ids()
            || plan_case.rubric_clause_ids().iter().any(|id| {
                input
                    .rubric
                    .clauses
                    .binary_search_by(|clause| clause.id.as_str().cmp(id))
                    .is_err()
            })
        {
            return Err(relationship(
                CandidateJudgePreparationRelationship::RubricClauseClosure,
            ));
        }
    }
    Ok(eligible)
}

fn relationship(
    relationship: CandidateJudgePreparationRelationship,
) -> CandidateJudgePreparationError {
    CandidateJudgePreparationError::Relationship(relationship)
}

#[cfg(test)]
#[path = "validation/private_tests.rs"]
mod tests;
