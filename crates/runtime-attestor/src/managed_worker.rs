use std::time::Instant;

use rewrite_types::CancellationToken;

use crate::{
    AttachedProcessWitnessError, NativeManagedLinuxProcessLease,
    deadline::{ObservationBudget, earliest_deadline, operation_expired},
};

mod contract;
mod error;
mod evidence;
mod resource_observation;

pub use contract::{
    MANAGED_OLLAMA_MODEL_ROOT, MAXIMUM_GENERATION_WORKER_COMMAND_ARGUMENTS,
    MAXIMUM_GENERATION_WORKER_COMMAND_BYTES, MAXIMUM_GENERATION_WORKER_METADATA_BYTES,
    MAXIMUM_GENERATION_WORKER_OBSERVATION_MILLIS, MAXIMUM_GENERATION_WORKER_PARENT_DEPTH,
    MAXIMUM_GENERATION_WORKER_PROCESSES, MAXIMUM_RETAINED_MODEL_WEIGHT_BYTES,
    ManagedGenerationWorkerLimits, ManagedGenerationWorkerNativeLoadRequest,
    ManagedGenerationWorkerObservationRequest, ManagedGenerationWorkerProfile, RetainedModelWeight,
    RetainedModelWeightSink, RetainedModelWeightSource,
};
pub use error::ManagedGenerationWorkerError;
pub use evidence::{
    MANAGED_GENERATION_WORKER_OBSERVATION_SCHEMA_VERSION, ManagedGenerationWorkerEvidence,
    ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerNativeLoadEvidence,
};
#[cfg(target_os = "linux")]
pub(crate) use resource_observation::parse_high_water_resident_bytes;
pub use resource_observation::{
    MANAGED_GENERATION_WORKER_RESOURCE_OBSERVATION_SCHEMA_VERSION,
    ManagedGenerationWorkerResourceObservation,
};

impl NativeManagedLinuxProcessLease {
    /// Discovers and retains one distinct managed generation-worker process.
    ///
    /// This operation does not reuse listener-process evidence. The returned lease
    /// is an opaque, nonserializable capability over a separately retained worker
    /// incarnation. It remains inert unless a later caller explicitly joins it.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedGenerationWorkerError`] unless exactly one bounded worker
    /// descendant matches the exact retained executable, command, privilege, and
    /// namespace contract.
    pub fn observe_generation_worker(
        &mut self,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<NativeManagedGenerationWorkerLease, ManagedGenerationWorkerError> {
        self.observe_generation_worker_with_deadline(request, cancellation, None)
    }

    /// Discovers and retains the managed generation worker under an already-captured
    /// absolute operation deadline.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedGenerationWorkerError::DeadlineExceeded`] at or after the
    /// supplied deadline, or the same bounded errors as
    /// [`Self::observe_generation_worker`].
    pub fn observe_generation_worker_until(
        &mut self,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<NativeManagedGenerationWorkerLease, ManagedGenerationWorkerError> {
        self.observe_generation_worker_with_deadline(
            request,
            cancellation,
            Some(operation_deadline),
        )
    }

    fn observe_generation_worker_with_deadline(
        &mut self,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<NativeManagedGenerationWorkerLease, ManagedGenerationWorkerError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let limits = request.validate()?;
            let (started, limits) = bounded_worker_limits(limits, operation_deadline)?;
            ensure_worker_active(cancellation, started, limits)?;
            let (server_started, server_limits) = self
                .bounded_limits(operation_deadline)
                .map_err(map_server_error)?;
            let server = self
                .platform
                .reobserve(server_limits, cancellation, server_started)
                .map_err(map_server_error)?;
            crate::compare_evidence(&self.initial, &server).map_err(map_server_error)?;
            let platform = self.platform.observe_generation_worker(
                request,
                limits,
                cancellation,
                started,
                self.initial.evidence_digest(),
            )?;
            let evidence = platform.evidence().clone();
            Ok(NativeManagedGenerationWorkerLease::new(
                evidence,
                platform,
                limits,
                operation_deadline,
            ))
        })();
        worker_deadline_precedence(result, cancellation, operation_deadline)
    }
}

