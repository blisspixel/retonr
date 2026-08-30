use super::{
    CANDIDATE_JUDGE_SCHEMA_VERSION, CandidateJudgeCaseV1, CandidateJudgeOrderPolicyV1,
    CandidateJudgePlanV1Input, CandidateJudgePlanV1Relations, MAX_JUDGE_CASES,
};
use crate::generation_qualification::{
    GENERATION_QUALIFICATION_SCHEMA_VERSION, GenerationCaseId,
    GenerationQualificationContractError, GenerationSystemId,
};
pub(super) fn validate_relations(
    schema_version: u32,
    relations: CandidateJudgePlanV1Relations<'_>,
    input: &CandidateJudgePlanV1Input,
) -> Result<(), GenerationQualificationContractError> {
    if schema_version != CANDIDATE_JUDGE_SCHEMA_VERSION
        || schema_version != GENERATION_QUALIFICATION_SCHEMA_VERSION
    {
        return Err(GenerationQualificationContractError::UnsupportedSchema(
            schema_version,
        ));
    }
    let candidate_a = relations.candidate_a_system.generation_system_id();
    let candidate_b = relations.candidate_b_system.generation_system_id();
    let judge = relations.judge_system.generation_system_id();
    let systems = relations.qualification_plan.generation_system_ids();
    if relations.qualification_plan.suite_manifest_id() != relations.suite.suite_manifest_id()
        || relations.repetition.suite_manifest_id() != relations.suite.suite_manifest_id()
        || relations.selection_policy.suite_manifest_id() != relations.suite.suite_manifest_id()
        || relations.qualification_plan.selection_policy_digest()
            != relations.selection_policy.selection_policy_id().digest()
        || !planned_repetition_closes(relations, candidate_a, candidate_b)
        || systems.len() != 2
        || !systems.contains(candidate_a)
        || !systems.contains(candidate_b)
        || systems.contains(judge)
        || candidate_a == candidate_b
        || judge == candidate_a
        || judge == candidate_b
        || input.prompt_contract_digest != *relations.judge_system.prompt_digest()
        || input.output_schema_digest != *relations.judge_system.output_schema_digest()
        || input.attempts_per_order != 1
        || input.order_policy != CandidateJudgeOrderPolicyV1::BothOrders
        || !input.limits.valid()
        || input.cases.is_empty()
        || u32::try_from(input.cases.len()).ok().is_none_or(|count| {
            count > input.limits.maximum_judge_cases() || count > MAX_JUDGE_CASES
        })
        || input.cases.iter().any(|value| value.validate().is_err())
        || !semantic_subsequence(relations.suite.case_ids(), &input.cases)
    {
        return Err(GenerationQualificationContractError::CandidateJudgePlanRelationshipMismatch);
    }
    Ok(())
}

fn planned_repetition_closes(
    relations: CandidateJudgePlanV1Relations<'_>,
    candidate_a: &GenerationSystemId,
    candidate_b: &GenerationSystemId,
) -> bool {
    let planned_ids = relations.qualification_plan.planned_attempt_ids();
    if relations.planned_attempts.len() != planned_ids.len()
        || !relations
            .planned_attempts
            .iter()
            .zip(planned_ids)
            .all(|(attempt, id)| attempt.planned_attempt_id() == id)
    {
        return false;
    }
    for system in [candidate_a, candidate_b] {
        for case_id in relations.suite.case_ids() {
            if relations
                .planned_attempts
                .iter()
                .filter(|attempt| {
                    attempt.repetition_id() == relations.repetition.repetition_id()
                        && attempt.generation_system_id() == system
                        && attempt.case_id() == case_id
                })
                .count()
                != 1
            {
                return false;
            }
        }
    }
    relations
        .planned_attempts
        .iter()
        .filter(|attempt| attempt.repetition_id() == relations.repetition.repetition_id())
        .all(|attempt| {
            [candidate_a, candidate_b].contains(&attempt.generation_system_id())
                && relations.suite.case_ids().contains(attempt.case_id())
        })
}

fn semantic_subsequence(suite: &[GenerationCaseId], cases: &[CandidateJudgeCaseV1]) -> bool {
    let mut next = 0_usize;
    for case in cases {
        let Some(relative) = suite[next..]
            .iter()
            .position(|value| value == case.case_id())
        else {
            return false;
        };
        next += relative + 1;
    }
    true
}
