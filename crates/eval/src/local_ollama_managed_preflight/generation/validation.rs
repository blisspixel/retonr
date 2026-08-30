use std::{cell::RefCell, rc::Rc};

use rewrite_app::{
    MANAGED_OLLAMA_V0_32_15_ENDPOINT, ManagedOllamaIsolationLease, VerifiedAdmittedRuntime,
    VerifiedManagedGenerationPath, VerifiedManagedOllamaLaunchPlan,
};
use rewrite_inference::StructuredCompletionRequest;
use rewrite_model::{NativeLoadObservation, RuntimePackageManifest, RuntimePackageManifestId};
use rewrite_ollama::{
    OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES, OllamaModelBinding, OllamaObservedSessionError,
    OllamaResponseObservation, OllamaResponseObservationPhase, OllamaVersion,
};
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessLease, ManagedGenerationWorkerError,
    ManagedGenerationWorkerEvidence, ManagedGenerationWorkerLimits,
    ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerNativeLoadEvidence,
    ManagedGenerationWorkerNativeLoadRequest, ManagedGenerationWorkerObservationRequest,
    ManagedGenerationWorkerProfile, ManagedGenerationWorkerResourceObservation,
    NativeLoadObservationRequest, NativeManagedGenerationWorkerLease,
    NativeManagedLinuxProcessLease, RetainedNativePackageMember,
    VerifiedFrozenExternalNativeComponentSet,
};
use rewrite_types::{CancellationToken, Digest};

use crate::{
    LocalOllamaBoundPreflightError, LocalOllamaBoundPreflightPlan, LocalOllamaModelBindingEvidence,
    local_ollama_bound_preflight::ConnectionObservationSequence,
    local_ollama_model_binding::{
        LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION, validate_local_ollama_model_binding_evidence,
    },
};

use super::super::{
    LocalOllamaManagedPreflightError, LocalOllamaManagedPreflightLimits,
    validation::validate_process_binding,
};
use super::LocalOllamaManagedGenerationError;
use super::deadline::CandidateOperationDeadline;

pub(super) struct ManagedSessionObserver {
    pub(super) process: NativeManagedLinuxProcessLease,
    pub(super) connections: ConnectionObservationSequence,
    pub(super) worker: Option<ManagedGenerationWorkerObservation>,
}

pub(super) struct ManagedGenerationWorkerObservation {
    lease: NativeManagedGenerationWorkerLease,
    pub(super) initial: ManagedGenerationWorkerEvidence,
    pub(super) native_load: ManagedGenerationWorkerNativeLoadEvidence,
    pub(super) model_mapping: ManagedGenerationWorkerModelMappingEvidence,
}

