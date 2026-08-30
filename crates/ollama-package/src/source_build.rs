use rewrite_model::{ArtifactSetManifest, ArtifactSetRelativePath, RuntimeTarget};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

mod error;
mod executable;
mod parse;
mod plan;
mod program_lineage;
#[cfg(feature = "retained-program-closure")]
mod retained_program_closure;
mod verify;

pub use error::{RuntimeSourceBuildInputError, RuntimeSourceBuildInputOpenError};
pub use executable::SelfContainedLinuxExecutable;
use parse::parse_runtime_source_build_input_manifest;
pub use plan::{RuntimeSourceBuildExecutionPolicy, RuntimeSourceBuildPlan};
pub use program_lineage::{
    RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_ID, RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_VERSION,
    RETAINED_PROGRAM_LINEAGE_PROCEDURE_ID, RETAINED_PROGRAM_LINEAGE_PROCEDURE_VERSION,
    RetainedProgramBuildRecipeId, RetainedProgramLineageId, VerifiedRetainedProgramLineage,
};
#[cfg(feature = "retained-program-closure")]
pub use retained_program_closure::{
    CARGO_SOURCE_CLOSURE_PROCEDURE_ID, CARGO_SOURCE_CLOSURE_PROCEDURE_VERSION,
    CargoSourceClosureError, CargoSourceClosureLimits, CargoSourceClosureReviewerFacts,
    RETAINED_PROGRAM_LICENSE_EVIDENCE_PROCEDURE_ID,
    RETAINED_PROGRAM_LICENSE_EVIDENCE_PROCEDURE_VERSION, RetainedProgramLicenseEvidenceClosureId,
    RetainedProgramLicenseEvidenceError, RetainedProgramLicenseEvidenceLimits,
    RetainedProgramLicenseEvidenceReviewerFacts, RetainedProgramUpstreamClosureError,
    RetainedProgramUpstreamClosureId, VerifiedAlpineReleaseUpstream, VerifiedCargoSourceClosure,
    VerifiedRetainedProgramLicenseEvidenceClosure, VerifiedRetainedProgramUpstreamClosure,
    VerifiedRustReleaseUpstream, verify_cargo_source_closure,
    verify_retained_program_license_evidence_closure, verify_retained_program_upstream_closure,
};
pub use verify::verify_runtime_source_build_inputs;

/// Controlled runtime source-build input manifest contract version.
pub const RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION: u32 = 1;

const DEFAULT_MANIFEST_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_COMPONENTS: usize = 1_024;
const DEFAULT_COMPONENT_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const DEFAULT_TOTAL_BYTES: u64 = 512 * 1024 * 1024 * 1024;

/// Fixed ceilings for one controlled runtime source-build input manifest.
///
/// Explicit limits may only lower the defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildInputLimits {
    /// Maximum encoded canonical manifest bytes.
    pub manifest_bytes: usize,
    /// Maximum number of content-addressed input components.
    pub maximum_components: usize,
    /// Maximum declared bytes in any one input component.
    pub maximum_component_bytes: u64,
    /// Maximum aggregate declared bytes across all input components.
    pub maximum_total_bytes: u64,
}

impl Default for RuntimeSourceBuildInputLimits {
    fn default() -> Self {
        Self {
            manifest_bytes: DEFAULT_MANIFEST_BYTES,
            maximum_components: DEFAULT_COMPONENTS,
            maximum_component_bytes: DEFAULT_COMPONENT_BYTES,
            maximum_total_bytes: DEFAULT_TOTAL_BYTES,
        }
    }
}

impl RuntimeSourceBuildInputLimits {
    /// Validates that every selected ceiling is nonzero and no greater than the
    /// contract hard limit.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildInputError::LimitExceeded`] when any ceiling
    /// is zero, exceeds its hard limit, or is internally inconsistent.
    pub fn validate(self) -> Result<Self, RuntimeSourceBuildInputError> {
        if self.manifest_bytes == 0
            || self.manifest_bytes > DEFAULT_MANIFEST_BYTES
            || self.maximum_components == 0
            || self.maximum_components > DEFAULT_COMPONENTS
            || self.maximum_component_bytes == 0
            || self.maximum_component_bytes > DEFAULT_COMPONENT_BYTES
            || self.maximum_total_bytes == 0
            || self.maximum_total_bytes > DEFAULT_TOTAL_BYTES
            || self.maximum_component_bytes > self.maximum_total_bytes
        {
            return Err(RuntimeSourceBuildInputError::LimitExceeded);
        }
        Ok(self)
    }
}

