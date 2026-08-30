use rewrite_app::MANAGED_OLLAMA_V0_32_15_ENDPOINT;
use rewrite_model::{RuntimeBuildIdentity, RuntimeBuildMode};
use rewrite_ollama::OllamaEndpoint;
use rewrite_types::CancellationToken;

use crate::LocalOllamaPreflightMode;
use crate::local_ollama_bound_preflight::validate_bound_plan;
use crate::local_ollama_model_binding::validate_local_ollama_model_binding_evidence;
use crate::local_ollama_preflight::local_ollama_preflight_targets;

use super::super::super::validation::{exact_helper_member, validate_static_inputs};
use super::super::validation::{
    validate_generation_authority, validate_generation_model_binding,
    validate_managed_input_binding,
};
use super::context::{ManagedJudgeRunnerConfigurationInput, ManagedJudgeRunnerContext};
use super::{
    MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT, ManagedJudgeRunnerConfigurationError,
    ManagedJudgeRunnerConfigurationRelationship, ensure_not_cancelled, relationship,
};

mod kernel;

use kernel::{RelationshipValidationSubject, validate_relationships};

const CALLER_RELATIONSHIPS: [ManagedJudgeRunnerConfigurationRelationship; 3] = [
    ManagedJudgeRunnerConfigurationRelationship::PreflightProfile,
    ManagedJudgeRunnerConfigurationRelationship::WorkerLimits,
    ManagedJudgeRunnerConfigurationRelationship::Model,
];

const CONTEXT_RELATIONSHIPS: [ManagedJudgeRunnerConfigurationRelationship; 6] = [
    ManagedJudgeRunnerConfigurationRelationship::PortableJudge,
    ManagedJudgeRunnerConfigurationRelationship::Runtime,
    ManagedJudgeRunnerConfigurationRelationship::PreflightLimits,
    ManagedJudgeRunnerConfigurationRelationship::Isolation,
    ManagedJudgeRunnerConfigurationRelationship::Model,
    ManagedJudgeRunnerConfigurationRelationship::InstallationGeneration,
];

pub(super) fn validate_caller_configuration(
    input: &ManagedJudgeRunnerConfigurationInput,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    validate_relationships(
        &mut CallerConfigurationValidation { input },
        &CALLER_RELATIONSHIPS,
        cancellation,
    )
}

struct CallerConfigurationValidation<'a> {
    input: &'a ManagedJudgeRunnerConfigurationInput,
}

impl RelationshipValidationSubject for CallerConfigurationValidation<'_> {
    fn validate_relationship(
        &mut self,
        relationship: ManagedJudgeRunnerConfigurationRelationship,
        _cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeRunnerConfigurationError> {
        match relationship {
            ManagedJudgeRunnerConfigurationRelationship::PreflightProfile => {
                validate_preflight_profile(self.input)
            }
            ManagedJudgeRunnerConfigurationRelationship::WorkerLimits => {
                if self.input.worker_limits.validate().is_err() {
                    Err(super::relationship(relationship))
                } else {
                    Ok(())
                }
            }
            ManagedJudgeRunnerConfigurationRelationship::Model => {
                if validate_local_ollama_model_binding_evidence(&self.input.model_evidence) {
                    Ok(())
                } else {
                    Err(super::relationship(relationship))
                }
            }
            _ => Err(super::relationship(relationship)),
        }
    }
}

fn validate_preflight_profile(
    input: &ManagedJudgeRunnerConfigurationInput,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    validate_bound_plan(&input.preflight_plan).map_err(|_error| {
        relationship(ManagedJudgeRunnerConfigurationRelationship::PreflightProfile)
    })?;
    let endpoint =
        OllamaEndpoint::parse(&input.preflight_plan.preflight.endpoint).map_err(|_error| {
            relationship(ManagedJudgeRunnerConfigurationRelationship::PreflightProfile)
        })?;
    let response_count = local_ollama_preflight_targets(&input.preflight_plan.preflight)
        .map_err(|_error| {
            relationship(ManagedJudgeRunnerConfigurationRelationship::PreflightProfile)
        })?
        .len()
        .saturating_add(6);
    if input.preflight_plan.preflight.mode != LocalOllamaPreflightMode::Verify
        || !input.preflight_plan.preflight.require_idle
        || input.preflight_plan.preflight.models.len() != 1
        || endpoint.socket_addr() != MANAGED_OLLAMA_V0_32_15_ENDPOINT
        || response_count != MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT
    {
        return Err(relationship(
            ManagedJudgeRunnerConfigurationRelationship::PreflightProfile,
        ));
    }
    Ok(())
}

