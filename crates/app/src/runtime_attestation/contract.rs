use std::{io, path::PathBuf};

use rewrite_model::{
    ComputeBackend, EffectiveRuntimeState, EffectiveRuntimeStateError, ExecutionPlacement,
    RuntimeArchitecture, RuntimeBuildIdentity, RuntimeBuildIdentityError, RuntimeOperatingSystem,
    RuntimeTarget,
};
use rewrite_model_store::WriteDisposition;
use rewrite_types::Digest;
use thiserror::Error;

/// Explicit request to attest one managed runtime entrypoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedRuntimeAttestationRequest {
    /// Regular file opened read-only and hashed as the launched entrypoint.
    pub entrypoint: PathBuf,
    /// Expected entrypoint digest. When set, a live mismatch fails closed.
    pub expected_entrypoint_digest: Option<Digest>,
    /// Caller-declared identity facts that are not observed from the file.
    pub identity: ManagedRuntimeIdentityFacts,
    /// Caller-declared effective-state facts that are not observed from the file.
    pub state: ManagedRuntimeStateFacts,
}

/// Identity facts that remain caller-owned during managed-process attestation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedRuntimeIdentityFacts {
    /// Stable lowercase runtime-family identifier.
    pub runtime_family: String,
    /// Exact runtime-reported or package version.
    pub reported_version: String,
    /// Exact source or build revision when one exists.
    pub build_revision: Option<String>,
    /// Native package target declared by the caller.
    pub target: RuntimeTarget,
    /// Canonical package or environment-manifest digest.
    pub package_manifest_digest: Digest,
    /// Canonical packaged-dependency manifest digest.
    pub packaged_dependencies_digest: Digest,
    /// Digest of output-affecting build features and flags.
    pub build_configuration_digest: Digest,
}

/// Effective-state facts that remain caller-owned during managed-process attestation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedRuntimeStateFacts {
    /// Stable adapter-owned provider snapshot contract identifier.
    pub provider_snapshot_contract: String,
    /// Provider snapshot contract version.
    pub provider_snapshot_schema_version: u32,
    /// Digest of the bounded canonical provider snapshot.
    pub provider_snapshot_digest: Digest,
    /// Digest of normalized launch arguments, environment, and lifecycle policy.
    pub launch_policy_digest: Digest,
    /// Digest of effective output-affecting runtime defaults and configuration.
    pub effective_configuration_digest: Digest,
    /// Digest of exact operating-system, framework, and driver evidence.
    pub platform_digest: Digest,
    /// Digest of device class, offload, cache, data type, and parallelism state.
    pub execution_class_digest: Digest,
    /// Digest of endpoint, offline, update, fallback, plugin, and telemetry policy.
    pub isolation_policy_digest: Digest,
    /// Effective runtime context capacity in tokens.
    pub effective_context_tokens: u32,
    /// Effective compute software backend.
    pub compute_backend: ComputeBackend,
    /// Effective CPU and accelerator placement class.
    pub placement: ExecutionPlacement,
}

/// Caller-owned ceilings for one managed-process attestation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAttestationLimits {
    /// Maximum admitted entrypoint size.
    pub maximum_entrypoint_bytes: u64,
}

/// Successful managed-process attestation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAttestationResult {
    /// Content-addressed runtime-build identity with a live entrypoint digest.
    pub build: RuntimeBuildIdentity,
    /// Effective state bound to that exact build.
    pub state: EffectiveRuntimeState,
    /// Observed entrypoint length at attestation time.
    pub entrypoint_bytes: u64,
    /// Whether the records were persisted and reloaded.
    pub persistence: RuntimeAttestationPersistence,
}

/// Persistence outcome for one attestation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAttestationPersistence {
    /// The caller did not request durable storage.
    NotRequested,
    /// Both records were written or confirmed under the current schema.
    Stored {
        /// Runtime-build insert or exact-repeat outcome.
        build: WriteDisposition,
        /// Effective-state insert or exact-repeat outcome.
        state: WriteDisposition,
    },
}

/// Failure from the managed-process attestation trust boundary.
#[derive(Debug, Error)]
pub enum RuntimeAttestationError {
    /// One or more configured attestation ceilings were zero.
    #[error("runtime attestation limits are invalid")]
    InvalidLimits,
    /// Cancellation was observed before the records were produced.
    #[error("runtime attestation was cancelled")]
    Cancelled,
    /// The entrypoint path is a symlink, junction, reparse point, or other link.
    #[error("runtime entrypoint must not be an indirect filesystem link")]
    IndirectEntrypoint,
    /// The opened entrypoint is not one regular file.
    #[error("runtime entrypoint must be one regular file")]
    EntrypointNotFile,
    /// The entrypoint could not be inspected or read.
    #[error("runtime entrypoint could not be read")]
    EntrypointIo(#[source] io::Error),
    /// The entrypoint exceeds the configured byte ceiling.
    #[error("runtime entrypoint size {actual} exceeds the configured maximum {maximum}")]
    EntrypointTooLarge {
        /// Observed entrypoint size.
        actual: u64,
        /// Configured ceiling.
        maximum: u64,
    },
    /// The live entrypoint digest did not match the expected digest.
    #[error("runtime entrypoint digest does not match the expected digest")]
    DigestMismatch,
    /// The entrypoint changed between the first and confirming hash.
    #[error("runtime entrypoint changed during attestation")]
    EntrypointChanged,
    /// Caller-declared identity facts failed the runtime-build contract.
    #[error("runtime-build identity is invalid")]
    InvalidIdentity(#[source] RuntimeBuildIdentityError),
    /// Caller-declared state facts failed the effective-state contract.
    #[error("effective runtime state is invalid")]
    InvalidState(#[source] EffectiveRuntimeStateError),
    /// Durable persistence or reload failed.
    #[error("runtime attestation state registration failed")]
    Persistence(#[source] rewrite_model_store::StoreError),
    /// Reloaded durable records did not match the attested identities.
    #[error("runtime attestation state and attested records disagree")]
    PersistenceMismatch,
}

/// Host native target used by tests and fixture-managed processes.
#[must_use]
pub fn host_runtime_target() -> Option<RuntimeTarget> {
    let operating_system = if cfg!(windows) {
        RuntimeOperatingSystem::Windows
    } else if cfg!(target_os = "macos") {
        RuntimeOperatingSystem::MacOs
    } else if cfg!(target_os = "linux") {
        RuntimeOperatingSystem::Linux
    } else {
        return None;
    };
    let architecture = if cfg!(target_arch = "x86_64") {
        RuntimeArchitecture::X86_64
    } else if cfg!(target_arch = "aarch64") {
        RuntimeArchitecture::Aarch64
    } else {
        return None;
    };
    let abi = if cfg!(windows) {
        rewrite_model::RuntimeAbi::WindowsMsvc
    } else if cfg!(target_os = "macos") {
        rewrite_model::RuntimeAbi::Darwin
    } else {
        rewrite_model::RuntimeAbi::LinuxGnuLibc
    };
    RuntimeTarget::new(operating_system, architecture, abi).ok()
}
