use rewrite_model::{
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations, PlannedCandidateAttemptV1Relations,
};

use super::{
    GenerationQualificationRequestProjectionCaseAuthoritiesV1,
    GenerationQualificationRequestProjectionError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SystemRole {
    Target,
    Baseline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AttemptRoute {
    pub(super) case_index: usize,
    pub(super) repetition_index: usize,
    pub(super) role: SystemRole,
}

pub(super) fn validate_and_derive_routes(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    operation_input: &GenerationQualificationOperationPolicyV1Input,
    cases: &[GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
) -> Result<Vec<AttemptRoute>, GenerationQualificationRequestProjectionError> {
    operation_policy
        .validate_against(relations, operation_input)
        .map_err(|_| GenerationQualificationRequestProjectionError::OperationMismatch)?;
    validate_case_authorities(relations, cases)?;
    derive_routes(relations, cases)
}

fn validate_case_authorities(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    cases: &[GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
) -> Result<(), GenerationQualificationRequestProjectionError> {
    if cases.len() != relations.suite.case_ids().len() {
        return Err(GenerationQualificationRequestProjectionError::CaseAuthorityMismatch);
    }
    for (case_id, authorities) in relations.suite.case_ids().iter().zip(cases) {
        authorities
            .target_builder
            .revalidate_static()
            .map_err(|_| GenerationQualificationRequestProjectionError::CaseAuthorityMismatch)?;
        authorities
            .baseline_builder
            .revalidate_static()
            .map_err(|_| GenerationQualificationRequestProjectionError::CaseAuthorityMismatch)?;
        let target_case = authorities.target_builder.case();
        let baseline_case = authorities.baseline_builder.case();
        if target_case.case_id() != case_id
            || baseline_case != target_case
            || !authorities.source.matches_case_manifest(target_case)
            || target_case.cluster_id() != authorities.cluster.cluster_id()
            || authorities.target_builder.generation_system()
                != relations.target_system.generation_system
            || authorities.baseline_builder.generation_system()
                != relations.baseline_system.generation_system
        {
            return Err(GenerationQualificationRequestProjectionError::CaseAuthorityMismatch);
        }
    }
    Ok(())
}

fn derive_routes(
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    cases: &[GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
) -> Result<Vec<AttemptRoute>, GenerationQualificationRequestProjectionError> {
    let mut routes = Vec::with_capacity(relations.planned_attempts.len());
    for (ordinal, attempt) in relations.planned_attempts.iter().enumerate() {
        let case_index = relations
            .suite
            .case_ids()
            .iter()
            .position(|case_id| case_id == attempt.case_id())
            .ok_or(GenerationQualificationRequestProjectionError::RouteMismatch)?;
        let repetition_index = relations
            .repetitions
            .iter()
            .position(|value| value.repetition_id() == attempt.repetition_id())
            .ok_or(GenerationQualificationRequestProjectionError::RouteMismatch)?;
        let role = if attempt.generation_system_id()
            == relations
                .target_system
                .generation_system
                .generation_system_id()
        {
            SystemRole::Target
        } else if attempt.generation_system_id()
            == relations
                .baseline_system
                .generation_system
                .generation_system_id()
        {
            SystemRole::Baseline
        } else {
            return Err(GenerationQualificationRequestProjectionError::RouteMismatch);
        };
        let authorities = cases
            .get(case_index)
            .ok_or(GenerationQualificationRequestProjectionError::RouteMismatch)?;
        let generation_system = match role {
            SystemRole::Target => authorities.target_builder.generation_system(),
            SystemRole::Baseline => authorities.baseline_builder.generation_system(),
        };
        if usize::try_from(attempt.attempt_ordinal()) != Ok(ordinal)
            || relations.plan.planned_attempt_ids().get(ordinal)
                != Some(attempt.planned_attempt_id())
        {
            return Err(GenerationQualificationRequestProjectionError::RouteMismatch);
        }
        attempt
            .validate_against(PlannedCandidateAttemptV1Relations {
                suite: relations.suite,
                case: authorities.target_builder.case(),
                cluster: &authorities.cluster,
                repetition: &relations.repetitions[repetition_index],
                generation_system,
            })
            .map_err(|_| GenerationQualificationRequestProjectionError::RouteMismatch)?;
        routes.push(AttemptRoute {
            case_index,
            repetition_index,
            role,
        });
    }
    Ok(routes)
}