pub(super) fn validate_exact_context(
    context: &mut ManagedJudgeRunnerContext<'_, '_, '_, '_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    validate_relationships(
        &mut ExactContextValidation { context },
        &CONTEXT_RELATIONSHIPS,
        cancellation,
    )?;
    validate_retained_context_authorities(context, cancellation)
}

struct ExactContextValidation<'borrow, 'store, 'records, 'model, 'runtime, 'characterized> {
    context:
        &'borrow mut ManagedJudgeRunnerContext<'store, 'records, 'model, 'runtime, 'characterized>,
}

impl RelationshipValidationSubject for ExactContextValidation<'_, '_, '_, '_, '_, '_> {
    fn validate_relationship(
        &mut self,
        relationship: ManagedJudgeRunnerConfigurationRelationship,
        _cancellation: &CancellationToken,
    ) -> Result<(), ManagedJudgeRunnerConfigurationError> {
        match relationship {
            ManagedJudgeRunnerConfigurationRelationship::PortableJudge => {
                validate_portable_context(self.context)
            }
            ManagedJudgeRunnerConfigurationRelationship::Runtime => {
                validate_runtime_context(self.context)
            }
            ManagedJudgeRunnerConfigurationRelationship::PreflightLimits => validate_static_inputs(
                self.context.runtime_manifest,
                self.context.runtime_package,
                &self.context.preflight_plan,
                self.context.frozen_components.expected_components(),
                self.context.preflight_limits,
            )
            .map_err(|_error| super::relationship(relationship)),
            ManagedJudgeRunnerConfigurationRelationship::Isolation => {
                validate_isolation_context(self.context)
            }
            ManagedJudgeRunnerConfigurationRelationship::Model => {
                validate_model_context(self.context)
            }
            ManagedJudgeRunnerConfigurationRelationship::InstallationGeneration => {
                validate_installation_generations(self.context)
            }
            ManagedJudgeRunnerConfigurationRelationship::WorkerLimits
            | ManagedJudgeRunnerConfigurationRelationship::PreflightProfile => {
                Err(super::relationship(relationship))
            }
        }
    }
}

fn validate_retained_context_authorities(
    context: &mut ManagedJudgeRunnerContext<'_, '_, '_, '_, '_>,
    cancellation: &CancellationToken,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    if cancellation.is_cancelled() {
        return Err(
            ManagedJudgeRunnerConfigurationError::RetainedAuthorityValidation {
                eval: None,
                runtime: None,
                cancelled: true,
            },
        );
    }
    let eval = context.eval.revalidate(cancellation).err().map(Box::new);
    if cancellation.is_cancelled() {
        return Err(
            ManagedJudgeRunnerConfigurationError::RetainedAuthorityValidation {
                eval,
                runtime: None,
                cancelled: true,
            },
        );
    }
    let runtime = context
        .runtime_package
        .revalidate(cancellation)
        .err()
        .map(Box::new);
    let cancelled = cancellation.is_cancelled();
    if eval.is_some() || runtime.is_some() || cancelled {
        Err(
            ManagedJudgeRunnerConfigurationError::RetainedAuthorityValidation {
                eval,
                runtime,
                cancelled,
            },
        )
    } else {
        Ok(())
    }
}

fn validate_portable_context(
    context: &ManagedJudgeRunnerContext<'_, '_, '_, '_, '_>,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    if context.eval.judge_plan() != &context.app_judge_plan
        || context.eval.judge_schedule() != &context.app_judge_schedule
        || context.eval.request_aggregate() != &context.app_request_aggregate
        || context.eval.judge_system() != &context.app_judge_system
    {
        return Err(relationship(
            ManagedJudgeRunnerConfigurationRelationship::PortableJudge,
        ));
    }
    Ok(())
}

