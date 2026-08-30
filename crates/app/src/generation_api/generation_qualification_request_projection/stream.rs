use std::time::Instant;

use rewrite_model::{
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Relations,
    GenerationQualificationRequestProjectionEntryV1Input,
};
use rewrite_types::CancellationToken;

use super::validation::{AttemptRoute, SystemRole};
use super::{
    GenerationQualificationRequestBuildError,
    GenerationQualificationRequestProjectionCaseAuthoritiesV1,
    GenerationQualificationRequestProjectionError, check_cancelled, check_gate,
};
use crate::GenerationQualificationRequestBuildInput;

pub(super) fn derive_entry_inputs(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    cases: &mut [GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
    routes: &[AttemptRoute],
    cancellation: &CancellationToken,
    deadline: Instant,
) -> Result<
    Vec<GenerationQualificationRequestProjectionEntryV1Input>,
    GenerationQualificationRequestProjectionError,
> {
    derive_entry_inputs_with_gate(
        operation_policy,
        relations,
        cases,
        routes,
        cancellation,
        || check_gate(cancellation, deadline),
    )
}

pub(super) fn derive_entry_inputs_for_mandatory_finalization(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    cases: &mut [GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
    routes: &[AttemptRoute],
    cancellation: &CancellationToken,
) -> Result<
    Vec<GenerationQualificationRequestProjectionEntryV1Input>,
    GenerationQualificationRequestProjectionError,
> {
    derive_entry_inputs_with_gate(
        operation_policy,
        relations,
        cases,
        routes,
        cancellation,
        || check_cancelled(cancellation),
    )
}

fn derive_entry_inputs_with_gate(
    operation_policy: &GenerationQualificationOperationPolicyV1,
    relations: GenerationQualificationOperationPolicyV1Relations<'_>,
    cases: &mut [GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
    routes: &[AttemptRoute],
    cancellation: &CancellationToken,
    mut check_active: impl FnMut() -> Result<(), GenerationQualificationRequestProjectionError>,
) -> Result<
    Vec<GenerationQualificationRequestProjectionEntryV1Input>,
    GenerationQualificationRequestProjectionError,
> {
    if routes.len() != relations.planned_attempts.len() {
        return Err(GenerationQualificationRequestProjectionError::RouteMismatch);
    }
    let mut inputs = Vec::with_capacity(routes.len());
    for (ordinal, route) in routes.iter().copied().enumerate() {
        check_active()?;
        let attempt = relations
            .planned_attempts
            .get(ordinal)
            .ok_or(GenerationQualificationRequestProjectionError::RouteMismatch)?;
        let repetition = relations
            .repetitions
            .get(route.repetition_index)
            .ok_or(GenerationQualificationRequestProjectionError::RouteMismatch)?;
        let authorities = cases
            .get_mut(route.case_index)
            .ok_or(GenerationQualificationRequestProjectionError::RouteMismatch)?;
        let source = &authorities.source;
        let cluster = &authorities.cluster;
        let builder = match route.role {
            SystemRole::Target => &mut authorities.target_builder,
            SystemRole::Baseline => &mut authorities.baseline_builder,
        };
        let built_result = builder.build(
            GenerationQualificationRequestBuildInput {
                source,
                operation_policy,
                qualification_plan: relations.plan,
                suite: relations.suite,
                cluster,
                repetition,
                planned_attempt: attempt,
            },
            cancellation,
        );
        check_active()?;
        let built =
            built_result.map_err(|error| map_build_error(attempt.attempt_ordinal(), error))?;
        let validation_result = built.revalidate(cancellation);
        check_active()?;
        validation_result.map_err(|error| map_build_error(attempt.attempt_ordinal(), error))?;
        let projection_result = built.projection_input();
        check_active()?;
        let projection_input =
            projection_result.map_err(|error| map_build_error(attempt.attempt_ordinal(), error))?;
        drop(built);
        inputs.push(projection_input);
    }
    Ok(inputs)
}

fn map_build_error(
    attempt_ordinal: u32,
    source: GenerationQualificationRequestBuildError,
) -> GenerationQualificationRequestProjectionError {
    if source == GenerationQualificationRequestBuildError::Cancelled {
        GenerationQualificationRequestProjectionError::Cancelled
    } else {
        GenerationQualificationRequestProjectionError::RequestBuild {
            attempt_ordinal,
            source,
        }
    }
}