/// Closed purpose vocabulary for a frozen source-build input.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSourceBuildInputRole {
    /// Exact Ollama source archive or repository export.
    OllamaSource,
    /// Exact embedded llama.cpp source archive or repository export.
    LlamaCppSource,
    /// Exact Go toolchain payload.
    GoToolchain,
    /// One complete Go module payload.
    GoModule,
    /// Complete Go module checksum-set evidence.
    GoChecksumSet,
    /// One OCI base image payload identified by digest.
    OciBase,
    /// One exact native package payload.
    NativePackage,
    /// Exact POSIX-compatible shell payload used by controlled build tools.
    PosixShell,
    /// Exact `CMake` build tool payload.
    Cmake,
    /// Exact Ninja build tool payload.
    Ninja,
    /// Exact C compiler payload.
    CCompiler,
    /// Exact C++ compiler payload.
    CxxCompiler,
    /// Exact assembler payload.
    Assembler,
    /// Exact linker payload.
    Linker,
    /// Relevant native or language standard-library payload.
    StandardLibrary,
    /// Exact build script or declarative build program.
    BuildScript,
    /// Exact reviewer-retained patch applied to a frozen source component.
    SourcePatch,
    /// Canonical parameters consumed by the build transformation.
    BuildParameters,
    /// Exact Retonr managed-isolation helper payload.
    IsolationHelper,
    /// Complete source provenance evidence for the selected revision.
    SourceProvenance,
    /// Complete build-toolchain provenance and transformation evidence.
    ToolEvidence,
    /// License and source disposition evidence for selected inputs.
    LicenseEvidence,
    /// Canonical retained-program lineage record.
    RetainedProgramLineage,
    /// Canonical recipe for the retained build and preparation programs.
    CanonicalBuildRecipe,
    /// Exact Retonr repository source archive used to build retained programs.
    RetonrRepositorySource,
    /// Exact Cargo lockfile used to resolve retained-program dependencies.
    CargoLockfile,
    /// Exact vendored Cargo dependency-source archive.
    CargoVendorSource,
    /// Exact raw Cargo crate-source archive used to reproduce the vendor tree.
    CargoRawCrateSource,
    /// Exact signed Rust channel manifest.
    RustChannelManifest,
    /// Exact Rust channel-manifest checksum file.
    RustChannelManifestChecksum,
    /// Exact Rust channel-manifest detached signature.
    RustChannelManifestSignature,
    /// Exact Rust release-signing trust-root material.
    RustReleaseTrustRoot,
    /// Exact host Cargo distribution component.
    RustCargoDistribution,
    /// Exact host Rust standard-library distribution component.
    RustHostStandardLibraryDistribution,
    /// Exact target Rust standard-library distribution component.
    RustTargetStandardLibraryDistribution,
    /// Exact host Rust compiler distribution component.
    RustCompilerDistribution,
    /// Exact source-archive preparation tool.
    SourceArchivePreparationTool,
    /// Exact source-manifest preparation tool.
    SourceManifestPreparationTool,
    /// Exact Alpine minirootfs used as the retained build host.
    AlpineMinirootfs,
    /// Exact checksum file for the retained Alpine minirootfs.
    AlpineMinirootfsChecksum,
    /// Exact detached signature for the retained Alpine minirootfs.
    AlpineMinirootfsSignature,
    /// Exact Alpine release-signing trust-root material.
    AlpineReleaseTrustRoot,
    /// Exact signed Alpine `libgcc` package installed into the retained host.
    AlpineLibgccPackage,
    /// Exact signed Alpine `busybox-static` package authenticating the bootstrap shell.
    AlpineBusyboxStaticPackage,
}

/// Network policy for the controlled build phase.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSourceBuildNetworkPolicy {
    /// Outbound network access is denied for the build phase.
    Denied,
}

/// Accelerator policy for the first controlled build.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSourceBuildAcceleratorPolicy {
    /// Only the reviewed CPU backend may be produced.
    CpuOnly,
}

/// One exact content-addressed input and its build meaning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildInputComponent {
    relative_path: ArtifactSetRelativePath,
    byte_size: u64,
    digest: Digest,
    name: String,
    revision: String,
    source_locator: String,
    roles: Vec<RuntimeSourceBuildInputRole>,
}

impl RuntimeSourceBuildInputComponent {
    /// Returns the portable input-bundle member path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns the exact declared byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the exact declared byte digest.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    /// Returns the reviewed component name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the exact version, revision, or package identity.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Returns the immutable source locator recorded by the fetch phase.
    #[must_use]
    pub fn source_locator(&self) -> &str {
        &self.source_locator
    }

    /// Returns roles in canonical order.
    #[must_use]
    pub fn roles(&self) -> &[RuntimeSourceBuildInputRole] {
        &self.roles
    }
}

/// Exact content-free policy applied to the network-disabled build phase.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildPolicy {
    target: RuntimeTarget,
    network_access: RuntimeSourceBuildNetworkPolicy,
    accelerator: RuntimeSourceBuildAcceleratorPolicy,
    cpu_feature_policy: String,
    locale: String,
    timezone: String,
    source_date_epoch: u64,
    environment: Vec<RuntimeSourceBuildEnvironmentVariable>,
    build_arguments: Vec<String>,
    environment_digest: Digest,
    build_arguments_digest: Digest,
}

