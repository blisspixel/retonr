use std::{cell::RefCell, rc::Rc};

use rewrite_app::effective_runtime_state_observation::{
    observe_linux_platform_framework, observe_ollama_cpu_execution,
    observe_ollama_provider_snapshot, observe_ollama_wire_output_configuration,
};
use rewrite_app::{
    ManagedOllamaIsolationLease, RuntimePackageLease, VerifiedAdmittedRuntime,
    VerifiedManagedGenerationPath,
};
use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::RuntimePackageManifest;
use rewrite_ollama::OllamaModelBinding;
use rewrite_runtime_attestor::{
    RetainedNativePackageMember, VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_runtime_isolation::IsolationEvidence;
use rewrite_types::CancellationToken;

use super::deadline::CandidateOperationDeadline;
use super::effective_state_join::{
    ManagedGenerationEffectiveStateInput, join_managed_generation_effective_state,
};
use super::evidence::{
    GenerationBracketObservationInput, GenerationEvidenceInput,
    build_generation_bracket_observation, build_generation_evidence,
};
use super::validation::{
    ManagedSessionObserver, observe_native_load, observe_resource_and_reobserve_worker,
    reobserve_process, reobserve_worker,
};
use super::{
    LiveManagedGenerationCompletion, LocalOllamaManagedGenerationError,
    PendingLocalOllamaManagedGenerationOutcome, PendingManagedGenerationCompletion,
    PendingResourceObservedGenerationCompletion,
};
use crate::{
    LocalOllamaManagedBuildBinding, LocalOllamaManagedPreflightError,
    LocalOllamaManagedPreflightLimits, LocalOllamaManagedPreflightReport,
    LocalOllamaModelBindingEvidence,
};

pub(super) struct FinalObservationInput<'a, 'lease> {
    pub(super) package: &'a RuntimePackageManifest,
    pub(super) package_lease: &'a mut RuntimePackageLease,
    pub(super) admitted_runtime: &'a VerifiedAdmittedRuntime,
    pub(super) generation_path: &'a VerifiedManagedGenerationPath,
    pub(super) frozen_components: &'a VerifiedFrozenExternalNativeComponentSet,
    pub(super) limits: LocalOllamaManagedPreflightLimits,
    pub(super) static_model: &'a LocalOllamaModelBindingEvidence,
    pub(super) model: &'a OllamaModelBinding,
    pub(super) isolation_lease: &'a ManagedOllamaIsolationLease<'lease>,
    pub(super) observer: &'a Rc<RefCell<ManagedSessionObserver>>,
    pub(super) initial_isolation: &'a IsolationEvidence,
    pub(super) retained_members: &'a [RetainedNativePackageMember],
    pub(super) retained_request: StructuredCompletionRequest,
    pub(super) managed_preflight: LocalOllamaManagedPreflightReport,
    pub(super) managed_build: LocalOllamaManagedBuildBinding,
    pub(super) completion: LiveManagedGenerationCompletion,
    pub(super) operation_deadline: CandidateOperationDeadline,
    pub(super) cancellation: &'a CancellationToken,
}

#[expect(
    clippy::too_many_lines,
    reason = "the final live observation and evidence order remains one auditable deadline bracket"
)]
pub(super) fn finish_final_observation(
    input: FinalObservationInput<'_, '_>,
) -> Result<PendingLocalOllamaManagedGenerationOutcome, LocalOllamaManagedGenerationError> {
    let FinalObservationInput {
        package,
        package_lease,
        admitted_runtime,
        generation_path,
        frozen_components,
        limits,
        static_model,
        model,
        isolation_lease,
        observer,
        initial_isolation,
        retained_members,
        retained_request,
        managed_preflight,
        managed_build,
        completion,
        operation_deadline,
        cancellation,
    } = input;
    macro_rules! step {
        ($result:expr) => {{
            operation_deadline.ensure_active(cancellation)?;
            let result = $result;
            operation_deadline.precedence(result, cancellation)?
        }};
    }

    operation_deadline.ensure_active(cancellation)?;
    let residency_receipt = completion.receipt();
    operation_deadline.ensure_active(cancellation)?;
    step!(
        package_lease
            .revalidate(cancellation)
            .map_err(LocalOllamaManagedPreflightError::Package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let model_result = match operation_deadline.instant() {
        Some(deadline) => isolation_lease.revalidate_model_package_until(cancellation, deadline),
        None => isolation_lease.revalidate_model_package(cancellation),
    }
    .map_err(LocalOllamaManagedGenerationError::from);
    step!(model_result);
    let worker = reobserve_worker(observer, operation_deadline, cancellation)?;
    let post_generation_process =
        reobserve_process(observer, package, operation_deadline, cancellation)?;
    let post_generation_native_load = observe_native_load(
        observer,
        package,
        retained_members,
        frozen_components,
        limits,
        operation_deadline,
        cancellation,
    )?;
    let final_process = reobserve_process(observer, package, operation_deadline, cancellation)?;
    step!(
        (final_process == post_generation_process)
            .then_some(())
            .ok_or(LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let isolation_result = match operation_deadline.instant() {
        Some(deadline) => isolation_lease.reobserve_until(cancellation, deadline),
        None => isolation_lease.reobserve(cancellation),
    }
    .map_err(LocalOllamaManagedGenerationError::from);
    let final_isolation = step!(isolation_result);
    step!(
        super::super::validation::validate_final_isolation(initial_isolation, &final_isolation)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    step!(
        package_lease
            .revalidate(cancellation)
            .map_err(LocalOllamaManagedPreflightError::Package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let connection_observations = step!((|| {
        let state = observer
            .try_borrow()
            .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)
            .map_err(LocalOllamaManagedGenerationError::from)?;
        state
            .connections
            .validate_complete()
            .map_err(LocalOllamaManagedPreflightError::BoundObservation)
            .map_err(LocalOllamaManagedGenerationError::from)?;
        Ok(state.connections.evidence().to_vec())
    })());
    let runtime_build = managed_build.runtime_build();
    let managed_input = isolation_lease.input_evidence();
    let provider_snapshot = step!(
        observe_ollama_provider_snapshot(
            runtime_build,
            managed_input,
            model,
            &retained_request,
            residency_receipt,
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    let wire_configuration = step!(
        observe_ollama_wire_output_configuration(
            runtime_build,
            managed_input,
            model,
            &retained_request,
            residency_receipt,
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    let platform = step!(
        observe_linux_platform_framework(runtime_build, &worker.native_load, cancellation)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let cpu_execution = step!(
        observe_ollama_cpu_execution(
            managed_input,
            &retained_request,
            residency_receipt,
            initial_isolation,
            &final_isolation,
            &worker.initial,
            &worker.final_evidence,
            &worker.native_load,
            &worker.model_mapping,
        )
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    step!(
        package_lease
            .revalidate(cancellation)
            .map_err(LocalOllamaManagedPreflightError::Package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let effective_runtime_state = join_managed_generation_effective_state(
        &ManagedGenerationEffectiveStateInput {
            runtime_build,
            admitted_runtime,
            runtime_package_lease: package_lease,
            generation_path,
            managed_ollama: isolation_lease,
            request: &retained_request,
            completion: &completion,
            initial_isolation,
            final_isolation: &final_isolation,
            worker: &worker,
            server_process: &final_process,
            server_native_load: &post_generation_native_load,
            provider_snapshot: &provider_snapshot,
            wire_configuration: &wire_configuration,
            platform: &platform,
            cpu_execution: &cpu_execution,
        },
        operation_deadline,
        cancellation,
    )?;
    step!(
        package_lease
            .revalidate(cancellation)
            .map_err(LocalOllamaManagedPreflightError::Package)
            .map_err(LocalOllamaManagedGenerationError::from)
    );
    let evidence = step!(
        build_generation_evidence(&GenerationEvidenceInput {
            managed_report: &managed_preflight,
            build_binding: &managed_build,
            static_model,
            model,
            request: &retained_request,
            receipt: residency_receipt,
            post_generation_process: &final_process,
            post_generation_native_load: &post_generation_native_load,
            final_isolation: &final_isolation,
            connection_observations: &connection_observations,
        })
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    let bracket_observation = step!(
        build_generation_bracket_observation(&GenerationBracketObservationInput {
            managed_report: &managed_preflight,
            build_binding: &managed_build,
            admitted_runtime,
            generation_path,
            runtime_package_lease: package_lease,
            static_model,
            model,
            request: &retained_request,
            receipt: residency_receipt,
            post_generation_process: &final_process,
            post_generation_native_load: &post_generation_native_load,
            worker: &worker,
            managed_input: isolation_lease.input_evidence(),
            input_bound_launch_spec_digest: isolation_lease.input_bound_launch_spec_digest(),
            final_isolation: &final_isolation,
            connection_observations: &connection_observations,
            effective_runtime_state: &effective_runtime_state,
        })
        .map_err(LocalOllamaManagedGenerationError::from)
    );
    let completion = match completion {
        LiveManagedGenerationCompletion::Compatibility { response, receipt } => {
            PendingManagedGenerationCompletion::Compatibility {
                response,
                residency_receipt: receipt,
            }
        }
        LiveManagedGenerationCompletion::ResourceObserved(completion) => {
            let (worker_observation, worker_evidence) =
                observe_resource_and_reobserve_worker(observer, operation_deadline, cancellation)?;
            PendingManagedGenerationCompletion::ResourceObserved(Box::new(
                PendingResourceObservedGenerationCompletion {
                    completion,
                    worker_observation,
                    worker_evidence,
                },
            ))
        }
    };
    operation_deadline.precedence(
        Ok(PendingLocalOllamaManagedGenerationOutcome {
            completion,
            managed_preflight,
            managed_build,
            evidence,
            bracket_observation,
            effective_runtime_state,
        }),
        cancellation,
    )
}
