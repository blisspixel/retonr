use rewrite_model::{
    ArtifactSetManifest, CandidateJudgeRequestAggregateV1, EffectivePackageEvidenceV2,
    GenerationSystemRecordV1Relations, RuntimeBuildIdentity, RuntimeBuildMode,
};
use rewrite_types::{CancellationToken, Digest};

use crate::effective_runtime_state_observation::{
    MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT, MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET,
    MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT, ManagedJudgeAttemptObserverBinding,
};

use super::{
    ManagedJudgeEffectivePackageDerivationError, ManagedJudgeEffectivePackagePlanInput,
    ManagedJudgeEffectivePackageRelationship,
};

pub(super) fn validate_input(
    input: &ManagedJudgeEffectivePackagePlanInput<'_, '_, '_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    validate_portable(input)?;
    validate_runtime(input)?;
    validate_model(input)?;
    validate_attempts(input, cancellation)?;
    input
        .observation_authority
        .revalidate_retained_bindings(cancellation)
        .map_err(|_error| relationship(ManagedJudgeEffectivePackageRelationship::EffectiveState))
}

fn validate_portable(
    input: &ManagedJudgeEffectivePackagePlanInput<'_, '_, '_, '_>,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    let plan = input.observation_authority.judge_plan();
    let schedule = input.observation_authority.judge_schedule();
    let system = &input.judge_system;
    if plan.judge_generation_system_id() != system.generation_system_id()
        || plan.prompt_contract_digest() != system.prompt_digest()
        || plan.output_schema_digest() != system.output_schema_digest()
        || input.request_aggregate.candidate_judge_plan_id() != plan.candidate_judge_plan_id()
        || input.request_aggregate.candidate_judge_schedule_id()
            != schedule.candidate_judge_schedule_id()
        || input.request_aggregate.entry_count() != schedule.entry_count()
    {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::PortableLineage,
        ));
    }
    let expected = CandidateJudgeRequestAggregateV1::new(
        plan,
        schedule,
        input
            .request_aggregate
            .structured_request_binding_ids()
            .to_vec(),
    )
    .map_err(|_error| relationship(ManagedJudgeEffectivePackageRelationship::PortableLineage))?;
    if expected != input.request_aggregate {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::PortableLineage,
        ));
    }
    Ok(())
}

fn validate_runtime(
    input: &ManagedJudgeEffectivePackagePlanInput<'_, '_, '_, '_>,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    let package_id = input.runtime_manifest.runtime_package_manifest_id();
    let expected_build = RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        input.runtime_manifest,
    )
    .map_err(|_error| relationship(ManagedJudgeEffectivePackageRelationship::Runtime))?;
    let runtime_generation = input
        .runtime_package
        .installation_key()
        .installation_generation();
    if expected_build != *input.runtime_build
        || input.runtime_build.package_manifest_digest() != package_id.digest()
        || input
            .runtime_package
            .evidence()
            .runtime_package_manifest_id()
            != &package_id
        || input.admitted_runtime.runtime_package_manifest_id() != &package_id
        || input.generation_path.runtime_package_manifest_id() != &package_id
        || input.frozen_components.runtime_package_manifest_id() != &package_id
        || !input.generation_path.matches_runtime(
            input.admitted_runtime,
            input.runtime_manifest,
            input.generation_path.runtime_version(),
            input.frozen_components.frozen_set_id(),
        )
        || runtime_generation == 0
        || runtime_generation != input.runtime_installation_generation
        || input.expected_runtime_state.runtime_build_id()
            != &input.runtime_build.runtime_build_id()
    {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::Runtime,
        ));
    }
    if input.prepared_isolation.policy_digest()
        != *input.expected_runtime_state.isolation_policy_digest()
        || input.managed_ollama.isolation_policy_digest()
            != input.expected_runtime_state.isolation_policy_digest()
        || !input
            .managed_ollama
            .binds_exact_runtime_package(input.runtime_package)
        || !input
            .managed_ollama
            .binds_exact_prepared_isolation(input.prepared_isolation)
    {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::Isolation,
        ));
    }
    Ok(())
}

fn validate_model(
    input: &ManagedJudgeEffectivePackagePlanInput<'_, '_, '_, '_>,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    let view = input.model_package.private_view();
    let managed = input.managed_ollama.input_evidence();
    let model_generation = view.installation_generation();
    let target = input.managed_ollama.model_target();
    if !input
        .managed_ollama
        .binds_exact_model_package(input.model_package)
        || managed.artifact_set_id() != &view.artifact_set_manifest().artifact_set_id()
        || managed.model_package_manifest_id()
            != &view.model_package_manifest().model_package_manifest_id()
        || managed.installation_generation() != model_generation
        || target.artifact_id() != input.judge_system.model_artifact_id()
        || input.static_model.artifact_set_id() != managed.artifact_set_id()
        || input.static_model.model_package_manifest_id() != managed.model_package_manifest_id()
        || input.static_model.model_artifact_id() != target.artifact_id()
        || input.judge_system.static_model_binding_digest() != input.static_model.binding_digest()
        || input.characterized_package.foundation_id() != input.model_package.foundation_id()
        || input.characterized_package.license_control_id()
            != input.managed_ollama.model_license_control_id()
        || model_generation == 0
        || model_generation != input.model_installation_generation
    {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::Model,
        ));
    }
    Ok(())
}

