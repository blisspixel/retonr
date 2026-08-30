use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{
    ArtifactSetId, EffectiveRuntimeState, EffectiveRuntimeStateId,
    ManagedOllamaEffectiveRuntimeStateJoinId, ModelPackageManifestId, NativeLoadObservation,
    RuntimeBuildIdentity,
};
use rewrite_ollama::{OllamaResidentSessionExecutionReceipt, OllamaRetainedSessionSubjectToken};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessLaunchMode,
    ManagedGenerationWorkerEvidence, ManagedGenerationWorkerModelMappingEvidence,
    ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerProfile,
};
use rewrite_runtime_isolation::{IsolationError, IsolationEvidence, ManagedRuntimeInputEvidence};
use rewrite_types::{CancellationToken, Digest};
use std::time::Instant;

use crate::package_attestation::{
    ManagedOllamaLiveSubjectToken, ModelPackageIdentityToken, RuntimePackageIdentityToken,
};
use crate::{
    ManagedOllamaIsolationLease, ManagedOllamaLaunchError, RuntimePackageLease,
    VerifiedAdmittedRuntime, VerifiedManagedGenerationPath,
};

use super::{
    EffectiveRuntimeStateObservationError, LinuxPlatformFrameworkEvidence,
    OllamaCpuExecutionEvidence, OllamaProviderSnapshotEvidence,
    OllamaWireOutputConfigurationEvidence,
};

mod managed_judge;
mod resource_attempt;
#[cfg(test)]
mod test_support;
mod validation;
pub(crate) use resource_attempt::ManagedOllamaResourceAttemptSubject;
pub use resource_attempt::{
    ManagedOllamaResourceEffectiveRuntimeStateJoin,
    join_managed_ollama_resource_effective_runtime_state,
    join_managed_ollama_resource_effective_runtime_state_until,
};
use validation::{
    BracketBindings, ExecutionBindings, LiveEffectiveStateFacts, ModelBindings, RuntimeBindings,
    build_effective_state,
};

/// Stable provider-snapshot contract used by the managed Ollama live join.
pub const MANAGED_OLLAMA_PROVIDER_SNAPSHOT_CONTRACT: &str = "ollama-retained-provider-snapshot";

/// Inert app-observed effective runtime state from one retained Ollama bracket.
///
/// Construction cross-validates opaque admission and launch capabilities with
/// all four generation-bound leaf observations. This wrapper grants no model-use,
/// handler-execution, formal-placement, qualification, or live-use authority.
#[derive(Debug)]
pub struct ManagedOllamaEffectiveRuntimeState {
    state: EffectiveRuntimeState,
    binding_digest: Digest,
    effective_package_subject: ManagedOllamaEffectivePackageSubject,
    managed_judge_subject: ManagedOllamaManagedJudgeSubject,
}

#[derive(Debug)]
pub(super) struct ManagedOllamaManagedJudgeSubject {
    pub(super) server_process: Digest,
    pub(super) server_native_load: Digest,
    pub(super) initial_worker: Digest,
    pub(super) final_worker: Digest,
    pub(super) worker_native_load: Digest,
    pub(super) model_mapping: Digest,
    pub(super) request: Digest,
    pub(super) response: Digest,
    pub(super) receipt: Digest,
    pub(super) preflight: Digest,
    pub(super) first_response_ordinal: u64,
    pub(super) last_response_ordinal: u64,
    pub(super) first_residency_ordinal: u64,
    pub(super) last_residency_ordinal: u64,
}

pub(super) struct ManagedOllamaManagedJudgeAttemptFacts<'a> {
    pub(super) server_process: &'a AttachedProcessEvidence,
    pub(super) server_native_load: &'a NativeLoadObservation,
    pub(super) initial_worker: &'a ManagedGenerationWorkerEvidence,
    pub(super) final_worker: &'a ManagedGenerationWorkerEvidence,
    pub(super) worker_native_load: &'a ManagedGenerationWorkerNativeLoadEvidence,
    pub(super) model_mapping: &'a ManagedGenerationWorkerModelMappingEvidence,
    pub(super) request: &'a Digest,
    pub(super) response: &'a Digest,
    pub(super) receipt: &'a Digest,
    pub(super) preflight: &'a Digest,
    pub(super) first_response_ordinal: u64,
    pub(super) last_response_ordinal: u64,
    pub(super) first_residency_ordinal: u64,
    pub(super) last_residency_ordinal: u64,
}

