use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use rewrite_types::Digest;

mod state;

pub use state::{
    ComputeBackend, EffectiveRuntimeState, EffectiveRuntimeStateError, EffectiveRuntimeStateId,
    EffectiveRuntimeStateInput, ExecutionPlacement,
};

/// Current runtime-build identity contract version.
pub const RUNTIME_BUILD_IDENTITY_SCHEMA_VERSION: u32 = 1;
/// Current effective-runtime-state identity contract version.
pub const EFFECTIVE_RUNTIME_STATE_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded JSON bytes admitted by an identity decoder.
pub const MAX_RUNTIME_IDENTITY_JSON_BYTES: usize = 16_384;
pub(super) const MAX_MACHINE_ID_BYTES: usize = 64;
const MAX_OPAQUE_IDENTITY_BYTES: usize = 128;
pub(super) const MAX_CANONICAL_IDENTITY_BYTES: usize = 1_024;

/// Evidence class used to identify one exact runtime build.
///
/// Observed-only attached runtimes are deliberately absent. They cannot construct
/// an authoritative build identity without package and process attestation.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBuildMode {
    /// Retonr verified and owns the runtime package, launch, and lifetime.
    ManagedProcess,
    /// A reviewed local attestor bound an attached listener to an exact process.
    AttachedAttestedProcess,
    /// A reviewed local attestor bound an attached listener to an exact container.
    AttachedAttestedContainer,
}

/// Native operating-system family for a runtime build.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperatingSystem {
    /// Microsoft Windows.
    Windows,
    /// Apple macOS.
    MacOs,
    /// Linux.
    Linux,
}

/// Native instruction-set architecture for a runtime build.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeArchitecture {
    /// AMD64 or Intel 64.
    X86_64,
    /// 64-bit Arm.
    Aarch64,
}

/// Application binary interface for a runtime build.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeAbi {
    /// Microsoft Visual C++ Windows ABI.
    WindowsMsvc,
    /// MinGW-w64 GNU Windows ABI.
    WindowsGnu,
    /// GNU libc Linux ABI.
    LinuxGnuLibc,
    /// musl libc Linux ABI.
    LinuxMusl,
    /// Apple Darwin ABI.
    Darwin,
}

/// Validated native target for one exact runtime build.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeTarget {
    operating_system: RuntimeOperatingSystem,
    architecture: RuntimeArchitecture,
    abi: RuntimeAbi,
}

impl RuntimeTarget {
    /// Creates a target from a supported operating-system and ABI combination.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeTargetError`] when the ABI cannot describe the selected
    /// operating-system family.
    pub fn new(
        operating_system: RuntimeOperatingSystem,
        architecture: RuntimeArchitecture,
        abi: RuntimeAbi,
    ) -> Result<Self, RuntimeTargetError> {
        let valid = matches!(
            (operating_system, abi),
            (
                RuntimeOperatingSystem::Windows,
                RuntimeAbi::WindowsMsvc | RuntimeAbi::WindowsGnu
            ) | (RuntimeOperatingSystem::MacOs, RuntimeAbi::Darwin)
                | (
                    RuntimeOperatingSystem::Linux,
                    RuntimeAbi::LinuxGnuLibc | RuntimeAbi::LinuxMusl
                )
        );
        if !valid {
            return Err(RuntimeTargetError);
        }
        Ok(Self {
            operating_system,
            architecture,
            abi,
        })
    }

    /// Returns the operating-system family.
    #[must_use]
    pub const fn operating_system(&self) -> RuntimeOperatingSystem {
        self.operating_system
    }

    /// Returns the instruction-set architecture.
    #[must_use]
    pub const fn architecture(&self) -> RuntimeArchitecture {
        self.architecture
    }

    /// Returns the application binary interface.
    #[must_use]
    pub const fn abi(&self) -> RuntimeAbi {
        self.abi
    }
}

impl<'de> Deserialize<'de> for RuntimeTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            operating_system: RuntimeOperatingSystem,
            architecture: RuntimeArchitecture,
            abi: RuntimeAbi,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.operating_system, wire.architecture, wire.abi).map_err(D::Error::custom)
    }
}

/// Content-derived identifier for one exact runtime build record.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeBuildId(Digest);

