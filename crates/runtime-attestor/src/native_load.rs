use std::{fs::File, time::Duration};

use rewrite_model::{
    ArtifactId, ArtifactSetRelativePath, NativeMappingClass, RuntimePackageManifest,
    RuntimePackageManifestId, RuntimePackageMember, RuntimePackageMemberRole,
};
use thiserror::Error;

/// Hard maximum virtual-memory or proc-maps rows inspected in one snapshot.
pub const MAXIMUM_NATIVE_MAPPING_REGIONS: usize = 65_536;
/// Hard maximum metadata bytes admitted from one native mapping snapshot.
pub const MAXIMUM_NATIVE_MAPPING_METADATA_BYTES: usize = 16 * 1024 * 1024;
/// Hard maximum distinct native file objects admitted by one snapshot.
pub const MAXIMUM_NATIVE_LOADED_COMPONENTS: usize = 4_096;
/// Hard maximum aggregate file bytes hashed across one observation bracket.
pub const MAXIMUM_NATIVE_LOAD_HASH_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// Hard maximum elapsed time for one native-load observation bracket.
pub const MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS: u64 = 120_000;

/// Caller-owned ceilings for native loaded-component observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeLoadObservationLimits {
    /// Maximum mapping rows inspected in one snapshot.
    pub maximum_mapping_regions: usize,
    /// Maximum native mapping metadata bytes admitted in one snapshot.
    pub maximum_mapping_metadata_bytes: usize,
    /// Maximum distinct file objects admitted in one snapshot.
    pub maximum_components: usize,
    /// Maximum aggregate file bytes hashed across the complete bracket.
    pub maximum_aggregate_hash_bytes: u64,
    /// Maximum elapsed time for the complete observation bracket.
    pub maximum_elapsed: Duration,
}

impl Default for NativeLoadObservationLimits {
    fn default() -> Self {
        Self {
            maximum_mapping_regions: 16_384,
            maximum_mapping_metadata_bytes: 4 * 1024 * 1024,
            maximum_components: 1_024,
            maximum_aggregate_hash_bytes: 4 * 1024 * 1024 * 1024,
            maximum_elapsed: Duration::from_secs(30),
        }
    }
}

impl NativeLoadObservationLimits {
    pub(crate) fn validate(self) -> Result<Self, NativeLoadObserverError> {
        if self.maximum_mapping_regions == 0
            || self.maximum_mapping_regions > MAXIMUM_NATIVE_MAPPING_REGIONS
            || self.maximum_mapping_metadata_bytes == 0
            || self.maximum_mapping_metadata_bytes > MAXIMUM_NATIVE_MAPPING_METADATA_BYTES
            || self.maximum_components == 0
            || self.maximum_components > MAXIMUM_NATIVE_LOADED_COMPONENTS
            || self.maximum_aggregate_hash_bytes == 0
            || self.maximum_aggregate_hash_bytes > MAXIMUM_NATIVE_LOAD_HASH_BYTES
            || self.maximum_elapsed.is_zero()
            || self.maximum_elapsed > Duration::from_millis(MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS)
        {
            return Err(NativeLoadObserverError::InvalidLimits);
        }
        Ok(self)
    }
}

/// One frozen external platform component admitted by an observation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedExternalNativeComponent {
    artifact_id: ArtifactId,
    byte_size: u64,
    mapping_class: NativeMappingClass,
}

impl ExpectedExternalNativeComponent {
    /// Creates one exact expected external native object.
    #[must_use]
    pub const fn new(
        artifact_id: ArtifactId,
        byte_size: u64,
        mapping_class: NativeMappingClass,
    ) -> Self {
        Self {
            artifact_id,
            byte_size,
            mapping_class,
        }
    }

    /// Returns the complete expected byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact expected byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the expected mapping class.
    #[must_use]
    pub const fn mapping_class(&self) -> NativeMappingClass {
        self.mapping_class
    }
}

/// One exact retained packaged-code file object admitted to native-load observation.
///
/// The object is non-serializable and retains no pathname. Its logical path and byte
/// identity must exactly match the runtime package supplied in the observation request.
pub struct RetainedNativePackageMember {
    relative_path: ArtifactSetRelativePath,
    artifact_id: ArtifactId,
    byte_size: u64,
    #[cfg_attr(
        not(any(target_os = "linux", test)),
        expect(
            dead_code,
            reason = "only the Linux object-bound observer consumes the retained file"
        )
    )]
    file: File,
}

impl RetainedNativePackageMember {
    /// Creates one retained packaged-code object after checking its basic file shape.
    ///
    /// The observer hashes the mapped object and validates the complete relationship
    /// against the package manifest. Construction alone grants no evidence.
    ///
    /// # Errors
    ///
    /// Returns [`NativeLoadObserverError::InvalidRequest`] unless `file` is a
    /// nonempty regular file with the declared byte length.
    pub fn new(
        relative_path: ArtifactSetRelativePath,
        artifact_id: ArtifactId,
        byte_size: u64,
        file: File,
    ) -> Result<Self, NativeLoadObserverError> {
        let metadata = file
            .metadata()
            .map_err(|_error| NativeLoadObserverError::InvalidRequest)?;
        if byte_size == 0 || !metadata.is_file() || metadata.len() != byte_size {
            return Err(NativeLoadObserverError::InvalidRequest);
        }
        Ok(Self {
            relative_path,
            artifact_id,
            byte_size,
            file,
        })
    }

    /// Returns the canonical package-relative logical path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns the declared exact byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the declared exact byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) const fn file(&self) -> &File {
        &self.file
    }
}