fn validate_runtime_context(
    context: &ManagedJudgeRunnerContext<'_, '_, '_, '_, '_>,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    let package_id = context.runtime_manifest.runtime_package_manifest_id();
    let expected_build = RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        context.runtime_manifest,
    )
    .map_err(|_error| relationship(ManagedJudgeRunnerConfigurationRelationship::Runtime))?;
    let runtime_matches = expected_build == *context.runtime_build
        && context
            .runtime_package
            .evidence()
            .runtime_package_manifest_id()
            == &package_id
        && context.runtime_build.package_manifest_digest() == package_id.digest()
        && context.expected_runtime_state.runtime_build_id()
            == &context.runtime_build.runtime_build_id()
        && context.prepared_isolation.policy_digest()
            == *context.expected_runtime_state.isolation_policy_digest()
        && context.admitted_runtime.runtime_package_manifest_id() == &package_id
        && context.generation_path.runtime_package_manifest_id() == &package_id
        && context.frozen_components.runtime_package_manifest_id() == &package_id
        && context.admitted_runtime.runtime_admission_join_id()
            == *context.app_judge_system.runtime_admission_join_id()
        && context.generation_path.managed_generation_path_id()
            == *context.app_judge_system.managed_generation_path_id()
        && context.frozen_components.frozen_external_component_set_id()
            == *context.app_judge_system.frozen_external_component_set_id()
        && context.app_judge_system.runtime_package_manifest_id() == &package_id
        && context.app_judge_system.runtime_build_id() == &context.runtime_build.runtime_build_id()
        && context.app_judge_system.effective_runtime_state_id()
            == &context.expected_runtime_state.effective_runtime_state_id()
        && validate_generation_authority(
            context.runtime_manifest,
            &context.preflight_plan,
            context.admitted_runtime,
            context.generation_path,
            context.frozen_components,
        )
        .is_ok();
    if !runtime_matches {
        return Err(relationship(
            ManagedJudgeRunnerConfigurationRelationship::Runtime,
        ));
    }
    Ok(())
}

fn validate_isolation_context(
    context: &ManagedJudgeRunnerContext<'_, '_, '_, '_, '_>,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    let preparation = context.prepared_isolation.preparation_evidence();
    if !preparation.all_canaries_passed()
        || exact_helper_member(
            context.runtime_manifest,
            preparation.helper_digest(),
            preparation.helper_bytes(),
        )
        .is_err()
    {
        return Err(relationship(
            ManagedJudgeRunnerConfigurationRelationship::Isolation,
        ));
    }
    Ok(())
}

fn validate_model_context(
    context: &ManagedJudgeRunnerContext<'_, '_, '_, '_, '_>,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    let launch_evidence = context.launch_plan.input_evidence();
    let model_matches = context
        .launch_plan
        .binds_exact_model_package(context.model_package)
        && context
            .app_static_model
            .validate_against(&context.launch_plan)
            .is_ok()
        && context.app_judge_system.model_artifact_id()
            == context.launch_plan.model_target().artifact_id()
        && context.app_judge_system.model_artifact_id() == context.model.artifact_id()
        && context.app_judge_system.static_model_binding_digest()
            == context.app_static_model.binding_digest()
        && context.app_judge_system.model_artifact_set_id()
            == context.app_static_model.artifact_set_id()
        && context.app_judge_system.model_package_manifest_id()
            == context.app_static_model.model_package_manifest_id()
        && context.app_judge_system.effective_package_evidence_v2_id()
            == &context
                .characterized_package
                .evidence()
                .effective_package_evidence_v2_id()
        && context.characterized_package.foundation_id() == context.launch_plan.foundation_id()
        && context.characterized_package.license_control_id()
            == context.launch_plan.model_license_control_id()
        && context.model_evidence.artifact_set_id == *context.app_static_model.artifact_set_id()
        && context.model_evidence.model_package_manifest_id
            == *context.app_static_model.model_package_manifest_id()
        && context.model_evidence.model_artifact_id
            == *context.app_static_model.model_artifact_id()
        && launch_evidence.artifact_set_id() == &context.model_evidence.artifact_set_id
        && launch_evidence.model_package_manifest_id()
            == &context.model_evidence.model_package_manifest_id
        && validate_generation_model_binding(
            context.runtime_manifest,
            &context.preflight_plan,
            &context.model_evidence,
            &context.model,
        )
        .is_ok()
        && validate_managed_input_binding(
            &context.launch_plan,
            &context.model_evidence,
            &context.model,
            &context.preflight_plan,
        )
        .is_ok();
    if !model_matches {
        return Err(relationship(
            ManagedJudgeRunnerConfigurationRelationship::Model,
        ));
    }
    Ok(())
}

fn validate_installation_generations(
    context: &ManagedJudgeRunnerContext<'_, '_, '_, '_, '_>,
) -> Result<(), ManagedJudgeRunnerConfigurationError> {
    let runtime_generation = context
        .runtime_package
        .installation_key()
        .installation_generation();
    let model_generation = context
        .launch_plan
        .input_evidence()
        .installation_generation();
    if runtime_generation == 0
        || model_generation == 0
        || context.runtime_installation_generation != runtime_generation
        || context.model_installation_generation != model_generation
        || context.model_evidence.artifact_set_installation_generation != model_generation
    {
        return Err(relationship(
            ManagedJudgeRunnerConfigurationRelationship::InstallationGeneration,
        ));
    }
    Ok(())
}
