use std::collections::HashSet;

use super::super::GenerationQualificationOperationContractError;
use super::{
    GENERATION_QUALIFICATION_OPERATION_CANDIDATES_PER_COMPLETION,
    GenerationQualificationOperationLimitsV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
    generation_qualification_plan_failure_policy_digest,
};
use crate::generation_qualification::MAX_PLANNED_GENERATION_ATTEMPTS;

pub(super) fn validate_relations(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    input: &GenerationQualificationOperationPolicyV1Input,
) -> Result<(), GenerationQualificationOperationContractError> {
    let repetition_count = relations.repetitions.len();
    let case_count = relations.suite.case_ids().len();
    let expected_attempt_count = repetition_count
        .checked_mul(case_count)
        .and_then(|value| value.checked_mul(2))
        .filter(|value| *value > 0 && *value <= MAX_PLANNED_GENERATION_ATTEMPTS)
        .ok_or(GenerationQualificationOperationContractError::InvalidCount)?;
    if relations.planned_attempts.len() != expected_attempt_count {
        return Err(GenerationQualificationOperationContractError::InvalidCount);
    }

    input.limits.validate()?;
    validate_system_relations(relations)?;
    validate_plan_scope(relations, expected_attempt_count)?;
    validate_limits_against_relations(relations, input.limits, expected_attempt_count)?;
    validate_cross_product(relations, input.limits, expected_attempt_count)?;

    let expected_failure_policy = generation_qualification_plan_failure_policy_digest(
        input.decision_rule,
        &input.platform_assessment_policy_id,
        input.required_license_permission,
        &input.license_assessment_policy_id,
        &input.attempt_ledger_policy_digest,
        &input.repeatability_policy_digest,
        &input.resource_policy_digest,
        &input.human_adjudication_policy_digest,
    );
    if relations.plan.failure_policy_digest() != &expected_failure_policy {
        return Err(GenerationQualificationOperationContractError::InvalidDecisionRule);
    }
    Ok(())
}

fn validate_system_relations(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
) -> Result<(), GenerationQualificationOperationContractError> {
    let target = relations.target_system;
    let baseline = relations.baseline_system;
    if target.generation_system.generation_system_id()
        == baseline.generation_system.generation_system_id()
    {
        return Err(GenerationQualificationOperationContractError::ScopeMismatch);
    }
    target
        .generation_system
        .validate_against(target.relations)
        .map_err(|_| GenerationQualificationOperationContractError::RelationshipMismatch)?;
    baseline
        .generation_system
        .validate_against(baseline.relations)
        .map_err(|_| GenerationQualificationOperationContractError::RelationshipMismatch)?;
    if target.generation_system.language_digest() != baseline.generation_system.language_digest()
        || target.generation_system.mode_digest() != baseline.generation_system.mode_digest()
        || target.generation_system.format_digest() != baseline.generation_system.format_digest()
    {
        return Err(GenerationQualificationOperationContractError::ScopeMismatch);
    }
    Ok(())
}

fn validate_plan_scope(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    expected_attempt_count: usize,
) -> Result<(), GenerationQualificationOperationContractError> {
    if relations.plan.suite_manifest_id() != relations.suite.suite_manifest_id()
        || relations.plan.generation_system_ids().len() != 2
        || relations.plan.planned_attempt_ids().len() != expected_attempt_count
    {
        return Err(GenerationQualificationOperationContractError::ScopeMismatch);
    }
    let target_id = relations
        .target_system
        .generation_system
        .generation_system_id();
    let baseline_id = relations
        .baseline_system
        .generation_system
        .generation_system_id();
    if !relations.plan.generation_system_ids().contains(target_id)
        || !relations.plan.generation_system_ids().contains(baseline_id)
    {
        return Err(GenerationQualificationOperationContractError::ScopeMismatch);
    }
    for (ordinal, repetition) in relations.repetitions.iter().enumerate() {
        if repetition.suite_manifest_id() != relations.suite.suite_manifest_id()
            || usize::try_from(repetition.repetition_ordinal()) != Ok(ordinal)
        {
            return Err(GenerationQualificationOperationContractError::ScopeMismatch);
        }
    }
    Ok(())
}

