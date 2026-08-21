use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use rewrite_types::Digest;

use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetRelativePath, PackageSource, PackageTransformation,
    RuntimeTarget,
};

mod codec;

/// Current runtime-package manifest contract version.
pub const RUNTIME_PACKAGE_MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded JSON bytes accepted for one runtime-package manifest.
pub const MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES: usize = 1_048_576;
const MAX_RUNTIME_PACKAGE_CANONICAL_BYTES: usize = 1_048_576;
const MAX_RUNTIME_MEMBER_ROLES: usize = 8;
const MAX_RUNTIME_ROLE_ASSIGNMENTS: usize = 8_192;
const MAX_RUNTIME_FAMILY_BYTES: usize = 64;
const MAX_RUNTIME_VERSION_BYTES: usize = 128;

/// Content-derived identifier for one runtime-package manifest.
#[derive(Clone, Debug, serde::Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimePackageManifestId(Digest);

impl RuntimePackageManifestId {
    /// Returns the digest defining this runtime package.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Closed purpose vocabulary for one runtime-package member.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePackageMemberRole {
    /// The exact process entrypoint selected for this build.
    Entrypoint,
    /// A packaged native shared-library dependency.
    NativeDependency,
    /// A helper executable not mapped into the selected process.
    HelperExecutable,
    /// Non-code runtime resource bytes.
    RuntimeResource,
    /// Default runtime configuration.
    DefaultConfiguration,
    /// Output-affecting build configuration evidence.
    BuildConfiguration,
    /// Exact reviewed license text.
    LicenseText,
    /// Exact source or acquisition provenance.
    ProvenanceRecord,
    /// Exact transformation evidence.
    TransformationRecord,
}

/// Static load policy for one runtime-package member.
///
/// This is a declared package policy, not evidence that a process loaded the member.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePackageLoadPolicy {
    /// The member must appear in a ready-process native load observation.
    RequiredAtReady,
    /// The member may appear only for an admitted backend or execution class.
    BackendConditional,
    /// The member must not appear as native code in the selected process.
    MustNotBeCodeLoaded,
}

/// One exact byte member and its declared runtime-package meaning.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePackageMember {
    artifact_id: ArtifactId,
    byte_size: u64,
    relative_path: ArtifactSetRelativePath,
    roles: Vec<RuntimePackageMemberRole>,
    load_policy: RuntimePackageLoadPolicy,
}

impl RuntimePackageMember {
    /// Creates one runtime member. The enclosing manifest validates relationships.
    #[must_use]
    pub const fn new(
        artifact_id: ArtifactId,
        byte_size: u64,
        relative_path: ArtifactSetRelativePath,
        roles: Vec<RuntimePackageMemberRole>,
        load_policy: RuntimePackageLoadPolicy,
    ) -> Self {
        Self {
            artifact_id,
            byte_size,
            relative_path,
            roles,
            load_policy,
        }
    }

    /// Returns the complete member byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact member byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the portable package path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns roles in stable canonical order.
    #[must_use]
    pub fn roles(&self) -> &[RuntimePackageMemberRole] {
        &self.roles
    }

    /// Returns the static load policy.
    #[must_use]
    pub const fn load_policy(&self) -> RuntimePackageLoadPolicy {
        self.load_policy
    }

    fn validate(&self) -> Result<(), RuntimePackageManifestError> {
        if self.roles.is_empty()
            || self.roles.len() > MAX_RUNTIME_MEMBER_ROLES
            || self
                .roles
                .windows(2)
                .any(|pair| codec::role_byte(pair[0]) >= codec::role_byte(pair[1]))
        {
            return Err(RuntimePackageManifestError::InvalidMemberRoles);
        }
        let entrypoint = self.roles.contains(&RuntimePackageMemberRole::Entrypoint);
        let dependency = self
            .roles
            .contains(&RuntimePackageMemberRole::NativeDependency);
        let has_other = self.roles.iter().any(|role| {
            !matches!(
                role,
                RuntimePackageMemberRole::Entrypoint | RuntimePackageMemberRole::NativeDependency
            )
        });
        let policy_valid = if entrypoint {
            self.roles.len() == 1 && self.load_policy == RuntimePackageLoadPolicy::RequiredAtReady
        } else if dependency {
            !has_other && self.load_policy != RuntimePackageLoadPolicy::MustNotBeCodeLoaded
        } else {
            self.load_policy == RuntimePackageLoadPolicy::MustNotBeCodeLoaded
        };
        if !policy_valid {
            return Err(RuntimePackageManifestError::InvalidLoadPolicy);
        }
        Ok(())
    }

    pub(crate) fn is_code(&self) -> bool {
        self.roles.iter().any(|role| {
            matches!(
                role,
                RuntimePackageMemberRole::Entrypoint | RuntimePackageMemberRole::NativeDependency
            )
        })
    }
}

