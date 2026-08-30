use super::{
    HostAcceleratorScopeV1, HostArchitectureDigestV1, HostArchitectureV1, HostEnvironmentId,
    HostEnvironmentV1, HostExecutionClassDigestV1, HostExecutionClassV1, HostExecutionProfileV1,
    HostHardwareEnvelopeDigestV1, HostHardwareEnvelopeV1, HostOperatingSystemDigestV1,
    HostOperatingSystemV1, ObserverBinaryAssertionModeV1, canonical_json,
};
use crate::{
    ComputeBackend, ExecutionPlacement, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
};

/// One inseparable typed view of the complete host identity and its projections.
///
/// The private fields prevent callers from constructing a transposed digest set.
/// Projection types additionally make cross-component substitution a type error.
///
/// ```compile_fail
/// use rewrite_model::{HostArchitectureDigestV1, HostOperatingSystemDigestV1};
///
/// fn require_operating_system(_: &HostOperatingSystemDigestV1) {}
/// fn cannot_transpose(value: &HostArchitectureDigestV1) {
///     require_operating_system(value);
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostEnvironmentDigestSetV1 {
    host_environment_id: HostEnvironmentId,
    operating_system_digest: HostOperatingSystemDigestV1,
    architecture_digest: HostArchitectureDigestV1,
    execution_class_digest: HostExecutionClassDigestV1,
    hardware_envelope_digest: HostHardwareEnvelopeDigestV1,
}

impl HostEnvironmentDigestSetV1 {
    /// Returns the complete canonical host identity.
    #[must_use]
    pub const fn host_environment_id(&self) -> &HostEnvironmentId {
        &self.host_environment_id
    }

    /// Returns the operating-system projection digest.
    #[must_use]
    pub const fn operating_system_digest(&self) -> &HostOperatingSystemDigestV1 {
        &self.operating_system_digest
    }

    /// Returns the architecture projection digest.
    #[must_use]
    pub const fn architecture_digest(&self) -> &HostArchitectureDigestV1 {
        &self.architecture_digest
    }

    /// Returns the execution-class projection digest.
    #[must_use]
    pub const fn execution_class_digest(&self) -> &HostExecutionClassDigestV1 {
        &self.execution_class_digest
    }

    /// Returns the hardware-envelope projection digest.
    #[must_use]
    pub const fn hardware_envelope_digest(&self) -> &HostHardwareEnvelopeDigestV1 {
        &self.hardware_envelope_digest
    }
}

impl HostEnvironmentV1 {
    /// Returns one typed bundle containing the full identity and all projections.
    #[must_use]
    pub fn digest_set(&self) -> HostEnvironmentDigestSetV1 {
        HostEnvironmentDigestSetV1 {
            host_environment_id: self.id.clone(),
            operating_system_digest: self.operating_system.digest.clone(),
            architecture_digest: self.architecture.digest.clone(),
            execution_class_digest: self.execution_class.digest.clone(),
            hardware_envelope_digest: self.hardware_envelope.digest.clone(),
        }
    }
}

impl HostOperatingSystemV1 {
    /// Returns the projection schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact operating-system family.
    #[must_use]
    pub const fn family(&self) -> RuntimeOperatingSystem {
        self.family
    }
    /// Returns the normalized Linux kernel release.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
    /// Returns the typed projection digest.
    #[must_use]
    pub const fn operating_system_digest(&self) -> &HostOperatingSystemDigestV1 {
        &self.digest
    }
    /// Returns the exact compact canonical projection JSON.
    #[must_use]
    pub fn to_canonical_json_bytes(&self) -> Vec<u8> {
        canonical_json(self)
    }
}

impl HostArchitectureV1 {
    /// Returns the projection schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the exact processor instruction set.
    #[must_use]
    pub const fn instruction_set(&self) -> RuntimeArchitecture {
        self.instruction_set
    }
    /// Returns the exact application binary interface.
    #[must_use]
    pub const fn abi(&self) -> RuntimeAbi {
        self.abi
    }
    /// Returns the typed projection digest.
    #[must_use]
    pub const fn architecture_digest(&self) -> &HostArchitectureDigestV1 {
        &self.digest
    }
    /// Returns the exact compact canonical projection JSON.
    #[must_use]
    pub fn to_canonical_json_bytes(&self) -> Vec<u8> {
        canonical_json(self)
    }
}

impl HostExecutionClassV1 {
    /// Returns the projection schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the managed execution profile.
    #[must_use]
    pub const fn profile(&self) -> HostExecutionProfileV1 {
        self.profile
    }
    /// Returns the exact compute backend.
    #[must_use]
    pub const fn compute_backend(&self) -> ComputeBackend {
        self.compute_backend
    }
    /// Returns the exact compute placement.
    #[must_use]
    pub const fn placement(&self) -> ExecutionPlacement {
        self.placement
    }
    /// Returns whether the observer binary included debug assertions.
    #[must_use]
    pub const fn observer_binary_assertion_mode(&self) -> ObserverBinaryAssertionModeV1 {
        self.observer_binary_assertion_mode
    }
    /// Returns the deliberately bounded accelerator-observation scope.
    #[must_use]
    pub const fn accelerator_scope(&self) -> HostAcceleratorScopeV1 {
        self.accelerator_scope
    }
    /// Returns the typed projection digest.
    #[must_use]
    pub const fn execution_class_digest(&self) -> &HostExecutionClassDigestV1 {
        &self.digest
    }
    /// Returns the exact compact canonical projection JSON.
    #[must_use]
    pub fn to_canonical_json_bytes(&self) -> Vec<u8> {
        canonical_json(self)
    }
}

impl HostHardwareEnvelopeV1 {
    /// Returns the projection schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the normalized CPU model class.
    #[must_use]
    pub fn cpu_model(&self) -> &str {
        &self.cpu_model
    }
    /// Returns the physical CPU core count.
    #[must_use]
    pub const fn physical_core_count(&self) -> u32 {
        self.physical_core_count
    }
    /// Returns the logical CPU core count.
    #[must_use]
    pub const fn logical_core_count(&self) -> u32 {
        self.logical_core_count
    }
    /// Returns rounded total system memory in MiB.
    #[must_use]
    pub const fn total_system_memory_mib(&self) -> u64 {
        self.total_system_memory_mib
    }
    /// Returns the fixed memory rounding granularity in MiB.
    #[must_use]
    pub const fn memory_rounding_granularity_mib(&self) -> u64 {
        self.memory_rounding_granularity_mib
    }
    /// Returns the typed projection digest.
    #[must_use]
    pub const fn hardware_envelope_digest(&self) -> &HostHardwareEnvelopeDigestV1 {
        &self.digest
    }
    /// Returns the exact compact canonical projection JSON.
    #[must_use]
    pub fn to_canonical_json_bytes(&self) -> Vec<u8> {
        canonical_json(self)
    }
}
