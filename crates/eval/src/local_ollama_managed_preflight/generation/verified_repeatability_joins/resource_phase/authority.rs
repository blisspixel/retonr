use rewrite_app::GenerationQualificationPhasePolicySourceDisposition;

use super::GenerationQualificationResourcePhaseAuthorityError;

#[derive(Clone, Copy)]
pub(super) struct ResourceAuthorityBindingView {
    pub(super) source_disposition: GenerationQualificationPhasePolicySourceDisposition,
    pub(super) policy_binding_matches: bool,
    pub(super) operation_scope_matches: bool,
}

pub(super) fn validate_resource_authority_bindings(
    view: ResourceAuthorityBindingView,
) -> Result<(), GenerationQualificationResourcePhaseAuthorityError> {
    if view.source_disposition != GenerationQualificationPhasePolicySourceDisposition::Approved {
        Err(GenerationQualificationResourcePhaseAuthorityError::PolicyDenied)
    } else if !view.policy_binding_matches {
        Err(GenerationQualificationResourcePhaseAuthorityError::PolicyBinding)
    } else if !view.operation_scope_matches {
        Err(GenerationQualificationResourcePhaseAuthorityError::OperationScope)
    } else {
        Ok(())
    }
}
