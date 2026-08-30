use rewrite_model::{
    ArtifactSetId, ArtifactSetRelativePath, RuntimePackageManifestId, RuntimeTarget,
};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

use crate::{
    ReconstructedRuntimePackage, RuntimeLayoutLimits, RuntimePackageReviewCheck,
    RuntimePackageReviewCheckStatus, RuntimeSourceBuildInputLimits,
    RuntimeSourceBuildInputManifest,
};

mod compile;
mod error;
mod parse;
mod verify;

pub use compile::{
    CompiledRuntimePackageReviewV2, RuntimePackageReviewV2CheckInput,
    RuntimePackageReviewV2CompilationInput, RuntimePackageReviewV2EvidenceInput,
    compile_runtime_package_review_v2,
};
pub use error::{RuntimePackageReviewEvidenceOpenError, RuntimePackageReviewV2Error};
pub use verify::verify_runtime_package_review_v2;

/// Controlled source-build runtime-package review contract version.
pub const RUNTIME_PACKAGE_REVIEW_V2_SCHEMA_VERSION: u32 = 2;

const DEFAULT_REVIEW_BYTES: usize = 256 * 1024;
const DEFAULT_EVIDENCE_RECORDS: usize = 128;
const DEFAULT_EVIDENCE_BYTES: u64 = 4 * 1024 * 1024;
const DEFAULT_TOTAL_EVIDENCE_BYTES: u64 = 64 * 1024 * 1024;

/// Fixed ceilings for one controlled source-build review.
///
/// Explicit limits may only lower the defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePackageReviewV2Limits {
    /// Maximum encoded review JSON bytes.
    pub review_bytes: usize,
    /// Maximum number of referenced metadata records.
    pub maximum_evidence_records: usize,
    /// Maximum bytes in any one referenced metadata record.
    pub maximum_evidence_bytes: u64,
    /// Maximum aggregate bytes across referenced metadata records.
    pub maximum_total_evidence_bytes: u64,
    /// Limits applied while parsing and reconstructing the final runtime layout.
    pub runtime_layout: RuntimeLayoutLimits,
    /// Limits applied to the frozen controlled-build input manifest.
    pub source_build_inputs: RuntimeSourceBuildInputLimits,
}

impl Default for RuntimePackageReviewV2Limits {
    fn default() -> Self {
        Self {
            review_bytes: DEFAULT_REVIEW_BYTES,
            maximum_evidence_records: DEFAULT_EVIDENCE_RECORDS,
            maximum_evidence_bytes: DEFAULT_EVIDENCE_BYTES,
            maximum_total_evidence_bytes: DEFAULT_TOTAL_EVIDENCE_BYTES,
            runtime_layout: RuntimeLayoutLimits::default(),
            source_build_inputs: RuntimeSourceBuildInputLimits::default(),
        }
    }
}

impl RuntimePackageReviewV2Limits {
    /// Validates that each ceiling is nonzero, internally coherent, and no
    /// greater than its fixed contract maximum.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimePackageReviewV2Error::LimitExceeded`] for an invalid or
    /// relaxed ceiling.
    pub fn validate(self) -> Result<Self, RuntimePackageReviewV2Error> {
        if self.review_bytes == 0
            || self.review_bytes > DEFAULT_REVIEW_BYTES
            || self.maximum_evidence_records == 0
            || self.maximum_evidence_records > DEFAULT_EVIDENCE_RECORDS
            || self.maximum_evidence_bytes == 0
            || self.maximum_evidence_bytes > DEFAULT_EVIDENCE_BYTES
            || self.maximum_total_evidence_bytes == 0
            || self.maximum_total_evidence_bytes > DEFAULT_TOTAL_EVIDENCE_BYTES
            || self.maximum_evidence_bytes > self.maximum_total_evidence_bytes
            || self.runtime_layout.validate().is_err()
            || self.source_build_inputs.validate().is_err()
        {
            return Err(RuntimePackageReviewV2Error::LimitExceeded);
        }
        Ok(self)
    }
}

/// Purpose of one retained metadata record in a controlled source-build review.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePackageReviewEvidenceClass {
    /// Frozen source, module, toolchain, package, or license input metadata.
    FetchedInput,
    /// Builder, compiler, linker, build-script, or environment evidence.
    BuildTool,
    /// Provenance, SBOM, transformation, comparison, or final layout output.
    BuildOutput,
    /// Managed startup, isolation, native-load, connection, or teardown evidence.
    Execution,
}

