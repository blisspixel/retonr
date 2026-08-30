use std::{fs::File, marker::PhantomData, time::Duration};

use rewrite_model::{
    ArtifactId, NativeMappingClass, RuntimeOperatingSystem, RuntimePackageLoadPolicy,
    RuntimePackageManifest, RuntimePackageManifestId, RuntimePackageMemberRole,
};
use rewrite_types::{CancellationToken, Digest};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

use super::ManagedGenerationWorkerError;
use crate::{
    ExpectedExternalNativeComponent, NativeLoadObservationLimits, RetainedNativePackageMember,
};

/// Fixed model-store root visible inside the managed runtime mount namespace.
pub const MANAGED_OLLAMA_MODEL_ROOT: &str = "/tmp/retonr-managed-runtime-input-v1";
/// Hard maximum bytes admitted while retaining one exact model weight.
pub const MAXIMUM_RETAINED_MODEL_WEIGHT_BYTES: u64 = 128 * 1024 * 1024 * 1024;
/// Hard maximum processes inspected while discovering one worker.
pub const MAXIMUM_GENERATION_WORKER_PROCESSES: usize = 65_536;
/// Hard maximum retained descendant depth from worker to server.
pub const MAXIMUM_GENERATION_WORKER_PARENT_DEPTH: usize = 32;
/// Hard maximum bytes read from one worker metadata file.
pub const MAXIMUM_GENERATION_WORKER_METADATA_BYTES: usize = 1024 * 1024;
/// Hard maximum bytes read from one worker command line.
pub const MAXIMUM_GENERATION_WORKER_COMMAND_BYTES: usize = 64 * 1024;
/// Hard maximum worker command-line arguments.
pub const MAXIMUM_GENERATION_WORKER_COMMAND_ARGUMENTS: usize = 128;
/// Hard maximum elapsed time for one worker observation bracket.
pub const MAXIMUM_GENERATION_WORKER_OBSERVATION_MILLIS: u64 = 1_800_000;

/// Closed command and process policy for one reviewed generation-worker version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedGenerationWorkerProfile {
    /// Ollama v0.32.15 CPU-only runner contract.
    OllamaV0_32_15Cpu,
}

impl ManagedGenerationWorkerProfile {
    pub(crate) const fn reported_version(self) -> &'static str {
        match self {
            Self::OllamaV0_32_15Cpu => "0.32.15",
        }
    }

    #[cfg(any(target_os = "linux", feature = "test-support"))]
    pub(crate) const fn command_contract_id(self) -> &'static str {
        match self {
            Self::OllamaV0_32_15Cpu => "ollama-v0.32.15-linux-llama-server-cpu-v1",
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn admits_server_code_mapping(self) -> bool {
        match self {
            Self::OllamaV0_32_15Cpu => false,
        }
    }
}

/// Caller-selected ceilings for one generation-worker observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagedGenerationWorkerLimits {
    /// Maximum numeric process directories inspected.
    pub maximum_processes: usize,
    /// Maximum parent-chain depth from worker to retained server.
    pub maximum_parent_depth: usize,
    /// Maximum bytes read from one proc metadata file.
    pub maximum_metadata_bytes: usize,
    /// Maximum command-line bytes admitted.
    pub maximum_command_bytes: usize,
    /// Maximum command-line arguments admitted.
    pub maximum_command_arguments: usize,
    /// Native mapping and hashing ceilings.
    pub native_load: NativeLoadObservationLimits,
    /// Maximum bytes hashed for one model-weight object on each verification pass.
    pub maximum_model_weight_bytes: u64,
    /// Maximum elapsed time for one complete observation bracket.
    pub maximum_elapsed: Duration,
}

impl Default for ManagedGenerationWorkerLimits {
    fn default() -> Self {
        Self {
            maximum_processes: 16_384,
            maximum_parent_depth: 16,
            maximum_metadata_bytes: 256 * 1024,
            maximum_command_bytes: 16 * 1024,
            maximum_command_arguments: 64,
            native_load: NativeLoadObservationLimits::default(),
            maximum_model_weight_bytes: MAXIMUM_RETAINED_MODEL_WEIGHT_BYTES,
            maximum_elapsed: Duration::from_mins(10),
        }
    }
}

