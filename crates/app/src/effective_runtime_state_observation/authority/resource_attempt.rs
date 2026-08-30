use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{NativeLoadObservation, RuntimeBuildIdentity};
use rewrite_ollama::{OllamaResidentResourceObservedCompletion, OllamaRetainedSessionSubjectToken};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, ManagedGenerationWorkerEvidence,
    ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerNativeLoadEvidence,
    ManagedGenerationWorkerResourceObservation,
};
use rewrite_runtime_isolation::IsolationEvidence;
use rewrite_types::{CancellationToken, Digest};
use std::time::Instant;

use crate::package_attestation::{ModelPackageIdentityToken, RuntimePackageIdentityToken};
use crate::{
    ManagedOllamaIsolationLease, RuntimePackageLease, VerifiedAdmittedRuntime,
    VerifiedManagedGenerationPath, VerifiedManagedOllamaModelPackageLease,
};

use super::{
    EffectiveRuntimeStateObservationError, LinuxPlatformFrameworkEvidence,
    ManagedOllamaEffectiveRuntimeState, ManagedOllamaEffectiveRuntimeStateJoin,
    OllamaCpuExecutionEvidence, OllamaProviderSnapshotEvidence,
    OllamaWireOutputConfigurationEvidence, effective_join_deadline_precedence,
    join_managed_ollama_effective_runtime_state, join_managed_ollama_effective_runtime_state_until,
};

/// Exact live capabilities and observations for a resource-observed state join.
///
/// The complete Ollama composite is borrowed and remains inseparable. Its exact
/// retained-session subject is carried through cleanup for the later consuming
/// resource-attempt measurement boundary.
pub struct ManagedOllamaResourceEffectiveRuntimeStateJoin<'a, 'lease> {
    /// Package-declared runtime build.
    pub runtime_build: &'a RuntimeBuildIdentity,
    /// Exact all-pass runtime admission.
    pub admitted_runtime: &'a VerifiedAdmittedRuntime,
    /// Exact retained runtime installation.
    pub runtime_package_lease: &'a RuntimePackageLease,
    /// Exact reviewed generation path.
    pub generation_path: &'a VerifiedManagedGenerationPath,
    /// Exact retained managed isolation.
    pub managed_ollama: &'a ManagedOllamaIsolationLease<'lease>,
    /// Exact structured request.
    pub request: &'a StructuredCompletionRequest,
    /// Inseparable resource-observed completion.
    pub completion: &'a OllamaResidentResourceObservedCompletion,
    /// Launch-time isolation evidence.
    pub initial_isolation: &'a IsolationEvidence,
    /// Final equal isolation evidence.
    pub final_isolation: &'a IsolationEvidence,
    /// Initial retained worker evidence.
    pub initial_worker: &'a ManagedGenerationWorkerEvidence,
    /// Final equal worker evidence.
    pub final_worker: &'a ManagedGenerationWorkerEvidence,
    /// Exact worker native-load evidence.
    pub worker_native_load: &'a ManagedGenerationWorkerNativeLoadEvidence,
    /// Final managed server-process evidence.
    pub server_process: &'a AttachedProcessEvidence,
    /// Final server native-load observation.
    pub server_native_load: &'a NativeLoadObservation,
    /// Exact retained model mapping.
    pub model_mapping: &'a ManagedGenerationWorkerModelMappingEvidence,
    /// Generation-bound provider snapshot.
    pub provider_snapshot: &'a OllamaProviderSnapshotEvidence,
    /// Reviewed wire configuration.
    pub wire_configuration: &'a OllamaWireOutputConfigurationEvidence,
    /// Bounded platform evidence.
    pub platform: &'a LinuxPlatformFrameworkEvidence,
    /// Bounded CPU execution evidence.
    pub cpu_execution: &'a OllamaCpuExecutionEvidence,
}