fn validate_attempts(
    input: &ManagedJudgeEffectivePackagePlanInput<'_, '_, '_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    let bindings = input.observation_authority.completed_sequence().bindings();
    let request_ids = input.request_aggregate.structured_request_binding_ids();
    if bindings.len() != request_ids.len()
        || u64::try_from(bindings.len()).ok() != Some(input.observation_authority.attempt_count())
    {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ));
    }
    let mut expected_first = u64::from(MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT) + 1;
    let mut retained_preflight: Option<Digest> = None;
    for (index, (binding, request_id)) in bindings.iter().zip(request_ids).enumerate() {
        ensure_active(cancellation)?;
        validate_attempt(
            input,
            binding,
            request_id.digest(),
            index,
            expected_first,
            &mut retained_preflight,
        )?;
        expected_first = expected_first
            .checked_add(u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT))
            .ok_or_else(|| relationship(ManagedJudgeEffectivePackageRelationship::Attempt))?;
    }
    Ok(())
}

fn validate_attempt(
    input: &ManagedJudgeEffectivePackagePlanInput<'_, '_, '_, '_>,
    binding: &ManagedJudgeAttemptObserverBinding,
    request_digest: &Digest,
    index: usize,
    expected_first: u64,
    retained_preflight: &mut Option<Digest>,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    let expected_cursor = u32::try_from(index)
        .map_err(|_error| relationship(ManagedJudgeEffectivePackageRelationship::Attempt))?;
    let expected_last = expected_first
        .checked_add(u64::from(MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT) - 1)
        .ok_or_else(|| relationship(ManagedJudgeEffectivePackageRelationship::Attempt))?;
    let expected_residency_first = expected_first
        .checked_add(u64::from(MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET))
        .ok_or_else(|| relationship(ManagedJudgeEffectivePackageRelationship::Attempt))?;
    let receipt = binding.receipt();
    let execution = receipt.execution();
    let execution_first = ordinal(execution.first_response_ordinal())?;
    let execution_last = ordinal(execution.last_response_ordinal())?;
    let residency_first = ordinal(receipt.first_residency_ordinal())?;
    let residency_last = ordinal(receipt.last_residency_ordinal())?;
    if binding.schedule_cursor() != expected_cursor
        || binding.request_binding_digest() != request_digest
        || execution.request_digest() != request_digest
        || binding.retained_response_id() != execution.retained_response_id()
        || binding.first_response_ordinal() != expected_first
        || binding.last_response_ordinal() != expected_last
        || execution_first != expected_first
        || execution_last != expected_last
        || residency_first != expected_residency_first
        || residency_last != expected_last
        || binding.complete_receipt_binding_digest() != &receipt.complete_binding_digest()
    {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::Attempt,
        ));
    }
    match retained_preflight {
        None => *retained_preflight = Some(binding.retained_preflight_digest().clone()),
        Some(expected) if expected == binding.retained_preflight_digest() => {}
        Some(_) => {
            return Err(relationship(
                ManagedJudgeEffectivePackageRelationship::Attempt,
            ));
        }
    }
    let state = binding.effective_runtime_state().ok_or_else(|| {
        relationship(ManagedJudgeEffectivePackageRelationship::RealEffectiveState)
    })?;
    if !binding.binds_effective_runtime_state(state)
        || state.state() != &input.expected_runtime_state
        || !state.binds_effective_package_inputs(
            input.admitted_runtime,
            input.generation_path,
            input.runtime_package,
            &input.managed_ollama,
        )
    {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::EffectiveState,
        ));
    }
    Ok(())
}

pub(super) fn validate_derived(
    input: &ManagedJudgeEffectivePackagePlanInput<'_, '_, '_, '_>,
    artifact_set: &ArtifactSetManifest,
    evidence: &EffectivePackageEvidenceV2,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    let view = input.model_package.private_view();
    let relations = GenerationSystemRecordV1Relations {
        runtime_package_manifest: input.runtime_manifest,
        runtime_build: input.runtime_build,
        effective_runtime_state: &input.expected_runtime_state,
        model_artifact_set: view.artifact_set_manifest(),
        model_package_manifest: view.model_package_manifest(),
        effective_package_evidence_v2: evidence,
    };
    if evidence != input.characterized_package.evidence()
        || evidence
            .validate_against(
                artifact_set,
                input.runtime_build,
                &input.expected_runtime_state,
            )
            .is_err()
        || input.judge_system.validate_against(relations).is_err()
        || input.admitted_runtime.runtime_admission_join_id()
            != *input.judge_system.runtime_admission_join_id()
        || input.generation_path.managed_generation_path_id()
            != *input.judge_system.managed_generation_path_id()
        || input.frozen_components.frozen_external_component_set_id()
            != *input.judge_system.frozen_external_component_set_id()
        || input.judge_system.effective_package_evidence_v2_id()
            != &evidence.effective_package_evidence_v2_id()
    {
        return Err(relationship(
            ManagedJudgeEffectivePackageRelationship::Evidence,
        ));
    }
    Ok(())
}

fn ordinal(value: usize) -> Result<u64, ManagedJudgeEffectivePackageDerivationError> {
    u64::try_from(value)
        .map_err(|_error| relationship(ManagedJudgeEffectivePackageRelationship::Attempt))
}

fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeEffectivePackageDerivationError> {
    if cancellation.is_cancelled() {
        Err(ManagedJudgeEffectivePackageDerivationError::Cancelled)
    } else {
        Ok(())
    }
}

const fn relationship(
    value: ManagedJudgeEffectivePackageRelationship,
) -> ManagedJudgeEffectivePackageDerivationError {
    ManagedJudgeEffectivePackageDerivationError::Relationship(value)
}
