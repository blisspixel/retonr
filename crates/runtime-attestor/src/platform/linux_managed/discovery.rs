use std::time::Instant;

use rewrite_types::{CancellationToken, Digest};

use super::Lease;
use crate::{
    NativeLoadDiscovery, NativeLoadDiscoveryRequest, NativeLoadObservationLimits,
    NativeLoadObserverError,
};

impl Lease {
    pub(crate) fn discover_external_native_components(
        &mut self,
        request: &NativeLoadDiscoveryRequest<'_>,
        limits: NativeLoadObservationLimits,
        cancellation: &CancellationToken,
        started: Instant,
        process_evidence_digest: &Digest,
    ) -> Result<NativeLoadDiscovery, NativeLoadObserverError> {
        super::super::linux_native_load::discover(
            self.expected.outer_pid(),
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