/// Opaque retained generation-worker capability.
///
/// The type intentionally implements neither `Clone` nor serialization. Its public
/// evidence records remain inert and cannot recreate the retained native handles.
pub struct NativeManagedGenerationWorkerLease {
    initial: ManagedGenerationWorkerEvidence,
    platform: crate::platform::WorkerLease,
    limits: ManagedGenerationWorkerLimits,
    operation_deadline: Option<Instant>,
}

impl NativeManagedGenerationWorkerLease {
    pub(crate) const fn new(
        initial: ManagedGenerationWorkerEvidence,
        platform: crate::platform::WorkerLease,
        limits: ManagedGenerationWorkerLimits,
        operation_deadline: Option<Instant>,
    ) -> Self {
        Self {
            initial,
            platform,
            limits,
            operation_deadline,
        }
    }

    /// Returns the initial inert redacted worker observation.
    #[must_use]
    pub const fn initial_evidence(&self) -> &ManagedGenerationWorkerEvidence {
        &self.initial
    }

    /// Observes and retains the exact file-backed executable closure for this worker.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedGenerationWorkerError`] for an incomplete, changed,
    /// unadmitted, server-code, accelerator, cancelled, or over-limit mapping view.
    pub fn observe_native_load(
        &mut self,
        request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        self.observe_native_load_with_deadline(request, cancellation, None)
    }

    /// Observes the retained worker native-load closure under an absolute deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, relationship, resource, or mapping error.
    pub fn observe_native_load_until(
        &mut self,
        request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        self.observe_native_load_with_deadline(request, cancellation, Some(operation_deadline))
    }

    fn observe_native_load_with_deadline(
        &mut self,
        request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            request.validate(
                self.initial.profile(),
                self.initial.runtime_package_manifest_id(),
                self.initial.worker_artifact_id(),
                self.limits,
            )?;
            let (started, limits) = bounded_worker_limits(self.limits, operation_deadline)?;
            ensure_worker_active(cancellation, started, limits)?;
            self.platform
                .observe_native_load(request, limits, cancellation, started)
        })();
        worker_deadline_precedence(result, cancellation, operation_deadline)
    }

    /// Observes and retains the exact GGUF file-object mapping.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedGenerationWorkerError`] unless exactly the expected path,
    /// bytes, and retained private mapping relationship remain stable. This permits
    /// byte-identical private materialization with an inode distinct from the source.
    pub fn observe_model_mapping(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        self.observe_model_mapping_with_deadline(cancellation, None)
    }

    /// Observes the exact retained model mapping under an absolute deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, resource, or mapping error.
    pub fn observe_model_mapping_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        self.observe_model_mapping_with_deadline(cancellation, Some(operation_deadline))
    }

    fn observe_model_mapping_with_deadline(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let (started, limits) = bounded_worker_limits(self.limits, operation_deadline)?;
            ensure_worker_active(cancellation, started, limits)?;
            self.platform
                .observe_model_mapping(limits, cancellation, started)
        })();
        worker_deadline_precedence(result, cancellation, operation_deadline)
    }

    /// Reads the kernel-reported peak resident set for this retained worker.
    ///
    /// The result is one process-scoped Linux `VmHWM` observation. It is not a
    /// formal memory bound and does not establish working-set composition, model
    /// residency, accelerator memory, or cgroup-wide consumption.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedGenerationWorkerError`] when the worker changes across
    /// the observation bracket, procfs is unavailable or malformed, or a caller
    /// ceiling, cancellation, or deadline is reached.
    pub fn observe_resource(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerResourceObservation, ManagedGenerationWorkerError> {
        self.observe_resource_with_deadline(cancellation, None)
    }

    /// Reads the worker resource observation under an absolute operation deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, resource, process, or observation error.
    pub fn observe_resource_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerResourceObservation, ManagedGenerationWorkerError> {
        self.observe_resource_with_deadline(cancellation, Some(operation_deadline))
    }

    fn observe_resource_with_deadline(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<ManagedGenerationWorkerResourceObservation, ManagedGenerationWorkerError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let (started, limits) = bounded_worker_limits(self.limits, operation_deadline)?;
            ensure_worker_active(cancellation, started, limits)?;
            self.platform
                .observe_resource(limits, cancellation, started)
        })();
        worker_deadline_precedence(result, cancellation, operation_deadline)
    }

    /// Reobserves the worker and both separately retained load phases.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedGenerationWorkerError`] when either load phase was not first
    /// established, or any process, command, namespace, privilege, native closure,
    /// model mapping, cancellation, deadline, or ceiling fact changed.
    pub fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        self.reobserve_with_deadline(cancellation, None)
    }

    /// Reobserves the worker and retained load phases under an absolute deadline.
    ///
    /// # Errors
    ///
    /// Returns a deadline, cancellation, process, resource, or evidence error.
    pub fn reobserve_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        self.reobserve_with_deadline(cancellation, Some(operation_deadline))
    }

    fn reobserve_with_deadline(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        let operation_deadline = self.deadline(operation_deadline);
        let result = (|| {
            let (started, limits) = bounded_worker_limits(self.limits, operation_deadline)?;
            ensure_worker_active(cancellation, started, limits)?;
            let evidence = self.platform.reobserve(limits, cancellation, started)?;
            if evidence != self.initial {
                return Err(ManagedGenerationWorkerError::ObservationChanged);
            }
            Ok(evidence)
        })();
        worker_deadline_precedence(result, cancellation, operation_deadline)
    }

    fn deadline(&self, operation_deadline: Option<Instant>) -> Option<Instant> {
        earliest_deadline(self.operation_deadline, operation_deadline)
    }
}