/// Admission result carried by a controlled source-build review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimePackageReviewDispositionV2 {
    /// The candidate has no execution-policy authority.
    NotAdmitted {
        /// Exact controls preventing admission, in canonical control order.
        blockers: Vec<RuntimePackageReviewCheck>,
    },
    /// Every control passed and the declared identity was derived from verified bytes.
    Admitted {
        /// Evidence path containing the exact final runtime layout.
        runtime_layout: ArtifactSetRelativePath,
        /// Digest of the exact final runtime layout bytes.
        layout_digest: Digest,
        /// Content-derived identity of the reconstructed runtime package.
        runtime_package_manifest_id: RuntimePackageManifestId,
    },
}

/// Parsed, bounded declaration for one controlled source-build review.
///
/// Parsing validates shape and relationships but does not verify referenced bytes.
/// Only [`VerifiedRuntimePackageReviewV2`] represents a byte-verified review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimePackageReviewV2 {
    runtime_family: String,
    reported_version: String,
    build_revision: String,
    target: RuntimeTarget,
    source_build_inputs_path: ArtifactSetRelativePath,
    source_build_inputs_id: ArtifactSetId,
    evidence: Vec<ReviewEvidence>,
    checks: Vec<ReviewCheckResult>,
    disposition: RuntimePackageReviewDispositionV2,
}

impl RuntimePackageReviewV2 {
    /// Returns the reviewed runtime family.
    #[must_use]
    pub fn runtime_family(&self) -> &str {
        &self.runtime_family
    }

    /// Returns the exact reported runtime version.
    #[must_use]
    pub fn reported_version(&self) -> &str {
        &self.reported_version
    }

    /// Returns the exact source revision selected for the build.
    #[must_use]
    pub fn build_revision(&self) -> &str {
        &self.build_revision
    }

    /// Returns the reviewed native target.
    #[must_use]
    pub const fn target(&self) -> RuntimeTarget {
        self.target
    }

    /// Returns the declared frozen source-build input-set identity.
    #[must_use]
    pub const fn source_build_inputs_id(&self) -> &ArtifactSetId {
        &self.source_build_inputs_id
    }

    /// Returns the number of byte-bound metadata records.
    #[must_use]
    pub fn evidence_count(&self) -> usize {
        self.evidence.len()
    }

    /// Returns the result for one required review control.
    ///
    /// # Panics
    ///
    /// Panics only if an already validated review is corrupted in memory.
    #[must_use]
    pub fn check_status(
        &self,
        check: RuntimePackageReviewCheck,
    ) -> RuntimePackageReviewCheckStatus {
        self.checks
            .iter()
            .find(|result| result.check == check)
            .expect("validated review contains every control")
            .status
    }

    /// Returns the declared admission disposition.
    #[must_use]
    pub const fn disposition(&self) -> &RuntimePackageReviewDispositionV2 {
        &self.disposition
    }
}

/// A controlled source-build review whose metadata and, when admitted, package
/// member bytes have been independently verified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimePackageReviewV2 {
    review: RuntimePackageReviewV2,
    source_build_inputs: RuntimeSourceBuildInputManifest,
    reconstructed_runtime: Option<ReconstructedRuntimePackage>,
}

impl VerifiedRuntimePackageReviewV2 {
    /// Returns the validated review declaration.
    #[must_use]
    pub const fn review(&self) -> &RuntimePackageReviewV2 {
        &self.review
    }

    /// Returns the canonical frozen source-build input manifest.
    #[must_use]
    pub const fn source_build_inputs(&self) -> &RuntimeSourceBuildInputManifest {
        &self.source_build_inputs
    }

    /// Returns the reconstructed runtime only for an admitted review.
    #[must_use]
    pub const fn reconstructed_runtime(&self) -> Option<&ReconstructedRuntimePackage> {
        self.reconstructed_runtime.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewEvidence {
    class: RuntimePackageReviewEvidenceClass,
    relative_path: ArtifactSetRelativePath,
    byte_size: u64,
    digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewCheckResult {
    check: RuntimePackageReviewCheck,
    status: RuntimePackageReviewCheckStatus,
    evidence: Vec<ArtifactSetRelativePath>,
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::{
    runtime_fixture as controlled_runtime_fixture, source_fixture as controlled_source_fixture,
};
