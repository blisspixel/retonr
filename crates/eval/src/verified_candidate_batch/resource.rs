use rewrite_app::GenerationQualificationPhasePolicySourceDisposition;
use rewrite_model::{
    GenerationQualificationPhaseEvidenceError, GenerationResourceAttemptResultRecordV1,
    GenerationResourceAttemptResultRecordV1Input, GenerationResourceAttemptResultRecordV1Relations,
};
use rewrite_ollama::derive_ollama_retained_session_response_id;

use crate::local_ollama_managed_preflight::ResourceObservedManagedCandidateAttemptClosure;
use crate::{VerifiedCandidateBatchError, VerifiedCandidateBatchRelationship};

use super::VerifiedCandidateBatchResourceInput;

pub(super) fn validate_resource_mode(
    has_resource_closure: bool,
    resource_compilation_requested: bool,
) -> Result<(), VerifiedCandidateBatchError> {
    match (has_resource_closure, resource_compilation_requested) {
        (false, false) | (true, true) => Ok(()),
        (true, false) => Err(VerifiedCandidateBatchError::Relationship(
            VerifiedCandidateBatchRelationship::UncompiledResourceObservation,
        )),
        (false, true) => Err(VerifiedCandidateBatchError::Relationship(
            VerifiedCandidateBatchRelationship::MissingResourceObservation,
        )),
    }
}

trait ResourceObservationView {
    fn revalidates(&self) -> bool;
    fn exact_bindings_match(
        &self,
        input: &VerifiedCandidateBatchResourceInput<'_>,
        attempt: &crate::VerifiedCompletedManagedCandidateAttempt,
        receipt: &rewrite_model::CandidateGenerationReceiptV1,
    ) -> bool;
    fn record_input(&self) -> GenerationResourceAttemptResultRecordV1Input;
}

#[derive(Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the private kernel tests each independent trust-boundary check"
)]
struct ResourceCompilationKernelChecks {
    source_approved: bool,
    operation_policy_matches: bool,
    observation_revalidates_before: bool,
    exact_bindings_match: bool,
}

impl ResourceCompilationKernelChecks {
    fn validate(self) -> Result<(), VerifiedCandidateBatchError> {
        if self.source_approved
            && self.operation_policy_matches
            && self.observation_revalidates_before
            && self.exact_bindings_match
        {
            Ok(())
        } else {
            Err(VerifiedCandidateBatchError::Relationship(
                VerifiedCandidateBatchRelationship::ResourceObservation,
            ))
        }
    }
}

fn reviewed_generate_ordinal(first_response_ordinal: usize) -> Option<u64> {
    first_response_ordinal
        .checked_add(3)
        .and_then(|value| u64::try_from(value).ok())
}

impl ResourceObservationView for ResourceObservedManagedCandidateAttemptClosure {
    fn revalidates(&self) -> bool {
        self.observation.revalidate().is_ok()
            && self.worker_observation.worker_evidence_digest()
                == self.worker_evidence.evidence_digest()
            && self.worker_observation.observation_digest()
                == self.observation.worker_observation_digest()
            && self.worker_observation.high_water_resident_bytes()
                == self.observation.worker_high_water_resident_bytes()
            && self.worker_evidence.evidence_digest() == self.observation.worker_evidence_digest()
    }

    fn exact_bindings_match(
        &self,
        input: &VerifiedCandidateBatchResourceInput<'_>,
        attempt: &crate::VerifiedCompletedManagedCandidateAttempt,
        receipt: &rewrite_model::CandidateGenerationReceiptV1,
    ) -> bool {
        let observed = &self.observation;
        let residency = observed.resident_execution_receipt();
        let execution = residency.execution();
        let expected_ordinal = reviewed_generate_ordinal(execution.first_response_ordinal());
        observed.resource_policy_digest() == input.resource_policy.policy_digest()
            && observed.operation_policy_id() == input.operation_policy.operation_policy_id()
            && observed.qualification_plan_id()
                == input.scope.qualification_plan.qualification_plan_id()
            && observed.suite_manifest_id() == input.scope.suite.suite_manifest_id()
            && observed.target_generation_system_id()
                == input.scope.generation_system.generation_system_id()
            && observed.target_generation_system_id()
                == attempt.generation_system().generation_system_id()
            && observed.request_binding_digest() == &attempt.structured_request().binding_digest()
            && observed.response_id() == receipt.response_id()
            && observed.response_id()
                == &derive_ollama_retained_session_response_id(observed.response())
            && observed.receipt_binding_digest() == &residency.complete_binding_digest()
            && expected_ordinal == Some(observed.response_ordinal())
    }

    fn record_input(&self) -> GenerationResourceAttemptResultRecordV1Input {
        let observed = &self.observation;
        GenerationResourceAttemptResultRecordV1Input {
            prompt_token_count: observed.prompt_token_count(),
            generated_token_count: observed.generated_token_count(),
            total_duration_nanoseconds: observed.total_duration_nanoseconds(),
            load_duration_nanoseconds: observed.load_duration_nanoseconds(),
            prompt_evaluation_duration_nanoseconds: observed
                .prompt_evaluation_duration_nanoseconds(),
            evaluation_duration_nanoseconds: observed.evaluation_duration_nanoseconds(),
            attempt_elapsed_nanoseconds: observed.attempt_elapsed_nanoseconds(),
            first_response_elapsed_nanoseconds: observed.first_response_elapsed_nanoseconds(),
            cleanup_elapsed_nanoseconds: observed.cleanup_elapsed_nanoseconds(),
            worker_high_water_resident_bytes: observed.worker_high_water_resident_bytes(),
            runtime_installed_payload_bytes: observed.runtime_installed_payload_bytes(),
            model_installed_payload_bytes: observed.model_installed_payload_bytes(),
            installed_footprint_bytes: observed.installed_footprint_bytes(),
            exceeded_limits: observed.exceeded_limits().to_vec(),
        }
    }
}

