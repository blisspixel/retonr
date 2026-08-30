use super::{
    GENERATION_RESOURCE_ATTEMPT_RESULT_ID_DOMAIN, GenerationResourceAttemptResultRecordV1,
    GenerationResourceAttemptResultRecordV1Input, GenerationResourceAttemptResultRecordV1Relations,
    MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_CANONICAL_BYTES,
};
use crate::generation_qualification::CandidateGenerationAttemptOutcomeV1;
use crate::generation_qualification::codec::{append_count, append_digest, append_u32, append_u64};
use crate::generation_qualification::phase_evidence::common::{
    GenerationQualificationPhaseEvidenceError, validate_scope,
};

pub(super) fn validate_relationships(
    relations: GenerationResourceAttemptResultRecordV1Relations<'_>,
    input: &GenerationResourceAttemptResultRecordV1Input,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    validate_scope(relations.scope)?;
    let policy = relations.operation_policy;
    let scope = relations.scope;
    if policy.generation_qualification_plan_id() != scope.qualification_plan.qualification_plan_id()
        || policy.suite_manifest_id() != scope.suite.suite_manifest_id()
        || policy.target_generation_system_id() != scope.generation_system.generation_system_id()
        || relations.case.case_id() != relations.planned_attempt.case_id()
        || relations.repetition.repetition_id() != relations.planned_attempt.repetition_id()
        || relations.repetition.suite_manifest_id() != scope.suite.suite_manifest_id()
        || relations.planned_attempt.suite_manifest_id() != scope.suite.suite_manifest_id()
        || relations.planned_attempt.generation_system_id()
            != scope.generation_system.generation_system_id()
        || scope
            .suite
            .case_ids()
            .iter()
            .filter(|value| *value == relations.case.case_id())
            .count()
            != 1
        || scope
            .qualification_plan
            .planned_attempt_ids()
            .iter()
            .filter(|value| *value == relations.planned_attempt.planned_attempt_id())
            .count()
            != 1
    {
        return Err(GenerationQualificationPhaseEvidenceError::ScopeMismatch);
    }
    validate_attempt_and_receipt(relations, input)?;
    validate_observations(input)
}

fn validate_attempt_and_receipt(
    relations: GenerationResourceAttemptResultRecordV1Relations<'_>,
    input: &GenerationResourceAttemptResultRecordV1Input,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    let receipt = relations.candidate_generation_receipt;
    let completed = matches!(
        relations.attempt_record.outcome(),
        CandidateGenerationAttemptOutcomeV1::Completed {
            planned_attempt_id,
            precursor_id,
            receipt_id,
        } if planned_attempt_id == relations.planned_attempt.planned_attempt_id()
            && precursor_id == receipt.precursor_id()
            && receipt_id == receipt.receipt_id()
    );
    let usage = receipt.usage_observation();
    if !completed
        || receipt.qualification_plan_id()
            != relations.scope.qualification_plan.qualification_plan_id()
        || receipt.suite_manifest_id() != relations.scope.suite.suite_manifest_id()
        || receipt.case_id() != relations.case.case_id()
        || receipt.repetition_id() != relations.repetition.repetition_id()
        || receipt.planned_attempt_id() != relations.planned_attempt.planned_attempt_id()
        || receipt.generation_system_id()
            != relations.scope.generation_system.generation_system_id()
        || usage.input_tokens() != Some(input.prompt_token_count)
        || usage.output_tokens() != Some(input.generated_token_count)
        || usage.generation_micros() != Some(input.evaluation_duration_nanoseconds / 1_000)
    {
        return Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch);
    }
    Ok(())
}

fn validate_observations(
    input: &GenerationResourceAttemptResultRecordV1Input,
) -> Result<(), GenerationQualificationPhaseEvidenceError> {
    let component_duration = input
        .load_duration_nanoseconds
        .checked_add(input.prompt_evaluation_duration_nanoseconds)
        .and_then(|value| value.checked_add(input.evaluation_duration_nanoseconds));
    let footprint = input
        .runtime_installed_payload_bytes
        .checked_add(input.model_installed_payload_bytes);
    if component_duration.is_none_or(|value| value > input.total_duration_nanoseconds)
        || input.total_duration_nanoseconds > input.attempt_elapsed_nanoseconds
        || input.first_response_elapsed_nanoseconds > input.attempt_elapsed_nanoseconds
        || input.cleanup_elapsed_nanoseconds > input.attempt_elapsed_nanoseconds
        || footprint != Some(input.installed_footprint_bytes)
    {
        return Err(GenerationQualificationPhaseEvidenceError::InvalidResourceObservation);
    }
    if input
        .exceeded_limits
        .windows(2)
        .any(|pair| pair[0].tag() >= pair[1].tag())
    {
        return Err(GenerationQualificationPhaseEvidenceError::InvalidResourceLimitList);
    }
    Ok(())
}

pub(super) fn canonical_bytes(
    value: &GenerationResourceAttemptResultRecordV1,
) -> Result<Vec<u8>, GenerationQualificationPhaseEvidenceError> {
    let mut output = GENERATION_RESOURCE_ATTEMPT_RESULT_ID_DOMAIN.to_vec();
    append_u32(&mut output, value.schema_version);
    for digest in [
        value.generation_system_id.digest(),
        value.generation_qualification_plan_id.digest(),
        value.suite_manifest_id.digest(),
        value.case_id.digest(),
        value.repetition_id.digest(),
        value.planned_attempt_id.digest(),
        value.attempt_record_id.digest(),
        value.candidate_generation_receipt_id.digest(),
        &value.resource_policy_digest,
    ] {
        append_digest(&mut output, digest);
    }
    output.push(0); // GenerationResourceObservationProfileV1::ManagedOllamaV0_32_15LinuxV1
    for observation in [
        value.prompt_token_count,
        value.generated_token_count,
        value.total_duration_nanoseconds,
        value.load_duration_nanoseconds,
        value.prompt_evaluation_duration_nanoseconds,
        value.evaluation_duration_nanoseconds,
        value.attempt_elapsed_nanoseconds,
        value.first_response_elapsed_nanoseconds,
        value.cleanup_elapsed_nanoseconds,
        value.worker_high_water_resident_bytes,
        value.runtime_installed_payload_bytes,
        value.model_installed_payload_bytes,
        value.installed_footprint_bytes,
    ] {
        append_u64(&mut output, observation);
    }
    append_count(&mut output, value.exceeded_limits.len())
        .map_err(|_| GenerationQualificationPhaseEvidenceError::CountOverflow)?;
    output.extend(value.exceeded_limits.iter().map(|limit| limit.tag()));
    if output.len() > MAX_GENERATION_RESOURCE_ATTEMPT_RESULT_CANONICAL_BYTES {
        Err(GenerationQualificationPhaseEvidenceError::CanonicalEncodingTooLarge)
    } else {
        Ok(output)
    }
}
