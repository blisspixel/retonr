use rewrite_model::{
    ArtifactSetId, ArtifactSetRelativePath, PackageSourceKind, PackageTransformation,
    RuntimePackageManifestId,
};
use rewrite_types::Digest;
use serde::Deserialize;

use crate::{
    ReconstructedRuntimePackage, RuntimeLayoutLimits, RuntimeSourceBuildInputManifest,
    RuntimeSourceBuildInputRole,
};

mod compile;
mod error;
mod execution_receipt;
mod output_tree;
mod parse;
mod verify;

pub(crate) fn validates_transformed_runtime_binding(
    source_inputs: &RuntimeSourceBuildInputManifest,
    runtime: &ReconstructedRuntimePackage,
    standard_output_digest: Option<&Digest>,
) -> bool {
    let component = |role| {
        source_inputs
            .components()
            .iter()
            .find(|component| component.roles().contains(&role))
    };
    let Some(ollama) = component(RuntimeSourceBuildInputRole::OllamaSource) else {
        return false;
    };
    let Some(source_provenance) = component(RuntimeSourceBuildInputRole::SourceProvenance) else {
        return false;
    };
    let Some(parameters) = component(RuntimeSourceBuildInputRole::BuildParameters) else {
        return false;
    };
    let Some(tool_evidence) = component(RuntimeSourceBuildInputRole::ToolEvidence) else {
        return false;
    };
    let package = runtime.runtime_package();
    let source = package.source();
    if package.build_revision() != Some(ollama.revision())
        || source.kind() != PackageSourceKind::RepositoryRevision
        || source.revision() != ollama.revision()
        || source.provenance_digest() != source_provenance.digest()
    {
        return false;
    }
    matches!(
        package.transformation(),
        PackageTransformation::Transformed {
            source_artifact_set_id,
            tool_evidence_digest,
            parameters_digest,
            log_digest,
        } if source_artifact_set_id == &source_inputs.artifact_set().artifact_set_id()
            && tool_evidence_digest == tool_evidence.digest()
            && parameters_digest == parameters.digest()
            && standard_output_digest.is_none_or(|expected| log_digest == expected)
    )
}

pub use compile::compile_runtime_source_build_report;
pub use error::{RuntimeSourceBuildReportError, RuntimeSourceBuildReportOpenError};
pub use execution_receipt::{
    VerifiedRuntimeSourceBuildExecutionReceipt, verify_runtime_source_build_execution_receipt,
};
pub use output_tree::{
    RUNTIME_SOURCE_BUILD_OUTPUT_TREE_SCHEMA_VERSION, RuntimeSourceBuildOutputTree,
    RuntimeSourceBuildOutputTreeEntry, RuntimeSourceBuildOutputTreeEntryKind,
};
pub use verify::verify_runtime_source_build_report;

/// Controlled source-build report schema version.
pub const RUNTIME_SOURCE_BUILD_REPORT_SCHEMA_VERSION: u32 = 3;

const DEFAULT_REPORT_BYTES: usize = 256 * 1024;
const DEFAULT_EVIDENCE_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_TOTAL_EVIDENCE_BYTES: u64 = 64 * 1024 * 1024;

/// Caller-owned ceilings for one two-attempt controlled-build report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildReportLimits {
    /// Maximum canonical report JSON bytes.
    pub report_bytes: usize,
    /// Maximum bytes in any one referenced metadata record.
    pub maximum_evidence_bytes: u64,
    /// Maximum aggregate bytes across all referenced metadata records.
    pub maximum_total_evidence_bytes: u64,
    /// Limits used to parse and reconstruct each runtime layout.
    pub runtime_layout: RuntimeLayoutLimits,
}

impl Default for RuntimeSourceBuildReportLimits {
    fn default() -> Self {
        Self {
            report_bytes: DEFAULT_REPORT_BYTES,
            maximum_evidence_bytes: DEFAULT_EVIDENCE_BYTES,
            maximum_total_evidence_bytes: DEFAULT_TOTAL_EVIDENCE_BYTES,
            runtime_layout: RuntimeLayoutLimits::default(),
        }
    }
}

