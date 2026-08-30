//! Initial operation-policy state.

use rewrite_app::{
    GenerationQualificationOperationDeadlineV1,
    GenerationQualificationRequestProjectionCaseAuthoritiesV1,
    GenerationQualificationRequestProjectionCompilerV1Input,
    VerifiedGenerationQualificationRequestProjectionV1,
};
use rewrite_model::{
    GenerationQualificationOperationPolicyV1, GenerationQualificationOperationPolicyV1Input,
    GenerationQualificationOperationPolicyV1Relations,
};
use rewrite_types::CancellationToken;

use super::{
    GenerationQualificationPreparationError, ProjectedGenerationQualificationOperation, check_gate,
    map_projection_error,
};

/// Policy-validated draft before exact request projection.
///
/// This state has no `finish` method. Request projection is a mandatory state
/// transition before preregistration can be prepared.
///
/// ```compile_fail
/// use rewrite_eval::GenerationQualificationOperationDraft;
/// fn cannot_finish_draft(value: GenerationQualificationOperationDraft<'_>) {
///     let _ = value.finish();
/// }
/// ```
pub struct GenerationQualificationOperationDraft<'records> {
    policy: GenerationQualificationOperationPolicyV1,
    relations: GenerationQualificationOperationPolicyV1Relations<'records>,
    input: GenerationQualificationOperationPolicyV1Input,
    deadline: GenerationQualificationOperationDeadlineV1,
}

impl<'records> GenerationQualificationOperationDraft<'records> {
    /// Captures the operation clock and constructs the exact portable policy.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for an unavailable deadline,
    /// cancellation, expiry, or any invalid policy relationship.
    pub fn begin(
        relations: GenerationQualificationOperationPolicyV1Relations<'records>,
        owned_policy_input: GenerationQualificationOperationPolicyV1Input,
        cancellation: &CancellationToken,
    ) -> Result<Self, GenerationQualificationPreparationError> {
        let operation_deadline = GenerationQualificationOperationDeadlineV1::start(
            owned_policy_input.limits,
            cancellation,
        )
        .map_err(map_projection_error)?;
        let deadline = operation_deadline.deadline();
        check_gate(deadline, cancellation)?;
        let policy_result =
            GenerationQualificationOperationPolicyV1::new(relations, owned_policy_input.clone());
        check_gate(deadline, cancellation)?;
        let operation_policy = policy_result
            .map_err(|_| GenerationQualificationPreparationError::OperationPolicyMismatch)?;
        Ok(Self {
            policy: operation_policy,
            relations,
            input: owned_policy_input,
            deadline: operation_deadline,
        })
    }

    /// Consumes the draft and streams all exact requests into one projection.
    ///
    /// # Errors
    ///
    /// Returns a content-redacted error for expiry, cancellation, source drift,
    /// invalid case authority routing, request drift, or projection mismatch.
    pub fn project<'store>(
        self,
        owned_case_authorities: Vec<
            GenerationQualificationRequestProjectionCaseAuthoritiesV1<'store>,
        >,
        cancellation: &CancellationToken,
    ) -> Result<
        ProjectedGenerationQualificationOperation<'records, 'store>,
        GenerationQualificationPreparationError,
    > {
        let deadline = self.deadline.deadline();
        check_gate(deadline, cancellation)?;
        let stream_result = VerifiedGenerationQualificationRequestProjectionV1::compile(
            GenerationQualificationRequestProjectionCompilerV1Input {
                operation_deadline: self.deadline,
                operation_policy: self.policy,
                operation_policy_relations: self.relations,
                operation_policy_input: self.input,
                case_authorities: owned_case_authorities,
            },
            cancellation,
        );
        check_gate(deadline, cancellation)?;
        let stream = stream_result.map_err(map_projection_error)?;
        Ok(ProjectedGenerationQualificationOperation::from_verified_stream(stream))
    }
}
