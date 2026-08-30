use std::{fs::File, net::SocketAddr, time::Instant};

use rewrite_types::CancellationToken;

use crate::{
    ControlledBuildExecution, ControlledBuildInputFile, ControlledBuildLaunchSpec,
    IsolationEvidence, IsolationPolicy, IsolationPreparationEvidence, IsolationResult, LaunchSpec,
    ManagedLoopbackChannel, RetainedProgramBootstrapCapabilities,
    RetainedProgramBootstrapExecution, RetainedProgramBootstrapLaunchSpec,
    RetainedRuntimeInputTree,
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_bootstrap;
#[cfg(target_os = "linux")]
mod linux_bootstrap_archive;
#[cfg(target_os = "linux")]
mod linux_bootstrap_archive_regeneration;
#[cfg(target_os = "linux")]
mod linux_bootstrap_execution;
#[cfg(target_os = "linux")]
mod linux_bootstrap_protocol;
#[cfg(target_os = "linux")]
mod linux_bootstrap_root;
#[cfg(target_os = "linux")]
mod linux_bootstrap_root_native;
#[cfg(target_os = "linux")]
mod linux_bootstrap_sandbox;
#[cfg(target_os = "linux")]
mod linux_bootstrap_target;
#[cfg(target_os = "linux")]
mod linux_build;
#[cfg(target_os = "linux")]
mod linux_build_input;
#[cfg(target_os = "linux")]
mod linux_build_mount;
#[cfg(target_os = "linux")]
mod linux_build_output;
#[cfg(target_os = "linux")]
mod linux_build_protocol;
#[cfg(target_os = "linux")]
mod linux_build_response;
#[cfg(target_os = "linux")]
mod linux_build_sandbox;
#[cfg(target_os = "linux")]
mod linux_build_target;
#[cfg(target_os = "linux")]
mod linux_command;
#[cfg(target_os = "linux")]
mod linux_control;
#[cfg(target_os = "linux")]
mod linux_executable;
#[cfg(target_os = "linux")]
mod linux_fd_exec;
#[cfg(target_os = "linux")]
mod linux_helper;
#[cfg(target_os = "linux")]
mod linux_helper_bootstrap;
#[cfg(target_os = "linux")]
mod linux_helper_build;
#[cfg(target_os = "linux")]
mod linux_helper_channel;
#[cfg(target_os = "linux")]
mod linux_helper_failure;
#[cfg(target_os = "linux")]
mod linux_helper_guards;
#[cfg(target_os = "linux")]
mod linux_helper_identity;
#[cfg(target_os = "linux")]
mod linux_helper_setup;
#[cfg(target_os = "linux")]
mod linux_helper_support;
#[cfg(all(test, target_os = "linux"))]
mod linux_helper_tests;
#[cfg(target_os = "linux")]
mod linux_link;
#[cfg(target_os = "linux")]
mod linux_managed_input_mount;
#[cfg(target_os = "linux")]
mod linux_managed_input_protocol;
#[cfg(target_os = "linux")]
mod linux_managed_mount;
#[cfg(target_os = "linux")]
mod linux_managed_protocol;
#[cfg(target_os = "linux")]
mod linux_process;
#[cfg(target_os = "linux")]
mod linux_protocol;
#[cfg(target_os = "linux")]
mod linux_socket_policy;
#[cfg(target_os = "linux")]
mod linux_startup;
#[cfg(target_os = "linux")]
mod linux_target;
#[cfg(target_os = "linux")]
mod linux_validation;
#[cfg(not(target_os = "linux"))]
mod unsupported;

#[cfg(target_os = "linux")]
pub(crate) use linux::prepare_retained;
#[cfg(target_os = "linux")]
pub(crate) use linux::{Lease, Prepared, prepare};
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::prepare_retained;
#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::{Lease, Prepared, prepare};

pub(crate) fn run_helper() -> i32 {
    #[cfg(target_os = "linux")]
    {
        linux_helper::run()
    }
    #[cfg(not(target_os = "linux"))]
    {
        64
    }
}

trait PreparedPlatform {
    fn launch(
        &self,
        specification: &LaunchSpec,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease>;
    fn launch_retained(
        &self,
        specification: &LaunchSpec,
        executable: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease>;
    fn launch_retained_with_inputs(
        &self,
        specification: &LaunchSpec,
        executable: File,
        inputs: RetainedRuntimeInputTree,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease>;
    #[expect(
        clippy::too_many_arguments,
        reason = "the internal trust-boundary handoff keeps each capability explicit"
    )]
    fn run_controlled_build_retained(
        &self,
        specification: &ControlledBuildLaunchSpec,
        program: File,
        input_root: File,
        input_files: Vec<ControlledBuildInputFile>,
        output_root: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<ControlledBuildExecution>;
    fn run_retained_program_bootstrap(
        &self,
        specification: &RetainedProgramBootstrapLaunchSpec,
        capabilities: RetainedProgramBootstrapCapabilities,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<RetainedProgramBootstrapExecution>;
}

trait LeasePlatform {
    fn initial_evidence(&self) -> IsolationEvidence;
    fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<IsolationEvidence>;
    fn connect_loopback(
        &mut self,
        endpoint: SocketAddr,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<ManagedLoopbackChannel>;
    fn close(&mut self, cancellation: &CancellationToken) -> IsolationResult<()>;
}

#[cfg(feature = "test-support")]
pub(crate) fn test_support_prepared(preparation: crate::IsolationPreparationEvidence) -> Prepared {
    Prepared::test_support(preparation)
}

impl Prepared {
    pub(crate) fn launch(
        &self,
        specification: &LaunchSpec,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        PreparedPlatform::launch(
            self,
            specification,
            policy,
            cancellation,
            operation_deadline,
        )
    }

    pub(crate) fn launch_retained(
        &self,
        specification: &LaunchSpec,
        executable: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        PreparedPlatform::launch_retained(
            self,
            specification,
            executable,
            policy,
            cancellation,
            operation_deadline,
        )
    }

    pub(crate) fn launch_retained_with_inputs(
        &self,
        specification: &LaunchSpec,
        executable: File,
        inputs: RetainedRuntimeInputTree,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<Lease> {
        PreparedPlatform::launch_retained_with_inputs(
            self,
            specification,
            executable,
            inputs,
            policy,
            cancellation,
            operation_deadline,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the internal trust-boundary handoff keeps each capability explicit"
    )]
    pub(crate) fn run_controlled_build_retained(
        &self,
        specification: &ControlledBuildLaunchSpec,
        program: File,
        input_root: File,
        input_files: Vec<ControlledBuildInputFile>,
        output_root: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<ControlledBuildExecution> {
        PreparedPlatform::run_controlled_build_retained(
            self,
            specification,
            program,
            input_root,
            input_files,
            output_root,
            policy,
            cancellation,
        )
    }

    pub(crate) fn run_retained_program_bootstrap(
        &self,
        specification: &RetainedProgramBootstrapLaunchSpec,
        capabilities: RetainedProgramBootstrapCapabilities,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<RetainedProgramBootstrapExecution> {
        PreparedPlatform::run_retained_program_bootstrap(
            self,
            specification,
            capabilities,
            policy,
            cancellation,
        )
    }
}

impl Lease {
    pub(crate) fn initial_evidence(&self) -> IsolationEvidence {
        LeasePlatform::initial_evidence(self)
    }

    pub(crate) fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<IsolationEvidence> {
        LeasePlatform::reobserve(self, cancellation, operation_deadline)
    }

    pub(crate) fn connect_loopback(
        &mut self,
        endpoint: SocketAddr,
        cancellation: &CancellationToken,
        operation_deadline: Option<Instant>,
    ) -> IsolationResult<ManagedLoopbackChannel> {
        LeasePlatform::connect_loopback(self, endpoint, cancellation, operation_deadline)
    }

    pub(crate) fn close(&mut self, cancellation: &CancellationToken) -> IsolationResult<()> {
        LeasePlatform::close(self, cancellation)
    }
}

type PrepareOutput = (Prepared, IsolationPreparationEvidence);
