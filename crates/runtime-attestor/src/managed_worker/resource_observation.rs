use rewrite_types::Digest;

/// Schema version for one retained worker resource observation.
pub const MANAGED_GENERATION_WORKER_RESOURCE_OBSERVATION_SCHEMA_VERSION: u32 = 1;

/// Opaque, noncloneable observation of one retained worker's peak resident set.
///
/// The byte count is the Linux kernel's process-scoped `VmHWM` value converted
/// from kibibytes. It is observational evidence, not a formal memory guarantee.
/// It does not establish current working set, allocation provenance, model-page
/// residency, accelerator memory, or cgroup-wide consumption.
///
/// This type cannot be reconstructed from serialized data.
///
/// ```compile_fail
/// use rewrite_runtime_attestor::ManagedGenerationWorkerResourceObservation;
///
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ManagedGenerationWorkerResourceObservation>();
/// ```
///
/// ```compile_fail
/// use rewrite_runtime_attestor::ManagedGenerationWorkerResourceObservation;
///
/// fn requires_serialize<T: serde::Serialize>() {}
/// requires_serialize::<ManagedGenerationWorkerResourceObservation>();
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct ManagedGenerationWorkerResourceObservation {
    schema_version: u32,
    worker_evidence_digest: Digest,
    high_water_resident_bytes: u64,
    observation_digest: Digest,
}

impl ManagedGenerationWorkerResourceObservation {
    #[cfg(any(target_os = "linux", test, feature = "test-support"))]
    pub(crate) fn new(worker_evidence_digest: Digest, high_water_resident_bytes: u64) -> Self {
        let mut material = Vec::with_capacity(128);
        material.extend_from_slice(b"retonr:managed-generation-worker-resource:v1\0");
        material.extend_from_slice(worker_evidence_digest.as_str().as_bytes());
        material.extend_from_slice(&high_water_resident_bytes.to_be_bytes());
        let observation_digest = Digest::sha256(&material);
        Self {
            schema_version: MANAGED_GENERATION_WORKER_RESOURCE_OBSERVATION_SCHEMA_VERSION,
            worker_evidence_digest,
            high_water_resident_bytes,
            observation_digest,
        }
    }

    /// Constructs inert worker resource facts for downstream contract tests.
    ///
    /// This helper is absent unless the `test-support` feature is enabled. It
    /// does not create, retain, or imitate a worker capability.
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn for_test(
        worker: &super::ManagedGenerationWorkerEvidence,
        high_water_resident_bytes: u64,
    ) -> Self {
        Self::new(worker.evidence_digest().clone(), high_water_resident_bytes)
    }

    /// Returns the observation schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact retained worker evidence identity.
    #[must_use]
    pub const fn worker_evidence_digest(&self) -> &Digest {
        &self.worker_evidence_digest
    }

    /// Returns the kernel-reported peak resident set in bytes.
    #[must_use]
    pub const fn high_water_resident_bytes(&self) -> u64 {
        self.high_water_resident_bytes
    }

    /// Returns the digest binding the worker identity and byte count.
    #[must_use]
    pub const fn observation_digest(&self) -> &Digest {
        &self.observation_digest
    }
}

#[cfg(any(target_os = "linux", test))]
pub(crate) fn parse_high_water_resident_bytes(
    text: &str,
) -> Result<u64, crate::ManagedGenerationWorkerError> {
    use crate::ManagedGenerationWorkerError::{PlatformObservationFailed, ResourceLimit};

    let mut raw = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("VmHWM:") {
            if raw.replace(value).is_some() {
                return Err(PlatformObservationFailed);
            }
        } else if line.trim_start().starts_with("VmHWM") {
            return Err(PlatformObservationFailed);
        }
    }
    let raw = raw.ok_or(PlatformObservationFailed)?;
    let mut fields = raw.split_ascii_whitespace();
    let kibibytes = fields
        .next()
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or(PlatformObservationFailed)?
        .parse::<u64>()
        .map_err(|_error| PlatformObservationFailed)?;
    if fields.next() != Some("kB") || fields.next().is_some() {
        return Err(PlatformObservationFailed);
    }
    kibibytes.checked_mul(1024).ok_or(ResourceLimit)
}

#[cfg(test)]
mod tests {
    use rewrite_types::Digest;

    use super::{
        MANAGED_GENERATION_WORKER_RESOURCE_OBSERVATION_SCHEMA_VERSION,
        ManagedGenerationWorkerResourceObservation, parse_high_water_resident_bytes,
    };
    use crate::ManagedGenerationWorkerError;

    #[test]
    fn observation_binds_worker_and_big_endian_byte_count() {
        let worker = Digest::sha256(b"worker");
        let observed = ManagedGenerationWorkerResourceObservation::new(worker.clone(), 65_536);
        let mut material = b"retonr:managed-generation-worker-resource:v1\0".to_vec();
        material.extend_from_slice(worker.as_str().as_bytes());
        material.extend_from_slice(&65_536_u64.to_be_bytes());

        assert_eq!(
            observed.schema_version(),
            MANAGED_GENERATION_WORKER_RESOURCE_OBSERVATION_SCHEMA_VERSION
        );
        assert_eq!(observed.worker_evidence_digest(), &worker);
        assert_eq!(observed.high_water_resident_bytes(), 65_536);
        assert_eq!(observed.observation_digest(), &Digest::sha256(&material));
        assert_ne!(
            observed.observation_digest(),
            ManagedGenerationWorkerResourceObservation::new(worker, 65_537).observation_digest()
        );
    }

    #[test]
    fn high_water_parser_requires_one_exact_kernel_row() {
        assert_eq!(
            parse_high_water_resident_bytes("Name:\tworker\nVmHWM:\t12345 kB\n"),
            Ok(12_641_280)
        );
        assert_eq!(parse_high_water_resident_bytes("VmHWM:\t0 kB\n"), Ok(0));
        for malformed in [
            "Name:\tworker\n",
            "VmHWM:\t1 kB\nVmHWM:\t2 kB\n",
            "VmHWM:\t-1 kB\n",
            "VmHWM:\t+1 kB\n",
            "VmHWM:\t1 KB\n",
            "VmHWM:\t1 kB trailing\n",
            "VmHWM:\tnot-a-number kB\n",
            " VmHWM:\t1 kB\nVmHWM:\t2 kB\n",
            "VmHWM :\t1 kB\nVmHWM:\t2 kB\n",
        ] {
            assert_eq!(
                parse_high_water_resident_bytes(malformed),
                Err(ManagedGenerationWorkerError::PlatformObservationFailed)
            );
        }
        assert_eq!(
            parse_high_water_resident_bytes(&format!("VmHWM:\t{} kB\n", u64::MAX)),
            Err(ManagedGenerationWorkerError::ResourceLimit)
        );
    }
}