impl RuntimeBuildId {
    /// Returns the digest that defines this runtime build.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Caller-supplied facts required to construct a runtime-build identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeBuildIdentityInput {
    /// Package and process ownership or attestation mode.
    pub mode: RuntimeBuildMode,
    /// Stable lowercase runtime-family identifier.
    pub runtime_family: String,
    /// Exact runtime-reported or package version.
    pub reported_version: String,
    /// Exact source or build revision when one exists.
    pub build_revision: Option<String>,
    /// Native package target.
    pub target: RuntimeTarget,
    /// Canonical package or environment-manifest digest.
    pub package_manifest_digest: Digest,
    /// Exact launched entrypoint digest.
    pub entrypoint_digest: Digest,
    /// Canonical packaged-dependency manifest digest.
    pub packaged_dependencies_digest: Digest,
    /// Digest of output-affecting build features and flags.
    pub build_configuration_digest: Digest,
}

/// Validated, content-addressed runtime-build identity.
///
/// Structural validity does not prove that the supplied digests describe a live
/// listener. The application and qualification workflow own that attestation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBuildIdentity {
    schema_version: u32,
    mode: RuntimeBuildMode,
    runtime_family: String,
    reported_version: String,
    build_revision: Option<String>,
    target: RuntimeTarget,
    package_manifest_digest: Digest,
    entrypoint_digest: Digest,
    packaged_dependencies_digest: Digest,
    build_configuration_digest: Digest,
}

impl RuntimeBuildIdentity {
    /// Creates and validates a version 1 runtime-build identity.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeBuildIdentityError`] for invalid, unsupported, or
    /// noncanonical identity metadata.
    pub fn new(input: RuntimeBuildIdentityInput) -> Result<Self, RuntimeBuildIdentityError> {
        Self::from_wire(RUNTIME_BUILD_IDENTITY_SCHEMA_VERSION, input)
    }

    fn from_wire(
        schema_version: u32,
        input: RuntimeBuildIdentityInput,
    ) -> Result<Self, RuntimeBuildIdentityError> {
        if schema_version != RUNTIME_BUILD_IDENTITY_SCHEMA_VERSION {
            return Err(RuntimeBuildIdentityError::UnsupportedSchema(schema_version));
        }
        if !valid_machine_id(&input.runtime_family)
            || !valid_opaque_identity(&input.reported_version)
            || input
                .build_revision
                .as_deref()
                .is_some_and(|value| !valid_opaque_identity(value))
        {
            return Err(RuntimeBuildIdentityError::InvalidMetadata);
        }
        let identity = Self {
            schema_version,
            mode: input.mode,
            runtime_family: input.runtime_family,
            reported_version: input.reported_version,
            build_revision: input.build_revision,
            target: input.target,
            package_manifest_digest: input.package_manifest_digest,
            entrypoint_digest: input.entrypoint_digest,
            packaged_dependencies_digest: input.packaged_dependencies_digest,
            build_configuration_digest: input.build_configuration_digest,
        };
        if identity.canonical_bytes().len() > MAX_CANONICAL_IDENTITY_BYTES {
            return Err(RuntimeBuildIdentityError::CanonicalEncodingTooLarge);
        }
        Ok(identity)
    }

