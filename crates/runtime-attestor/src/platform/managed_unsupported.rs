use std::{fs::File, time::Instant};

use rewrite_types::{CancellationToken, Digest};

use crate::{
    AttachedProcessEvidence, AttachedProcessWitnessError, AttachedProcessWitnessLimits,
    ListenerEndpoint, ManagedLinuxProcessExpectation, NativeLoadObservationLimits,
    NativeLoadObservationRequest, NativeLoadObserverError, RetainedTcpConnection,
    RetainedTcpConnectionEvidence,
};

pub(crate) struct Lease;

impl Lease {
    pub(crate) fn attach(
        _endpoint: ListenerEndpoint,
        _diagnostics: File,
        _expected: ManagedLinuxProcessExpectation,
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
        unreachable!("an unsupported managed Linux lease cannot be constructed")
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
        _request: &NativeLoadObservationRequest<'_>,
        _limits: NativeLoadObservationLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
        _process_evidence_digest: &Digest,
    ) -> Result<rewrite_model::NativeLoadObservation, NativeLoadObserverError> {
        Err(NativeLoadObserverError::Unsupported)
    }
}
