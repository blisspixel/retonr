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
mod platform;

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
    ListenerEndpoint, MAXIMUM_DESCRIPTORS_PER_PROCESS, MAXIMUM_ENTRYPOINT_BYTES,
    MAXIMUM_OBSERVATION_MILLIS, MAXIMUM_OBSERVED_PROCESSES, MAXIMUM_SOCKET_TABLE_BYTES,
    MAXIMUM_SOCKET_TABLE_ENTRIES,
};

/// Native observer selected for the current operating system.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NativeAttachedProcessObserver;

/// Retained native listener-owner capability.
pub struct NativeAttachedProcessLease {
    initial: AttachedProcessEvidence,
    endpoint: ListenerEndpoint,
    platform: platform::Lease,
    limits: AttachedProcessWitnessLimits,
    started: Instant,
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
    if started.elapsed() > limits.maximum_elapsed {
        return Err(AttachedProcessWitnessError::DeadlineExceeded);
    }
    Ok(())
}

fn compare_evidence(
    initial: &AttachedProcessEvidence,
    observed: &AttachedProcessEvidence,
) -> Result<(), AttachedProcessWitnessError> {
    if initial.owner_pid() != observed.owner_pid()
        || initial.ownership_snapshot_digest() != observed.ownership_snapshot_digest()
    {
        return Err(AttachedProcessWitnessError::ListenerRebound);
    }
    if initial.process_instance_digest() != observed.process_instance_digest() {
        return Err(AttachedProcessWitnessError::ProcessInstanceChanged);
    }
    if initial.entrypoint_object_digest() != observed.entrypoint_object_digest()
        || initial.entrypoint_digest() != observed.entrypoint_digest()
        || initial.entrypoint_bytes() != observed.entrypoint_bytes()
    {
        return Err(AttachedProcessWitnessError::EntrypointChanged);
    }
    if initial.evidence_class() != observed.evidence_class()
        || initial.platform_evidence_digest() != observed.platform_evidence_digest()
    {
        return Err(AttachedProcessWitnessError::PlatformObservationFailed);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