pub(super) fn compile_resource_result(
    closure: &ResourceObservedManagedCandidateAttemptClosure,
    attempt: &crate::VerifiedCompletedManagedCandidateAttempt,
    receipt_compilation: &rewrite_app::CandidateGenerationReceiptCompilation,
    input: &VerifiedCandidateBatchResourceInput<'_>,
) -> Result<GenerationResourceAttemptResultRecordV1, VerifiedCandidateBatchError> {
    compile_resource_result_from_view(closure, attempt, receipt_compilation, input)
}

fn compile_resource_result_from_view<V: ResourceObservationView>(
    observed: &V,
    attempt: &crate::VerifiedCompletedManagedCandidateAttempt,
    receipt_compilation: &rewrite_app::CandidateGenerationReceiptCompilation,
    input: &VerifiedCandidateBatchResourceInput<'_>,
) -> Result<GenerationResourceAttemptResultRecordV1, VerifiedCandidateBatchError> {
    ResourceCompilationKernelChecks {
        source_approved: input.resource_policy.source_disposition()
            == GenerationQualificationPhasePolicySourceDisposition::Approved,
        operation_policy_matches: input
            .resource_policy
            .revalidate_operation_policy(input.operation_policy)
            .is_ok(),
        observation_revalidates_before: observed.revalidates(),
        exact_bindings_match: observed.exact_bindings_match(
            input,
            attempt,
            receipt_compilation.receipt(),
        ),
    }
    .validate()?;
    let relations = GenerationResourceAttemptResultRecordV1Relations {
        scope: input.scope,
        operation_policy: input.operation_policy,
        case: input.case,
        repetition: input.repetition,
        planned_attempt: attempt.planned_attempt(),
        attempt_record: receipt_compilation.attempt_record(),
        candidate_generation_receipt: receipt_compilation.receipt(),
    };
    let record_input = observed.record_input();
    let result = GenerationResourceAttemptResultRecordV1::new(relations, record_input.clone())?;
    if !observed.revalidates() || result.validate_against(relations, &record_input).is_err() {
        return Err(VerifiedCandidateBatchError::Relationship(
            VerifiedCandidateBatchRelationship::ResourceObservation,
        ));
    }
    Ok(result)
}

pub(super) fn validate_retained_resource(
    closure: &ResourceObservedManagedCandidateAttemptClosure,
    result: &GenerationResourceAttemptResultRecordV1,
    attempt: &crate::VerifiedCompletedManagedCandidateAttempt,
    receipt: &rewrite_model::CandidateGenerationReceiptV1,
) -> Result<(), VerifiedCandidateBatchError> {
    let observed = &closure.observation;
    let values_match = closure.revalidates()
        && result.generation_system_id() == attempt.generation_system().generation_system_id()
        && result.planned_attempt_id() == attempt.planned_attempt().planned_attempt_id()
        && result.candidate_generation_receipt_id() == receipt.receipt_id()
        && result.resource_policy_digest() == observed.resource_policy_digest()
        && result.observation_profile() == observed.observation_profile()
        && result.prompt_token_count() == observed.prompt_token_count()
        && result.generated_token_count() == observed.generated_token_count()
        && result.total_duration_nanoseconds() == observed.total_duration_nanoseconds()
        && result.load_duration_nanoseconds() == observed.load_duration_nanoseconds()
        && result.prompt_evaluation_duration_nanoseconds()
            == observed.prompt_evaluation_duration_nanoseconds()
        && result.evaluation_duration_nanoseconds() == observed.evaluation_duration_nanoseconds()
        && result.attempt_elapsed_nanoseconds() == observed.attempt_elapsed_nanoseconds()
        && result.first_response_elapsed_nanoseconds()
            == observed.first_response_elapsed_nanoseconds()
        && result.cleanup_elapsed_nanoseconds() == observed.cleanup_elapsed_nanoseconds()
        && result.worker_high_water_resident_bytes() == observed.worker_high_water_resident_bytes()
        && result.runtime_installed_payload_bytes() == observed.runtime_installed_payload_bytes()
        && result.model_installed_payload_bytes() == observed.model_installed_payload_bytes()
        && result.installed_footprint_bytes() == observed.installed_footprint_bytes()
        && result.exceeded_limits() == observed.exceeded_limits();
    if values_match {
        Ok(())
    } else {
        Err(VerifiedCandidateBatchError::Relationship(
            VerifiedCandidateBatchRelationship::ResourceResult,
        ))
    }
}

impl From<GenerationQualificationPhaseEvidenceError> for VerifiedCandidateBatchError {
    fn from(error: GenerationQualificationPhaseEvidenceError) -> Self {
        Self::ResourceResult(error)
    }
}

#[cfg(test)]
#[path = "resource_tests.rs"]
mod tests;
