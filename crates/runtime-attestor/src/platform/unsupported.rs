use std::time::Instant;

use rewrite_types::CancellationToken;

use crate::{
    AttachedProcessEvidence, AttachedProcessWitnessError, AttachedProcessWitnessLimits,
    ListenerEndpoint,
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

    pub(crate) fn initial_evidence(&self) -> &AttachedProcessEvidence {
        unreachable!("an unsupported platform lease cannot be constructed")
    }

    pub(crate) fn reobserve(
        &mut self,
        _limits: AttachedProcessWitnessLimits,
        _cancellation: &CancellationToken,
        _started: Instant,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        Err(AttachedProcessWitnessError::Unsupported)
    }
}