/// Joins one exact resource-observed retained Ollama session to its live state.
///
/// Unlike the compatibility join, this boundary derives the receipt only from
/// the inseparable completion and retains its nonportable session subject.
///
/// # Errors
///
/// Returns [`EffectiveRuntimeStateObservationError`] when the completion is
/// inconsistent or any live relationship is substituted or stale.
pub fn join_managed_ollama_resource_effective_runtime_state(
    input: &ManagedOllamaResourceEffectiveRuntimeStateJoin<'_, '_>,
    cancellation: &CancellationToken,
) -> Result<ManagedOllamaEffectiveRuntimeState, EffectiveRuntimeStateObservationError> {
    join_managed_ollama_resource_effective_runtime_state_with_deadline(input, cancellation, None)
}

/// Joins an exact resource-observed retained session under an absolute deadline.
///
/// # Errors
///
/// Returns the same relationship errors as
/// [`join_managed_ollama_resource_effective_runtime_state`], including a live
/// deadline failure at or after `operation_deadline`.
pub fn join_managed_ollama_resource_effective_runtime_state_until(
    input: &ManagedOllamaResourceEffectiveRuntimeStateJoin<'_, '_>,
    cancellation: &CancellationToken,
    operation_deadline: Instant,
) -> Result<ManagedOllamaEffectiveRuntimeState, EffectiveRuntimeStateObservationError> {
    let result = join_managed_ollama_resource_effective_runtime_state_with_deadline(
        input,
        cancellation,
        Some(operation_deadline),
    );
    effective_join_deadline_precedence(result, cancellation, operation_deadline)
}

