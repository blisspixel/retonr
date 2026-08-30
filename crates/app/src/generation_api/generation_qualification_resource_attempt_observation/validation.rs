use std::time::Duration;

use rewrite_model::{GenerationQualificationOperationPolicyV1, GenerationResourceExceededLimitV1};
use rewrite_ollama::{
    OllamaResidentResourceObservedCompletion, derive_ollama_retained_session_response_id,
};
use rewrite_types::CancellationToken;

use crate::{
    GenerationQualificationPhasePolicySourceDisposition,
    GenerationQualificationResourcePolicyLimitsV1, ReleasedGenerationEffectivePackageV2,
    RuntimePackageLease, VerifiedGenerationQualificationResourcePolicy,
    VerifiedManagedOllamaModelPackageLease,
};

use super::{
    GenerationQualificationResourceAttemptClock,
    GenerationQualificationResourceAttemptObservationError as Error,
    GenerationQualificationResourceAttemptObservationInput, ResourceAttemptValues,
    VerifiedGenerationQualificationResourceAttemptObservation,
};

const GENERATE_RESPONSE_OFFSET: usize = 3;
const FIRST_RESIDENCY_RESPONSE_OFFSET: usize = 4;
const COMPLETE_RESPONSE_OFFSET: usize = 8;

pub(super) fn validate_policy(
    policy: &VerifiedGenerationQualificationResourcePolicy,
    operation_policy: &GenerationQualificationOperationPolicyV1,
    cancellation: &CancellationToken,
) -> Result<(), Error> {
    check_cancelled(cancellation)?;
    if policy.source_disposition() != GenerationQualificationPhasePolicySourceDisposition::Approved
    {
        return Err(Error::PolicyDenied);
    }
    policy
        .revalidate_operation_policy(operation_policy)
        .map_err(|_error| Error::PolicyBindingMismatch)
}

pub(super) fn finish(
    clock: GenerationQualificationResourceAttemptClock,
    input: GenerationQualificationResourceAttemptObservationInput<'_>,
) -> Result<VerifiedGenerationQualificationResourceAttemptObservation, Error> {
    let GenerationQualificationResourceAttemptObservationInput {
        policy,
        operation_policy,
        completion,
        worker_observation,
        worker_evidence,
        released_package,
        runtime_package,
        model_package,
        cancellation,
    } = input;
    validate_clock_policy(&clock, policy, operation_policy, cancellation)?;
    validate_response_binding(&completion)?;
    if !released_package.binds_resource_attempt_completion(&completion) {
        return Err(Error::ResponseBindingMismatch);
    }
    validate_worker_binding(worker_observation, worker_evidence)?;
    if !released_package.binds_resource_attempt_worker(worker_observation, worker_evidence) {
        return Err(Error::WorkerBindingMismatch);
    }
    validate_package_binding(released_package, runtime_package, model_package)?;
    if !released_package.binds_resource_attempt_packages(runtime_package, model_package) {
        return Err(Error::PackageBindingMismatch);
    }

    let attempt_elapsed = released_package
        .managed_attempt_elapsed_since(clock.started)
        .ok_or(Error::ClockOrderInvalid)?;
    let first_response_elapsed = completion
        .response_head_elapsed_since(clock.started)
        .map_err(|_error| Error::ClockOrderInvalid)?;
    let values = values(
        &completion,
        worker_observation.high_water_resident_bytes(),
        released_package.cleanup_elapsed(),
        attempt_elapsed,
        first_response_elapsed,
        runtime_package.evidence().payload_byte_size(),
        model_package.byte_size(),
    )?;
    validate_observation_consistency(values)?;
    fresh_package_revalidation(runtime_package, model_package, cancellation)?;
    check_cancelled(cancellation)?;
    validate_clock_policy(&clock, policy, operation_policy, cancellation)?;

    let exceeded_limits = exceeded_limits(values, clock.limits);
    let response = completion.response();
    let receipt = completion.resident_execution_receipt();
    let request_binding_digest = response.request_binding_digest().clone();
    let response_id = derive_ollama_retained_session_response_id(response);
    let response_ordinal = ordinal(
        receipt
            .execution()
            .first_response_ordinal()
            .checked_add(GENERATE_RESPONSE_OFFSET)
            .ok_or(Error::MeasurementOverflow)?,
    )?;
    let receipt_binding_digest = receipt.complete_binding_digest();
    let retained_session_subject = released_package
        .resource_attempt_session_subject()
        .ok_or(Error::ResponseBindingMismatch)?;
    let mut observed = VerifiedGenerationQualificationResourceAttemptObservation {
        completion,
        retained_session_subject,
        policy_digest: clock.policy_digest,
        operation_policy_id: clock.operation_policy_id,
        qualification_plan_id: clock.qualification_plan_id,
        suite_manifest_id: clock.suite_manifest_id,
        target_generation_system_id: clock.target_generation_system_id,
        request_binding_digest,
        response_id,
        response_ordinal,
        receipt_binding_digest,
        worker_evidence_digest: worker_evidence.evidence_digest().clone(),
        worker_observation_digest: worker_observation.observation_digest().clone(),
        effective_package_id: released_package
            .evidence()
            .effective_package_evidence_v2_id(),
        runtime_package_manifest_id: runtime_package
            .evidence()
            .runtime_package_manifest_id()
            .clone(),
        model_package_manifest_id: model_package.model_package_manifest_id().clone(),
        values,
        limits: clock.limits,
        exceeded_limits,
        snapshot_digest: rewrite_types::Digest::sha256(b"uninitialized resource snapshot"),
    };
    observed.snapshot_digest = snapshot_digest(&observed);
    Ok(observed)
}

