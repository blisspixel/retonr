use std::time::Instant;

use rewrite_types::CancellationToken;

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
}
