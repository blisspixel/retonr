use std::{fs::File, net::SocketAddr, path::Path};

use rewrite_types::CancellationToken;

use super::{LeasePlatform, PrepareOutput, PreparedPlatform};
use crate::{
    IsolationError, IsolationEvidence, IsolationPolicy, IsolationResult, LaunchSpec,
    ManagedLoopbackChannel,
};

#[derive(Debug)]
pub(crate) struct Prepared;

#[derive(Debug)]
pub(crate) struct Lease;

pub(crate) fn prepare(
    _helper_executable: &Path,
    _policy: IsolationPolicy,
    _cancellation: &CancellationToken,
) -> IsolationResult<PrepareOutput> {
    Err(IsolationError::UnsupportedPlatform)
}

impl PreparedPlatform for Prepared {
    fn launch(
        &self,
        _specification: &LaunchSpec,
        _policy: IsolationPolicy,
        _cancellation: &CancellationToken,
    ) -> IsolationResult<Lease> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn launch_retained(
        &self,
        _specification: &LaunchSpec,
        _executable: File,
        _policy: IsolationPolicy,
        _cancellation: &CancellationToken,
    ) -> IsolationResult<Lease> {
        Err(IsolationError::UnsupportedPlatform)
    }
}

impl LeasePlatform for Lease {
    fn initial_evidence(&self) -> IsolationEvidence {
        unreachable!("an unsupported-platform lease cannot be constructed")
    }

    fn reobserve(
        &mut self,
        _cancellation: &CancellationToken,
    ) -> IsolationResult<IsolationEvidence> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn connect_loopback(
        &mut self,
        _endpoint: SocketAddr,
        _cancellation: &CancellationToken,
    ) -> IsolationResult<ManagedLoopbackChannel> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn close(&mut self, _cancellation: &CancellationToken) -> IsolationResult<()> {
        Err(IsolationError::UnsupportedPlatform)
    }
}