impl ManagedGenerationWorkerLimits {
    /// Validates every caller-selected worker observation ceiling.
    ///
    /// # Errors
    ///
    /// Returns [`ManagedGenerationWorkerError::InvalidLimits`] for a zero value or
    /// a value above its hard implementation maximum.
    pub fn validate(self) -> Result<Self, ManagedGenerationWorkerError> {
        if self.maximum_processes == 0
            || self.maximum_processes > MAXIMUM_GENERATION_WORKER_PROCESSES
            || self.maximum_parent_depth == 0
            || self.maximum_parent_depth > MAXIMUM_GENERATION_WORKER_PARENT_DEPTH
            || self.maximum_metadata_bytes == 0
            || self.maximum_metadata_bytes > MAXIMUM_GENERATION_WORKER_METADATA_BYTES
            || self.maximum_command_bytes == 0
            || self.maximum_command_bytes > MAXIMUM_GENERATION_WORKER_COMMAND_BYTES
            || self.maximum_command_arguments == 0
            || self.maximum_command_arguments > MAXIMUM_GENERATION_WORKER_COMMAND_ARGUMENTS
            || self.maximum_model_weight_bytes == 0
            || self.maximum_model_weight_bytes > MAXIMUM_RETAINED_MODEL_WEIGHT_BYTES
            || self.maximum_elapsed.is_zero()
            || self.maximum_elapsed
                > Duration::from_millis(MAXIMUM_GENERATION_WORKER_OBSERVATION_MILLIS)
            || self.native_load.validate().is_err()
        {
            return Err(ManagedGenerationWorkerError::InvalidLimits);
        }
        Ok(self)
    }
}

/// Opaque source that transfers one lease-bound weight into an attestor-owned sink.
///
/// Implementations can keep the retained handle private. The transfer exposes no
/// path and the sink independently checks the exact digest and size.
pub trait RetainedModelWeightSource<'lease> {
    /// Transfers exactly one retained model-weight object.
    ///
    /// # Errors
    ///
    /// Returns a bounded error on cancellation, invalid shape, byte drift, or a
    /// source that transfers zero or more than one object.
    fn transfer(
        self,
        sink: &mut RetainedModelWeightSink<'_, 'lease>,
    ) -> Result<(), ManagedGenerationWorkerError>;
}

/// Narrow receiver used only while constructing an opaque retained model weight.
pub struct RetainedModelWeightSink<'sink, 'lease> {
    retained: &'sink mut Option<RetainedModelWeight<'lease>>,
    cancellation: &'sink CancellationToken,
}

impl RetainedModelWeightSink<'_, '_> {
    /// Retains one exact content-bound regular file without retaining a host path.
    ///
    /// # Errors
    ///
    /// Returns a bounded error for cancellation, a duplicate transfer, an invalid
    /// file, a size mismatch, or a content digest mismatch.
    pub fn retain(
        &mut self,
        artifact_id: ArtifactId,
        byte_size: u64,
        file: File,
    ) -> Result<(), ManagedGenerationWorkerError> {
        if self.retained.is_some() || self.cancellation.is_cancelled() {
            return Err(if self.cancellation.is_cancelled() {
                ManagedGenerationWorkerError::Cancelled
            } else {
                ManagedGenerationWorkerError::InvalidRequest
            });
        }
        let metadata = file
            .metadata()
            .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?;
        if byte_size == 0
            || byte_size > MAXIMUM_RETAINED_MODEL_WEIGHT_BYTES
            || !metadata.is_file()
            || metadata.len() != byte_size
            || positioned_artifact_id(&file, byte_size, self.cancellation)? != artifact_id
            || file
                .metadata()
                .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?
                .len()
                != byte_size
        {
            return Err(ManagedGenerationWorkerError::InvalidRequest);
        }
        *self.retained = Some(RetainedModelWeight {
            artifact_id,
            byte_size,
            file,
            _lease: PhantomData,
        });
        Ok(())
    }
}

/// Retained exact GGUF weight file used by worker command and mapping observation.
///
/// This capability is nonserializable, lease-bound, and retains no host pathname.
/// Construction validates exact bytes but does not attest loading or use.
pub struct RetainedModelWeight<'lease> {
    artifact_id: ArtifactId,
    byte_size: u64,
    #[cfg_attr(
        not(target_os = "linux"),
        expect(
            dead_code,
            reason = "only the Linux observer consumes the retained file"
        )
    )]
    file: File,
    _lease: PhantomData<&'lease ()>,
}

