use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use rewrite_types::Digest;

use crate::{ArtifactId, ArtifactSetRelativePath, RuntimePackageManifest};

mod codec;
mod validation;

/// Current native-load observation contract version.
pub const NATIVE_LOAD_OBSERVATION_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded JSON bytes accepted for one native-load observation.
pub const MAX_NATIVE_LOAD_OBSERVATION_JSON_BYTES: usize = 1_048_576;
/// Maximum native objects admitted by one observation.
pub const MAX_NATIVE_LOAD_COMPONENTS: usize = 4_096;
const MAX_NATIVE_LOAD_CANONICAL_BYTES: usize = 1_048_576;
const MAX_OBSERVATION_CONTRACT_BYTES: usize = 64;

/// Platform evidence mechanism used to enumerate native file-backed objects.
///
/// Version 1 admits only Linux `map_files` evidence because the reviewed Windows
/// public APIs do not bind a reported mapped pathname to the exact section-backed
/// file object. Other platforms fail closed before constructing this record.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeLoadEvidenceClass {
    /// Linux `/proc/<pid>/map_files` inspection.
    LinuxProcMapFiles,
}

/// Exact visibility claimed by a native-load observation.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeLoadVisibilityScope {
    /// All file-backed executable mappings visible to the evidence mechanism.
    FileBackedExecutableMappings,
    /// All file-backed mappings visible to the evidence mechanism.
    FileBackedMappings,
}

/// Observed mapping class for one collapsed native object.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeMappingClass {
    /// The selected process executable image.
    ExecutableImage,
    /// A file-backed object with at least one executable mapping.
    ExecutableMapped,
    /// A file-backed object observed only in non-executable mappings.
    DataMapped,
}

/// Package relationship of one observed native object.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeLoadOrigin {
    /// Exact member of the bound runtime package.
    PackagedMember {
        /// Portable path from the package manifest.
        relative_path: ArtifactSetRelativePath,
    },
    /// Native platform object outside the runtime package.
    ExternalPlatformComponent,
}

/// One exact file-backed object collapsed across all of its mappings.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeLoadedComponent {
    artifact_id: ArtifactId,
    byte_size: u64,
    origin: NativeLoadOrigin,
    mapping_class: NativeMappingClass,
    object_evidence_digest: Digest,
}

impl NativeLoadedComponent {
    /// Creates one native object descriptor. The observation validates relationships.
    #[must_use]
    pub const fn new(
        artifact_id: ArtifactId,
        byte_size: u64,
        origin: NativeLoadOrigin,
        mapping_class: NativeMappingClass,
        object_evidence_digest: Digest,
    ) -> Self {
        Self {
            artifact_id,
            byte_size,
            origin,
            mapping_class,
            object_evidence_digest,
        }
    }

    /// Returns the exact object byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact object byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the object's package relationship.
    #[must_use]
    pub const fn origin(&self) -> &NativeLoadOrigin {
        &self.origin
    }

    /// Returns the collapsed mapping class.
    #[must_use]
    pub const fn mapping_class(&self) -> NativeMappingClass {
        self.mapping_class
    }

    /// Returns the digest of bounded object-level platform evidence.
    #[must_use]
    pub const fn object_evidence_digest(&self) -> &Digest {
        &self.object_evidence_digest
    }
}

/// Caller-supplied facts for one native-load observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLoadObservationInput {
    /// Platform evidence mechanism used for enumeration.
    pub evidence_class: NativeLoadEvidenceClass,
    /// Exact mapping visibility supplied by the mechanism.
    pub visibility_scope: NativeLoadVisibilityScope,
    /// Digest binding the observation to exact process evidence.
    pub process_evidence_digest: Digest,
    /// Stable lowercase observation contract identifier.
    pub observation_contract_id: String,
    /// Observation contract version.
    pub observation_contract_schema_version: u32,
    /// Native objects in required canonical order.
    pub components: Vec<NativeLoadedComponent>,
}

