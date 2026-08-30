use std::{fs::File, time::Instant};

use rewrite_model::{NativeLoadEvidenceClass, RuntimeOperatingSystem};
use rewrite_types::{CancellationToken, Digest};
use rustix::fd::OwnedFd;

use super::{CONTRACT_ID, stable_components};
use crate::{
    NativeLoadDiscovery, NativeLoadDiscoveryRequest, NativeLoadObservationLimits,
    NativeLoadObserverError, platform::native_load_common::finish_discovery,
};

impl super::super::linux::Lease {
    pub(crate) fn discover_external_native_components(
        &mut self,
        request: &NativeLoadDiscoveryRequest<'_>,
        limits: NativeLoadObservationLimits,
        cancellation: &CancellationToken,
        started: Instant,
        process_evidence_digest: &Digest,
    ) -> Result<NativeLoadDiscovery, NativeLoadObserverError> {
        discover(
            self.owner.pid,
            &self.pidfd,
            &self.entrypoint,
            request,
            limits,
            cancellation,
            started,
            process_evidence_digest,
        )
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "discovery makes every retained capability and resource boundary explicit"
)]
pub(in crate::platform) fn discover(
    pid: u32,
    pidfd: &OwnedFd,
    entrypoint: &File,
    request: &NativeLoadDiscoveryRequest<'_>,
    limits: NativeLoadObservationLimits,
    cancellation: &CancellationToken,
    started: Instant,
    process_evidence_digest: &Digest,
) -> Result<NativeLoadDiscovery, NativeLoadObserverError> {
    if request.package.target().operating_system() != RuntimeOperatingSystem::Linux {
        return Err(NativeLoadObserverError::InvalidRequest);
    }
    let components = stable_components(
        pid,
        pidfd,
        entrypoint,
        request.package,
        request.retained_package_members,
        limits,
        cancellation,
        started,
    )?;
    finish_discovery(
        request.package,
        NativeLoadEvidenceClass::LinuxProcMapFiles,
        CONTRACT_ID,
        process_evidence_digest,
        components,
    )
}
