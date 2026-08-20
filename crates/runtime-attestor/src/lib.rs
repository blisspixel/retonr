//! Bounded native listener and process observation for local runtimes.

#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::time::Instant;

use rewrite_types::CancellationToken;

mod contract;
mod platform;

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
