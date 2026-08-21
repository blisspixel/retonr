use std::time::Instant;

use rewrite_types::CancellationToken;
use rewrite_types::Digest;

use crate::{
    AttachedProcessEvidence, AttachedProcessWitnessError, AttachedProcessWitnessLimits,
    ListenerEndpoint, RetainedTcpConnection, RetainedTcpConnectionEvidence,
};

pub(crate) struct Lease;

impl Lease {
    pub(crate) fn attach(
        _endpoint: ListenerEndpoint,
        _limits: AttachedProcessWitnessLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
    ) -> Result<Self, AttachedProcessWitnessError> {
        Err(AttachedProcessWitnessError::Unsupported)
    }

    #[expect(
        clippy::unused_self,
        reason = "the platform lease facade has one method shape on every target"
    )]
    pub(crate) fn initial_evidence(&self) -> &AttachedProcessEvidence {
        unreachable!("an unsupported macOS lease cannot be constructed")
    }

    #[expect(
        clippy::unused_self,
        reason = "the platform lease facade has one method shape on every target"
    )]
    pub(crate) fn reobserve(
        &mut self,
        _limits: AttachedProcessWitnessLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        Err(AttachedProcessWitnessError::Unsupported)
    }

    #[expect(
        clippy::unused_self,
        reason = "the platform lease facade has one method shape on every target"
    )]
    pub(crate) fn observe_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _limits: AttachedProcessWitnessLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        Err(AttachedProcessWitnessError::Unsupported)
    }

    #[expect(
        clippy::unused_self,
        reason = "the platform lease facade has one method shape on every target"
    )]
    pub(crate) fn observe_native_load(
        &mut self,
        _request: &crate::NativeLoadObservationRequest<'_>,
        _limits: crate::NativeLoadObservationLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
        _process_evidence_digest: &Digest,
    ) -> Result<rewrite_model::NativeLoadObservation, crate::NativeLoadObserverError> {
        Err(crate::NativeLoadObserverError::Unsupported)
    }
}