pub(super) fn revalidate(
    observed: &VerifiedGenerationQualificationResourceAttemptObservation,
) -> Result<(), Error> {
    validate_response_binding(&observed.completion)?;
    if !observed
        .completion
        .binds_retained_session_subject(&observed.retained_session_subject)
    {
        return Err(Error::ObservationInconsistent);
    }
    validate_observation_consistency(observed.values)?;
    let response = observed.completion.response();
    let receipt = observed.completion.resident_execution_receipt();
    let expected_response_ordinal = receipt
        .execution()
        .first_response_ordinal()
        .checked_add(GENERATE_RESPONSE_OFFSET)
        .ok_or(Error::MeasurementOverflow)
        .and_then(ordinal)?;
    if observed.request_binding_digest != *response.request_binding_digest()
        || observed.response_id != derive_ollama_retained_session_response_id(response)
        || observed.response_ordinal != expected_response_ordinal
        || observed.receipt_binding_digest != receipt.complete_binding_digest()
        || !completion_values_match(observed.values, &observed.completion)
        || observed
            .values
            .runtime_installed_payload_bytes
            .checked_add(observed.values.model_installed_payload_bytes)
            != Some(observed.values.installed_footprint_bytes)
        || observed.exceeded_limits != exceeded_limits(observed.values, observed.limits)
        || observed.snapshot_digest != snapshot_digest(observed)
    {
        Err(Error::ObservationInconsistent)
    } else {
        Ok(())
    }
}

fn validate_clock_policy(
    clock: &GenerationQualificationResourceAttemptClock,
    policy: &VerifiedGenerationQualificationResourcePolicy,
    operation: &GenerationQualificationOperationPolicyV1,
    cancellation: &CancellationToken,
) -> Result<(), Error> {
    validate_policy(policy, operation, cancellation)?;
    if clock.policy_digest != *policy.policy_digest()
        || clock.operation_policy_id != *operation.operation_policy_id()
        || clock.qualification_plan_id != *operation.generation_qualification_plan_id()
        || clock.suite_manifest_id != *operation.suite_manifest_id()
        || clock.target_generation_system_id != *operation.target_generation_system_id()
        || clock.limits != policy.limits()
    {
        Err(Error::PolicyBindingMismatch)
    } else {
        Ok(())
    }
}

pub(super) fn validate_response_binding(
    completion: &OllamaResidentResourceObservedCompletion,
) -> Result<(), Error> {
    completion
        .verify_completion_binding()
        .map_err(|_error| Error::ResponseBindingMismatch)?;
    let response = completion.response();
    let receipt = completion.resident_execution_receipt();
    let execution = receipt.execution();
    let first = execution.first_response_ordinal();
    let first_residency = first
        .checked_add(FIRST_RESIDENCY_RESPONSE_OFFSET)
        .ok_or(Error::MeasurementOverflow)?;
    let last = first
        .checked_add(COMPLETE_RESPONSE_OFFSET)
        .ok_or(Error::MeasurementOverflow)?;
    let response_id = derive_ollama_retained_session_response_id(response);
    let usage = response.usage();
    if execution.request_digest() != response.request_binding_digest()
        || execution.response_digest() != response_id.digest()
        || execution.last_response_ordinal() != last
        || receipt.first_residency_ordinal() != first_residency
        || receipt.last_residency_ordinal() != last
        || usage.input_tokens != Some(completion.prompt_token_count())
        || usage.output_tokens != Some(completion.generated_token_count())
        || usage.generation_micros != Some(completion.evaluation_duration_nanoseconds() / 1_000)
    {
        Err(Error::ResponseBindingMismatch)
    } else {
        Ok(())
    }
}

