use std::{ffi::OsString, io, path::PathBuf};

use rewrite_model::{ArtifactSetManifestError, ArtifactSetRelativePath};
use rewrite_ollama_package::{
    RuntimeSourceBuildInputError, RuntimeSourceBuildInputLimits, RuntimeSourceBuildReportError,
};
use thiserror::Error;

use crate::{
    ArtifactInventoryError, RuntimeSourceBuildBundleError, RuntimeSourceBuildExecutionError,
    RuntimeSourceBuildReportCompilationError, RuntimeSourceBuildReportCompilationLimits,
};

/// Canonical manifest path inside a durable controlled-build evidence root.
pub const RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_MANIFEST_PATH: &str =
    "evidence-bundle-manifest.json";
/// Canonical frozen-input manifest path inside a durable evidence root.
pub const RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH: &str = "source-build-inputs.json";
/// Canonical inert retained-program plan binding inside a durable evidence root.
pub const RUNTIME_SOURCE_BUILD_PLAN_BINDING_PATH: &str = "retained-program-plan-binding.json";
/// Canonical two-attempt report path inside a durable evidence root.
pub const RUNTIME_SOURCE_BUILD_REPORT_PATH: &str = "source-build-report.json";
/// Hard ceiling for files plus directories in one durable evidence root.
pub const MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_TREE_ENTRIES: usize = 275_000;
/// Hard aggregate byte ceiling for one durable evidence closure.
pub const MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_BYTES: u64 = 2 * 1_024_u64.pow(4);
/// Hard ceiling for direct entries in a destination parent.
pub const MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_DESTINATION_ENTRIES: usize = 262_144;
/// Hard ceiling for direct entries while reserving a staging root.
pub const MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_STAGING_ROOTS: usize = 1_024;

/// Caller-owned ceilings for publication and later verification of one closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildEvidenceBundleLimits {
    /// Frozen-input manifest and component ceilings.
    pub source_build_inputs: RuntimeSourceBuildInputLimits,
    /// Two-attempt report compilation and output-tree ceilings.
    pub report_compilation: RuntimeSourceBuildReportCompilationLimits,
    /// Maximum files plus directories in the durable closure.
    pub maximum_tree_entries: usize,
    /// Maximum aggregate bytes in manifest-declared closure members.
    pub maximum_total_bytes: u64,
    /// Maximum direct entries permitted in the destination parent.
    pub maximum_destination_entries: usize,
    /// Maximum direct entries permitted while reserving a staging root.
    pub maximum_staging_roots: usize,
}

impl Default for RuntimeSourceBuildEvidenceBundleLimits {
    fn default() -> Self {
        Self {
            source_build_inputs: RuntimeSourceBuildInputLimits::default(),
            report_compilation: RuntimeSourceBuildReportCompilationLimits::default(),
            maximum_tree_entries: MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_TREE_ENTRIES,
            maximum_total_bytes: MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_BYTES,
            maximum_destination_entries: MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_DESTINATION_ENTRIES,
            maximum_staging_roots: MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_STAGING_ROOTS,
        }
    }
}

impl RuntimeSourceBuildEvidenceBundleLimits {
    pub(super) fn validate(self) -> Result<Self, RuntimeSourceBuildEvidenceBundleError> {
        self.source_build_inputs.validate()?;
        self.report_compilation.report.validate()?;
        if self.maximum_tree_entries == 0
            || self.maximum_tree_entries > MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_TREE_ENTRIES
            || self.report_compilation.maximum_output_tree_entries == 0
            || self.report_compilation.maximum_output_tree_entries
                > crate::MAX_RUNTIME_SOURCE_BUILD_OUTPUT_TREE_ENTRIES
            || self.maximum_total_bytes == 0
            || self.maximum_total_bytes > MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_BUNDLE_BYTES
            || self.maximum_destination_entries == 0
            || self.maximum_destination_entries
                > MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_DESTINATION_ENTRIES
            || self.maximum_staging_roots == 0
            || self.maximum_staging_roots > MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_STAGING_ROOTS
        {
            return Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded);
        }
        Ok(self)
    }
}

/// Caller-selected absent path for no-replace evidence publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildEvidenceBundleDestination {
    pub(super) path: PathBuf,
    pub(super) parent: PathBuf,
    pub(super) name: OsString,
}

impl RuntimeSourceBuildEvidenceBundleDestination {
    /// Forms one absolute, portable final selection without creating it.
    ///
    /// The parent must already exist when publication begins. The final entry
    /// must be absent and is never replaced.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildEvidenceBundleError::InvalidPath`] when the
    /// path cannot be made absolute, or `UnsafeBoundary` for a nonportable name.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, RuntimeSourceBuildEvidenceBundleError> {
        let path = std::path::absolute(path.into())
            .map_err(RuntimeSourceBuildEvidenceBundleError::InvalidPath)?;
        let parent = path
            .parent()
            .ok_or(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary)?
            .to_path_buf();
        let name = path
            .file_name()
            .ok_or(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary)?
            .to_os_string();
        let portable = name
            .to_str()
            .ok_or(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary)?;
        let parsed = ArtifactSetRelativePath::new(portable.to_owned())
            .map_err(|_| RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary)?;
        if parsed.as_str().contains('/') {
            return Err(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary);
        }
        Ok(Self { path, parent, name })
    }

    /// Returns the selected absolute final path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Caller-selected existing durable evidence root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildEvidenceBundleSource {
    pub(super) path: PathBuf,
}