impl RuntimeSourceBuildReportLimits {
    /// Validates that each selected ceiling is nonzero, internally coherent,
    /// and no greater than its fixed contract maximum.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildReportError::LimitExceeded`] for an invalid
    /// or relaxed ceiling.
    pub fn validate(self) -> Result<Self, RuntimeSourceBuildReportError> {
        if self.report_bytes == 0
            || self.report_bytes > DEFAULT_REPORT_BYTES
            || self.maximum_evidence_bytes == 0
            || self.maximum_evidence_bytes > DEFAULT_EVIDENCE_BYTES
            || self.maximum_total_evidence_bytes == 0
            || self.maximum_total_evidence_bytes > DEFAULT_TOTAL_EVIDENCE_BYTES
            || self.maximum_evidence_bytes > self.maximum_total_evidence_bytes
            || self.runtime_layout.validate().is_err()
        {
            Err(RuntimeSourceBuildReportError::LimitExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Identity of one independent controlled-build attempt.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSourceBuildAttempt {
    /// First controlled build.
    Primary,
    /// Independently recreated controlled build.
    Rebuild,
}

/// Purpose of one byte-bound build metadata record.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeSourceBuildEvidenceKind {
    /// Canonical final runtime layout.
    RuntimeLayout,
    /// Software bill of materials.
    Sbom,
    /// Source and toolchain provenance statement.
    Provenance,
    /// Source-to-output transformation record.
    Transformation,
    /// Portable path, kind, mode, size, and digest closure for the original output.
    OutputTree,
    /// Canonical typed managed-build execution receipt.
    ExecutionReceipt,
    /// Bounded standard output bytes.
    StandardOutput,
    /// Bounded standard error bytes.
    StandardError,
}

pub(crate) const RUNTIME_SOURCE_BUILD_EVIDENCE_KINDS: [RuntimeSourceBuildEvidenceKind; 8] = [
    RuntimeSourceBuildEvidenceKind::RuntimeLayout,
    RuntimeSourceBuildEvidenceKind::Sbom,
    RuntimeSourceBuildEvidenceKind::Provenance,
    RuntimeSourceBuildEvidenceKind::Transformation,
    RuntimeSourceBuildEvidenceKind::OutputTree,
    RuntimeSourceBuildEvidenceKind::ExecutionReceipt,
    RuntimeSourceBuildEvidenceKind::StandardOutput,
    RuntimeSourceBuildEvidenceKind::StandardError,
];

pub(crate) fn canonical_evidence_path(
    attempt: RuntimeSourceBuildAttempt,
    kind: RuntimeSourceBuildEvidenceKind,
) -> Result<ArtifactSetRelativePath, RuntimeSourceBuildReportError> {
    ArtifactSetRelativePath::new(format!(
        "attempts/{}/{}",
        attempt_name(attempt),
        evidence_file_name(kind)
    ))
    .map_err(|_| RuntimeSourceBuildReportError::InvalidAttempt)
}

pub(crate) const fn attempt_name(attempt: RuntimeSourceBuildAttempt) -> &'static str {
    match attempt {
        RuntimeSourceBuildAttempt::Primary => "primary",
        RuntimeSourceBuildAttempt::Rebuild => "rebuild",
    }
}

const fn evidence_file_name(kind: RuntimeSourceBuildEvidenceKind) -> &'static str {
    match kind {
        RuntimeSourceBuildEvidenceKind::RuntimeLayout => "runtime-layout.json",
        RuntimeSourceBuildEvidenceKind::Sbom => "sbom.json",
        RuntimeSourceBuildEvidenceKind::Provenance => "provenance.json",
        RuntimeSourceBuildEvidenceKind::Transformation => "transformation.json",
        RuntimeSourceBuildEvidenceKind::OutputTree => "output-tree.json",
        RuntimeSourceBuildEvidenceKind::ExecutionReceipt => "execution-receipt.json",
        RuntimeSourceBuildEvidenceKind::StandardOutput => "stdout.bin",
        RuntimeSourceBuildEvidenceKind::StandardError => "stderr.bin",
    }
}

/// Independently derived comparison of the two runtime outputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeSourceBuildComparison {
    /// Both attempts produced the same complete output tree, portable Unix
    /// modes, exact artifact set, and semantic package.
    ByteIdentical {
        /// Shared exact runtime artifact-set identity.
        artifact_set_id: ArtifactSetId,
        /// Shared semantic runtime-package identity.
        runtime_package_manifest_id: RuntimePackageManifestId,
        /// Shared original output-tree identity including portable Unix modes.
        output_tree_digest: Digest,
    },
    /// The attempts differed in output-tree structure, portable Unix modes,
    /// runtime bytes, or semantic package state.
    Different {
        /// Primary runtime artifact-set identity.
        primary_artifact_set_id: ArtifactSetId,
        /// Rebuild runtime artifact-set identity.
        rebuild_artifact_set_id: ArtifactSetId,
        /// Primary semantic runtime-package identity.
        primary_runtime_package_manifest_id: RuntimePackageManifestId,
        /// Rebuild semantic runtime-package identity.
        rebuild_runtime_package_manifest_id: RuntimePackageManifestId,
        /// Primary original output-tree identity including portable Unix modes.
        primary_output_tree_digest: Digest,
        /// Rebuild original output-tree identity including portable Unix modes.
        rebuild_output_tree_digest: Digest,
    },
}

