use std::time::Instant;

use rewrite_model::NativeLoadObservation;
use rewrite_runtime_attestor::{
    AttachedProcessEvidence, AttachedProcessWitnessError, NativeLoadObservationRequest,
    NativeLoadObserverError, RetainedTcpConnection, RetainedTcpConnectionEvidence,
};
#[cfg(target_os = "linux")]
use rewrite_runtime_attestor::{AttachedProcessLease, NativeManagedLinuxProcessLease};
use rewrite_runtime_attestor::{
    ManagedGenerationWorkerError, ManagedGenerationWorkerObservationRequest,
};
use rewrite_types::CancellationToken;

use super::worker::ManagedJudgeWorkerAuthority;

pub(super) trait ManagedJudgeProcessAuthority {
    #[cfg(any(target_os = "linux", test))]
    fn initial_evidence(&self) -> &AttachedProcessEvidence;

    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError>;

    fn reobserve_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        let _ = operation_deadline;
        self.reobserve(cancellation)
    }

    fn observe_connection(
        &mut self,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>;

    fn observe_connection_until(
        &mut self,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        let _ = operation_deadline;
        self.observe_connection(connection, cancellation)
    }

    fn reobserve_connection(
        &mut self,
        connection: RetainedTcpConnection,
        initial: &RetainedTcpConnectionEvidence,
        cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError>;

    fn reobserve_connection_until(
        &mut self,
        connection: RetainedTcpConnection,
        initial: &RetainedTcpConnectionEvidence,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        let _ = operation_deadline;
        self.reobserve_connection(connection, initial, cancellation)
    }

    fn observe_native_load(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<NativeLoadObservation, NativeLoadObserverError>;

    fn observe_native_load_until(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<NativeLoadObservation, NativeLoadObserverError> {
        let _ = operation_deadline;
        self.observe_native_load(request, cancellation)
    }

    fn observe_generation_worker(
        &mut self,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn ManagedJudgeWorkerAuthority>, ManagedGenerationWorkerError>;

    fn observe_generation_worker_until(
        &mut self,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<Box<dyn ManagedJudgeWorkerAuthority>, ManagedGenerationWorkerError> {
        let _ = operation_deadline;
        self.observe_generation_worker(request, cancellation)
    }
}

#[cfg(target_os = "linux")]
impl ManagedJudgeProcessAuthority for NativeManagedLinuxProcessLease {
    #[cfg(any(target_os = "linux", test))]
    fn initial_evidence(&self) -> &AttachedProcessEvidence {
        AttachedProcessLease::initial_evidence(self)
    }

    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        AttachedProcessLease::reobserve(self, cancellation)
    }

    fn reobserve_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        NativeManagedLinuxProcessLease::reobserve_until(self, cancellation, operation_deadline)
    }

    fn observe_connection(
        &mut self,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        AttachedProcessLease::observe_connection(self, connection, cancellation)
    }

    fn observe_connection_until(
        &mut self,
        connection: RetainedTcpConnection,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        NativeManagedLinuxProcessLease::observe_connection_until(
            self,
            connection,
            cancellation,
            operation_deadline,
        )
    }

    fn reobserve_connection(
        &mut self,
        connection: RetainedTcpConnection,
        initial: &RetainedTcpConnectionEvidence,
        cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        AttachedProcessLease::reobserve_connection(self, connection, initial, cancellation)
    }

    fn reobserve_connection_until(
        &mut self,
        connection: RetainedTcpConnection,
        initial: &RetainedTcpConnectionEvidence,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        NativeManagedLinuxProcessLease::reobserve_connection_until(
            self,
            connection,
            initial,
            cancellation,
            operation_deadline,
        )
    }

    fn observe_native_load(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<NativeLoadObservation, NativeLoadObserverError> {
        AttachedProcessLease::observe_native_load(self, request, cancellation)
    }

    fn observe_native_load_until(
        &mut self,
        request: &NativeLoadObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<NativeLoadObservation, NativeLoadObserverError> {
        NativeManagedLinuxProcessLease::observe_native_load_until(
            self,
            request,
            cancellation,
            operation_deadline,
        )
    }

    fn observe_generation_worker(
        &mut self,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn ManagedJudgeWorkerAuthority>, ManagedGenerationWorkerError> {
        NativeManagedLinuxProcessLease::observe_generation_worker(self, request, cancellation)
            .map(|lease| Box::new(lease) as Box<dyn ManagedJudgeWorkerAuthority>)
    }

    fn observe_generation_worker_until(
        &mut self,
        request: &ManagedGenerationWorkerObservationRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<Box<dyn ManagedJudgeWorkerAuthority>, ManagedGenerationWorkerError> {
        NativeManagedLinuxProcessLease::observe_generation_worker_until(
            self,
            request,
            cancellation,
            operation_deadline,
        )
        .map(|lease| Box::new(lease) as Box<dyn ManagedJudgeWorkerAuthority>)
    }
}
