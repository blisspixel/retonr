use super::{
    HOST_ENVIRONMENT_SCHEMA_VERSION, HostEnvironmentV1Error, HostEnvironmentV1Input,
    HostExecutionProfileV1, MAX_HOST_ENVIRONMENT_STRING_BYTES,
};
use crate::{
    ComputeBackend, ExecutionPlacement, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
};

const MIN_PHYSICAL_CORES: u32 = 1;
const MAX_PHYSICAL_CORES: u32 = 4_096;
const MIN_LOGICAL_CORES: u32 = 1;
const MAX_LOGICAL_CORES: u32 = 8_192;
const MIN_MEMORY_MIB: u64 = 1_024;
const MAX_MEMORY_MIB: u64 = 16 * 1_024 * 1_024;
const MEMORY_GRANULARITY_MIB: u64 = 1_024;

pub(super) fn validate_input(input: &HostEnvironmentV1Input) -> Result<(), HostEnvironmentV1Error> {
    validate_platform(input)?;
    validate_execution(input)?;
    validate_os_version(&input.operating_system.version)?;
    validate_cpu_model(&input.hardware_envelope.cpu_model)?;
    validate_core_counts(input)?;
    validate_memory(input)
}

fn validate_platform(input: &HostEnvironmentV1Input) -> Result<(), HostEnvironmentV1Error> {
    if input.operating_system.family == RuntimeOperatingSystem::Linux
        && input.architecture.instruction_set == RuntimeArchitecture::X86_64
        && input.architecture.abi == RuntimeAbi::LinuxGnuLibc
    {
        Ok(())
    } else {
        Err(HostEnvironmentV1Error::UnsupportedPlatform)
    }
}

fn validate_execution(input: &HostEnvironmentV1Input) -> Result<(), HostEnvironmentV1Error> {
    let execution = input.execution_class;
    if execution.profile == HostExecutionProfileV1::ManagedLinuxNativeCpu
        && execution.compute_backend == ComputeBackend::NativeCpu
        && execution.placement == ExecutionPlacement::CpuOnly
        && matches!(
            execution.accelerator_scope,
            super::HostAcceleratorScopeV1::NotAssessedForManagedNativeCpu
        )
    {
        Ok(())
    } else {
        Err(HostEnvironmentV1Error::InvalidExecutionClass)
    }
}

fn validate_os_version(value: &str) -> Result<(), HostEnvironmentV1Error> {
    validate_length(value, HostEnvironmentV1Error::InvalidOperatingSystemVersion)?;
    if !value.is_ascii() {
        return Err(HostEnvironmentV1Error::PrivacyProhibitedCharacter);
    }
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
    {
        Ok(())
    } else {
        Err(HostEnvironmentV1Error::PrivacyProhibitedCharacter)
    }
}

fn validate_cpu_model(value: &str) -> Result<(), HostEnvironmentV1Error> {
    validate_length(value, HostEnvironmentV1Error::InvalidCpuModel)?;
    if value.trim() != value || value.contains("  ") {
        return Err(HostEnvironmentV1Error::InvalidCpuModel);
    }
    if !value.is_ascii() {
        return Err(HostEnvironmentV1Error::PrivacyProhibitedCharacter);
    }
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b' ' | b'.' | b'_' | b'+' | b'-' | b'(' | b')' | b',' | b'@'
            )
    }) {
        Ok(())
    } else {
        Err(HostEnvironmentV1Error::PrivacyProhibitedCharacter)
    }
}

fn validate_length(
    value: &str,
    error: HostEnvironmentV1Error,
) -> Result<(), HostEnvironmentV1Error> {
    if value.is_empty() || value.len() > MAX_HOST_ENVIRONMENT_STRING_BYTES {
        Err(error)
    } else {
        Ok(())
    }
}

fn validate_core_counts(input: &HostEnvironmentV1Input) -> Result<(), HostEnvironmentV1Error> {
    let hardware = &input.hardware_envelope;
    if (MIN_PHYSICAL_CORES..=MAX_PHYSICAL_CORES).contains(&hardware.physical_core_count)
        && (MIN_LOGICAL_CORES..=MAX_LOGICAL_CORES).contains(&hardware.logical_core_count)
        && hardware.logical_core_count >= hardware.physical_core_count
    {
        Ok(())
    } else {
        Err(HostEnvironmentV1Error::InvalidCoreCount)
    }
}

fn validate_memory(input: &HostEnvironmentV1Input) -> Result<(), HostEnvironmentV1Error> {
    let hardware = &input.hardware_envelope;
    if hardware.memory_rounding_granularity_mib == MEMORY_GRANULARITY_MIB
        && (MIN_MEMORY_MIB..=MAX_MEMORY_MIB).contains(&hardware.total_system_memory_mib)
        && hardware
            .total_system_memory_mib
            .is_multiple_of(MEMORY_GRANULARITY_MIB)
    {
        Ok(())
    } else {
        Err(HostEnvironmentV1Error::InvalidMemoryEnvelope)
    }
}

pub(super) fn validate_schema(schema_version: u32) -> Result<(), HostEnvironmentV1Error> {
    if schema_version == HOST_ENVIRONMENT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(HostEnvironmentV1Error::UnsupportedSchema)
    }
}