/// One explicit entry in the otherwise cleared controlled-build environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildEnvironmentVariable {
    name: String,
    value: String,
}

impl RuntimeSourceBuildEnvironmentVariable {
    /// Returns the canonical environment name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the exact environment value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl RuntimeSourceBuildPolicy {
    /// Returns the exact native build target.
    #[must_use]
    pub const fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Returns the required network policy.
    #[must_use]
    pub const fn network_access(&self) -> RuntimeSourceBuildNetworkPolicy {
        self.network_access
    }

    /// Returns the required accelerator policy.
    #[must_use]
    pub const fn accelerator(&self) -> RuntimeSourceBuildAcceleratorPolicy {
        self.accelerator
    }

    /// Returns the exact CPU feature policy identifier.
    #[must_use]
    pub fn cpu_feature_policy(&self) -> &str {
        &self.cpu_feature_policy
    }

    /// Returns the exact build locale.
    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Returns the exact build timezone.
    #[must_use]
    pub fn timezone(&self) -> &str {
        &self.timezone
    }

    /// Returns the selected reproducible-build source epoch.
    #[must_use]
    pub const fn source_date_epoch(&self) -> u64 {
        self.source_date_epoch
    }

    /// Returns the exact sorted environment applied after clearing inheritance.
    #[must_use]
    pub fn environment(&self) -> &[RuntimeSourceBuildEnvironmentVariable] {
        &self.environment
    }

    /// Returns the exact ordered arguments passed to the controlled build program.
    #[must_use]
    pub fn build_arguments(&self) -> &[String] {
        &self.build_arguments
    }

    /// Returns the digest of the cleared and explicitly selected environment.
    #[must_use]
    pub const fn environment_digest(&self) -> &Digest {
        &self.environment_digest
    }

    /// Returns the digest of the exact ordered build arguments.
    #[must_use]
    pub const fn build_arguments_digest(&self) -> &Digest {
        &self.build_arguments_digest
    }
}

pub(super) fn sequence_digest<'a>(
    domain: &[u8],
    values: impl IntoIterator<Item = &'a [u8]>,
) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for value in values {
        hasher.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(value);
    }
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .expect("SHA-256 formatting always produces one valid digest")
}

/// Canonical frozen closure for one controlled runtime source build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildInputManifest {
    artifact_set: ArtifactSetManifest,
    policy: RuntimeSourceBuildPolicy,
    components: Vec<RuntimeSourceBuildInputComponent>,
    manifest_digest: Digest,
}

/// Controlled-build input manifest whose every declared component byte was verified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeSourceBuildInputs {
    manifest: RuntimeSourceBuildInputManifest,
    program_lineage: Option<VerifiedRetainedProgramLineage>,
}

impl VerifiedRuntimeSourceBuildInputs {
    /// Returns the canonical manifest for the verified frozen closure.
    #[must_use]
    pub const fn manifest(&self) -> &RuntimeSourceBuildInputManifest {
        &self.manifest
    }

    /// Returns the exact verified retained-program lineage when the manifest
    /// declares the complete lineage contract.
    ///
    /// Older manifests remain verifiable as byte closures but return `None` and
    /// therefore cannot be mistaken for lineage-verified inputs.
    #[must_use]
    pub const fn retained_program_lineage(&self) -> Option<&VerifiedRetainedProgramLineage> {
        self.program_lineage.as_ref()
    }
}

impl RuntimeSourceBuildInputManifest {
    /// Parses and validates one canonical controlled-build input manifest.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildInputError`] for malformed, noncanonical,
    /// incomplete, unsupported, excessive, or internally inconsistent input.
    pub fn parse(
        bytes: &[u8],
        limits: RuntimeSourceBuildInputLimits,
    ) -> Result<Self, RuntimeSourceBuildInputError> {
        parse_runtime_source_build_input_manifest(bytes, limits)
    }

    /// Returns the derived canonical artifact set for every frozen input byte.
    #[must_use]
    pub const fn artifact_set(&self) -> &ArtifactSetManifest {
        &self.artifact_set
    }

    /// Returns the exact build policy.
    #[must_use]
    pub const fn policy(&self) -> &RuntimeSourceBuildPolicy {
        &self.policy
    }

    /// Returns components in canonical portable path order.
    #[must_use]
    pub fn components(&self) -> &[RuntimeSourceBuildInputComponent] {
        &self.components
    }

    /// Returns the domain-separated digest of the complete canonical manifest.
    ///
    /// Unlike the artifact-set identity, this digest also binds component names,
    /// revisions, source locators, roles, and the complete reviewed policy.
    #[must_use]
    pub const fn manifest_digest(&self) -> &Digest {
        &self.manifest_digest
    }
}

#[cfg(test)]
mod tests;