/// Canonical static contents and meaning of one runtime package.
///
/// This record does not claim that any member was loaded or executed.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePackageManifest {
    schema_version: u32,
    artifact_set_id: crate::ArtifactSetId,
    runtime_family: String,
    reported_version: String,
    build_revision: Option<String>,
    target: RuntimeTarget,
    source: PackageSource,
    transformation: PackageTransformation,
    members: Vec<RuntimePackageMember>,
}

impl RuntimePackageManifest {
    /// Creates and relationship-checks a version 1 runtime package.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimePackageManifestError`] for incomplete or noncanonical input.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor makes every identity input explicit"
    )]
    pub fn new(
        artifact_set: &ArtifactSetManifest,
        runtime_family: impl Into<String>,
        reported_version: impl Into<String>,
        build_revision: Option<String>,
        target: RuntimeTarget,
        source: PackageSource,
        transformation: PackageTransformation,
        members: Vec<RuntimePackageMember>,
    ) -> Result<Self, RuntimePackageManifestError> {
        Self::from_wire(
            RUNTIME_PACKAGE_MANIFEST_SCHEMA_VERSION,
            artifact_set,
            runtime_family.into(),
            reported_version.into(),
            build_revision,
            target,
            source,
            transformation,
            members,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "wire reconstruction validates every explicit field"
    )]
    fn from_wire(
        schema_version: u32,
        artifact_set: &ArtifactSetManifest,
        runtime_family: String,
        reported_version: String,
        build_revision: Option<String>,
        target: RuntimeTarget,
        source: PackageSource,
        transformation: PackageTransformation,
        members: Vec<RuntimePackageMember>,
    ) -> Result<Self, RuntimePackageManifestError> {
        if schema_version != RUNTIME_PACKAGE_MANIFEST_SCHEMA_VERSION {
            return Err(RuntimePackageManifestError::UnsupportedSchema(
                schema_version,
            ));
        }
        if !valid_machine_text(&runtime_family, MAX_RUNTIME_FAMILY_BYTES)
            || !valid_identity_text(&reported_version, MAX_RUNTIME_VERSION_BYTES)
            || build_revision
                .as_deref()
                .is_some_and(|value| !valid_identity_text(value, MAX_RUNTIME_VERSION_BYTES))
        {
            return Err(RuntimePackageManifestError::InvalidMetadata);
        }
        let manifest = Self {
            schema_version,
            artifact_set_id: artifact_set.artifact_set_id(),
            runtime_family,
            reported_version,
            build_revision,
            target,
            source,
            transformation,
            members,
        };
        manifest.validate_against(artifact_set)?;
        if manifest.canonical_bytes().len() > MAX_RUNTIME_PACKAGE_CANONICAL_BYTES {
            return Err(RuntimePackageManifestError::CanonicalEncodingTooLarge);
        }
        Ok(manifest)
    }

    /// Returns the content-derived runtime package identity.
    #[must_use]
    pub fn runtime_package_manifest_id(&self) -> RuntimePackageManifestId {
        RuntimePackageManifestId(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the runtime-package contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Revalidates complete semantic coverage against the byte manifest.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimePackageManifestError`] on any relationship or policy drift.
    pub fn validate_against(
        &self,
        artifact_set: &ArtifactSetManifest,
    ) -> Result<(), RuntimePackageManifestError> {
        if self.artifact_set_id != artifact_set.artifact_set_id() {
            return Err(RuntimePackageManifestError::ArtifactSetMismatch);
        }
        if self.members.len() != artifact_set.members().len() {
            return Err(RuntimePackageManifestError::MemberCoverageMismatch);
        }
        let mut assignments = 0usize;
        let mut entrypoints = 0usize;
        let mut license = false;
        let mut provenance = false;
        let mut transformation_record = false;
        for (semantic, content) in self.members.iter().zip(artifact_set.members()) {
            semantic.validate()?;
            if semantic.relative_path() != content.relative_path()
                || semantic.artifact_id() != content.artifact_id()
                || semantic.byte_size() != content.byte_size()
            {
                return Err(RuntimePackageManifestError::MemberCoverageMismatch);
            }
            assignments = assignments
                .checked_add(semantic.roles.len())
                .ok_or(RuntimePackageManifestError::TooManyRoleAssignments)?;
            entrypoints += usize::from(
                semantic
                    .roles
                    .contains(&RuntimePackageMemberRole::Entrypoint),
            );
            license |= semantic
                .roles
                .contains(&RuntimePackageMemberRole::LicenseText);
            provenance |= semantic
                .roles
                .contains(&RuntimePackageMemberRole::ProvenanceRecord);
            transformation_record |= semantic
                .roles
                .contains(&RuntimePackageMemberRole::TransformationRecord);
        }
        if assignments > MAX_RUNTIME_ROLE_ASSIGNMENTS {
            return Err(RuntimePackageManifestError::TooManyRoleAssignments);
        }
        if entrypoints != 1 {
            return Err(RuntimePackageManifestError::InvalidEntrypoint);
        }
        if !license || !provenance {
            return Err(RuntimePackageManifestError::MissingEvidence);
        }
        if self.transformation.requires_transformation_record() && !transformation_record {
            return Err(RuntimePackageManifestError::MissingTransformationEvidence);
        }
        Ok(())
    }

    /// Returns the referenced byte-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &crate::ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the stable runtime family.
    #[must_use]
    pub fn runtime_family(&self) -> &str {
        &self.runtime_family
    }

    /// Returns the exact reported version.
    #[must_use]
    pub fn reported_version(&self) -> &str {
        &self.reported_version
    }

    /// Returns the exact build revision when present.
    #[must_use]
    pub fn build_revision(&self) -> Option<&str> {
        self.build_revision.as_deref()
    }

    /// Returns the native build target.
    #[must_use]
    pub const fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Returns the exact package source.
    #[must_use]
    pub const fn source(&self) -> &PackageSource {
        &self.source
    }

    /// Returns the transformation disposition.
    #[must_use]
    pub const fn transformation(&self) -> &PackageTransformation {
        &self.transformation
    }

    /// Returns semantic members in byte-manifest path order.
    #[must_use]
    pub fn members(&self) -> &[RuntimePackageMember] {
        &self.members
    }

    /// Returns the exact selected entrypoint member.
    ///
    /// # Panics
    ///
    /// Panics only if an already validated manifest is corrupted in memory.
    #[must_use]
    pub fn entrypoint(&self) -> &RuntimePackageMember {
        self.members
            .iter()
            .find(|member| member.roles.contains(&RuntimePackageMemberRole::Entrypoint))
            .expect("validated runtime package has one entrypoint")
    }

    /// Returns the canonical packaged dependency subset digest.
    #[must_use]
    pub fn packaged_dependencies_digest(&self) -> Digest {
        codec::subset_digest(b"retonr:runtime-packaged-dependencies:v1\0", self, |role| {
            matches!(
                role,
                RuntimePackageMemberRole::NativeDependency
                    | RuntimePackageMemberRole::HelperExecutable
            )
        })
    }

    /// Returns the canonical build-configuration subset digest.
    #[must_use]
    pub fn build_configuration_digest(&self) -> Digest {
        codec::subset_digest(b"retonr:runtime-build-configuration:v1\0", self, |role| {
            role == RuntimePackageMemberRole::BuildConfiguration
        })
    }
}

fn valid_machine_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

fn valid_identity_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

/// Runtime-package manifest validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimePackageManifestError {
    /// Encoded input exceeds its fixed ceiling.
    #[error("encoded runtime-package manifest exceeds its limit")]
    EncodedManifestTooLarge,
    /// JSON is malformed or contains unknown fields.
    #[error("runtime-package manifest encoding is invalid")]
    InvalidEncoding,
    /// The schema version is unsupported.
    #[error("unsupported runtime-package manifest schema {0}")]
    UnsupportedSchema(u32),
    /// Runtime family, version, or revision is invalid.
    #[error("runtime-package metadata is invalid")]
    InvalidMetadata,
    /// The referenced artifact set differs.
    #[error("runtime-package artifact set does not match")]
    ArtifactSetMismatch,
    /// Semantic members do not exactly cover byte members.
    #[error("runtime-package member coverage does not match")]
    MemberCoverageMismatch,
    /// A member has empty, duplicated, unordered, or excessive roles.
    #[error("runtime-package member roles are invalid")]
    InvalidMemberRoles,
    /// A static load policy conflicts with its member roles.
    #[error("runtime-package member load policy is invalid")]
    InvalidLoadPolicy,
    /// The package does not name exactly one entrypoint.
    #[error("runtime-package entrypoint coverage is invalid")]
    InvalidEntrypoint,
    /// License or provenance evidence is absent.
    #[error("runtime-package required evidence is missing")]
    MissingEvidence,
    /// A transformed package lacks a transformation-record member.
    #[error("runtime-package transformation evidence is missing")]
    MissingTransformationEvidence,
    /// Aggregate semantic role assignments exceed their ceiling.
    #[error("runtime-package role assignment limit exceeded")]
    TooManyRoleAssignments,
    /// The native target is invalid.
    #[error("runtime-package target is invalid")]
    InvalidTarget,
    /// A nested package source is invalid.
    #[error("runtime-package source is invalid")]
    InvalidSource,
    /// A decoded path is invalid.
    #[error("runtime-package member path is invalid")]
    InvalidMemberPath,
    /// Canonical identity bytes exceed their ceiling.
    #[error("runtime-package canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
}

#[cfg(test)]
#[path = "runtime_package/tests.rs"]
mod tests;