pub(super) struct ManagedOllamaEffectivePackageSubject {
    pub(super) admitted_runtime_id: Digest,
    pub(super) generation_path_id: Digest,
    pub(super) runtime_installation_generation: u64,
    pub(super) model_artifact_set_id: ArtifactSetId,
    pub(super) model_package_manifest_id: ModelPackageManifestId,
    pub(super) model_installation_generation: u64,
    pub(super) input_bound_launch_spec_digest: Digest,
    pub(super) isolation_policy_digest: Digest,
    live_subject: Option<ManagedOllamaLiveSubjectToken>,
    runtime_package_identity: Option<RuntimePackageIdentityToken>,
    model_package_identity: Option<ModelPackageIdentityToken>,
    retained_session_subject: Option<OllamaRetainedSessionSubjectToken>,
}

impl std::fmt::Debug for ManagedOllamaEffectivePackageSubject {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagedOllamaEffectivePackageSubject")
            .field("admitted_runtime_id", &self.admitted_runtime_id)
            .field("generation_path_id", &self.generation_path_id)
            .field(
                "runtime_installation_generation",
                &self.runtime_installation_generation,
            )
            .field("model_artifact_set_id", &self.model_artifact_set_id)
            .field("model_package_manifest_id", &self.model_package_manifest_id)
            .field(
                "model_installation_generation",
                &self.model_installation_generation,
            )
            .field("live_subject_retained", &self.live_subject.is_some())
            .field(
                "runtime_package_identity_retained",
                &self.runtime_package_identity.is_some(),
            )
            .field(
                "model_package_identity_retained",
                &self.model_package_identity.is_some(),
            )
            .field(
                "retained_session_subject_retained",
                &self.retained_session_subject.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl ManagedOllamaEffectiveRuntimeState {
    /// Returns the structurally validated effective runtime state.
    #[must_use]
    pub const fn state(&self) -> &EffectiveRuntimeState {
        &self.state
    }

    /// Returns the effective-state content identity.
    #[must_use]
    pub fn effective_runtime_state_id(&self) -> EffectiveRuntimeStateId {
        self.state.effective_runtime_state_id()
    }

    /// Returns the app-owned digest of the complete live relationship join.
    #[must_use]
    pub const fn binding_digest(&self) -> &Digest {
        &self.binding_digest
    }

    /// Returns the typed compatibility view of the complete live join identity.
    ///
    /// This wraps the existing app binding digest without hashing it again.
    #[must_use]
    pub fn effective_runtime_state_join_id(&self) -> ManagedOllamaEffectiveRuntimeStateJoinId {
        ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(self.binding_digest.clone())
    }

    /// Always false because an effective state is inert evidence, not qualification.
    #[must_use]
    pub const fn qualified(&self) -> bool {
        false
    }

    /// Always false because bounded observations are not formal placement proof.
    #[must_use]
    pub const fn formal_placement_proven(&self) -> bool {
        false
    }

    /// Always false because mapping and residency do not prove model use.
    #[must_use]
    pub const fn model_use_proven(&self) -> bool {
        false
    }

    /// Always false because process attribution does not identify a handler.
    #[must_use]
    pub const fn application_handler_proven(&self) -> bool {
        false
    }

    /// Consumes the wrapper after its owning workflow has completed cleanup.
    #[must_use]
    pub fn into_state(self) -> EffectiveRuntimeState {
        self.state
    }
}

/// Exact live capabilities and observations consumed by one effective-state join.
pub struct ManagedOllamaEffectiveRuntimeStateJoin<'a, 'lease> {
    /// Package-declared runtime build from the successful managed preflight.
    pub runtime_build: &'a RuntimeBuildIdentity,
    /// Opaque all-pass admission capability used for the exact launch.
    pub admitted_runtime: &'a VerifiedAdmittedRuntime,
    /// Exact retained runtime-package installation used for this attempt.
    pub runtime_package_lease: &'a RuntimePackageLease,
    /// Opaque reviewed generation-path capability used for the worker.
    pub generation_path: &'a VerifiedManagedGenerationPath,
    /// Retained closed launch, private input tree, and model-package lease.
    pub managed_ollama: &'a ManagedOllamaIsolationLease<'lease>,
    /// Exact structured request sent in this bracket.
    pub request: &'a StructuredCompletionRequest,
    /// Exact retained-session response and residency receipt.
    pub receipt: &'a OllamaResidentSessionExecutionReceipt,
    /// Launch-time isolation evidence.
    pub initial_isolation: &'a IsolationEvidence,
    /// Final equal isolation evidence observed after generation.
    pub final_isolation: &'a IsolationEvidence,
    /// Initial retained generation-worker evidence.
    pub initial_worker: &'a ManagedGenerationWorkerEvidence,
    /// Final equal retained generation-worker evidence.
    pub final_worker: &'a ManagedGenerationWorkerEvidence,
    /// Exact observed generation-worker native closure.
    pub worker_native_load: &'a ManagedGenerationWorkerNativeLoadEvidence,
    /// Final opaque managed server-process evidence for the native observation.
    pub server_process: &'a AttachedProcessEvidence,
    /// Final server-process native load observed inside this bracket.
    pub server_native_load: &'a NativeLoadObservation,
    /// Exact observed retained GGUF mapping.
    pub model_mapping: &'a ManagedGenerationWorkerModelMappingEvidence,
    /// Generation-bound provider snapshot leaf.
    pub provider_snapshot: &'a OllamaProviderSnapshotEvidence,
    /// Reviewed wire output-configuration leaf.
    pub wire_configuration: &'a OllamaWireOutputConfigurationEvidence,
    /// Bounded platform and native-framework leaf.
    pub platform: &'a LinuxPlatformFrameworkEvidence,
    /// Bounded CPU backend and placement leaf.
    pub cpu_execution: &'a OllamaCpuExecutionEvidence,
}

/// Cross-validates one live retained bracket and constructs its inert state.
///
/// This boundary never accepts [`rewrite_model::EffectiveRuntimeStateInput`].
/// Every state field is derived from opaque app-owned capabilities and observed
/// leaf records that name the same runtime, model, request, response, and bracket.
/// It is the compatibility join and does not retain an exact resource-observed
/// Ollama session subject. Use [`join_managed_ollama_resource_effective_runtime_state`]
/// when a later resource-attempt authority must prove exact session continuity.
///
/// # Errors
///
/// Returns [`EffectiveRuntimeStateObservationError`] for any substituted,
/// incomplete, unsupported, or structurally invalid relationship.
pub fn join_managed_ollama_effective_runtime_state(
    input: &ManagedOllamaEffectiveRuntimeStateJoin<'_, '_>,
    cancellation: &CancellationToken,
) -> Result<ManagedOllamaEffectiveRuntimeState, EffectiveRuntimeStateObservationError> {
    join_managed_ollama_effective_runtime_state_with_deadline(input, cancellation, None)
}

/// Cross-validates one live retained bracket under an already-captured absolute deadline.
///
/// # Errors
///
/// Returns the same relationship errors as
/// [`join_managed_ollama_effective_runtime_state`], including a live deadline
/// failure at or after `operation_deadline`.
pub fn join_managed_ollama_effective_runtime_state_until(
    input: &ManagedOllamaEffectiveRuntimeStateJoin<'_, '_>,
    cancellation: &CancellationToken,
    operation_deadline: Instant,
) -> Result<ManagedOllamaEffectiveRuntimeState, EffectiveRuntimeStateObservationError> {
    let result = join_managed_ollama_effective_runtime_state_with_deadline(
        input,
        cancellation,
        Some(operation_deadline),
    );
    effective_join_deadline_precedence(result, cancellation, operation_deadline)
}

pub(super) fn effective_join_deadline_precedence<T>(
    result: Result<T, EffectiveRuntimeStateObservationError>,
    cancellation: &CancellationToken,
    operation_deadline: Instant,
) -> Result<T, EffectiveRuntimeStateObservationError> {
    let isolation = if Instant::now() >= operation_deadline {
        Some(IsolationError::OperationDeadlineExceeded)
    } else if cancellation.is_cancelled() {
        Some(IsolationError::Cancelled)
    } else {
        None
    };
    if let Some(isolation) = isolation {
        Err(EffectiveRuntimeStateObservationError::LiveReobservation(
            ManagedOllamaLaunchError::Isolation(isolation),
        ))
    } else {
        result
    }
}

fn join_managed_ollama_effective_runtime_state_with_deadline(
    input: &ManagedOllamaEffectiveRuntimeStateJoin<'_, '_>,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<ManagedOllamaEffectiveRuntimeState, EffectiveRuntimeStateObservationError> {
    let joined_isolation = match operation_deadline {
        Some(deadline) => input.managed_ollama.reobserve_until(cancellation, deadline),
        None => input.managed_ollama.reobserve(cancellation),
    }
    .map_err(EffectiveRuntimeStateObservationError::LiveReobservation)?;
    if joined_isolation != *input.final_isolation {
        return Err(EffectiveRuntimeStateObservationError::RelationshipMismatch);
    }
    if !input
        .managed_ollama
        .binds_exact_runtime_package(input.runtime_package_lease)
    {
        return Err(EffectiveRuntimeStateObservationError::RelationshipMismatch);
    }
    let facts = build_live_facts(input, joined_isolation.runtime_inputs())?;
    let mut observed = build_effective_state(input.runtime_build, &facts)?;
    observed
        .retain_effective_package_live_subject(input.runtime_package_lease, input.managed_ollama);
    Ok(observed)
}

fn build_live_facts(
    input: &ManagedOllamaEffectiveRuntimeStateJoin<'_, '_>,
    runtime_inputs: &ManagedRuntimeInputEvidence,
) -> Result<LiveEffectiveStateFacts, EffectiveRuntimeStateObservationError> {
    Ok(LiveEffectiveStateFacts {
        runtime: build_runtime_bindings(input),
        model: build_model_bindings(input),
        bracket: build_bracket_bindings(input, runtime_inputs)?,
        execution: build_execution_bindings(input)?,
        leaf_schema_versions: [
            input.provider_snapshot.schema_version(),
            input.wire_configuration.schema_version(),
            input.platform.schema_version(),
            input.cpu_execution.schema_version(),
        ],
        provider_snapshot_digest: input.provider_snapshot.snapshot_digest().clone(),
        provider_observation_binding_digest: input
            .provider_snapshot
            .observation_binding_digest()
            .clone(),
        cpu_observation_binding_digest: input.cpu_execution.observation_binding_digest().clone(),
    })
}

fn build_runtime_bindings(
    input: &ManagedOllamaEffectiveRuntimeStateJoin<'_, '_>,
) -> RuntimeBindings {
    RuntimeBindings {
        runtime_build_id: input.runtime_build.runtime_build_id(),
        provider_runtime_build_id: input.provider_snapshot.runtime_build_id().clone(),
        wire_runtime_build_id: input.wire_configuration.runtime_build_id().clone(),
        platform_runtime_build_id: input.platform.runtime_build_id().clone(),
        runtime_package_digest: input.runtime_build.package_manifest_digest().clone(),
        runtime_installation_generation: input
            .runtime_package_lease
            .installation_key()
            .installation_generation(),
        lease_runtime_package_digest: input
            .runtime_package_lease
            .evidence()
            .runtime_package_manifest_id()
            .digest()
            .clone(),
        runtime_entrypoint_digest: input.runtime_build.entrypoint_digest().clone(),
        packaged_dependencies_digest: input.runtime_build.packaged_dependencies_digest().clone(),
        provider_runtime_package_digest: input
            .provider_snapshot
            .runtime_package_manifest_digest()
            .clone(),
        admitted_runtime_package_digest: input
            .admitted_runtime
            .runtime_package_manifest_id()
            .digest()
            .clone(),
        path_runtime_package_digest: input
            .generation_path
            .runtime_package_manifest_id()
            .digest()
            .clone(),
        admitted_frozen_component_set_digest: input
            .admitted_runtime
            .frozen_external_component_set_id()
            .digest()
            .clone(),
        path_frozen_component_set_digest: input
            .generation_path
            .frozen_external_component_set_id()
            .digest()
            .clone(),
        admitted_runtime_id: input
            .admitted_runtime
            .admitted_runtime_id()
            .digest()
            .clone(),
        path_admitted_runtime_id: input.generation_path.admitted_runtime_id().digest().clone(),
        generation_path_id: input.generation_path.generation_path_id().clone(),
        admitted_plain_launch_digest: input.admitted_runtime.startup_launch_spec_digest().clone(),
        retained_plain_launch_digest: input.managed_ollama.plain_launch_spec_digest().clone(),
    }
}

fn build_model_bindings(input: &ManagedOllamaEffectiveRuntimeStateJoin<'_, '_>) -> ModelBindings {
    let managed_input = input.managed_ollama.input_evidence();
    ModelBindings {
        input_artifact_set_id: managed_input.artifact_set_id().clone(),
        provider_artifact_set_id: input.provider_snapshot.model_artifact_set_id().clone(),
        input_model_package_id: managed_input.model_package_manifest_id().clone(),
        provider_model_package_id: input.provider_snapshot.model_package_manifest_id().clone(),
        input_installation_generation: managed_input.installation_generation(),
        provider_installation_generation: input.provider_snapshot.model_installation_generation(),
        input_model_artifact_id: managed_input.model_artifact_id().clone(),
        provider_model_artifact_id: input.provider_snapshot.model_artifact_id().clone(),
        wire_model_artifact_id: input.wire_configuration.model_artifact_id().clone(),
        cpu_model_artifact_id: input.cpu_execution.model_artifact_id().clone(),
        worker_model_artifact_id: input.initial_worker.model_artifact_id().clone(),
        mapping_model_artifact_id: input.model_mapping.model_artifact_id().clone(),
        model_target_artifact_id: input.managed_ollama.model_target().artifact_id().clone(),
        input_model_target_digest: managed_input.model_target_digest().clone(),
        retained_model_target_digest: input.managed_ollama.model_target().target_digest().clone(),
    }
}

fn build_bracket_bindings(
    input: &ManagedOllamaEffectiveRuntimeStateJoin<'_, '_>,
    runtime_inputs: &ManagedRuntimeInputEvidence,
) -> Result<BracketBindings, EffectiveRuntimeStateObservationError> {
    let execution = input.receipt.execution();
    let managed_input = input.managed_ollama.input_evidence();
    Ok(BracketBindings {
        request_digest: input.request.binding_digest(),
        receipt_request_digest: execution.request_digest().clone(),
        provider_request_digest: input.provider_snapshot.request_binding_digest().clone(),
        wire_request_digest: input.wire_configuration.request_binding_digest().clone(),
        receipt_response_digest: execution.response_digest().clone(),
        provider_response_digest: input.provider_snapshot.response_binding_digest().clone(),
        wire_response_digest: input.wire_configuration.response_binding_digest().clone(),
        receipt_complete_binding_digest: input.receipt.complete_binding_digest(),
        receipt_preflight_digest: execution.preflight_digest().clone(),
        receipt_first_response_ordinal: receipt_ordinal(execution.first_response_ordinal())?,
        receipt_last_response_ordinal: receipt_ordinal(execution.last_response_ordinal())?,
        receipt_first_residency_ordinal: receipt_ordinal(input.receipt.first_residency_ordinal())?,
        receipt_last_residency_ordinal: receipt_ordinal(input.receipt.last_residency_ordinal())?,
        request_context_tokens: input.request.context_token_limit,
        receipt_context_tokens: input.receipt.context_tokens(),
        wire_context_tokens: input.wire_configuration.effective_context_tokens(),
        cpu_context_tokens: input.cpu_execution.effective_context_tokens(),
        input_layout_digest: managed_input.input_layout_digest().clone(),
        isolation_layout_digest: runtime_inputs.layout_digest().clone(),
        input_member_count: managed_input.member_count(),
        isolation_member_count: runtime_inputs.member_count(),
        input_total_bytes: managed_input.total_bytes(),
        isolation_total_bytes: runtime_inputs.total_bytes(),
        retained_input_bound_launch_digest: input
            .managed_ollama
            .input_bound_launch_spec_digest()
            .clone(),
        observed_input_bound_launch_digest: runtime_inputs
            .input_bound_launch_digest(input.managed_ollama.plain_launch_spec_digest()),
        input_mapping_digest: managed_input.mapping_digest().clone(),
    })
}

fn receipt_ordinal(value: usize) -> Result<u64, EffectiveRuntimeStateObservationError> {
    u64::try_from(value)
        .map_err(|_error| EffectiveRuntimeStateObservationError::RelationshipMismatch)
}

fn build_execution_bindings(
    input: &ManagedOllamaEffectiveRuntimeStateJoin<'_, '_>,
) -> Result<ExecutionBindings, EffectiveRuntimeStateObservationError> {
    let server_native_component_count = u32::try_from(input.server_native_load.components().len())
        .map_err(|_error| EffectiveRuntimeStateObservationError::RelationshipMismatch)?;
    Ok(ExecutionBindings {
        initial_isolation_digest: input.initial_isolation.redacted_digest(),
        final_isolation_digest: input.final_isolation.redacted_digest(),
        cpu_isolation_digest: input.cpu_execution.isolation_evidence_digest.clone(),
        initial_worker_digest: input.initial_worker.evidence_digest().clone(),
        final_worker_digest: input.final_worker.evidence_digest().clone(),
        cpu_worker_digest: input.cpu_execution.worker_evidence_digest.clone(),
        reviewed_worker_artifact_id: input.generation_path.worker_artifact_id().clone(),
        initial_worker_artifact_id: input.initial_worker.worker_artifact_id().clone(),
        final_worker_artifact_id: input.final_worker.worker_artifact_id().clone(),
        initial_worker_runtime_package_digest: input
            .initial_worker
            .runtime_package_manifest_id()
            .digest()
            .clone(),
        final_worker_runtime_package_digest: input
            .final_worker
            .runtime_package_manifest_id()
            .digest()
            .clone(),
        worker_portable_configuration_digest: input
            .initial_worker
            .portable_configuration_digest()
            .clone(),
        final_worker_portable_configuration_digest: input
            .final_worker
            .portable_configuration_digest()
            .clone(),
        cpu_portable_configuration_digest: input
            .cpu_execution
            .worker_portable_configuration_digest()
            .clone(),
        worker_portable_closure_digest: input.worker_native_load.portable_closure_digest().clone(),
        platform_portable_closure_digest: input.platform.worker_portable_closure_digest().clone(),
        cpu_portable_closure_digest: input.cpu_execution.worker_portable_closure_digest().clone(),
        server_portable_component_set_digest: input
            .server_native_load
            .portable_component_set_digest(),
        server_native_load_observation_digest: input
            .server_native_load
            .native_load_observation_id()
            .digest()
            .clone(),
        server_process_evidence_digest: input.server_process.evidence_digest().clone(),
        server_native_process_evidence_digest: input
            .server_native_load
            .process_evidence_digest()
            .clone(),
        server_entrypoint_digest: input.server_process.entrypoint_digest().clone(),
        server_process_profile_valid: input.server_process.evidence_class()
            == AttachedProcessEvidenceClass::LinuxManagedNamespaceSockDiag
            && input.server_process.launch_mode()
                == AttachedProcessLaunchMode::ManagedLinuxIsolation,
        server_native_load_runtime_package_digest: input
            .server_native_load
            .runtime_package_manifest_id()
            .digest()
            .clone(),
        server_native_component_count,
        worker_native_load_digest: input.worker_native_load.observation_digest().clone(),
        platform_native_load_digest: input.platform.worker_native_load_digest().clone(),
        cpu_native_load_digest: input.cpu_execution.worker_native_load_digest.clone(),
        model_mapping_digest: input.model_mapping.observation_digest().clone(),
        cpu_model_mapping_digest: input.cpu_execution.model_mapping_digest.clone(),
        residency_digest: input.receipt.residency_observation_digest().clone(),
        cpu_residency_digest: input.cpu_execution.residency_observation_digest.clone(),
        wire_configuration_digest: input.wire_configuration.configuration_digest().clone(),
        platform_digest: input.platform.platform_digest().clone(),
        platform_kernel_observation_digest: input.platform.kernel_observation_digest().clone(),
        execution_class_digest: input.cpu_execution.execution_class_digest().clone(),
        isolation_policy_digest: input.managed_ollama.isolation_policy_digest().clone(),
        worker_profile_valid: input.initial_worker.profile()
            == ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
        worker_native_component_count: input.worker_native_load.component_count(),
        model_mapping_region_count: input.model_mapping.mapping_region_count(),
        runtime_reported_accelerator_bytes: input.receipt.accelerator_bytes(),
        isolation_canaries_valid: input.initial_isolation.preparation().all_canaries_passed()
            && input
                .initial_isolation
                .device_boundary()
                .all_visibility_canaries_passed(),
        compute_backend: input.cpu_execution.compute_backend(),
        placement: input.cpu_execution.placement(),
    })
}
