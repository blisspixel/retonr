use serde::Deserialize;

use super::{
    HostAcceleratorScopeV1, HostArchitectureV1Input, HostEnvironmentV1Error,
    HostEnvironmentV1Input, HostExecutionClassV1Input, HostExecutionProfileV1,
    HostHardwareEnvelopeV1Input, HostOperatingSystemV1Input, ObserverBinaryAssertionModeV1,
};
use crate::{
    ComputeBackend, ExecutionPlacement, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HostEnvironmentWire {
    schema_version: u32,
    operating_system: OperatingSystemWire,
    architecture: ArchitectureWire,
    execution_class: ExecutionClassWire,
    hardware_envelope: HardwareEnvelopeWire,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatingSystemWire {
    schema_version: u32,
    family: RuntimeOperatingSystem,
    version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchitectureWire {
    schema_version: u32,
    instruction_set: RuntimeArchitecture,
    abi: RuntimeAbi,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionClassWire {
    schema_version: u32,
    profile: HostExecutionProfileV1,
    compute_backend: ComputeBackend,
    placement: ExecutionPlacement,
    observer_binary_assertion_mode: ObserverBinaryAssertionModeV1,
    accelerator_scope: HostAcceleratorScopeV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HardwareEnvelopeWire {
    schema_version: u32,
    cpu_model: String,
    physical_core_count: u32,
    logical_core_count: u32,
    total_system_memory_mib: u64,
    memory_rounding_granularity_mib: u64,
}

impl HostEnvironmentWire {
    pub(super) fn validate_schema(&self) -> Result<(), HostEnvironmentV1Error> {
        for version in [
            self.schema_version,
            self.operating_system.schema_version,
            self.architecture.schema_version,
            self.execution_class.schema_version,
            self.hardware_envelope.schema_version,
        ] {
            super::validation::validate_schema(version)?;
        }
        Ok(())
    }

    pub(super) fn into_input(self) -> HostEnvironmentV1Input {
        HostEnvironmentV1Input {
            operating_system: HostOperatingSystemV1Input {
                family: self.operating_system.family,
                version: self.operating_system.version,
            },
            architecture: HostArchitectureV1Input {
                instruction_set: self.architecture.instruction_set,
                abi: self.architecture.abi,
            },
            execution_class: HostExecutionClassV1Input {
                profile: self.execution_class.profile,
                compute_backend: self.execution_class.compute_backend,
                placement: self.execution_class.placement,
                observer_binary_assertion_mode: self.execution_class.observer_binary_assertion_mode,
                accelerator_scope: self.execution_class.accelerator_scope,
            },
            hardware_envelope: HostHardwareEnvelopeV1Input {
                cpu_model: self.hardware_envelope.cpu_model,
                physical_core_count: self.hardware_envelope.physical_core_count,
                logical_core_count: self.hardware_envelope.logical_core_count,
                total_system_memory_mib: self.hardware_envelope.total_system_memory_mib,
                memory_rounding_granularity_mib: self
                    .hardware_envelope
                    .memory_rounding_granularity_mib,
            },
        }
    }
}