fn validate_limits_against_relations(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    limits: GenerationQualificationOperationLimitsV1,
    expected_attempt_count: usize,
) -> Result<(), GenerationQualificationOperationContractError> {
    let exact_attempt_count = u32::try_from(expected_attempt_count)
        .map_err(|_| GenerationQualificationOperationContractError::EncodingOverflow)?;
    let plan_limits = relations.plan.limits();
    if limits.maximum_predeclared_attempts() != exact_attempt_count
        || plan_limits.maximum_predeclared_attempts() != exact_attempt_count
        || plan_limits.maximum_candidates_per_completion()
            != GENERATION_QUALIFICATION_OPERATION_CANDIDATES_PER_COMPLETION
        || limits.maximum_source_bytes() > plan_limits.maximum_retained_input_bytes()
        || limits.maximum_complete_input_bytes() > plan_limits.maximum_retained_input_bytes()
        || limits.maximum_context_tokens()
            > relations
                .target_system
                .relations
                .effective_runtime_state
                .effective_context_tokens()
        || limits.maximum_context_tokens()
            > relations
                .baseline_system
                .relations
                .effective_runtime_state
                .effective_context_tokens()
        || limits.maximum_candidate_bytes() > plan_limits.maximum_evidence_bundle_bytes()
        || limits.maximum_aggregate_candidate_bytes() > plan_limits.maximum_evidence_bundle_bytes()
        || limits.maximum_output_bytes() > plan_limits.maximum_evidence_bundle_bytes()
    {
        return Err(GenerationQualificationOperationContractError::InvalidLimits);
    }
    Ok(())
}

fn validate_cross_product(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    operation_limits: GenerationQualificationOperationLimitsV1,
    expected_attempt_count: usize,
) -> Result<(), GenerationQualificationOperationContractError> {
    let target_id = relations
        .target_system
        .generation_system
        .generation_system_id();
    let baseline_id = relations
        .baseline_system
        .generation_system
        .generation_system_id();
    let mut combinations = HashSet::with_capacity(expected_attempt_count);
    for (index, attempt) in relations.planned_attempts.iter().enumerate() {
        if relations.plan.planned_attempt_ids().get(index) != Some(attempt.planned_attempt_id()) {
            return Err(GenerationQualificationOperationContractError::RelationshipMismatch);
        }
        if usize::try_from(attempt.attempt_ordinal()) != Ok(index)
            || attempt.suite_manifest_id() != relations.suite.suite_manifest_id()
        {
            return Err(GenerationQualificationOperationContractError::NonCanonicalOrder);
        }
        if attempt.generation_system_id() != target_id
            && attempt.generation_system_id() != baseline_id
        {
            return Err(GenerationQualificationOperationContractError::ScopeMismatch);
        }
        if !relations
            .repetitions
            .iter()
            .any(|value| value.repetition_id() == attempt.repetition_id())
            || !relations.suite.case_ids().contains(attempt.case_id())
        {
            return Err(GenerationQualificationOperationContractError::ScopeMismatch);
        }
        if !combinations.insert((
            attempt.repetition_id(),
            attempt.case_id(),
            attempt.generation_system_id(),
        )) {
            return Err(GenerationQualificationOperationContractError::DuplicateEntry);
        }
        let output = attempt.output_ceilings();
        if attempt.source_byte_count() > operation_limits.maximum_source_bytes()
            || output.candidate_count()
                != GENERATION_QUALIFICATION_OPERATION_CANDIDATES_PER_COMPLETION
            || output.maximum_candidate_bytes() != operation_limits.maximum_candidate_bytes()
            || output.maximum_aggregate_candidate_bytes()
                != operation_limits.maximum_aggregate_candidate_bytes()
            || output.maximum_envelope_bytes() != operation_limits.maximum_output_bytes()
        {
            return Err(GenerationQualificationOperationContractError::InvalidLimits);
        }
    }
    if combinations.len() == expected_attempt_count {
        Ok(())
    } else {
        Err(GenerationQualificationOperationContractError::InvalidCount)
    }
}