fn worker_deadline_precedence<T>(
    result: Result<T, ManagedGenerationWorkerError>,
    cancellation: &CancellationToken,
    operation_deadline: Option<Instant>,
) -> Result<T, ManagedGenerationWorkerError> {
    if operation_expired(operation_deadline) {
        Err(ManagedGenerationWorkerError::DeadlineExceeded)
    } else if operation_deadline.is_some() && cancellation.is_cancelled() {
        Err(ManagedGenerationWorkerError::Cancelled)
    } else {
        result
    }
}

pub(crate) fn ensure_worker_active(
    cancellation: &CancellationToken,
    started: Instant,
    limits: ManagedGenerationWorkerLimits,
) -> Result<(), ManagedGenerationWorkerError> {
    if cancellation.is_cancelled() {
        return Err(ManagedGenerationWorkerError::Cancelled);
    }
    if started.elapsed() >= limits.maximum_elapsed {
        return Err(ManagedGenerationWorkerError::DeadlineExceeded);
    }
    Ok(())
}

fn bounded_worker_limits(
    mut limits: ManagedGenerationWorkerLimits,
    operation_deadline: Option<Instant>,
) -> Result<(Instant, ManagedGenerationWorkerLimits), ManagedGenerationWorkerError> {
    let budget = ObservationBudget::fresh(limits.maximum_elapsed, operation_deadline)
        .map_err(|()| ManagedGenerationWorkerError::DeadlineExceeded)?;
    limits.maximum_elapsed = budget.maximum_elapsed();
    limits.native_load.maximum_elapsed = limits
        .native_load
        .maximum_elapsed
        .min(budget.maximum_elapsed());
    Ok((budget.started(), limits))
}

pub(crate) const fn map_server_error(
    error: AttachedProcessWitnessError,
) -> ManagedGenerationWorkerError {
    match error {
        AttachedProcessWitnessError::Cancelled => ManagedGenerationWorkerError::Cancelled,
        AttachedProcessWitnessError::DeadlineExceeded => {
            ManagedGenerationWorkerError::DeadlineExceeded
        }
        AttachedProcessWitnessError::ProcessAccessDenied => {
            ManagedGenerationWorkerError::ProcessVisibilityInsufficient
        }
        AttachedProcessWitnessError::ResourceLimit => ManagedGenerationWorkerError::ResourceLimit,
        AttachedProcessWitnessError::Unsupported => ManagedGenerationWorkerError::Unsupported,
        _ => ManagedGenerationWorkerError::ServerChanged,
    }
}

#[cfg(test)]
#[path = "managed_worker/tests.rs"]
mod tests;
