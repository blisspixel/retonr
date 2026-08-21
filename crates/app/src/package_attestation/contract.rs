use std::io;

use rewrite_model::{
    ArtifactId, ArtifactSetId, ModelPackageManifestError, ModelPackageManifestId,
    RuntimePackageManifestError, RuntimePackageManifestId,
};
use thiserror::Error;

use crate::ArtifactSetLeaseError;

/// Version of the static package-attestation evidence contract.
pub const PACKAGE_ATTESTATION_SCHEMA_VERSION: u32 = 1;

/// Authority scope of package evidence produced by this application boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageAttestationScope {
    /// Exact managed static bytes were verified under a retained lifecycle lease.
    StaticManagedBytes,
}

/// Caller-owned ceilings for retained runtime code verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePackageLeaseLimits {
    /// Maximum executable-code members retained by one package lease.
    pub maximum_code_members: usize,
    /// Maximum bytes hashed for any one executable-code member.
    pub maximum_code_member_bytes: u64,
    /// Maximum checked sum of all executable-code member bytes.
    pub maximum_code_bytes: u64,
}

impl RuntimePackageLeaseLimits {
    pub(super) fn validate(self) -> Result<(), PackageAttestationError> {
        if self.maximum_code_members == 0
            || self.maximum_code_member_bytes == 0
            || self.maximum_code_bytes == 0
            || self.maximum_code_members.checked_add(1).is_none()
        {
            Err(PackageAttestationError::InvalidLimits)
        } else {
            Ok(())
        }
    }
}

/// Redacted point-in-time evidence for one verified runtime package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePackageAttestationEvidence {
    schema_version: u32,
    scope: PackageAttestationScope,
    artifact_set_id: ArtifactSetId,
    runtime_package_manifest_id: RuntimePackageManifestId,
    entrypoint_artifact_id: ArtifactId,
    code_member_count: u32,
    code_byte_size: u64,
}

impl RuntimePackageAttestationEvidence {
    pub(super) const fn new(
        artifact_set_id: ArtifactSetId,
        runtime_package_manifest_id: RuntimePackageManifestId,
        entrypoint_artifact_id: ArtifactId,
        code_member_count: u32,
        code_byte_size: u64,
    ) -> Self {
        Self {
            schema_version: PACKAGE_ATTESTATION_SCHEMA_VERSION,
            scope: PackageAttestationScope::StaticManagedBytes,
            artifact_set_id,
            runtime_package_manifest_id,
            entrypoint_artifact_id,
            code_member_count,
            code_byte_size,
        }
    }

    /// Returns the evidence schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the deliberately static evidence scope.
    #[must_use]
    pub const fn scope(&self) -> PackageAttestationScope {
        self.scope
    }

    /// Returns the exact canonical managed byte-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact canonical runtime-package identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the exact retained entrypoint byte identity.
    #[must_use]
    pub const fn entrypoint_artifact_id(&self) -> &ArtifactId {
        &self.entrypoint_artifact_id
    }

    /// Returns the number of retained packaged executable-code members.
    #[must_use]
    pub const fn code_member_count(&self) -> u32 {
        self.code_member_count
    }

    /// Returns the checked sum of retained packaged code bytes.
    #[must_use]
    pub const fn code_byte_size(&self) -> u64 {
        self.code_byte_size
    }
}

/// Redacted point-in-time evidence for one verified model package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelPackageAttestationEvidence {
    schema_version: u32,
    scope: PackageAttestationScope,
    artifact_set_id: ArtifactSetId,
    model_package_manifest_id: ModelPackageManifestId,
    member_count: u32,
    byte_size: u64,
}

impl ModelPackageAttestationEvidence {
    pub(super) const fn new(
        artifact_set_id: ArtifactSetId,
        model_package_manifest_id: ModelPackageManifestId,
        member_count: u32,
        byte_size: u64,
    ) -> Self {
        Self {
            schema_version: PACKAGE_ATTESTATION_SCHEMA_VERSION,
            scope: PackageAttestationScope::StaticManagedBytes,
            artifact_set_id,
            model_package_manifest_id,
            member_count,
            byte_size,
        }
    }

    /// Returns the evidence schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the deliberately static evidence scope.
    #[must_use]
    pub const fn scope(&self) -> PackageAttestationScope {
        self.scope
    }

    /// Returns the exact canonical managed byte-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact canonical model-package identity.
    #[must_use]
    pub const fn model_package_manifest_id(&self) -> &ModelPackageManifestId {
        &self.model_package_manifest_id
    }

    /// Returns the exact number of verified model-package members.
    #[must_use]
    pub const fn member_count(&self) -> u32 {
        self.member_count
    }

    /// Returns the checked sum of verified model-package bytes.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
}

/// Failure from static package attestation or retained-handle verification.
#[derive(Debug, Error)]
pub enum PackageAttestationError {
    /// One or more caller-owned ceilings are zero or unrepresentable.
    #[error("runtime-package lease limits are invalid")]
    InvalidLimits,
    /// Runtime semantic meaning does not exactly cover the leased byte set.
    #[error("runtime package does not match the leased artifact set")]
    RuntimeRelationship(#[source] RuntimePackageManifestError),
    /// Model semantic meaning does not exactly cover the leased byte set.
    #[error("model package does not match the leased artifact set")]
    ModelRelationship(#[source] ModelPackageManifestError),
    /// Packaged executable-code member count exceeds the caller ceiling.
    #[error("runtime package has {actual} code members; the configured maximum is {maximum}")]
    TooManyCodeMembers {
        /// Manifest-declared packaged code member count.
        actual: usize,
        /// Caller-owned code member ceiling.
        maximum: usize,
    },
    /// One packaged executable-code member exceeds the caller byte ceiling.
    #[error("runtime code member has {actual} bytes; the configured maximum is {maximum}")]
    CodeMemberTooLarge {
        /// Manifest-declared member bytes.
        actual: u64,
        /// Caller-owned member byte ceiling.
        maximum: u64,
    },
    /// Aggregate packaged executable-code bytes exceed the caller ceiling.
    #[error("runtime package has {actual} code bytes; the configured maximum is {maximum}")]
    CodeBytesTooLarge {
        /// Checked manifest-declared code bytes.
        actual: u64,
        /// Caller-owned aggregate code byte ceiling.
        maximum: u64,
    },
    /// A retained member had unexpected length or digest.
    #[error("runtime package member bytes do not match their canonical identity")]
    MemberBytesConflict,
    /// A retained member object or its canonical managed name changed.
    #[error("runtime package member identity changed during verification")]
    MemberIdentityChanged,
    /// Cancellation was observed before static package evidence completed.
    #[error("package attestation was cancelled")]
    Cancelled,
    /// The underlying managed-set lease could not be revalidated.
    #[error("managed artifact-set lease revalidation failed")]
    ArtifactSet(#[source] ArtifactSetLeaseError),
    /// A retained member could not be read or inspected completely.
    #[error("runtime package member operation failed")]
    MemberIo(#[source] io::Error),
}

impl PackageAttestationError {
    pub(super) fn from_set_lease(error: ArtifactSetLeaseError) -> Self {
        if matches!(error, ArtifactSetLeaseError::Cancelled) {
            Self::Cancelled
        } else {
            Self::ArtifactSet(error)
        }
    }
}
