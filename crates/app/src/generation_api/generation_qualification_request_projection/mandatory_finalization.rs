use rewrite_types::CancellationToken;

use super::{
    GenerationQualificationRequestProjectionError,
    GenerationQualificationRequestProjectionV1Relations,
    VerifiedGenerationQualificationRequestProjectionV1, map_source_error, stream, validation,
};

impl VerifiedGenerationQualificationRequestProjectionV1<'_, '_> {
    /// Reconstructs the complete projection for mandatory operation finalization.
    ///
    /// This path creates its own fresh cancellation authority and never reads the
    /// retained operation deadline. It remains available after the original
    /// operation is cancelled or expired.
    ///
    /// # Errors
    ///
    /// Returns a content-free error for route, source, builder, request, or
    /// projection drift.
    pub fn revalidate_for_mandatory_finalization(
        &mut self,
    ) -> Result<(), GenerationQualificationRequestProjectionError> {
        let cancellation = CancellationToken::new();
        let routes = validation::validate_and_derive_routes(
            &self.operation_policy,
            self.operation_policy_relations,
            &self.operation_policy_input,
            &self.case_authorities,
        )?;
        if routes != self.routes {
            return Err(GenerationQualificationRequestProjectionError::ProjectionDrift);
        }
        revalidate_sources(&self.case_authorities, &cancellation)?;
        let entry_inputs = stream::derive_entry_inputs_for_mandatory_finalization(
            &self.operation_policy,
            self.operation_policy_relations,
            &mut self.case_authorities,
            &routes,
            &cancellation,
        )?;
        if entry_inputs != self.entry_inputs {
            return Err(GenerationQualificationRequestProjectionError::ProjectionDrift);
        }
        self.projection
            .validate_against(
                projection_relations(&self.operation_policy, self.operation_policy_relations),
                &entry_inputs,
            )
            .map_err(|_| GenerationQualificationRequestProjectionError::ProjectionDrift)?;
        let fresh_routes = validation::validate_and_derive_routes(
            &self.operation_policy,
            self.operation_policy_relations,
            &self.operation_policy_input,
            &self.case_authorities,
        )?;
        if fresh_routes != self.routes {
            return Err(GenerationQualificationRequestProjectionError::ProjectionDrift);
        }
        revalidate_sources(&self.case_authorities, &cancellation)
    }
}

fn revalidate_sources(
    cases: &[super::GenerationQualificationRequestProjectionCaseAuthoritiesV1<'_>],
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationRequestProjectionError> {
    for case in cases {
        case.source
            .revalidate(cancellation)
            .map_err(|error| map_source_error(&error))?;
    }
    Ok(())
}

fn projection_relations<'a>(
    operation_policy: &'a rewrite_model::GenerationQualificationOperationPolicyV1,
    relations: rewrite_model::GenerationQualificationOperationPolicyV1Relations<'a>,
) -> GenerationQualificationRequestProjectionV1Relations<'a> {
    GenerationQualificationRequestProjectionV1Relations {
        operation_policy,
        qualification_plan: relations.plan,
        suite: relations.suite,
        planned_attempts: relations.planned_attempts,
    }
}
