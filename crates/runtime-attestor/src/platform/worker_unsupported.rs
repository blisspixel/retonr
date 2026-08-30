use std::time::Instant;

use rewrite_types::CancellationToken;

use crate::{
    ManagedGenerationWorkerError, ManagedGenerationWorkerEvidence, ManagedGenerationWorkerLimits,
    ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerNativeLoadEvidence,
    ManagedGenerationWorkerNativeLoadRequest, ManagedGenerationWorkerResourceObservation,
};

pub(crate) struct Lease;

impl Lease {
    #[expect(
        clippy::unused_self,
        reason = "an unsupported worker lease cannot be constructed"
    )]
    pub(crate) fn evidence(&self) -> &ManagedGenerationWorkerEvidence {
        unreachable!("an unsupported generation-worker lease cannot be constructed")
    }

    #[expect(
        clippy::unused_self,
        reason = "the platform lease facade has one method shape on every target"
    )]
    pub(crate) fn observe_native_load(
        &mut self,
        _request: &ManagedGenerationWorkerNativeLoadRequest<'_>,
        _limits: ManagedGenerationWorkerLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
    ) -> Result<ManagedGenerationWorkerNativeLoadEvidence, ManagedGenerationWorkerError> {
        Err(ManagedGenerationWorkerError::Unsupported)
    }

    #[expect(
        clippy::unused_self,
        reason = "the platform lease facade has one method shape on every target"
    )]
    pub(crate) fn observe_model_mapping(
        &mut self,
        _limits: ManagedGenerationWorkerLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
    ) -> Result<ManagedGenerationWorkerModelMappingEvidence, ManagedGenerationWorkerError> {
        Err(ManagedGenerationWorkerError::Unsupported)
    }

    #[expect(
        clippy::unused_self,
        reason = "the platform lease facade has one method shape on every target"
    )]
    pub(crate) fn observe_resource(
        &mut self,
        _limits: ManagedGenerationWorkerLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
    ) -> Result<ManagedGenerationWorkerResourceObservation, ManagedGenerationWorkerError> {
        Err(ManagedGenerationWorkerError::Unsupported)
    }

    #[expect(
        clippy::unused_self,
        reason = "the platform lease facade has one method shape on every target"
    )]
    pub(crate) fn reobserve(
        &mut self,
        _limits: ManagedGenerationWorkerLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
    ) -> Result<ManagedGenerationWorkerEvidence, ManagedGenerationWorkerError> {
        Err(ManagedGenerationWorkerError::Unsupported)
    }
}
