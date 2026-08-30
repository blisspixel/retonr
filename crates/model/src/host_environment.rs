//! Canonical privacy-bounded host capability evidence.

use std::fmt;

use rewrite_types::Digest;
use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use crate::{
    ComputeBackend, ExecutionPlacement, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
};

mod accessors;
mod validation;
mod wire;

pub use accessors::HostEnvironmentDigestSetV1;

/// Current canonical host-environment schema.
pub const HOST_ENVIRONMENT_SCHEMA_VERSION: u32 = 1;
/// Maximum JSON bytes accepted by the host-environment decoder.
pub const MAX_HOST_ENVIRONMENT_JSON_BYTES: usize = 16 * 1_024;
/// Maximum canonical JSON bytes accepted for the complete host record.
pub const MAX_HOST_ENVIRONMENT_CANONICAL_BYTES: usize = 4 * 1_024;
/// Maximum canonical JSON bytes accepted for one typed projection.
pub const MAX_HOST_ENVIRONMENT_PROJECTION_CANONICAL_BYTES: usize = 1_024;
/// Maximum bytes accepted for one normalized observed string.
pub const MAX_HOST_ENVIRONMENT_STRING_BYTES: usize = 128;

/// Domain for the complete host-environment identity.
pub const HOST_ENVIRONMENT_ID_DOMAIN: &[u8] = b"retonr:host-environment:v1\0";
/// Domain for the operating-system projection digest.
pub const HOST_ENVIRONMENT_OPERATING_SYSTEM_DIGEST_DOMAIN: &[u8] =
    b"retonr:host-environment-operating-system:v1\0";
/// Domain for the architecture projection digest.
pub const HOST_ENVIRONMENT_ARCHITECTURE_DIGEST_DOMAIN: &[u8] =
    b"retonr:host-environment-architecture:v1\0";
/// Domain for the execution-class projection digest.
pub const HOST_ENVIRONMENT_EXECUTION_CLASS_DIGEST_DOMAIN: &[u8] =
    b"retonr:host-environment-execution-class:v1\0";
/// Domain for the hardware-envelope projection digest.
pub const HOST_ENVIRONMENT_HARDWARE_ENVELOPE_DIGEST_DOMAIN: &[u8] =
    b"retonr:host-environment-hardware-envelope:v1\0";

macro_rules! typed_digest {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, serde::Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
        #[serde(transparent)]
        pub struct $name(Digest);

        impl $name {
            /// Returns the exact domain-separated digest.
            #[must_use]
            pub const fn digest(&self) -> &Digest {
                &self.0
            }
        }
    };
}

typed_digest!(
    HostEnvironmentId,
    "Content-derived identity of one complete canonical host environment."
);
typed_digest!(
    HostOperatingSystemDigestV1,
    "Typed digest of the canonical operating-system projection."
);
typed_digest!(
    HostArchitectureDigestV1,
    "Typed digest of the canonical architecture projection."
);
typed_digest!(
    HostExecutionClassDigestV1,
    "Typed digest of the canonical execution-class projection."
);
typed_digest!(
    HostHardwareEnvelopeDigestV1,
    "Typed digest of the canonical privacy-bounded hardware projection."
);

/// Closed managed native CPU execution profile.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostExecutionProfileV1 {
    /// Managed Linux process execution through the native CPU backend.
    ManagedLinuxNativeCpu,
}

/// Whether the observer binary was compiled with debug assertions.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObserverBinaryAssertionModeV1 {
    /// Debug assertions were compiled into the observer binary.
    Enabled,
    /// Debug assertions were not compiled into the observer binary.
    Disabled,
}

/// Closed accelerator-observation scope for the managed native CPU profile.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostAcceleratorScopeV1 {
    /// Accelerator presence and capability were deliberately not assessed.
    NotAssessedForManagedNativeCpu,
}

/// Caller-supplied operating-system observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostOperatingSystemV1Input {
    /// Closed operating-system family.
    pub family: RuntimeOperatingSystem,
    /// Normalized Linux kernel release.
    pub version: String,
}

/// Caller-supplied architecture observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostArchitectureV1Input {
    /// Closed processor instruction set.
    pub instruction_set: RuntimeArchitecture,
    /// Closed application binary interface.
    pub abi: RuntimeAbi,
}

