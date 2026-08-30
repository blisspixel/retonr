use rewrite_app::effective_runtime_state_observation::{
    LinuxPlatformFrameworkEvidence, ManagedOllamaEffectiveRuntimeState,
    ManagedOllamaEffectiveRuntimeStateJoin, ManagedOllamaResourceEffectiveRuntimeStateJoin,
    OllamaCpuExecutionEvidence, OllamaProviderSnapshotEvidence,
    OllamaWireOutputConfigurationEvidence, join_managed_ollama_effective_runtime_state,
    join_managed_ollama_effective_runtime_state_until,
    join_managed_ollama_resource_effective_runtime_state,
    join_managed_ollama_resource_effective_runtime_state_until,
};
use rewrite_app::{
    ManagedOllamaIsolationLease, RuntimePackageLease, VerifiedAdmittedRuntime,
    VerifiedManagedGenerationPath,
};
use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{NativeLoadObservation, RuntimeBuildIdentity};
use rewrite_runtime_attestor::AttachedProcessEvidence;
use rewrite_runtime_isolation::IsolationEvidence;
use rewrite_types::CancellationToken;

use super::deadline::CandidateOperationDeadline;
use super::validation::FinalManagedGenerationWorkerEvidence;
use super::{LiveManagedGenerationCompletion, LocalOllamaManagedGenerationError};

pub(super) struct ManagedGenerationEffectiveStateInput<'a, 'lease> {
    pub(super) runtime_build: &'a RuntimeBuildIdentity,
    pub(super) admitted_runtime: &'a VerifiedAdmittedRuntime,
    pub(super) runtime_package_lease: &'a RuntimePackageLease,
    pub(super) generation_path: &'a VerifiedManagedGenerationPath,
    pub(super) managed_ollama: &'a ManagedOllamaIsolationLease<'lease>,
    pub(super) request: &'a StructuredCompletionRequest,
    pub(super) completion: &'a LiveManagedGenerationCompletion,
    pub(super) initial_isolation: &'a IsolationEvidence,
    pub(super) final_isolation: &'a IsolationEvidence,
    pub(super) worker: &'a FinalManagedGenerationWorkerEvidence,
    pub(super) server_process: &'a AttachedProcessEvidence,
    pub(super) server_native_load: &'a NativeLoadObservation,
    pub(super) provider_snapshot: &'a OllamaProviderSnapshotEvidence,
    pub(super) wire_configuration: &'a OllamaWireOutputConfigurationEvidence,
    pub(super) platform: &'a LinuxPlatformFrameworkEvidence,
    pub(super) cpu_execution: &'a OllamaCpuExecutionEvidence,
}

pub(super) fn join_managed_generation_effective_state(
    input: &ManagedGenerationEffectiveStateInput<'_, '_>,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<ManagedOllamaEffectiveRuntimeState, LocalOllamaManagedGenerationError> {
    operation_deadline.ensure_active(cancellation)?;
    let worker = input.worker;
    let observed = match input.completion {
        LiveManagedGenerationCompletion::Compatibility { receipt, .. } => {
            let join = ManagedOllamaEffectiveRuntimeStateJoin {
                runtime_build: input.runtime_build,
                admitted_runtime: input.admitted_runtime,
                runtime_package_lease: input.runtime_package_lease,
                generation_path: input.generation_path,
                managed_ollama: input.managed_ollama,
                request: input.request,
                receipt,
                initial_isolation: input.initial_isolation,
                final_isolation: input.final_isolation,
                initial_worker: &worker.initial,
                final_worker: &worker.final_evidence,
                worker_native_load: &worker.native_load,
                server_process: input.server_process,
                server_native_load: input.server_native_load,
                model_mapping: &worker.model_mapping,
                provider_snapshot: input.provider_snapshot,
                wire_configuration: input.wire_configuration,
                platform: input.platform,
                cpu_execution: input.cpu_execution,
            };
            match operation_deadline.instant() {
                Some(deadline) => {
                    join_managed_ollama_effective_runtime_state_until(&join, cancellation, deadline)
                }
                None => join_managed_ollama_effective_runtime_state(&join, cancellation),
            }
        }
        LiveManagedGenerationCompletion::ResourceObserved(completion) => {
            let join = ManagedOllamaResourceEffectiveRuntimeStateJoin {
                runtime_build: input.runtime_build,
                admitted_runtime: input.admitted_runtime,
                runtime_package_lease: input.runtime_package_lease,
                generation_path: input.generation_path,
                managed_ollama: input.managed_ollama,
                request: input.request,
                completion,
                initial_isolation: input.initial_isolation,
                final_isolation: input.final_isolation,
                initial_worker: &worker.initial,
                final_worker: &worker.final_evidence,
                worker_native_load: &worker.native_load,
                server_process: input.server_process,
                server_native_load: input.server_native_load,
                model_mapping: &worker.model_mapping,
                provider_snapshot: input.provider_snapshot,
                wire_configuration: input.wire_configuration,
                platform: input.platform,
                cpu_execution: input.cpu_execution,
            };
            match operation_deadline.instant() {
                Some(deadline) => join_managed_ollama_resource_effective_runtime_state_until(
                    &join,
                    cancellation,
                    deadline,
                ),
                None => join_managed_ollama_resource_effective_runtime_state(&join, cancellation),
            }
        }
    }
    .map_err(LocalOllamaManagedGenerationError::from);
    operation_deadline.precedence(observed, cancellation)
}