pub(super) fn validate_worker_binding(
    observation: &rewrite_runtime_attestor::ManagedGenerationWorkerResourceObservation,
    evidence: &rewrite_runtime_attestor::ManagedGenerationWorkerEvidence,
) -> Result<(), Error> {
    if observation.worker_evidence_digest() == evidence.evidence_digest() {
        Ok(())
    } else {
        Err(Error::WorkerBindingMismatch)
    }
}

pub(super) fn validate_package_binding(
    released: &ReleasedGenerationEffectivePackageV2,
    runtime: &RuntimePackageLease,
    model: &VerifiedManagedOllamaModelPackageLease,
) -> Result<(), Error> {
    let runtime_evidence = runtime.evidence();
    if released.runtime_artifact_set_id() != runtime_evidence.artifact_set_id()
        || released.runtime_package_manifest_id() != runtime_evidence.runtime_package_manifest_id()
        || released.model_artifact_set_id() != model.artifact_set_id()
        || released.model_package_manifest_id() != model.model_package_manifest_id()
        || released.evidence().artifact_set_id() != model.artifact_set_id()
        || released.foundation_id() != model.foundation_id()
    {
        Err(Error::PackageBindingMismatch)
    } else {
        Ok(())
    }
}

pub(super) fn fresh_package_revalidation(
    runtime: &mut RuntimePackageLease,
    model: &VerifiedManagedOllamaModelPackageLease,
    cancellation: &CancellationToken,
) -> Result<(), Error> {
    let model_result = model.revalidate(cancellation);
    let runtime_result = runtime.revalidate(cancellation);
    if cancellation.is_cancelled() {
        Err(Error::Cancelled)
    } else if model_result.is_err() || runtime_result.is_err() {
        Err(Error::PackageRevalidationFailed)
    } else {
        Ok(())
    }
}

fn values(
    completion: &OllamaResidentResourceObservedCompletion,
    worker_high_water_resident_bytes: u64,
    cleanup_elapsed: Duration,
    attempt_elapsed: Duration,
    first_response_elapsed: Duration,
    runtime_installed_payload_bytes: u64,
    model_installed_payload_bytes: u64,
) -> Result<ResourceAttemptValues, Error> {
    let installed_footprint_bytes = installed_footprint(
        runtime_installed_payload_bytes,
        model_installed_payload_bytes,
    )?;
    Ok(ResourceAttemptValues {
        prompt_token_count: completion.prompt_token_count(),
        generated_token_count: completion.generated_token_count(),
        total_duration_nanoseconds: completion.total_duration_nanoseconds(),
        load_duration_nanoseconds: completion.load_duration_nanoseconds(),
        prompt_evaluation_duration_nanoseconds: completion.prompt_evaluation_duration_nanoseconds(),
        evaluation_duration_nanoseconds: completion.evaluation_duration_nanoseconds(),
        attempt_elapsed_nanoseconds: duration_nanoseconds(attempt_elapsed)?,
        first_response_elapsed_nanoseconds: duration_nanoseconds(first_response_elapsed)?,
        cleanup_elapsed_nanoseconds: duration_nanoseconds(cleanup_elapsed)?,
        worker_high_water_resident_bytes,
        runtime_installed_payload_bytes,
        model_installed_payload_bytes,
        installed_footprint_bytes,
    })
}

fn completion_values_match(
    values: ResourceAttemptValues,
    completion: &OllamaResidentResourceObservedCompletion,
) -> bool {
    values.prompt_token_count == completion.prompt_token_count()
        && values.generated_token_count == completion.generated_token_count()
        && values.total_duration_nanoseconds == completion.total_duration_nanoseconds()
        && values.load_duration_nanoseconds == completion.load_duration_nanoseconds()
        && values.prompt_evaluation_duration_nanoseconds
            == completion.prompt_evaluation_duration_nanoseconds()
        && values.evaluation_duration_nanoseconds == completion.evaluation_duration_nanoseconds()
}

fn validate_observation_consistency(values: ResourceAttemptValues) -> Result<(), Error> {
    let provider_components = values
        .load_duration_nanoseconds
        .checked_add(values.prompt_evaluation_duration_nanoseconds)
        .and_then(|sum| sum.checked_add(values.evaluation_duration_nanoseconds))
        .ok_or(Error::MeasurementOverflow)?;
    if provider_components > values.total_duration_nanoseconds
        || values.total_duration_nanoseconds > values.attempt_elapsed_nanoseconds
        || values.first_response_elapsed_nanoseconds > values.attempt_elapsed_nanoseconds
        || values.cleanup_elapsed_nanoseconds > values.attempt_elapsed_nanoseconds
    {
        Err(Error::ObservationInconsistent)
    } else {
        Ok(())
    }
}