impl<'lease> RetainedModelWeight<'lease> {
    /// Consumes an opaque source and retains exactly one content-bound weight.
    ///
    /// # Errors
    ///
    /// Returns a bounded error when transfer fails, is cancelled, or does not
    /// provide exactly one valid weight.
    pub fn from_source(
        source: impl RetainedModelWeightSource<'lease>,
        cancellation: &CancellationToken,
    ) -> Result<Self, ManagedGenerationWorkerError> {
        if cancellation.is_cancelled() {
            return Err(ManagedGenerationWorkerError::Cancelled);
        }
        let mut retained = None;
        source.transfer(&mut RetainedModelWeightSink {
            retained: &mut retained,
            cancellation,
        })?;
        retained.ok_or(ManagedGenerationWorkerError::InvalidRequest)
    }

    /// Returns the expected complete weight byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the expected weight byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    #[cfg(target_os = "linux")]
    pub(crate) const fn file(&self) -> &File {
        &self.file
    }
}

impl std::fmt::Debug for RetainedModelWeight<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedModelWeight")
            .field("artifact_id", &self.artifact_id)
            .field("byte_size", &self.byte_size)
            .finish_non_exhaustive()
    }
}

/// Complete input for discovery of one exact managed generation worker.
pub struct ManagedGenerationWorkerObservationRequest<'a> {
    /// Exact runtime package containing one worker role.
    pub package: &'a RuntimePackageManifest,
    /// Expected content-derived package identity.
    pub expected_package_id: &'a RuntimePackageManifestId,
    /// Exact retained worker executable member.
    pub retained_worker: &'a RetainedNativePackageMember,
    /// Exact retained GGUF weight used to derive the private model alias.
    pub retained_model_weight: &'a RetainedModelWeight<'a>,
    /// Closed version and execution profile.
    pub profile: ManagedGenerationWorkerProfile,
    /// Hard caller-selected observation ceilings.
    pub limits: ManagedGenerationWorkerLimits,
}

impl ManagedGenerationWorkerObservationRequest<'_> {
    pub(crate) fn validate(
        &self,
    ) -> Result<ManagedGenerationWorkerLimits, ManagedGenerationWorkerError> {
        let limits = self.limits.validate()?;
        let mut workers = self
            .package
            .members()
            .iter()
            .filter(|member| member.roles() == [RuntimePackageMemberRole::WorkerExecutable]);
        let worker = workers
            .next()
            .ok_or(ManagedGenerationWorkerError::InvalidRequest)?;
        if workers.next().is_some()
            || self.package.target().operating_system() != RuntimeOperatingSystem::Linux
            || self.package.reported_version() != self.profile.reported_version()
            || &self.package.runtime_package_manifest_id() != self.expected_package_id
            || worker.load_policy() != RuntimePackageLoadPolicy::BackendConditional
            || worker.relative_path() != self.retained_worker.relative_path()
            || worker.artifact_id() != self.retained_worker.artifact_id()
            || worker.byte_size() != self.retained_worker.byte_size()
            || self.retained_model_weight.byte_size == 0
        {
            return Err(ManagedGenerationWorkerError::InvalidRequest);
        }
        Ok(limits)
    }
}

/// Exact admitted native-code closure for the separately retained worker.
pub struct ManagedGenerationWorkerNativeLoadRequest<'a> {
    /// Exact runtime package used during worker discovery.
    pub package: &'a RuntimePackageManifest,
    /// Expected content-derived package identity.
    pub expected_package_id: &'a RuntimePackageManifestId,
    /// Retained objects for every package code member, in manifest order.
    pub retained_package_code: &'a [RetainedNativePackageMember],
    /// Canonical frozen external executable dependencies admitted for this worker.
    pub expected_external_components: &'a [ExpectedExternalNativeComponent],
}