/// Parsed content-free report for two successful controlled-build attempts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildReport {
    source_build_inputs_id: ArtifactSetId,
    build_plan_digest: Digest,
    attempts: Vec<BuildAttempt>,
    comparison: RuntimeSourceBuildComparison,
}

impl RuntimeSourceBuildReport {
    /// Returns the exact frozen input-set identity.
    #[must_use]
    pub const fn source_build_inputs_id(&self) -> &ArtifactSetId {
        &self.source_build_inputs_id
    }

    /// Returns the deterministic build-plan digest.
    #[must_use]
    pub const fn build_plan_digest(&self) -> &Digest {
        &self.build_plan_digest
    }

    /// Returns the declared two-output comparison.
    #[must_use]
    pub const fn comparison(&self) -> &RuntimeSourceBuildComparison {
        &self.comparison
    }

    /// Returns the byte-bound evidence records for one controlled-build attempt.
    ///
    /// # Panics
    ///
    /// Panics only if an already validated report is corrupted in memory and no
    /// longer contains both controlled-build attempts.
    #[must_use]
    pub fn evidence(
        &self,
        attempt: RuntimeSourceBuildAttempt,
    ) -> &[RuntimeSourceBuildEvidenceRecord] {
        &self
            .attempts
            .iter()
            .find(|item| item.attempt == attempt)
            .expect("validated report contains both attempts")
            .evidence
    }

    /// Returns the semantic output-tree digest for one controlled-build attempt.
    ///
    /// # Panics
    ///
    /// Panics only if an already validated report is corrupted in memory and no
    /// longer contains both controlled-build attempts.
    #[must_use]
    pub fn output_tree_digest(&self, attempt: RuntimeSourceBuildAttempt) -> &Digest {
        &self
            .attempts
            .iter()
            .find(|item| item.attempt == attempt)
            .expect("validated report contains both attempts")
            .output_tree_digest
    }
}

/// Build report whose metadata and both runtime output trees were byte-verified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeSourceBuildReport {
    report: RuntimeSourceBuildReport,
    primary: ReconstructedRuntimePackage,
    rebuild: ReconstructedRuntimePackage,
    primary_output_tree: RuntimeSourceBuildOutputTree,
    rebuild_output_tree: RuntimeSourceBuildOutputTree,
}

