//! Inert app-observed leaf evidence for a future effective runtime-state join.
//!
//! These records close individual observation relationships. They do not construct
//! [`rewrite_model::EffectiveRuntimeState`], grant generation or live-use authority,
//! prove model use, or qualify any runtime or model. A later lease-owning bracket
//! must cross-bind all leaves to the exact admission, generation path, runtime and
//! model leases, worker incarnation, request, response, and cleanup result.

use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::RuntimeBuildIdentity;
use rewrite_ollama::{OllamaModelBinding, OllamaResidentSessionExecutionReceipt};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerEvidence, ManagedGenerationWorkerModelMappingEvidence,
    ManagedGenerationWorkerNativeLoadEvidence,
};
use rewrite_runtime_isolation::IsolationEvidence;
use rewrite_types::CancellationToken;

use crate::ManagedOllamaInputEvidence;

mod authority;
mod contract;
mod managed_judge;
mod platform;
mod validation;

pub(crate) use authority::ManagedOllamaResourceAttemptSubject;
pub use authority::{
    MANAGED_OLLAMA_PROVIDER_SNAPSHOT_CONTRACT, ManagedOllamaEffectiveRuntimeState,
    ManagedOllamaEffectiveRuntimeStateJoin, ManagedOllamaResourceEffectiveRuntimeStateJoin,
    join_managed_ollama_effective_runtime_state, join_managed_ollama_effective_runtime_state_until,
    join_managed_ollama_resource_effective_runtime_state,
    join_managed_ollama_resource_effective_runtime_state_until,
};
pub use contract::{
    EFFECTIVE_RUNTIME_STATE_OBSERVATION_SCHEMA_VERSION, EffectiveRuntimeStateObservationError,
    LinuxPlatformFrameworkEvidence, OllamaCpuExecutionEvidence, OllamaProviderSnapshotEvidence,
    OllamaWireOutputConfigurationEvidence, PlatformDriverEvidenceClass,
};
pub use managed_judge::{
    CompletedManagedJudgeObservationSequence, MANAGED_JUDGE_ATTEMPT_RESPONSE_COUNT,
    MANAGED_JUDGE_FIRST_RESIDENCY_RESPONSE_OFFSET, MANAGED_JUDGE_PREFLIGHT_RESPONSE_COUNT,
    MANAGED_JUDGE_WORKER_RESPONSE_OFFSET, ManagedJudgeAttemptObservation,
    ManagedJudgeAttemptObserverBinding, ManagedJudgeObservationError,
    ManagedJudgeObservationSequence, ManagedJudgePreflightObservation,
    ManagedJudgePreflightObserverBinding,
};
#[cfg(test)]
pub(crate) use managed_judge::{
    CompletedSequenceMutation, completed_sequence_for_effective_package,
    completed_sequence_for_observation_authority,
    completed_sequence_with_unsealed_preflight_binding, mutate_completed_sequence,
};
use validation::{
    CpuExecutionSources, ProviderSources, build_cpu_execution, build_effective_configuration,
    build_provider_snapshot,
};

/// Derives a provider snapshot from one retained Ollama response bracket.
///
/// The receipt is constructed only by the retained-stream adapter. This function
/// joins it to the exact runtime build, model input mapping, model binding, and
/// structured request. The returned leaf remains inert.
///
/// # Errors
///
/// Returns [`EffectiveRuntimeStateObservationError`] for any cross-binding,
/// version, target, request, residency, or context mismatch.
pub fn observe_ollama_provider_snapshot(
    runtime_build: &RuntimeBuildIdentity,
    model_input: &ManagedOllamaInputEvidence,
    model: &OllamaModelBinding,
    request: &StructuredCompletionRequest,
    receipt: &OllamaResidentSessionExecutionReceipt,
) -> Result<OllamaProviderSnapshotEvidence, EffectiveRuntimeStateObservationError> {
    build_provider_snapshot(&ProviderSources {
        runtime_build,
        model_input,
        model,
        request,
        receipt,
    })
}

/// Derives the exact output-affecting Ollama wire configuration for one request.
///
/// The mapping contract is deliberately version-scoped to Ollama v0.32.15 and
/// commits to every explicit structured-request field plus the reviewed wire
/// constants. It does not include the closed launch profile or private model-root
/// mapping, so it is not a complete effective configuration. The returned leaf
/// remains inert.
///
/// # Errors
///
/// Returns [`EffectiveRuntimeStateObservationError`] unless the retained receipt,
/// request, runtime, and selected model bind exactly.
pub fn observe_ollama_wire_output_configuration(
    runtime_build: &RuntimeBuildIdentity,
    model_input: &ManagedOllamaInputEvidence,
    model: &OllamaModelBinding,
    request: &StructuredCompletionRequest,
    receipt: &OllamaResidentSessionExecutionReceipt,
) -> Result<OllamaWireOutputConfigurationEvidence, EffectiveRuntimeStateObservationError> {
    build_effective_configuration(&ProviderSources {
        runtime_build,
        model_input,
        model,
        request,
        receipt,
    })
}

/// Observes bounded Linux kernel identity and binds it to the runtime target and
/// separately observed worker native-code closure.
///
/// The CPU profile marks accelerator-driver evidence as not applicable. It does
/// not infer or claim that no driver exists on the host.
///
/// # Errors
///
/// Returns [`EffectiveRuntimeStateObservationError`] on unsupported platforms,
/// invalid runtime identity, cancellation, excessive kernel metadata, I/O failure,
/// or drift between two observations.
pub fn observe_linux_platform_framework(
    runtime_build: &RuntimeBuildIdentity,
    worker_native_load: &ManagedGenerationWorkerNativeLoadEvidence,
    cancellation: &CancellationToken,
) -> Result<LinuxPlatformFrameworkEvidence, EffectiveRuntimeStateObservationError> {
    platform::observe(runtime_build, worker_native_load, cancellation)
}

/// Derives bounded native-CPU execution evidence from the complete observed leaf set.
///
/// CPU-only requires the reviewed worker command profile, stable worker and
/// isolation observations, private CPU device canaries, an accelerator-free
/// worker native-load observation, exact GGUF mapping, and zero runtime-reported
/// accelerator bytes. This is bounded evidence, not formal placement proof.
///
/// # Errors
///
/// Returns [`EffectiveRuntimeStateObservationError`] if any relationship or
/// required CPU observation is absent, mismatched, or changed.
#[expect(
    clippy::too_many_arguments,
    reason = "each independently observed relationship remains explicit"
)]
pub fn observe_ollama_cpu_execution(
    model_input: &ManagedOllamaInputEvidence,
    request: &StructuredCompletionRequest,
    receipt: &OllamaResidentSessionExecutionReceipt,
    initial_isolation: &IsolationEvidence,
    final_isolation: &IsolationEvidence,
    initial_worker: &ManagedGenerationWorkerEvidence,
    final_worker: &ManagedGenerationWorkerEvidence,
    worker_native_load: &ManagedGenerationWorkerNativeLoadEvidence,
    model_mapping: &ManagedGenerationWorkerModelMappingEvidence,
) -> Result<OllamaCpuExecutionEvidence, EffectiveRuntimeStateObservationError> {
    build_cpu_execution(&CpuExecutionSources {
        model_input,
        request,
        receipt,
        initial_isolation,
        final_isolation,
        initial_worker,
        final_worker,
        worker_native_load,
        model_mapping,
    })
}

#[cfg(test)]
mod tests;
