use std::{fs::File, net::SocketAddr};

use rewrite_types::CancellationToken;

use crate::{
    IsolationEvidence, IsolationPolicy, IsolationPreparationEvidence, IsolationResult, LaunchSpec,
    ManagedLoopbackChannel,
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod linux_command;
#[cfg(target_os = "linux")]
mod linux_control;
#[cfg(target_os = "linux")]
mod linux_helper;
#[cfg(target_os = "linux")]
mod linux_helper_channel;
#[cfg(target_os = "linux")]
mod linux_helper_identity;
#[cfg(target_os = "linux")]
mod linux_helper_setup;
#[cfg(all(test, target_os = "linux"))]
mod linux_helper_tests;
#[cfg(target_os = "linux")]
mod linux_link;
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
pub(crate) use linux::{Lease, Prepared, prepare};
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
    ) -> IsolationResult<Lease>;
    fn launch_retained(
        &self,
        specification: &LaunchSpec,
        executable: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Lease>;
}

trait LeasePlatform {
    fn initial_evidence(&self) -> IsolationEvidence;
    fn reobserve(&mut self, cancellation: &CancellationToken)
    -> IsolationResult<IsolationEvidence>;
    fn connect_loopback(
        &mut self,
        endpoint: SocketAddr,
        cancellation: &CancellationToken,
    ) -> IsolationResult<ManagedLoopbackChannel>;
    fn close(&mut self, cancellation: &CancellationToken) -> IsolationResult<()>;
}

impl Prepared {
    pub(crate) fn launch(
        &self,
        specification: &LaunchSpec,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Lease> {
        PreparedPlatform::launch(self, specification, policy, cancellation)
    }

    pub(crate) fn launch_retained(
        &self,
        specification: &LaunchSpec,
        executable: File,
        policy: IsolationPolicy,
        cancellation: &CancellationToken,
    ) -> IsolationResult<Lease> {
        PreparedPlatform::launch_retained(self, specification, executable, policy, cancellation)
    }
}

impl Lease {
    pub(crate) fn initial_evidence(&self) -> IsolationEvidence {
        LeasePlatform::initial_evidence(self)
    }

    pub(crate) fn reobserve(
        &mut self,
        cancellation: &CancellationToken,
    ) -> IsolationResult<IsolationEvidence> {
        LeasePlatform::reobserve(self, cancellation)
    }

    pub(crate) fn connect_loopback(
        &mut self,
        endpoint: SocketAddr,
        cancellation: &CancellationToken,
    ) -> IsolationResult<ManagedLoopbackChannel> {
        LeasePlatform::connect_loopback(self, endpoint, cancellation)
    }

    pub(crate) fn close(&mut self, cancellation: &CancellationToken) -> IsolationResult<()> {
        LeasePlatform::close(self, cancellation)
    }
}

type PrepareOutput = (Prepared, IsolationPreparationEvidence);