/// Caller-supplied execution-class observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostExecutionClassV1Input {
    /// Reviewed managed execution profile.
    pub profile: HostExecutionProfileV1,
    /// Effective compute backend.
    pub compute_backend: ComputeBackend,
    /// Effective compute placement.
    pub placement: ExecutionPlacement,
    /// Observer binary debug-assertion mode.
    pub observer_binary_assertion_mode: ObserverBinaryAssertionModeV1,
    /// Deliberately bounded accelerator-observation scope.
    pub accelerator_scope: HostAcceleratorScopeV1,
}

/// Caller-supplied privacy-bounded hardware observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostHardwareEnvelopeV1Input {
    /// Normalized CPU model class without machine identifiers.
    pub cpu_model: String,
    /// Observed physical CPU core count.
    pub physical_core_count: u32,
    /// Observed logical CPU core count.
    pub logical_core_count: u32,
    /// Total system memory rounded to the declared MiB granularity.
    pub total_system_memory_mib: u64,
    /// Fixed V1 memory rounding granularity in MiB.
    pub memory_rounding_granularity_mib: u64,
}

/// Complete caller-supplied host environment observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostEnvironmentV1Input {
    /// Operating-system observation.
    pub operating_system: HostOperatingSystemV1Input,
    /// Architecture observation.
    pub architecture: HostArchitectureV1Input,
    /// Execution-class observation.
    pub execution_class: HostExecutionClassV1Input,
    /// Privacy-bounded hardware observation.
    pub hardware_envelope: HostHardwareEnvelopeV1Input,
}

/// Canonical operating-system projection.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostOperatingSystemV1 {
    schema_version: u32,
    family: RuntimeOperatingSystem,
    version: String,
    #[serde(skip)]
    digest: HostOperatingSystemDigestV1,
}

/// Canonical architecture projection.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostArchitectureV1 {
    schema_version: u32,
    instruction_set: RuntimeArchitecture,
    abi: RuntimeAbi,
    #[serde(skip)]
    digest: HostArchitectureDigestV1,
}

/// Canonical execution-class projection.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostExecutionClassV1 {
    schema_version: u32,
    profile: HostExecutionProfileV1,
    compute_backend: ComputeBackend,
    placement: ExecutionPlacement,
    observer_binary_assertion_mode: ObserverBinaryAssertionModeV1,
    accelerator_scope: HostAcceleratorScopeV1,
    #[serde(skip)]
    digest: HostExecutionClassDigestV1,
}

/// Canonical privacy-bounded hardware projection.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostHardwareEnvelopeV1 {
    schema_version: u32,
    cpu_model: String,
    physical_core_count: u32,
    logical_core_count: u32,
    total_system_memory_mib: u64,
    memory_rounding_granularity_mib: u64,
    #[serde(skip)]
    digest: HostHardwareEnvelopeDigestV1,
}

/// Canonical content-addressed host capability environment.
///
/// The record contains capability facts only. It grants no runtime, generation,
/// qualification, or activation authority.
#[derive(Clone, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HostEnvironmentV1 {
    schema_version: u32,
    operating_system: HostOperatingSystemV1,
    architecture: HostArchitectureV1,
    execution_class: HostExecutionClassV1,
    hardware_envelope: HostHardwareEnvelopeV1,
    #[serde(skip)]
    id: HostEnvironmentId,
}