fn join_managed_ollama_resource_effective_runtime_state_with_deadline(
    input: &ManagedOllamaResourceEffectiveRuntimeStateJoin<'_, '_>,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<ManagedOllamaEffectiveRuntimeState, EffectiveRuntimeStateObservationError> {
    input
        .completion
        .verify_completion_binding()
        .map_err(|_error| EffectiveRuntimeStateObservationError::RelationshipMismatch)?;
    let join = ManagedOllamaEffectiveRuntimeStateJoin {
        runtime_build: input.runtime_build,
        admitted_runtime: input.admitted_runtime,
        runtime_package_lease: input.runtime_package_lease,
        generation_path: input.generation_path,
        managed_ollama: input.managed_ollama,
        request: input.request,
        receipt: input.completion.resident_execution_receipt(),
        initial_isolation: input.initial_isolation,
        final_isolation: input.final_isolation,
        initial_worker: input.initial_worker,
        final_worker: input.final_worker,
        worker_native_load: input.worker_native_load,
        server_process: input.server_process,
        server_native_load: input.server_native_load,
        model_mapping: input.model_mapping,
        provider_snapshot: input.provider_snapshot,
        wire_configuration: input.wire_configuration,
        platform: input.platform,
        cpu_execution: input.cpu_execution,
    };
    let mut observed = match operation_deadline {
        Some(deadline) => {
            join_managed_ollama_effective_runtime_state_until(&join, cancellation, deadline)
        }
        None => join_managed_ollama_effective_runtime_state(&join, cancellation),
    }?;
    observed.effective_package_subject.retained_session_subject =
        Some(input.completion.retained_session_subject_token());
    Ok(observed)
}

pub(crate) struct ManagedOllamaResourceAttemptSubject {
    request: Digest,
    response: Digest,
    receipt: Digest,
    preflight: Digest,
    first_response_ordinal: u64,
    last_response_ordinal: u64,
    first_residency_ordinal: u64,
    last_residency_ordinal: u64,
    initial_worker: Digest,
    final_worker: Digest,
    runtime_package_identity: RuntimePackageIdentityToken,
    model_package_identity: ModelPackageIdentityToken,
    retained_session_subject: OllamaRetainedSessionSubjectToken,
}

impl ManagedOllamaEffectiveRuntimeState {
    pub(crate) fn into_resource_attempt_subject(
        self,
    ) -> Option<ManagedOllamaResourceAttemptSubject> {
        let package = self.effective_package_subject;
        let judge = self.managed_judge_subject;
        let _closed_live_subject = package.live_subject?;
        Some(ManagedOllamaResourceAttemptSubject {
            request: judge.request,
            response: judge.response,
            receipt: judge.receipt,
            preflight: judge.preflight,
            first_response_ordinal: judge.first_response_ordinal,
            last_response_ordinal: judge.last_response_ordinal,
            first_residency_ordinal: judge.first_residency_ordinal,
            last_residency_ordinal: judge.last_residency_ordinal,
            initial_worker: judge.initial_worker,
            final_worker: judge.final_worker,
            runtime_package_identity: package.runtime_package_identity?,
            model_package_identity: package.model_package_identity?,
            retained_session_subject: package.retained_session_subject?,
        })
    }
}

impl ManagedOllamaResourceAttemptSubject {
    pub(crate) fn binds_completion(
        &self,
        completion: &OllamaResidentResourceObservedCompletion,
    ) -> bool {
        let response = completion.response();
        let receipt = completion.resident_execution_receipt();
        let execution = receipt.execution();
        completion.binds_retained_session_subject(&self.retained_session_subject)
            && self.request == *execution.request_digest()
            && self.response == *execution.response_digest()
            && self.receipt == receipt.complete_binding_digest()
            && self.preflight == *execution.preflight_digest()
            && usize::try_from(self.first_response_ordinal).ok()
                == Some(execution.first_response_ordinal())
            && usize::try_from(self.last_response_ordinal).ok()
                == Some(execution.last_response_ordinal())
            && usize::try_from(self.first_residency_ordinal).ok()
                == Some(receipt.first_residency_ordinal())
            && usize::try_from(self.last_residency_ordinal).ok()
                == Some(receipt.last_residency_ordinal())
            && execution.request_digest() == response.request_binding_digest()
    }

    pub(crate) fn binds_worker(
        &self,
        observation: &ManagedGenerationWorkerResourceObservation,
        evidence: &ManagedGenerationWorkerEvidence,
    ) -> bool {
        observation.worker_evidence_digest() == evidence.evidence_digest()
            && self.initial_worker == *evidence.evidence_digest()
            && self.final_worker == *evidence.evidence_digest()
    }

    pub(crate) fn binds_runtime_package(&self, package: &RuntimePackageLease) -> bool {
        package.binds_identity_token(&self.runtime_package_identity)
    }

    pub(crate) fn binds_model_package(
        &self,
        package: &VerifiedManagedOllamaModelPackageLease,
    ) -> bool {
        package.binds_identity_token(&self.model_package_identity)
    }

    pub(crate) fn retained_session_subject(&self) -> OllamaRetainedSessionSubjectToken {
        self.retained_session_subject.clone()
    }

    #[cfg(all(test, feature = "test-support"))]
    pub(crate) fn exact_test_fixture(
        completion: &OllamaResidentResourceObservedCompletion,
        evidence: &ManagedGenerationWorkerEvidence,
        runtime_package: &RuntimePackageLease,
        model_package: &VerifiedManagedOllamaModelPackageLease,
    ) -> Self {
        let receipt = completion.resident_execution_receipt();
        let execution = receipt.execution();
        Self {
            request: execution.request_digest().clone(),
            response: execution.response_digest().clone(),
            receipt: receipt.complete_binding_digest(),
            preflight: execution.preflight_digest().clone(),
            first_response_ordinal: u64::try_from(execution.first_response_ordinal())
                .expect("test response ordinal fits u64"),
            last_response_ordinal: u64::try_from(execution.last_response_ordinal())
                .expect("test response ordinal fits u64"),
            first_residency_ordinal: u64::try_from(receipt.first_residency_ordinal())
                .expect("test residency ordinal fits u64"),
            last_residency_ordinal: u64::try_from(receipt.last_residency_ordinal())
                .expect("test residency ordinal fits u64"),
            initial_worker: evidence.evidence_digest().clone(),
            final_worker: evidence.evidence_digest().clone(),
            runtime_package_identity: runtime_package.identity_token(),
            model_package_identity: model_package.identity_token(),
            retained_session_subject: completion.retained_session_subject_token(),
        }
    }
}