impl RuntimeSourceBuildEvidenceBundleSource {
    /// Forms one absolute evidence-root selection without opening it.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildEvidenceBundleError::InvalidPath`] when the
    /// path cannot be made absolute.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, RuntimeSourceBuildEvidenceBundleError> {
        let path = std::path::absolute(path.into())
            .map_err(RuntimeSourceBuildEvidenceBundleError::InvalidPath)?;
        Ok(Self { path })
    }

    /// Returns the selected absolute root path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Failure while publishing or verifying a durable controlled-build closure.
#[derive(Debug, Error)]
pub enum RuntimeSourceBuildEvidenceBundleError {
    /// A selected path could not be made absolute.
    #[error("runtime source-build evidence path is invalid")]
    InvalidPath(#[source] io::Error),
    /// A selected root is indirect, overlapping, special, or nonportable.
    #[error("runtime source-build evidence boundary is unsafe")]
    UnsafeBoundary,
    /// A selected or retained filesystem object could not be read or written.
    #[error("runtime source-build evidence storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Caller-owned ceilings were invalid or exceeded.
    #[error("runtime source-build evidence limit was exceeded")]
    LimitExceeded,
    /// A retained source, output, staging, or published object changed.
    #[error("runtime source-build evidence changed during the operation")]
    Changed,
    /// The durable tree did not contain exactly the declared closure.
    #[error("runtime source-build evidence tree does not match its manifest")]
    TreeMismatch,
    /// The inert retained-program identity did not reproduce the declared build plan.
    #[error("runtime source-build evidence plan binding is invalid")]
    InvalidPlanBinding,
    /// The complete closure manifest was invalid or noncanonical.
    #[error("runtime source-build evidence manifest is invalid")]
    Manifest(#[source] ArtifactSetManifestError),
    /// Cooperative cancellation was observed.
    #[error("runtime source-build evidence operation was cancelled")]
    Cancelled,
    /// The frozen source bundle failed revalidation.
    #[error(transparent)]
    SourceBundle(#[from] RuntimeSourceBuildBundleError),
    /// One retained controlled-build output failed revalidation.
    #[error(transparent)]
    Execution(#[from] RuntimeSourceBuildExecutionError),
    /// The two-attempt report could not be compiled from retained objects.
    #[error(transparent)]
    Compilation(#[from] RuntimeSourceBuildReportCompilationError),
    /// Frozen input bytes failed their pure contract.
    #[error(transparent)]
    Inputs(#[from] RuntimeSourceBuildInputError),
    /// Report, evidence, or reconstructed runtime bytes failed verification.
    #[error(transparent)]
    Report(#[from] RuntimeSourceBuildReportError),
}

pub(super) fn map_storage(error: ArtifactInventoryError) -> RuntimeSourceBuildEvidenceBundleError {
    match error {
        ArtifactInventoryError::StorageIo(error) => {
            RuntimeSourceBuildEvidenceBundleError::StorageIo(error)
        }
        ArtifactInventoryError::Cancelled => RuntimeSourceBuildEvidenceBundleError::Cancelled,
        ArtifactInventoryError::StorageEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::InvalidLimits => {
            RuntimeSourceBuildEvidenceBundleError::LimitExceeded
        }
        ArtifactInventoryError::ConcurrentModification => {
            RuntimeSourceBuildEvidenceBundleError::Changed
        }
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout
        | ArtifactInventoryError::StorageInUse
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::State(_) => RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary,
    }
}

pub(super) fn ensure_active(
    cancellation: &rewrite_types::CancellationToken,
) -> Result<(), RuntimeSourceBuildEvidenceBundleError> {
    if cancellation.is_cancelled() {
        Err(RuntimeSourceBuildEvidenceBundleError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn paths_overlap(left: &std::path::Path, right: &std::path::Path) -> bool {
    path_is_within(left, right) || path_is_within(right, left)
}

fn path_is_within(path: &std::path::Path, ancestor: &std::path::Path) -> bool {
    let path = path.components().collect::<Vec<_>>();
    let ancestor = ancestor.components().collect::<Vec<_>>();
    path.len() >= ancestor.len()
        && path
            .iter()
            .zip(&ancestor)
            .all(|(left, right)| left.as_os_str().eq_ignore_ascii_case(right.as_os_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_and_portable_destination_names_fail_before_io() {
        let limits = RuntimeSourceBuildEvidenceBundleLimits {
            maximum_total_bytes: 0,
            ..RuntimeSourceBuildEvidenceBundleLimits::default()
        };
        assert!(matches!(
            limits.validate(),
            Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded)
        ));
        for broadened in [
            RuntimeSourceBuildEvidenceBundleLimits {
                maximum_destination_entries: MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_DESTINATION_ENTRIES
                    + 1,
                ..RuntimeSourceBuildEvidenceBundleLimits::default()
            },
            RuntimeSourceBuildEvidenceBundleLimits {
                maximum_staging_roots: MAX_RUNTIME_SOURCE_BUILD_EVIDENCE_STAGING_ROOTS + 1,
                ..RuntimeSourceBuildEvidenceBundleLimits::default()
            },
        ] {
            assert!(matches!(
                broadened.validate(),
                Err(RuntimeSourceBuildEvidenceBundleError::LimitExceeded)
            ));
        }
        assert!(matches!(
            RuntimeSourceBuildEvidenceBundleDestination::new("CON"),
            Err(RuntimeSourceBuildEvidenceBundleError::UnsafeBoundary)
        ));
        assert!(RuntimeSourceBuildEvidenceBundleDestination::new("durable-evidence").is_ok());
    }

    #[test]
    fn overlap_is_component_aware_and_case_insensitive() {
        let root = std::path::Path::new("C:/evidence/root");
        assert!(paths_overlap(
            root,
            std::path::Path::new("c:/EVIDENCE/root/child")
        ));
        assert!(!paths_overlap(
            root,
            std::path::Path::new("C:/evidence/rooted")
        ));
    }
}