impl HostEnvironmentV1 {
    /// Constructs one exact structurally valid host environment.
    ///
    /// # Errors
    ///
    /// Returns [`HostEnvironmentV1Error`] for unsupported relationships,
    /// non-normalized strings, privacy-prohibited characters, or numeric bounds.
    pub fn new(input: HostEnvironmentV1Input) -> Result<Self, HostEnvironmentV1Error> {
        validation::validate_input(&input)?;
        let operating_system = HostOperatingSystemV1 {
            schema_version: HOST_ENVIRONMENT_SCHEMA_VERSION,
            family: input.operating_system.family,
            version: input.operating_system.version,
            digest: HostOperatingSystemDigestV1(Digest::sha256(b"uninitialized operating system")),
        };
        let architecture = HostArchitectureV1 {
            schema_version: HOST_ENVIRONMENT_SCHEMA_VERSION,
            instruction_set: input.architecture.instruction_set,
            abi: input.architecture.abi,
            digest: HostArchitectureDigestV1(Digest::sha256(b"uninitialized architecture")),
        };
        let execution_class = HostExecutionClassV1 {
            schema_version: HOST_ENVIRONMENT_SCHEMA_VERSION,
            profile: input.execution_class.profile,
            compute_backend: input.execution_class.compute_backend,
            placement: input.execution_class.placement,
            observer_binary_assertion_mode: input.execution_class.observer_binary_assertion_mode,
            accelerator_scope: input.execution_class.accelerator_scope,
            digest: HostExecutionClassDigestV1(Digest::sha256(b"uninitialized execution class")),
        };
        let hardware_envelope = HostHardwareEnvelopeV1 {
            schema_version: HOST_ENVIRONMENT_SCHEMA_VERSION,
            cpu_model: input.hardware_envelope.cpu_model,
            physical_core_count: input.hardware_envelope.physical_core_count,
            logical_core_count: input.hardware_envelope.logical_core_count,
            total_system_memory_mib: input.hardware_envelope.total_system_memory_mib,
            memory_rounding_granularity_mib: input
                .hardware_envelope
                .memory_rounding_granularity_mib,
            digest: HostHardwareEnvelopeDigestV1(Digest::sha256(
                b"uninitialized hardware envelope",
            )),
        };
        Self::finish(
            operating_system,
            architecture,
            execution_class,
            hardware_envelope,
        )
    }

    fn finish(
        mut operating_system: HostOperatingSystemV1,
        mut architecture: HostArchitectureV1,
        mut execution_class: HostExecutionClassV1,
        mut hardware_envelope: HostHardwareEnvelopeV1,
    ) -> Result<Self, HostEnvironmentV1Error> {
        let os_json = operating_system.to_canonical_json_bytes();
        let architecture_json = architecture.to_canonical_json_bytes();
        let execution_json = execution_class.to_canonical_json_bytes();
        let hardware_json = hardware_envelope.to_canonical_json_bytes();
        for bytes in [
            &os_json,
            &architecture_json,
            &execution_json,
            &hardware_json,
        ] {
            if bytes.len() > MAX_HOST_ENVIRONMENT_PROJECTION_CANONICAL_BYTES {
                return Err(HostEnvironmentV1Error::ProjectionEncodingTooLarge);
            }
        }
        operating_system.digest = HostOperatingSystemDigestV1(length_framed_digest(
            HOST_ENVIRONMENT_OPERATING_SYSTEM_DIGEST_DOMAIN,
            &os_json,
        ));
        architecture.digest = HostArchitectureDigestV1(length_framed_digest(
            HOST_ENVIRONMENT_ARCHITECTURE_DIGEST_DOMAIN,
            &architecture_json,
        ));
        execution_class.digest = HostExecutionClassDigestV1(length_framed_digest(
            HOST_ENVIRONMENT_EXECUTION_CLASS_DIGEST_DOMAIN,
            &execution_json,
        ));
        hardware_envelope.digest = HostHardwareEnvelopeDigestV1(length_framed_digest(
            HOST_ENVIRONMENT_HARDWARE_ENVELOPE_DIGEST_DOMAIN,
            &hardware_json,
        ));
        let mut value = Self {
            schema_version: HOST_ENVIRONMENT_SCHEMA_VERSION,
            operating_system,
            architecture,
            execution_class,
            hardware_envelope,
            id: HostEnvironmentId(Digest::sha256(b"uninitialized host environment")),
        };
        let json = value.to_canonical_json_bytes();
        if json.len() > MAX_HOST_ENVIRONMENT_CANONICAL_BYTES {
            return Err(HostEnvironmentV1Error::CanonicalEncodingTooLarge);
        }
        value.id = HostEnvironmentId(length_framed_digest(HOST_ENVIRONMENT_ID_DOMAIN, &json));
        Ok(value)
    }

