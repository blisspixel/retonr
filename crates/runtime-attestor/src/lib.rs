//! Bounded native listener and process observation for local runtimes.

#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    thread,
    time::{Duration, Instant},
};

use rewrite_types::CancellationToken;

mod connection;
mod contract;
mod deadline;
mod evidence_compare;
mod managed_contract;
mod managed_process;
mod managed_worker;
mod native_load;
mod platform;

use evidence_compare::compare_evidence;
#[cfg(test)]
use managed_process::{attached_deadline_precedence, native_deadline_precedence};

/// Maximum native snapshots used to admit initial connection-table publication delay.
pub const MAXIMUM_CONNECTION_PUBLICATION_ATTEMPTS: usize = 8;
/// Maximum elapsed time admitted for initial connection-table publication.
pub const MAXIMUM_CONNECTION_PUBLICATION_MILLIS: u64 = 50;

const CONNECTION_PUBLICATION_RETRY_INTERVAL: Duration = Duration::from_millis(5);

pub use connection::{
    RETAINED_TCP_CONNECTION_EVIDENCE_SCHEMA_VERSION, RetainedTcpConnection,
    RetainedTcpConnectionEvidence, RetainedTcpConnectionEvidenceInput,
    TcpConnectionAttributionKind, TcpConnectionSharingLimitation,
};
pub use contract::{
    ATTACHED_PROCESS_WITNESS_SCHEMA_VERSION, AttachedProcessEvidence, AttachedProcessEvidenceClass,
    AttachedProcessEvidenceInput, AttachedProcessLaunchMode, AttachedProcessLease,
    AttachedProcessObserver, AttachedProcessWitnessError, AttachedProcessWitnessLimits,
    ListenerEndpoint, MANAGED_LINUX_PROCESS_WITNESS_SCHEMA_VERSION,
    MAXIMUM_DESCRIPTORS_PER_PROCESS, MAXIMUM_ENTRYPOINT_BYTES, MAXIMUM_OBSERVATION_MILLIS,
    MAXIMUM_OBSERVED_PROCESSES, MAXIMUM_SOCKET_TABLE_BYTES, MAXIMUM_SOCKET_TABLE_ENTRIES,
};
pub use managed_contract::ManagedLinuxProcessExpectation;
pub use managed_worker::{
    MANAGED_GENERATION_WORKER_OBSERVATION_SCHEMA_VERSION,
    MANAGED_GENERATION_WORKER_RESOURCE_OBSERVATION_SCHEMA_VERSION, MANAGED_OLLAMA_MODEL_ROOT,
    MAXIMUM_GENERATION_WORKER_COMMAND_ARGUMENTS, MAXIMUM_GENERATION_WORKER_COMMAND_BYTES,
    MAXIMUM_GENERATION_WORKER_METADATA_BYTES, MAXIMUM_GENERATION_WORKER_OBSERVATION_MILLIS,
    MAXIMUM_GENERATION_WORKER_PARENT_DEPTH, MAXIMUM_GENERATION_WORKER_PROCESSES,
    MAXIMUM_RETAINED_MODEL_WEIGHT_BYTES, ManagedGenerationWorkerError,
    ManagedGenerationWorkerEvidence, ManagedGenerationWorkerLimits,
    ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerNativeLoadEvidence,
    ManagedGenerationWorkerNativeLoadRequest, ManagedGenerationWorkerObservationRequest,
    ManagedGenerationWorkerProfile, ManagedGenerationWorkerResourceObservation,
    NativeManagedGenerationWorkerLease, RetainedModelWeight, RetainedModelWeightSink,
    RetainedModelWeightSource,
};
pub use native_load::{
    CompiledFrozenExternalNativeComponentSet, DiscoveredExternalNativeComponent,
    EXTERNAL_NATIVE_COMPONENT_REVIEW_SCHEMA_VERSION, ExpectedExternalNativeComponent,
    ExternalNativeComponentReview, ExternalNativeComponentReviewDisposition,
    ExternalNativeComponentReviewError, ExternalNativeComponentReviewMember,
    FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_SCHEMA_VERSION, FrozenExternalNativeComponentSetError,
    FrozenExternalNativeComponentSetId, MAXIMUM_EXTERNAL_COMPONENT_REVIEW_EVIDENCE_BYTES,
    MAXIMUM_EXTERNAL_NATIVE_COMPONENT_REVIEW_JSON_BYTES,
    MAXIMUM_FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_JSON_BYTES,
    MAXIMUM_NATIVE_LOAD_DISCOVERY_JSON_BYTES, MAXIMUM_NATIVE_LOAD_HASH_BYTES,
    MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS, MAXIMUM_NATIVE_LOADED_COMPONENTS,
    MAXIMUM_NATIVE_MAPPING_METADATA_BYTES, MAXIMUM_NATIVE_MAPPING_REGIONS,
    NATIVE_LOAD_DISCOVERY_SCHEMA_VERSION, NativeLoadDiscovery, NativeLoadDiscoveryRequest,
    NativeLoadObservationLimits, NativeLoadObservationRequest, NativeLoadObserverError,
    RetainedNativePackageMember, VerifiedFrozenExternalNativeComponentSet,
};