/// Canonical report bytes and both packages derived while compiling them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRuntimeSourceBuildReport {
    canonical_bytes: Vec<u8>,
    primary: ReconstructedRuntimePackage,
    rebuild: ReconstructedRuntimePackage,
    primary_output_tree: RuntimeSourceBuildOutputTree,
    rebuild_output_tree: RuntimeSourceBuildOutputTree,
}

impl CompiledRuntimeSourceBuildReport {
    /// Returns the canonical content-free report bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the package reconstructed from the primary output.
    #[must_use]
    pub const fn primary(&self) -> &ReconstructedRuntimePackage {
        &self.primary
    }

    /// Returns the package reconstructed from the rebuild output.
    #[must_use]
    pub const fn rebuild(&self) -> &ReconstructedRuntimePackage {
        &self.rebuild
    }

    /// Returns the portable original output-tree identity for the primary attempt.
    #[must_use]
    pub const fn primary_output_tree(&self) -> &RuntimeSourceBuildOutputTree {
        &self.primary_output_tree
    }

    /// Returns the portable original output-tree identity for the rebuild attempt.
    #[must_use]
    pub const fn rebuild_output_tree(&self) -> &RuntimeSourceBuildOutputTree {
        &self.rebuild_output_tree
    }

    /// Reports whether both complete output trees, portable Unix modes, artifact
    /// sets, and package identities match.
    #[must_use]
    pub fn is_byte_identical(&self) -> bool {
        self.primary.artifact_set() == self.rebuild.artifact_set()
            && self.primary.runtime_package() == self.rebuild.runtime_package()
            && self.primary_output_tree == self.rebuild_output_tree
    }
}

impl VerifiedRuntimeSourceBuildReport {
    /// Returns the validated content-free report.
    #[must_use]
    pub const fn report(&self) -> &RuntimeSourceBuildReport {
        &self.report
    }

    /// Returns the reconstructed primary runtime package.
    #[must_use]
    pub const fn primary(&self) -> &ReconstructedRuntimePackage {
        &self.primary
    }

    /// Returns the reconstructed rebuild runtime package.
    #[must_use]
    pub const fn rebuild(&self) -> &ReconstructedRuntimePackage {
        &self.rebuild
    }

    /// Returns the verified portable original output tree for the primary attempt.
    #[must_use]
    pub const fn primary_output_tree(&self) -> &RuntimeSourceBuildOutputTree {
        &self.primary_output_tree
    }

    /// Returns the verified portable original output tree for the rebuild attempt.
    #[must_use]
    pub const fn rebuild_output_tree(&self) -> &RuntimeSourceBuildOutputTree {
        &self.rebuild_output_tree
    }

    /// Reports whether both complete output trees, portable Unix modes, artifact
    /// sets, and semantic package identities match.
    #[must_use]
    pub const fn is_byte_identical(&self) -> bool {
        matches!(
            self.report.comparison,
            RuntimeSourceBuildComparison::ByteIdentical { .. }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuildAttempt {
    attempt: RuntimeSourceBuildAttempt,
    environment_digest: Digest,
    build_arguments_digest: Digest,
    execution_receipt_digest: Digest,
    output_tree_digest: Digest,
    evidence: Vec<RuntimeSourceBuildEvidenceRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One exact metadata record referenced by a controlled-build attempt.
pub struct RuntimeSourceBuildEvidenceRecord {
    kind: RuntimeSourceBuildEvidenceKind,
    relative_path: ArtifactSetRelativePath,
    byte_size: u64,
    digest: Digest,
}

impl RuntimeSourceBuildEvidenceRecord {
    /// Returns the closed purpose of this evidence record.
    #[must_use]
    pub const fn kind(&self) -> RuntimeSourceBuildEvidenceKind {
        self.kind
    }

    /// Returns the canonical path within the durable evidence closure.
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
}

#[cfg(test)]
mod tests;