impl ManagedGenerationWorkerNativeLoadRequest<'_> {
    pub(crate) fn validate(
        &self,
        profile: ManagedGenerationWorkerProfile,
        package_id: &RuntimePackageManifestId,
        worker_artifact_id: &ArtifactId,
        limits: ManagedGenerationWorkerLimits,
    ) -> Result<(), ManagedGenerationWorkerError> {
        if &self.package.runtime_package_manifest_id() != self.expected_package_id
            || self.expected_package_id != package_id
            || self.package.reported_version() != profile.reported_version()
        {
            return Err(ManagedGenerationWorkerError::InvalidNativeLoadRequest);
        }
        let declared = self
            .package
            .members()
            .iter()
            .filter(|member| is_package_code(member.roles()))
            .collect::<Vec<_>>();
        if declared.len() != self.retained_package_code.len()
            || declared
                .iter()
                .zip(self.retained_package_code)
                .any(|(member, retained)| {
                    member.relative_path() != retained.relative_path()
                        || member.artifact_id() != retained.artifact_id()
                        || member.byte_size() != retained.byte_size()
                })
            || !declared
                .iter()
                .any(|member| member.artifact_id() == worker_artifact_id)
            || declared.len() > limits.native_load.maximum_components
            || self.expected_external_components.len() > limits.native_load.maximum_components
        {
            return Err(ManagedGenerationWorkerError::InvalidNativeLoadRequest);
        }
        let mut prior = None;
        for component in self.expected_external_components {
            let key = crate::native_load::expected_key(component);
            if component.byte_size() == 0
                || component.mapping_class() != NativeMappingClass::ExecutableMapped
                || prior.as_ref().is_some_and(|value| value >= &key)
            {
                return Err(ManagedGenerationWorkerError::InvalidNativeLoadRequest);
            }
            prior = Some(key);
        }
        Ok(())
    }
}

fn is_package_code(roles: &[RuntimePackageMemberRole]) -> bool {
    roles.iter().any(|role| {
        matches!(
            role,
            RuntimePackageMemberRole::Entrypoint
                | RuntimePackageMemberRole::NativeDependency
                | RuntimePackageMemberRole::HelperExecutable
                | RuntimePackageMemberRole::WorkerExecutable
        )
    })
}

fn positioned_artifact_id(
    file: &File,
    expected_bytes: u64,
    cancellation: &CancellationToken,
) -> Result<ArtifactId, ManagedGenerationWorkerError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    let buffer_bytes = u64::try_from(buffer.len())
        .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?;
    let mut offset = 0_u64;
    while offset < expected_bytes {
        if cancellation.is_cancelled() {
            return Err(ManagedGenerationWorkerError::Cancelled);
        }
        let remaining = expected_bytes - offset;
        let length = usize::try_from(remaining.min(buffer_bytes))
            .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?;
        let read = positioned_read(file, &mut buffer[..length], offset)
            .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?;
        if read == 0 {
            return Err(ManagedGenerationWorkerError::InvalidRequest);
        }
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(
                u64::try_from(read)
                    .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?,
            )
            .ok_or(ManagedGenerationWorkerError::InvalidRequest)?;
    }
    if cancellation.is_cancelled() {
        return Err(ManagedGenerationWorkerError::Cancelled);
    }
    if positioned_read(file, &mut buffer[..1], expected_bytes)
        .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?
        != 0
    {
        return Err(ManagedGenerationWorkerError::InvalidRequest);
    }
    let digest = Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_error| ManagedGenerationWorkerError::InvalidRequest)?;
    Ok(ArtifactId::from_digest(digest))
}

#[cfg(unix)]
fn positioned_read(file: &File, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buffer, offset)
}

#[cfg(windows)]
fn positioned_read(file: &File, buffer: &mut [u8], offset: u64) -> std::io::Result<usize> {
    use std::os::windows::{fs::FileExt as _, io::AsRawHandle as _, io::FromRawHandle as _};

    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        Storage::FileSystem::{
            FILE_GENERIC_READ, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, ReOpenFile,
        },
    };

    // SAFETY: the source handle remains owned by `file`; success returns a distinct
    // owned handle which is immediately wrapped in `File` and closed on drop.
    let reopened = unsafe {
        ReOpenFile(
            file.as_raw_handle(),
            FILE_GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            0,
        )
    };
    if reopened == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `ReOpenFile` returned a new valid owned handle on the success path.
    let reopened = unsafe { File::from_raw_handle(reopened) };
    reopened.seek_read(buffer, offset)
}