/// Native observer selected for the current operating system.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeAttachedProcessObserver;

/// Linux-only observer for an explicitly identified managed process and a
/// caller-supplied namespace-local socket-diagnostics capability.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeManagedLinuxProcessObserver;

/// Retained native listener-owner capability.
pub struct NativeAttachedProcessLease {
    initial: AttachedProcessEvidence,
    endpoint: ListenerEndpoint,
    platform: platform::Lease,
    limits: AttachedProcessWitnessLimits,
    started: Instant,
}

/// Retained managed Linux process capability.
pub struct NativeManagedLinuxProcessLease {
    initial: AttachedProcessEvidence,
    endpoint: ListenerEndpoint,
    platform: platform::ManagedLease,
    limits: AttachedProcessWitnessLimits,
    started: Instant,
    operation_deadline: Option<Instant>,
}

impl AttachedProcessObserver for NativeAttachedProcessObserver {
    type Lease = NativeAttachedProcessLease;

    fn attach(
        &self,
        endpoint: ListenerEndpoint,
        limits: AttachedProcessWitnessLimits,
        cancellation: &CancellationToken,
    ) -> Result<Self::Lease, AttachedProcessWitnessError> {
        let limits = limits.validate()?;
        ensure_active(cancellation, Instant::now(), limits)?;
        let started = Instant::now();
        let platform = platform::Lease::attach(endpoint, limits, cancellation, started)?;
        let initial = platform.initial_evidence().clone();
        Ok(NativeAttachedProcessLease {
            initial,
            endpoint,
            platform,
            limits,
            started,
        })
    }
}

