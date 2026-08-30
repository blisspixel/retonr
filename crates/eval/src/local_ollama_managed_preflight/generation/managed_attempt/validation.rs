use rewrite_app::{VerifiedAdmittedRuntime, VerifiedManagedGenerationPath};
use rewrite_inference::{
    CandidateOutputPolicy, GenerationCandidate, GenerationRequest, StructuredCompletionRequest,
    parse_candidate_output,
};
use rewrite_model::{
    CandidateGenerationAttemptPrecursorV1, CandidateOutputCeilingsV1, EffectiveRuntimeState,
    GenerationSystemRecordV1, PlannedCandidateAttemptV1,
};
use rewrite_ollama::derive_ollama_retained_session_response_id;
use rewrite_runtime_attestor::VerifiedFrozenExternalNativeComponentSet;

use super::super::{LocalOllamaManagedGenerationError, PendingLocalOllamaManagedGenerationOutcome};

#[expect(
    clippy::too_many_arguments,
    reason = "each independent portable or opaque relationship remains explicit"
)]
pub(super) fn validate_pending_attempt(
    pending: &PendingLocalOllamaManagedGenerationOutcome,
    precursor: &CandidateGenerationAttemptPrecursorV1,
    planned_attempt: &PlannedCandidateAttemptV1,
    generation_system: &GenerationSystemRecordV1,
    expected_state: &EffectiveRuntimeState,
    generation_request: &GenerationRequest,
    structured_request: &StructuredCompletionRequest,
    admitted_runtime: &VerifiedAdmittedRuntime,
    generation_path: &VerifiedManagedGenerationPath,
    frozen_components: &VerifiedFrozenExternalNativeComponentSet,
) -> Result<Vec<GenerationCandidate>, LocalOllamaManagedGenerationError> {
    let bracket = &pending.bracket_observation;
    let receipt = pending.residency_receipt();
    let response = pending.response();
    let observed_state = pending.effective_runtime_state.state();
    let structured_digest = structured_request.binding_digest();
    let response_id = derive_ollama_retained_session_response_id(response);

    let relationships_match = observed_state == expected_state
        && observed_state.effective_runtime_state_id()
            == *precursor.expected_effective_runtime_state_id()
        && observed_state.effective_runtime_state_id()
            == *generation_system.effective_runtime_state_id()
        && precursor.planned_attempt_id() == planned_attempt.planned_attempt_id()
        && planned_attempt.generation_system_id() == generation_system.generation_system_id()
        && generation_request.generation_request_binding_id()
            == *planned_attempt.generation_request_binding_id()
        && structured_request.structured_request_binding_id()
            == *precursor.structured_request_binding_id()
        && response.request_binding_digest() == &structured_digest
        && receipt.execution().request_digest() == &structured_digest
        && receipt.execution().response_digest() == response_id.digest()
        && bracket.response_binding_digest() == receipt.execution().response_digest()
        && bracket.effective_runtime_state_id() == precursor.expected_effective_runtime_state_id()
        && bracket.effective_runtime_state_id() == generation_system.effective_runtime_state_id()
        && bracket.effective_runtime_state_join_digest()
            == pending.effective_runtime_state.binding_digest()
        && bracket.runtime_build_id() == precursor.runtime_build_id()
        && bracket.runtime_build_id() == generation_system.runtime_build_id()
        && bracket.admitted_runtime_id() == admitted_runtime.admitted_runtime_id().digest()
        && bracket.admitted_runtime_id() == precursor.runtime_admission_join_id().digest()
        && bracket.generation_path_id() == generation_path.generation_path_id()
        && bracket.generation_path_id() == precursor.managed_generation_path_id().digest()
        && bracket.frozen_external_component_set_id() == frozen_components.frozen_set_id().digest()
        && bracket.frozen_external_component_set_id()
            == precursor.frozen_external_component_set_id().digest()
        && bracket.runtime_package_installation_generation()
            == precursor.runtime_installation_generation()
        && bracket.model_package_installation_generation()
            == precursor.model_installation_generation()
        && bracket.model_package_manifest_id() == precursor.model_package_manifest_id()
        && bracket.model_package_manifest_id() == generation_system.model_package_manifest_id()
        && bracket.model_artifact_id() == precursor.model_artifact_id()
        && bracket.model_artifact_id() == generation_system.model_artifact_id()
        && response.artifact_id() == &structured_request.artifact_id
        && response.artifact_digest() == &structured_request.artifact_digest;
    if !relationships_match {
        return Err(LocalOllamaManagedGenerationError::InvalidCandidateAttemptRelationship);
    }

    parse_response_candidates(
        pending.response().output_json().as_bytes(),
        planned_attempt.output_ceilings(),
    )
}

pub(super) fn parse_response_candidates(
    response_bytes: &[u8],
    ceilings: CandidateOutputCeilingsV1,
) -> Result<Vec<GenerationCandidate>, LocalOllamaManagedGenerationError> {
    let policy = CandidateOutputPolicy::new(
        ceilings.candidate_count(),
        ceilings.maximum_candidate_bytes(),
        ceilings.maximum_aggregate_candidate_bytes(),
    )
    .map_err(LocalOllamaManagedGenerationError::ResponseValidation)?;
    if policy.maximum_envelope_bytes() != ceilings.maximum_envelope_bytes() {
        return Err(LocalOllamaManagedGenerationError::InvalidCandidateAttemptRelationship);
    }
    parse_candidate_output(response_bytes, policy)
        .map_err(LocalOllamaManagedGenerationError::ResponseValidation)
}