    /// Decodes bounded canonical JSON and independently revalidates every field.
    ///
    /// # Errors
    ///
    /// Returns [`HostEnvironmentV1Error`] for oversized, malformed,
    /// noncanonical, unsupported, privacy-prohibited, or inconsistent input.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, HostEnvironmentV1Error> {
        if bytes.len() > MAX_HOST_ENVIRONMENT_JSON_BYTES {
            return Err(HostEnvironmentV1Error::EncodedRecordTooLarge);
        }
        let wire: wire::HostEnvironmentWire = serde_json::from_slice(bytes)
            .map_err(|_error| HostEnvironmentV1Error::InvalidEncoding)?;
        wire.validate_schema()?;
        let value = Self::new(wire.into_input())?;
        if value.to_canonical_json_bytes() != bytes {
            return Err(HostEnvironmentV1Error::NonCanonicalEncoding);
        }
        Ok(value)
    }

    /// Returns the exact compact canonical host-environment JSON.
    #[must_use]
    pub fn to_canonical_json_bytes(&self) -> Vec<u8> {
        canonical_json(self)
    }

    /// Returns the host-environment schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    /// Returns the canonical operating-system projection.
    #[must_use]
    pub const fn operating_system(&self) -> &HostOperatingSystemV1 {
        &self.operating_system
    }
    /// Returns the canonical architecture projection.
    #[must_use]
    pub const fn architecture(&self) -> &HostArchitectureV1 {
        &self.architecture
    }
    /// Returns the canonical execution-class projection.
    #[must_use]
    pub const fn execution_class(&self) -> &HostExecutionClassV1 {
        &self.execution_class
    }
    /// Returns the canonical hardware-envelope projection.
    #[must_use]
    pub const fn hardware_envelope(&self) -> &HostHardwareEnvelopeV1 {
        &self.hardware_envelope
    }
    /// Returns the content-derived complete host identity.
    #[must_use]
    pub const fn host_environment_id(&self) -> &HostEnvironmentId {
        &self.id
    }
}

impl fmt::Debug for HostEnvironmentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostEnvironmentV1")
            .field("schema_version", &self.schema_version)
            .finish_non_exhaustive()
    }
}

/// Failure while constructing or decoding a host environment.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum HostEnvironmentV1Error {
    /// JSON was malformed, empty, used an unknown field or value, or had trailing bytes.
    #[error("host environment encoding is invalid")]
    InvalidEncoding,
    /// JSON was valid but not the exact compact canonical representation.
    #[error("host environment encoding is noncanonical")]
    NonCanonicalEncoding,
    /// Input JSON exceeded the fixed decoder ceiling.
    #[error("host environment encoded record is too large")]
    EncodedRecordTooLarge,
    /// The canonical complete record exceeded its fixed ceiling.
    #[error("host environment canonical record is too large")]
    CanonicalEncodingTooLarge,
    /// A canonical projection exceeded its fixed ceiling.
    #[error("host environment projection is too large")]
    ProjectionEncodingTooLarge,
    /// A full or nested schema version was unsupported.
    #[error("host environment schema is unsupported")]
    UnsupportedSchema,
    /// The operating-system, instruction-set, and ABI tuple is outside narrow V1 scope.
    #[error("host environment platform is unsupported")]
    UnsupportedPlatform,
    /// The execution profile, backend, placement, or scope relationship was invalid.
    #[error("host environment execution class is invalid")]
    InvalidExecutionClass,
    /// The Linux kernel release was empty, oversized, or not normalized.
    #[error("host environment operating-system version is invalid")]
    InvalidOperatingSystemVersion,
    /// The CPU model was empty, oversized, or not normalized.
    #[error("host environment CPU model is invalid")]
    InvalidCpuModel,
    /// A string contained a character prohibited by the privacy-bounded vocabulary.
    #[error("host environment contains a privacy-prohibited character")]
    PrivacyProhibitedCharacter,
    /// Physical or logical core counts were outside fixed bounds or inconsistent.
    #[error("host environment core count is invalid")]
    InvalidCoreCount,
    /// Rounded total memory or its fixed granularity was invalid.
    #[error("host environment memory envelope is invalid")]
    InvalidMemoryEnvelope,
}

fn canonical_json(value: &impl Serialize) -> Vec<u8> {
    serde_json::to_vec(value).expect("fixed host environment serialization must remain infallible")
}

fn length_framed_digest(domain: &[u8], json: &[u8]) -> Digest {
    let mut material = Vec::with_capacity(domain.len() + 8 + json.len());
    material.extend_from_slice(domain);
    material.extend_from_slice(
        &u64::try_from(json.len())
            .expect("bounded canonical JSON length must fit u64")
            .to_be_bytes(),
    );
    material.extend_from_slice(json);
    Digest::sha256(&material)
}

#[cfg(test)]
#[path = "host_environment/tests.rs"]
mod tests;