fn exceeded_limits(
    values: ResourceAttemptValues,
    limits: GenerationQualificationResourcePolicyLimitsV1,
) -> Vec<GenerationResourceExceededLimitV1> {
    let comparisons = [
        (
            values.attempt_elapsed_nanoseconds > limits.maximum_attempt_elapsed_nanoseconds,
            GenerationResourceExceededLimitV1::AttemptElapsed,
        ),
        (
            values.first_response_elapsed_nanoseconds > limits.maximum_first_response_nanoseconds,
            GenerationResourceExceededLimitV1::FirstResponse,
        ),
        (
            values.cleanup_elapsed_nanoseconds > limits.maximum_cleanup_nanoseconds,
            GenerationResourceExceededLimitV1::Cleanup,
        ),
        (
            values.worker_high_water_resident_bytes
                > limits.maximum_worker_high_water_resident_bytes,
            GenerationResourceExceededLimitV1::WorkerHighWaterResident,
        ),
        (
            values.installed_footprint_bytes > limits.maximum_installed_footprint_bytes,
            GenerationResourceExceededLimitV1::InstalledFootprint,
        ),
    ];
    comparisons
        .into_iter()
        .filter_map(|(exceeded, limit)| exceeded.then_some(limit))
        .collect()
}

fn duration_nanoseconds(duration: Duration) -> Result<u64, Error> {
    u64::try_from(duration.as_nanos()).map_err(|_error| Error::MeasurementOverflow)
}

fn installed_footprint(runtime_bytes: u64, model_bytes: u64) -> Result<u64, Error> {
    runtime_bytes
        .checked_add(model_bytes)
        .ok_or(Error::MeasurementOverflow)
}

fn ordinal(value: usize) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_error| Error::MeasurementOverflow)
}

fn snapshot_digest(
    observed: &VerifiedGenerationQualificationResourceAttemptObservation,
) -> rewrite_types::Digest {
    let mut material = b"retonr:generation-resource-attempt-observation-snapshot:v1\0".to_vec();
    for digest in [
        &observed.policy_digest,
        observed.operation_policy_id.digest(),
        observed.qualification_plan_id.digest(),
        observed.suite_manifest_id.digest(),
        observed.target_generation_system_id.digest(),
        &observed.request_binding_digest,
        observed.response_id.digest(),
        &observed.receipt_binding_digest,
        &observed.worker_evidence_digest,
        &observed.worker_observation_digest,
        observed.effective_package_id.digest(),
        observed.runtime_package_manifest_id.digest(),
        observed.model_package_manifest_id.digest(),
    ] {
        material.extend_from_slice(digest.as_str().as_bytes());
    }
    material.extend_from_slice(&observed.response_ordinal.to_be_bytes());
    for value in values_array(observed.values) {
        material.extend_from_slice(&value.to_be_bytes());
    }
    for limit in limits_array(observed.limits) {
        material.extend_from_slice(&limit.to_be_bytes());
    }
    material.extend_from_slice(
        &u64::try_from(observed.exceeded_limits.len())
            .expect("closed exceeded-limit count fits u64")
            .to_be_bytes(),
    );
    material.extend(observed.exceeded_limits.iter().map(|limit| match limit {
        GenerationResourceExceededLimitV1::AttemptElapsed => 0,
        GenerationResourceExceededLimitV1::FirstResponse => 1,
        GenerationResourceExceededLimitV1::Cleanup => 2,
        GenerationResourceExceededLimitV1::WorkerHighWaterResident => 3,
        GenerationResourceExceededLimitV1::InstalledFootprint => 4,
    }));
    rewrite_types::Digest::sha256(&material)
}

const fn values_array(values: ResourceAttemptValues) -> [u64; 13] {
    [
        values.prompt_token_count,
        values.generated_token_count,
        values.total_duration_nanoseconds,
        values.load_duration_nanoseconds,
        values.prompt_evaluation_duration_nanoseconds,
        values.evaluation_duration_nanoseconds,
        values.attempt_elapsed_nanoseconds,
        values.first_response_elapsed_nanoseconds,
        values.cleanup_elapsed_nanoseconds,
        values.worker_high_water_resident_bytes,
        values.runtime_installed_payload_bytes,
        values.model_installed_payload_bytes,
        values.installed_footprint_bytes,
    ]
}

const fn limits_array(limits: GenerationQualificationResourcePolicyLimitsV1) -> [u64; 5] {
    [
        limits.maximum_attempt_elapsed_nanoseconds,
        limits.maximum_first_response_nanoseconds,
        limits.maximum_cleanup_nanoseconds,
        limits.maximum_worker_high_water_resident_bytes,
        limits.maximum_installed_footprint_bytes,
    ]
}

fn check_cancelled(cancellation: &CancellationToken) -> Result<(), Error> {
    if cancellation.is_cancelled() {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