impl AttachedProcessLease for NativeAttachedProcessLease {
    fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.initial
    }

    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        ensure_active(cancellation, self.started, self.limits)?;
        let observed = self
            .platform
            .reobserve(self.limits, cancellation, self.started)?;
        compare_evidence(&self.initial, &observed)?;
        Ok(observed)
    }

    fn observe_connection(
        &mut self,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        ensure_active(cancellation, self.started, self.limits)?;
        if connection.server() != self.endpoint.socket() {
            return Err(AttachedProcessWitnessError::ConnectionProcessMismatch);
        }
        observe_initial_connection(cancellation, self.started, self.limits, || {
            self.platform
                .observe_connection(connection, self.limits, cancellation, self.started)
        })
    }

    fn reobserve_connection(
        &mut self,
        connection: RetainedTcpConnection,
        initial: &RetainedTcpConnectionEvidence,
        cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        ensure_active(cancellation, self.started, self.limits)?;
        if connection.server() != self.endpoint.socket() {
            return Err(AttachedProcessWitnessError::ConnectionProcessMismatch);
        }
        reobserve_connection_once(initial, || {
            self.platform
                .observe_connection(connection, self.limits, cancellation, self.started)
        })
    }

    fn observe_native_load(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<rewrite_model::NativeLoadObservation, NativeLoadObserverError> {
        let limits = request.validate()?;
        let native_started = Instant::now();
        ensure_native_active(cancellation, native_started, limits)?;
        let before = self
            .platform
            .reobserve(self.limits, cancellation, self.started)
            .map_err(map_native_process_error)?;
        compare_evidence(&self.initial, &before).map_err(map_native_process_error)?;
        let observation = self.platform.observe_native_load(
            request,
            limits,
            cancellation,
            native_started,
            self.initial.evidence_digest(),
        )?;
        let after = self
            .platform
            .reobserve(self.limits, cancellation, self.started)
            .map_err(map_native_process_error)?;
        compare_evidence(&self.initial, &after).map_err(map_native_process_error)?;
        if observation.process_evidence_digest() != self.initial.evidence_digest()
            || observation.runtime_package_manifest_id() != request.expected_package_id
        {
            return Err(NativeLoadObserverError::InvalidObservation);
        }
        Ok(observation)
    }

    fn discover_external_native_components(
        &mut self,
        request: &NativeLoadDiscoveryRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<NativeLoadDiscovery, NativeLoadObserverError> {
        let limits = request.validate()?;
        let native_started = Instant::now();
        ensure_native_active(cancellation, native_started, limits)?;
        let before = self
            .platform
            .reobserve(self.limits, cancellation, self.started)
            .map_err(map_native_process_error)?;
        compare_evidence(&self.initial, &before).map_err(map_native_process_error)?;
        let discovery = self.platform.discover_external_native_components(
            request,
            limits,
            cancellation,
            native_started,
            self.initial.evidence_digest(),
        )?;
        let after = self
            .platform
            .reobserve(self.limits, cancellation, self.started)
            .map_err(map_native_process_error)?;
        compare_evidence(&self.initial, &after).map_err(map_native_process_error)?;
        if discovery.process_evidence_digest() != self.initial.evidence_digest()
            || discovery.runtime_package_manifest_id() != request.expected_package_id
        {
            return Err(NativeLoadObserverError::InvalidObservation);
        }
        Ok(discovery)
    }
}

pub(crate) fn ensure_native_active(
    cancellation: &CancellationToken,
    started: Instant,
    limits: NativeLoadObservationLimits,
) -> Result<(), NativeLoadObserverError> {
    if cancellation.is_cancelled() {
        return Err(NativeLoadObserverError::Cancelled);
    }
    if started.elapsed() >= limits.maximum_elapsed {
        return Err(NativeLoadObserverError::DeadlineExceeded);
    }
    Ok(())
}

fn map_native_process_error(error: AttachedProcessWitnessError) -> NativeLoadObserverError {
    match error {
        AttachedProcessWitnessError::Cancelled => NativeLoadObserverError::Cancelled,
        AttachedProcessWitnessError::DeadlineExceeded => NativeLoadObserverError::DeadlineExceeded,
        AttachedProcessWitnessError::ProcessAccessDenied => {
            NativeLoadObserverError::ProcessVisibilityInsufficient
        }
        AttachedProcessWitnessError::ProcessExited
        | AttachedProcessWitnessError::ProcessInstanceChanged
        | AttachedProcessWitnessError::ListenerRebound
        | AttachedProcessWitnessError::EntrypointChanged => NativeLoadObserverError::ProcessChanged,
        AttachedProcessWitnessError::ResourceLimit
        | AttachedProcessWitnessError::EntrypointTooLarge => NativeLoadObserverError::ResourceLimit,
        _ => NativeLoadObserverError::PlatformObservationFailed,
    }
}

fn observe_initial_connection<F>(
    cancellation: &CancellationToken,
    started: Instant,
    limits: AttachedProcessWitnessLimits,
    observe: F,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>
where
    F: FnMut() -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>,
{
    observe_initial_connection_with_policy(
        cancellation,
        started,
        limits,
        MAXIMUM_CONNECTION_PUBLICATION_ATTEMPTS,
        Duration::from_millis(MAXIMUM_CONNECTION_PUBLICATION_MILLIS),
        CONNECTION_PUBLICATION_RETRY_INTERVAL,
        observe,
    )
}

fn observe_initial_connection_with_policy<F>(
    cancellation: &CancellationToken,
    started: Instant,
    limits: AttachedProcessWitnessLimits,
    maximum_attempts: usize,
    publication_limit: Duration,
    retry_interval: Duration,
    mut observe: F,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>
where
    F: FnMut() -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>,
{
    let publication_started = Instant::now();
    let mut last_publication_error = AttachedProcessWitnessError::ConnectionNotFound;
    for attempt in 0..maximum_attempts {
        ensure_active(cancellation, started, limits)?;
        if attempt > 0 && publication_started.elapsed() >= publication_limit {
            return Err(last_publication_error);
        }
        match observe() {
            Ok(evidence) => return Ok(evidence),
            Err(
                error @ (AttachedProcessWitnessError::ConnectionNotFound
                | AttachedProcessWitnessError::ConnectionNotEstablished),
            ) => {
                last_publication_error = error;
                let elapsed = publication_started.elapsed();
                if attempt + 1 == maximum_attempts || elapsed >= publication_limit {
                    return Err(error);
                }
                let remaining = publication_limit.saturating_sub(elapsed);
                thread::sleep(retry_interval.min(remaining));
            }
            Err(error) => return Err(error),
        }
    }
    Err(AttachedProcessWitnessError::ConnectionNotFound)
}

fn compare_connection_evidence(
    initial: &RetainedTcpConnectionEvidence,
    observed: &RetainedTcpConnectionEvidence,
) -> Result<(), AttachedProcessWitnessError> {
    if initial.attribution_kind() != observed.attribution_kind()
        || initial.sharing_limitation() != observed.sharing_limitation()
        || initial.evidence_digest() != observed.evidence_digest()
    {
        return Err(AttachedProcessWitnessError::ConnectionChanged);
    }
    Ok(())
}

fn reobserve_connection_once<F>(
    initial: &RetainedTcpConnectionEvidence,
    mut observe: F,
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>
where
    F: FnMut() -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>,
{
    let observed = match observe() {
        Err(
            AttachedProcessWitnessError::ConnectionNotFound
            | AttachedProcessWitnessError::ConnectionNotEstablished,
        ) => return Err(AttachedProcessWitnessError::ConnectionClosed),
        result => result?,
    };
    compare_connection_evidence(initial, &observed)?;
    Ok(observed)
}

pub(crate) fn ensure_active(
    cancellation: &CancellationToken,
    started: Instant,
    limits: AttachedProcessWitnessLimits,
) -> Result<(), AttachedProcessWitnessError> {
    if cancellation.is_cancelled() {
        return Err(AttachedProcessWitnessError::Cancelled);
    }
    if started.elapsed() >= limits.maximum_elapsed {
        return Err(AttachedProcessWitnessError::DeadlineExceeded);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