pub(super) struct FinalManagedGenerationWorkerEvidence {
    pub(super) initial: ManagedGenerationWorkerEvidence,
    pub(super) final_evidence: ManagedGenerationWorkerEvidence,
    pub(super) native_load: ManagedGenerationWorkerNativeLoadEvidence,
    pub(super) model_mapping: ManagedGenerationWorkerModelMappingEvidence,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ManagedGenerationSessionObservationError {
    #[error(transparent)]
    Connection(#[from] LocalOllamaBoundPreflightError),
    #[error(transparent)]
    Worker(#[from] ManagedGenerationWorkerError),
    #[error(transparent)]
    Gate(LocalOllamaManagedGenerationError),
}

pub(super) fn validate_generation_authority(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
    admitted_runtime: &VerifiedAdmittedRuntime,
    generation_path: &VerifiedManagedGenerationPath,
    frozen_external_components: &VerifiedFrozenExternalNativeComponentSet,
) -> Result<(), LocalOllamaManagedGenerationError> {
    let runtime_version = plan
        .preflight
        .expected_runtime_version
        .parse::<OllamaVersion>()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let package_id = package.runtime_package_manifest_id();
    let valid = generation_path.matches_runtime(
        admitted_runtime,
        package,
        runtime_version,
        frozen_external_components.frozen_set_id(),
    ) && frozen_external_components.runtime_package_manifest_id() == &package_id;
    if !valid {
        return Err(LocalOllamaManagedGenerationError::InvalidGenerationAuthority);
    }
    Ok(())
}

pub(super) fn validate_generation_binding(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
    request: &StructuredCompletionRequest,
) -> Result<(), LocalOllamaManagedPreflightError> {
    validate_generation_model_binding(package, plan, static_model, model)?;
    let valid = request.artifact_id == *model.artifact_id()
        && request.artifact_digest == *model.artifact_digest()
        && u64::try_from(request.input.len()).unwrap_or(u64::MAX)
            <= u64::from(OLLAMA_RETAINED_SESSION_MAX_INPUT_BYTES)
        && request.validate().is_ok();
    if !valid {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn validate_generation_model_binding(
    package: &RuntimePackageManifest,
    plan: &LocalOllamaBoundPreflightPlan,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
) -> Result<(), LocalOllamaManagedPreflightError> {
    let plan_digest = serde_json::to_vec(&plan.preflight)
        .map(|bytes| Digest::sha256(&bytes))
        .map_err(|_error| LocalOllamaManagedPreflightError::ReportEncoding)?;
    let Some(planned_model) = plan.preflight.models.first() else {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    };
    let valid = package.reported_version() == LOCAL_OLLAMA_MODEL_BINDING_RUNTIME_VERSION
        && plan.preflight.models.len() == 1
        && validate_local_ollama_model_binding_evidence(static_model)
        && static_model.preflight_plan_digest == plan_digest
        && model.reference() == planned_model.reference
        && model.inventory_digest() == &planned_model.inventory_digest
        && static_model.runtime_reference_digest == Digest::sha256(model.reference().as_bytes())
        && static_model.inventory_digest == *model.inventory_digest()
        && static_model.model_artifact_id == *model.artifact_id()
        && model.artifact_digest() == static_model.model_artifact_id.digest();
    if !valid {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn validate_managed_input_binding(
    input: &VerifiedManagedOllamaLaunchPlan<'_>,
    static_model: &LocalOllamaModelBindingEvidence,
    model: &OllamaModelBinding,
    plan: &LocalOllamaBoundPreflightPlan,
) -> Result<(), LocalOllamaManagedPreflightError> {
    let endpoint = rewrite_ollama::OllamaEndpoint::parse(&plan.preflight.endpoint)
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidInput)?;
    let evidence = input.input_evidence();
    let target = input.model_target();
    if endpoint.socket_addr() != MANAGED_OLLAMA_V0_32_15_ENDPOINT
        || evidence.model_package_manifest_id() != &static_model.model_package_manifest_id
        || evidence.artifact_set_id() != &static_model.artifact_set_id
        || evidence.installation_generation() != static_model.artifact_set_installation_generation
        || evidence.runtime_reference_digest() != &static_model.runtime_reference_digest
        || evidence.model_artifact_id() != &static_model.model_artifact_id
        || evidence.model_artifact_id() != model.artifact_id()
        || target.artifact_id() != evidence.model_artifact_id()
        || target.target_digest() != evidence.model_target_digest()
        || input.model_byte_size() != static_model.model_byte_size
    {
        return Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding);
    }
    Ok(())
}

pub(super) fn exact_retained_worker<'a>(
    retained: &'a [RetainedNativePackageMember],
    generation_path: &VerifiedManagedGenerationPath,
) -> Result<&'a RetainedNativePackageMember, LocalOllamaManagedGenerationError> {
    let mut matches = retained
        .iter()
        .filter(|member| member.artifact_id() == generation_path.worker_artifact_id());
    let worker = matches
        .next()
        .ok_or(LocalOllamaManagedGenerationError::InvalidGenerationAuthority)?;
    if matches.next().is_some() {
        return Err(LocalOllamaManagedGenerationError::InvalidGenerationAuthority);
    }
    Ok(worker)
}

fn validate_worker_relationships(
    initial: &ManagedGenerationWorkerEvidence,
    model_mapping: &ManagedGenerationWorkerModelMappingEvidence,
    retained_worker: &RetainedNativePackageMember,
    managed_ollama: &ManagedOllamaIsolationLease<'_>,
) -> Result<(), ManagedGenerationWorkerError> {
    let target = managed_ollama.model_target().artifact_id();
    if initial.worker_artifact_id() == retained_worker.artifact_id()
        && initial.model_artifact_id() == target
        && model_mapping.model_artifact_id() == target
        && model_mapping.mapping_region_count() != 0
    {
        Ok(())
    } else {
        Err(ManagedGenerationWorkerError::InvalidRequest)
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the callback joins independent connection, package, model, and worker capabilities"
)]
pub(super) fn observe_response_and_worker<'a>(
    state: &mut ManagedSessionObserver,
    observation: OllamaResponseObservation,
    worker_response_ordinal: usize,
    package: &'a RuntimePackageManifest,
    package_id: &'a RuntimePackageManifestId,
    retained_worker: &'a RetainedNativePackageMember,
    retained_package_code: &'a [RetainedNativePackageMember],
    frozen: &'a VerifiedFrozenExternalNativeComponentSet,
    managed_ollama: &'a ManagedOllamaIsolationLease<'a>,
    limits: ManagedGenerationWorkerLimits,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<(), ManagedGenerationSessionObservationError> {
    operation_deadline
        .ensure_active(cancellation)
        .map_err(ManagedGenerationSessionObservationError::Gate)?;
    let phase = observation.phase();
    let connection_result = match operation_deadline.instant() {
        Some(deadline) => {
            state
                .connections
                .observe_until(&mut state.process, cancellation, observation, deadline)
        }
        None => state
            .connections
            .observe(&mut state.process, cancellation, observation),
    };
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return Err(ManagedGenerationSessionObservationError::Gate(error));
    }
    connection_result?;
    if phase
        != (OllamaResponseObservationPhase::AfterResponse {
            ordinal: worker_response_ordinal,
        })
    {
        return Ok(());
    }
    if state.worker.is_some() {
        return Err(LocalOllamaBoundPreflightError::InvalidObservationSequence.into());
    }
    operation_deadline
        .ensure_active(cancellation)
        .map_err(ManagedGenerationSessionObservationError::Gate)?;
    let worker_request = ManagedGenerationWorkerObservationRequest {
        package,
        expected_package_id: package_id,
        retained_worker,
        retained_model_weight: managed_ollama.retained_model_weight(),
        profile: ManagedGenerationWorkerProfile::OllamaV0_32_15Cpu,
        limits,
    };
    let lease_result = match operation_deadline.instant() {
        Some(deadline) => {
            state
                .process
                .observe_generation_worker_until(&worker_request, cancellation, deadline)
        }
        None => state
            .process
            .observe_generation_worker(&worker_request, cancellation),
    };
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return Err(ManagedGenerationSessionObservationError::Gate(error));
    }
    let mut lease = lease_result?;
    let initial = lease.initial_evidence().clone();
    operation_deadline
        .ensure_active(cancellation)
        .map_err(ManagedGenerationSessionObservationError::Gate)?;
    let native_load_request = ManagedGenerationWorkerNativeLoadRequest {
        package,
        expected_package_id: package_id,
        retained_package_code,
        expected_external_components: frozen.expected_components(),
    };
    let native_load_result = match operation_deadline.instant() {
        Some(deadline) => {
            lease.observe_native_load_until(&native_load_request, cancellation, deadline)
        }
        None => lease.observe_native_load(&native_load_request, cancellation),
    };
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return Err(ManagedGenerationSessionObservationError::Gate(error));
    }
    let native_load = native_load_result?;
    operation_deadline
        .ensure_active(cancellation)
        .map_err(ManagedGenerationSessionObservationError::Gate)?;
    let model_mapping_result = match operation_deadline.instant() {
        Some(deadline) => lease.observe_model_mapping_until(cancellation, deadline),
        None => lease.observe_model_mapping(cancellation),
    };
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return Err(ManagedGenerationSessionObservationError::Gate(error));
    }
    let model_mapping = model_mapping_result?;
    let relationship_result =
        validate_worker_relationships(&initial, &model_mapping, retained_worker, managed_ollama);
    if let Some(error) = operation_deadline.terminal_override(cancellation) {
        return Err(ManagedGenerationSessionObservationError::Gate(error));
    }
    relationship_result?;
    state.worker = Some(ManagedGenerationWorkerObservation {
        lease,
        initial,
        native_load,
        model_mapping,
    });
    Ok(())
}