/// Validated native objects observed in one exact runtime process.
///
/// This record asserts only the visibility of its named evidence contract. It does
/// not claim complete operating-system dependency closure beyond that scope.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeLoadObservation {
    schema_version: u32,
    runtime_package_manifest_id: crate::RuntimePackageManifestId,
    evidence_class: NativeLoadEvidenceClass,
    visibility_scope: NativeLoadVisibilityScope,
    process_evidence_digest: Digest,
    observation_contract_id: String,
    observation_contract_schema_version: u32,
    components: Vec<NativeLoadedComponent>,
}

impl NativeLoadObservation {
    /// Creates and validates a version 1 native-load observation.
    ///
    /// # Errors
    ///
    /// Returns [`NativeLoadObservationError`] for incomplete, inconsistent, or
    /// noncanonical evidence.
    pub fn new(
        package: &RuntimePackageManifest,
        input: NativeLoadObservationInput,
    ) -> Result<Self, NativeLoadObservationError> {
        Self::from_wire(NATIVE_LOAD_OBSERVATION_SCHEMA_VERSION, package, input)
    }

    /// Returns the content-derived observation identity.
    #[must_use]
    pub fn native_load_observation_id(&self) -> NativeLoadObservationId {
        NativeLoadObservationId(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the native-load observation contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the bound runtime-package identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &crate::RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns the platform evidence mechanism.
    #[must_use]
    pub const fn evidence_class(&self) -> NativeLoadEvidenceClass {
        self.evidence_class
    }

    /// Returns the visibility scope.
    #[must_use]
    pub const fn visibility_scope(&self) -> NativeLoadVisibilityScope {
        self.visibility_scope
    }

    /// Returns the process evidence digest.
    #[must_use]
    pub const fn process_evidence_digest(&self) -> &Digest {
        &self.process_evidence_digest
    }

    /// Returns the observation contract identifier.
    #[must_use]
    pub fn observation_contract_id(&self) -> &str {
        &self.observation_contract_id
    }

    /// Returns the observation contract version.
    #[must_use]
    pub const fn observation_contract_schema_version(&self) -> u32 {
        self.observation_contract_schema_version
    }

    /// Returns collapsed native objects in canonical order.
    #[must_use]
    pub fn components(&self) -> &[NativeLoadedComponent] {
        &self.components
    }
}

/// Content-derived identifier for one native-load observation.
#[derive(Clone, Debug, serde::Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct NativeLoadObservationId(Digest);

impl NativeLoadObservationId {
    /// Returns the digest defining this observation.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Native-load observation validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum NativeLoadObservationError {
    /// Encoded input exceeds its fixed ceiling.
    #[error("encoded native-load observation exceeds its limit")]
    EncodedObservationTooLarge,
    /// JSON is malformed or contains unknown fields.
    #[error("native-load observation encoding is invalid")]
    InvalidEncoding,
    /// The schema version is unsupported.
    #[error("unsupported native-load observation schema {0}")]
    UnsupportedSchema(u32),
    /// Observation contract metadata is invalid.
    #[error("native-load observation contract metadata is invalid")]
    InvalidMetadata,
    /// The component count is empty or exceeds its ceiling.
    #[error("native-load component count is invalid")]
    InvalidComponentCount,
    /// Components are duplicated or not in canonical order.
    #[error("native-load components are duplicated or unordered")]
    InvalidComponentOrder,
    /// A packaged component does not exactly match its package member.
    #[error("native-load packaged component does not match its runtime package")]
    PackagedComponentMismatch,
    /// A package member forbidden as code was reported as native code.
    #[error("native-load component violates package load policy")]
    LoadPolicyViolation,
    /// Evidence mechanism does not support the package operating system.
    #[error("native-load evidence class does not match the runtime target")]
    EvidenceClassTargetMismatch,
    /// A mapping lies outside the claimed visibility scope.
    #[error("native-load component lies outside the claimed visibility scope")]
    VisibilityScopeViolation,
    /// A required package code member was not observed.
    #[error("native-load observation is missing required package code")]
    MissingRequiredComponent,
    /// A decoded package path is invalid.
    #[error("native-load packaged path is invalid")]
    InvalidMemberPath,
    /// Canonical identity bytes exceed their ceiling.
    #[error("native-load canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
}

#[cfg(test)]
#[path = "native_load/tests.rs"]
mod tests;