    /// Returns the content-derived runtime-build identifier.
    #[must_use]
    pub fn runtime_build_id(&self) -> RuntimeBuildId {
        RuntimeBuildId(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the build identity contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the package and process ownership or attestation mode.
    #[must_use]
    pub const fn mode(&self) -> RuntimeBuildMode {
        self.mode
    }

    /// Returns the stable runtime-family identifier.
    #[must_use]
    pub fn runtime_family(&self) -> &str {
        &self.runtime_family
    }

    /// Returns the exact reported or packaged version.
    #[must_use]
    pub fn reported_version(&self) -> &str {
        &self.reported_version
    }

    /// Returns the exact build revision when one was recorded.
    #[must_use]
    pub fn build_revision(&self) -> Option<&str> {
        self.build_revision.as_deref()
    }

    /// Returns the native runtime target.
    #[must_use]
    pub const fn target(&self) -> RuntimeTarget {
        self.target
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(b"retonr:runtime-build-identity:v1\0");
        append_u32(&mut output, self.schema_version);
        output.push(build_mode_byte(self.mode));
        append_text(&mut output, &self.runtime_family);
        append_text(&mut output, &self.reported_version);
        append_optional_text(&mut output, self.build_revision.as_deref());
        output.push(operating_system_byte(self.target.operating_system));
        output.push(architecture_byte(self.target.architecture));
        output.push(abi_byte(self.target.abi));
        for digest in [
            &self.package_manifest_digest,
            &self.entrypoint_digest,
            &self.packaged_dependencies_digest,
            &self.build_configuration_digest,
        ] {
            append_digest(&mut output, digest);
        }
        output
    }
}

impl RuntimeBuildIdentity {
    /// Parses a byte-bounded JSON record and revalidates every identity field.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeBuildIdentityError`] before decoding when the input exceeds
    /// the fixed ceiling, or when JSON or identity validation fails.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, RuntimeBuildIdentityError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            mode: RuntimeBuildMode,
            runtime_family: String,
            reported_version: String,
            build_revision: Option<String>,
            target: RuntimeTarget,
            package_manifest_digest: Digest,
            entrypoint_digest: Digest,
            packaged_dependencies_digest: Digest,
            build_configuration_digest: Digest,
        }

        if bytes.len() > MAX_RUNTIME_IDENTITY_JSON_BYTES {
            return Err(RuntimeBuildIdentityError::EncodedIdentityTooLarge);
        }

        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| RuntimeBuildIdentityError::InvalidEncoding)?;
        Self::from_wire(
            wire.schema_version,
            RuntimeBuildIdentityInput {
                mode: wire.mode,
                runtime_family: wire.runtime_family,
                reported_version: wire.reported_version,
                build_revision: wire.build_revision,
                target: wire.target,
                package_manifest_digest: wire.package_manifest_digest,
                entrypoint_digest: wire.entrypoint_digest,
                packaged_dependencies_digest: wire.packaged_dependencies_digest,
                build_configuration_digest: wire.build_configuration_digest,
            },
        )
    }
}

pub(super) fn valid_machine_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_MACHINE_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn valid_opaque_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OPAQUE_IDENTITY_BYTES
        && !value.chars().any(char::is_control)
}

pub(super) fn append_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

pub(super) fn append_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn append_optional_text(output: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            output.push(1);
            append_text(output, value);
        }
        None => output.push(0),
    }
}

pub(super) fn append_digest(output: &mut Vec<u8>, value: &Digest) {
    output.extend_from_slice(value.as_str().as_bytes());
}

const fn build_mode_byte(value: RuntimeBuildMode) -> u8 {
    match value {
        RuntimeBuildMode::ManagedProcess => 0,
        RuntimeBuildMode::AttachedAttestedProcess => 1,
        RuntimeBuildMode::AttachedAttestedContainer => 2,
    }
}

const fn operating_system_byte(value: RuntimeOperatingSystem) -> u8 {
    match value {
        RuntimeOperatingSystem::Windows => 0,
        RuntimeOperatingSystem::MacOs => 1,
        RuntimeOperatingSystem::Linux => 2,
    }
}

const fn architecture_byte(value: RuntimeArchitecture) -> u8 {
    match value {
        RuntimeArchitecture::X86_64 => 0,
        RuntimeArchitecture::Aarch64 => 1,
    }
}

const fn abi_byte(value: RuntimeAbi) -> u8 {
    match value {
        RuntimeAbi::WindowsMsvc => 0,
        RuntimeAbi::WindowsGnu => 1,
        RuntimeAbi::LinuxGnuLibc => 2,
        RuntimeAbi::LinuxMusl => 3,
        RuntimeAbi::Darwin => 4,
    }
}

/// Runtime-target validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("runtime target operating system and ABI are incompatible")]
pub struct RuntimeTargetError;

/// Runtime-build identity validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimeBuildIdentityError {
    /// Encoded input exceeds the fixed pre-decode byte ceiling.
    #[error("encoded runtime-build identity exceeds its limit")]
    EncodedIdentityTooLarge,
    /// Encoded JSON is malformed or contains an unknown field.
    #[error("runtime-build identity encoding is invalid")]
    InvalidEncoding,
    /// The runtime-build schema version is unsupported.
    #[error("unsupported runtime-build identity schema {0}")]
    UnsupportedSchema(u32),
    /// Runtime family, version, or revision metadata is invalid.
    #[error("runtime-build identity metadata is invalid")]
    InvalidMetadata,
    /// Canonical identity bytes exceed the fixed contract ceiling.
    #[error("runtime-build canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
}

#[cfg(test)]
#[path = "runtime_identity/tests.rs"]
mod tests;
