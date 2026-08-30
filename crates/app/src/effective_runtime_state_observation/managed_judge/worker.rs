use std::time::Instant;

use rewrite_runtime_attestor::{
    ManagedGenerationWorkerError, ManagedGenerationWorkerEvidence,
    ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerNativeLoadEvidence,
    ManagedGenerationWorkerNativeLoadRequest, NativeManagedGenerationWorkerLease,
};
use rewrite_types::CancellationToken;

pub(super) struct PendingManagedJudgeWorkerObservation {
    pub(super) schedule_cursor: u32,
    pub(super) lease: Box<dyn ManagedJudgeWorkerAuthority>,
    pub(super) initial: ManagedGenerationWorkerEvidence,
    pub(super) native_load: ManagedGenerationWorkerNativeLoadEvidence,
    pub(super) model_mapping: ManagedGenerationWorkerModelMappingEvidence,
}

pub(super) trait ManagedJudgeWorkerAuthority {
    fn initial_evidence(&self) -> &ManagedGenerationWorkerEvidence;

    fn observe_native_load(
        &mut self,
        request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError>;

    fn observe_native_load_until(
        &mut self,
        request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        let _ = operation_deadline;
        self.observe_native_load(request, cancellation)
    }

    fn observe_model_mapping(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError>;

    fn observe_model_mapping_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        let _ = operation_deadline;
        self.observe_model_mapping(cancellation)
    }

    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError>;

    fn reobserve_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        let _ = operation_deadline;
        self.reobserve(cancellation)
    }
}

impl ManagedJudgeWorkerAuthority for NativeManagedGenerationWorkerLease {
    fn initial_evidence(&self) -> &ManagedGenerationWorkerEvidence {
        NativeManagedGenerationWorkerLease::initial_evidence(self)
    }

    fn observe_native_load(
        &mut self,
        request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        NativeManagedGenerationWorkerLease::observe_native_load(self, request, cancellation)
    }

    fn observe_native_load_until(
        &mut self,
        request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        NativeManagedGenerationWorkerLease::observe_native_load_until(
            self,
            request,
            cancellation,
            operation_deadline,
        )
    }

    fn observe_model_mapping(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        NativeManagedGenerationWorkerLease::observe_model_mapping(self, cancellation)
    }

    fn observe_model_mapping_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        NativeManagedGenerationWorkerLease::observe_model_mapping_until(
            self,
            cancellation,
            operation_deadline,
        )
    }

    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        NativeManagedGenerationWorkerLease::reobserve(self, cancellation)
    }

    fn reobserve_until(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Instant,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        NativeManagedGenerationWorkerLease::reobserve_until(self, cancellation, operation_deadline)
    }
}