impl std::fmt::Debug for RetainedNativePackageMember {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedNativePackageMember")
            .field("relative_path", &self.relative_path)
            .field("artifact_id", &self.artifact_id)
            .field("byte_size", &self.byte_size)
            .finish_non_exhaustive()
    }
}

/// Complete caller input for one inert native loaded-component observation.
pub struct NativeLoadObservationRequest<'a> {
    /// Exact typed runtime package bound into the result.
    pub package: &'a RuntimePackageManifest,
    /// Expected content-derived identity of `package`.
    pub expected_package_id: &'a RuntimePackageManifestId,
    /// Exact retained file objects for every packaged code member.
    pub retained_package_members: &'a [RetainedNativePackageMember],
    /// Exact frozen external platform component set, in canonical order.
    pub expected_external_components: &'a [ExpectedExternalNativeComponent],
    /// Hard caller-selected resource ceilings.
    pub limits: NativeLoadObservationLimits,
}

impl NativeLoadObservationRequest<'_> {
    pub(crate) fn validate(&self) -> Result<NativeLoadObservationLimits, NativeLoadObserverError> {
        let limits = self.limits.validate()?;
        if &self.package.runtime_package_manifest_id() != self.expected_package_id
            || self.retained_package_members.len() > limits.maximum_components
            || self.expected_external_components.len() > limits.maximum_components
        {
            return Err(NativeLoadObserverError::InvalidRequest);
        }
        let packaged_code = self
            .package
            .members()
            .iter()
            .filter(|member| is_retained_package_member(member))
            .collect::<Vec<_>>();
        if packaged_code.len() != self.retained_package_members.len()
            || packaged_code.iter().zip(self.retained_package_members).any(
                |(declared, retained)| {
                    declared.relative_path() != retained.relative_path()
                        || declared.artifact_id() != retained.artifact_id()
                        || declared.byte_size() != retained.byte_size()
                },
            )
        {
            return Err(NativeLoadObserverError::InvalidRequest);
        }
        let mut prior = None;
        let mut prior_artifact = None;
        for expected in self.expected_external_components {
            if expected.byte_size == 0
                || expected.mapping_class != NativeMappingClass::ExecutableMapped
            {
                return Err(NativeLoadObserverError::InvalidRequest);
            }
            let key = expected_key(expected);
            if prior.as_ref().is_some_and(|value| value >= &key)
                || prior_artifact
                    .is_some_and(|value| value >= expected.artifact_id.digest().as_str())
            {
                return Err(NativeLoadObserverError::InvalidRequest);
            }
            prior = Some(key);
            prior_artifact = Some(expected.artifact_id.digest().as_str());
        }
        Ok(limits)
    }
}

pub(crate) fn expected_key(expected: &ExpectedExternalNativeComponent) -> Vec<u8> {
    let mut key = Vec::with_capacity(80);
    key.extend_from_slice(expected.artifact_id.digest().as_str().as_bytes());
    key.extend_from_slice(&expected.byte_size.to_be_bytes());
    key.push(match expected.mapping_class {
        NativeMappingClass::ExecutableImage => 0,
        NativeMappingClass::ExecutableMapped => 1,
        NativeMappingClass::DataMapped => 2,
    });
    key
}

pub(crate) fn is_retained_package_member(member: &RuntimePackageMember) -> bool {
    member.roles().iter().any(|role| {
        matches!(
            role,
            RuntimePackageMemberRole::Entrypoint
                | RuntimePackageMemberRole::NativeDependency
                | RuntimePackageMemberRole::HelperExecutable
        )
    })
}

/// Redacted failure from bounded native loaded-component observation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum NativeLoadObserverError {
    /// One or more observation ceilings are zero or exceed hard maxima.
    #[error("native-load observation limits are invalid")]
    InvalidLimits,
    /// The package identity, root, or external allowlist is invalid.
    #[error("native-load observation request is invalid")]
    InvalidRequest,
    /// Observation was cancelled.
    #[error("native-load observation was cancelled")]
    Cancelled,
    /// Complete observation exceeded its elapsed-time ceiling.
    #[error("native-load observation deadline was exceeded")]
    DeadlineExceeded,
    /// The current operating system has no admitted observation mechanism.
    #[error("native-load observation is unsupported on this platform")]
    Unsupported,
    /// Native process access was denied or visibility was insufficient.
    #[error("native-load process visibility is insufficient")]
    ProcessVisibilityInsufficient,
    /// The retained process exited or changed incarnation.
    #[error("native-load retained process changed")]
    ProcessChanged,
    /// An executable mapping was anonymous, deleted, or not file-backed.
    #[error("native-load executable mapping is unverifiable")]
    UnverifiableExecutableMapping,
    /// One mapped file could not be opened as the exact observed object.
    #[error("native-load mapped file object is unavailable")]
    MappedObjectUnavailable,
    /// One mapped object was not a stable regular file.
    #[error("native-load mapped file object is invalid")]
    InvalidMappedObject,
    /// A bounded row, byte, object, hash, or elapsed ceiling was exhausted.
    #[error("native-load observation exceeded a resource limit")]
    ResourceLimit,
    /// Mapping or object evidence changed across the observation bracket.
    #[error("native-load observation changed during its bracket")]
    ObservationChanged,
    /// Observed packaged or external components violated the frozen request.
    #[error("native-load component set does not match its frozen policy")]
    ComponentPolicyMismatch,
    /// The typed model rejected observer-produced relationship evidence.
    #[error("native-load typed observation relationship is invalid")]
    InvalidObservation,
    /// A redacted native observation operation failed.
    #[error("native-load native observation failed")]
    PlatformObservationFailed,
}

#[cfg(test)]
#[path = "native_load/tests.rs"]
mod tests;
