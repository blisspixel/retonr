use std::{fs::File, net::SocketAddr, path::Path, time::Instant};

use rewrite_types::{CancellationToken, Digest};

use super::{LeasePlatform, PrepareOutput, PreparedPlatform};
use crate::{
    ControlledBuildExecution, ControlledBuildInputFile, ControlledBuildLaunchSpec, IsolationError,
    IsolationEvidence, IsolationPolicy, IsolationResult, LaunchSpec, ManagedLoopbackChannel,
    RetainedProgramBootstrapCapabilities, RetainedProgramBootstrapExecution,
    RetainedProgramBootstrapLaunchSpec, RetainedRuntimeInputTree,
};

#[derive(Debug)]
pub(crate) struct Prepared;

#[derive(Debug)]
pub(crate) struct Lease;

#[cfg(feature = "test-support")]
impl Prepared {
    pub(crate) fn test_support(_preparation: crate::IsolationPreparationEvidence) -> Self {
        Self
    }
}

pub(crate) fn prepare(
    _helper_executable: &Path,
    _expected_digest: &Digest,
    _expected_bytes: u64,
    _policy: IsolationPolicy,
    _cancellation: &CancellationToken,
) -> IsolationResult<PrepareOutput> {
    Err(IsolationError::UnsupportedPlatform)
}

pub(crate) fn prepare_retained(
    _helper_executable: File,
    _expected_digest: &Digest,
    _expected_bytes: u64,
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
        _operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn launch_retained(
        &self,
        _specification: &LaunchSpec,
        _executable: File,
        _policy: IsolationPolicy,
        _cancellation: &CancellationToken,
        _operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn launch_retained_with_inputs(
        &self,
        _specification: &LaunchSpec,
        _executable: File,
        _inputs: RetainedRuntimeInputTree,
        _policy: IsolationPolicy,
        _cancellation: &CancellationToken,
        _operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn run_controlled_build_retained(
        &self,
        _specification: &ControlledBuildLaunchSpec,
        _program: File,
        _input_root: File,
        _input_files: Vec<ControlledBuildInputFile>,
        _output_root: File,
        _policy: IsolationPolicy,
        _cancellation: &CancellationToken,
    ) -> IsolationResult<ControlledBuildExecution> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn run_retained_program_bootstrap(
        &self,
        _specification: &RetainedProgramBootstrapLaunchSpec,
        _capabilities: RetainedProgramBootstrapCapabilities,
        _policy: IsolationPolicy,
        _cancellation: &CancellationToken,
    ) -> IsolationResult<RetainedProgramBootstrapExecution> {
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
        _operation_deadline: Option<Instant>,
    ) -> IsolationResult<IsolationEvidence> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn connect_loopback(
        &mut self,
        _endpoint: SocketAddr,
        _cancellation: &CancellationToken,
        _operation_deadline: Option<Instant>,
    ) -> IsolationResult<ManagedLoopbackChannel> {
        Err(IsolationError::UnsupportedPlatform)
    }

    fn close(&mut self, _cancellation: &CancellationToken) -> IsolationResult<()> {
        Err(IsolationError::UnsupportedPlatform)
    }
}