pub(super) fn reobserve_worker(
    observer: &Rc<RefCell<ManagedSessionObserver>>,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<FinalManagedGenerationWorkerEvidence, LocalOllamaManagedGenerationError> {
    operation_deadline.ensure_active(cancellation)?;
    let mut state = observer
        .try_borrow_mut()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
    let worker = state
        .worker
        .as_mut()
        .ok_or(LocalOllamaManagedGenerationError::InvalidGenerationAuthority)?;
    let result = match operation_deadline.instant() {
        Some(deadline) => worker.lease.reobserve_until(cancellation, deadline),
        None => worker.lease.reobserve(cancellation),
    }
    .map_err(LocalOllamaManagedGenerationError::from)
    .map(|final_evidence| FinalManagedGenerationWorkerEvidence {
        initial: worker.initial.clone(),
        final_evidence,
        native_load: worker.native_load.clone(),
        model_mapping: worker.model_mapping.clone(),
    });
    operation_deadline.precedence(result, cancellation)
}

pub(super) fn observe_resource_and_reobserve_worker(
    observer: &Rc<RefCell<ManagedSessionObserver>>,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<
    (
        ManagedGenerationWorkerResourceObservation,
        ManagedGenerationWorkerEvidence,
    ),
    LocalOllamaManagedGenerationError,
> {
    operation_deadline.ensure_active(cancellation)?;
    let mut state = observer
        .try_borrow_mut()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
    let worker = state
        .worker
        .as_mut()
        .ok_or(LocalOllamaManagedGenerationError::InvalidGenerationAuthority)?;
    let observation_result = match operation_deadline.instant() {
        Some(deadline) => worker.lease.observe_resource_until(cancellation, deadline),
        None => worker.lease.observe_resource(cancellation),
    }
    .map_err(LocalOllamaManagedGenerationError::from);
    let observation = operation_deadline.precedence(observation_result, cancellation)?;
    operation_deadline.ensure_active(cancellation)?;
    let evidence_result = match operation_deadline.instant() {
        Some(deadline) => worker.lease.reobserve_until(cancellation, deadline),
        None => worker.lease.reobserve(cancellation),
    }
    .map_err(LocalOllamaManagedGenerationError::from);
    let evidence = operation_deadline.precedence(evidence_result, cancellation)?;
    let result = if observation.worker_evidence_digest() == evidence.evidence_digest()
        && evidence == worker.initial
    {
        Ok((observation, evidence))
    } else {
        Err(LocalOllamaManagedPreflightError::InvalidEvidenceBinding.into())
    };
    operation_deadline.precedence(result, cancellation)
}

pub(super) fn reobserve_process(
    observer: &Rc<RefCell<ManagedSessionObserver>>,
    package: &RuntimePackageManifest,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<AttachedProcessEvidence, LocalOllamaManagedGenerationError> {
    operation_deadline.ensure_active(cancellation)?;
    let mut state = observer
        .try_borrow_mut()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
    let result = match operation_deadline.instant() {
        Some(deadline) => state.process.reobserve_until(cancellation, deadline),
        None => state.process.reobserve(cancellation),
    }
    .map_err(LocalOllamaManagedPreflightError::Witness)
    .map_err(LocalOllamaManagedGenerationError::from)
    .and_then(|evidence| {
        validate_process_binding(&evidence, package)
            .map_err(LocalOllamaManagedGenerationError::from)?;
        Ok(evidence)
    });
    operation_deadline.precedence(result, cancellation)
}

pub(super) fn observe_native_load(
    observer: &Rc<RefCell<ManagedSessionObserver>>,
    package: &RuntimePackageManifest,
    retained_members: &[RetainedNativePackageMember],
    frozen_external_components: &VerifiedFrozenExternalNativeComponentSet,
    limits: LocalOllamaManagedPreflightLimits,
    operation_deadline: CandidateOperationDeadline,
    cancellation: &CancellationToken,
) -> Result<NativeLoadObservation, LocalOllamaManagedGenerationError> {
    operation_deadline.ensure_active(cancellation)?;
    let mut state = observer
        .try_borrow_mut()
        .map_err(|_error| LocalOllamaManagedPreflightError::InvalidEvidenceBinding)?;
    let package_id = package.runtime_package_manifest_id();
    let request = NativeLoadObservationRequest {
        package,
        expected_package_id: &package_id,
        retained_package_members: retained_members,
        expected_external_components: frozen_external_components.expected_components(),
        limits: limits.native_load,
    };
    let result = match operation_deadline.instant() {
        Some(deadline) => state
            .process
            .observe_native_load_until(&request, cancellation, deadline),
        None => state.process.observe_native_load(&request, cancellation),
    }
    .map_err(LocalOllamaManagedPreflightError::NativeLoad)
    .map_err(LocalOllamaManagedGenerationError::from);
    operation_deadline.precedence(result, cancellation)
}

pub(super) fn map_session_error(
    error: OllamaObservedSessionError<ManagedGenerationSessionObservationError>,
) -> LocalOllamaManagedGenerationError {
    match error {
        OllamaObservedSessionError::Session(error) => {
            LocalOllamaManagedGenerationError::Session(error)
        }
        OllamaObservedSessionError::Observation(
            ManagedGenerationSessionObservationError::Connection(error),
        ) => LocalOllamaManagedPreflightError::BoundObservation(error).into(),
        OllamaObservedSessionError::Observation(
            ManagedGenerationSessionObservationError::Worker(error),
        ) => LocalOllamaManagedGenerationError::Worker(error),
        OllamaObservedSessionError::Observation(
            ManagedGenerationSessionObservationError::Gate(error),
        ) => error,
    }
}

#[cfg(test)]
#[path = "validation/tests.rs"]
mod tests;
