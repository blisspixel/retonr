use std::{ffi::OsString, io, path::PathBuf};

use rewrite_inference::CandidateOutputError;
use rewrite_model::{ArtifactSetRelativePath, GenerationQualificationContractError};
use rewrite_types::{CancellationToken, Digest};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::{ArtifactInventoryError, artifact_storage::is_indirect};

use super::response_artifact::RetainedStructuredResponseArtifactError;

/// Hard ceiling for files plus implied directories in one evidence bundle.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TREE_ENTRIES: usize = 8_193;
/// Hard ceiling for path components in one evidence member.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TREE_DEPTH: usize = 256;
/// Hard ceiling for all content and manifest bytes in one evidence tree.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TOTAL_BYTES: u64 = 256 * 1_024 * 1_024;
/// Hard ceiling for direct entries in the publication parent.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES: usize = 4_096;
/// Hard ceiling for direct entries while reserving a staging root.
pub const MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_STAGING_ROOTS: usize = 1_024;

/// Caller-owned ceilings for publication and later readback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateGenerationEvidenceBundleLimits {
    /// Maximum files plus implied directories, including the reserved manifest.
    pub maximum_tree_entries: usize,
    /// Maximum path components in one member.
    pub maximum_tree_depth: usize,
    /// Maximum aggregate content and reserved-manifest bytes.
    pub maximum_total_bytes: u64,
    /// Maximum direct entries permitted in the destination parent.
    pub maximum_destination_entries: usize,
    /// Maximum direct entries permitted while reserving a staging root.
    pub maximum_staging_roots: usize,
}

impl Default for CandidateGenerationEvidenceBundleLimits {
    fn default() -> Self {
        Self {
            maximum_tree_entries: MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TREE_ENTRIES,
            maximum_tree_depth: MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TREE_DEPTH,
            maximum_total_bytes: MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TOTAL_BYTES,
            maximum_destination_entries:
                MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES,
            maximum_staging_roots: MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_STAGING_ROOTS,
        }
    }
}

impl CandidateGenerationEvidenceBundleLimits {
    pub(super) fn validate(self) -> Result<Self, CandidateGenerationEvidenceBundleError> {
        if self.maximum_tree_entries == 0
            || self.maximum_tree_entries > MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TREE_ENTRIES
            || self.maximum_tree_depth == 0
            || self.maximum_tree_depth > MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TREE_DEPTH
            || self.maximum_total_bytes == 0
            || self.maximum_total_bytes > MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_TOTAL_BYTES
            || self.maximum_destination_entries == 0
            || self.maximum_destination_entries
                > MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_DESTINATION_ENTRIES
            || self.maximum_staging_roots == 0
            || self.maximum_staging_roots > MAX_CANDIDATE_GENERATION_EVIDENCE_BUNDLE_STAGING_ROOTS
        {
            Err(CandidateGenerationEvidenceBundleError::LimitExceeded)
        } else {
            Ok(self)
        }
    }
}

/// Caller-selected absent destination for atomic no-replace publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateGenerationEvidenceBundleDestination {
    pub(super) path: PathBuf,
    pub(super) parent: PathBuf,
    pub(super) name: OsString,
}

impl CandidateGenerationEvidenceBundleDestination {
    /// Forms one absolute portable destination without creating it.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceBundleError`] for an invalid path or
    /// a nonportable final component.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, CandidateGenerationEvidenceBundleError> {
        let path = std::path::absolute(path.into())
            .map_err(CandidateGenerationEvidenceBundleError::InvalidPath)?;
        let parent = path
            .parent()
            .ok_or(CandidateGenerationEvidenceBundleError::UnsafeBoundary)?
            .to_path_buf();
        let name = path
            .file_name()
            .ok_or(CandidateGenerationEvidenceBundleError::UnsafeBoundary)?
            .to_os_string();
        let portable = name
            .to_str()
            .ok_or(CandidateGenerationEvidenceBundleError::UnsafeBoundary)?;
        let parsed = ArtifactSetRelativePath::new(portable.to_owned())
            .map_err(|_| CandidateGenerationEvidenceBundleError::UnsafeBoundary)?;
        if parsed.as_str().contains('/') {
            return Err(CandidateGenerationEvidenceBundleError::UnsafeBoundary);
        }
        Ok(Self { path, parent, name })
    }

    /// Returns the selected absolute final path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Caller-selected existing evidence-bundle root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateGenerationEvidenceBundleSource {
    pub(super) path: PathBuf,
}

impl CandidateGenerationEvidenceBundleSource {
    /// Forms one absolute source selection without opening it.
    ///
    /// # Errors
    ///
    /// Returns [`CandidateGenerationEvidenceBundleError`] if the path cannot be
    /// made absolute.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, CandidateGenerationEvidenceBundleError> {
        Ok(Self {
            path: std::path::absolute(path.into())
                .map_err(CandidateGenerationEvidenceBundleError::InvalidPath)?,
        })
    }

    /// Returns the selected absolute source path.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Failure while compiling, publishing, or reading one generation evidence bundle.
#[derive(Debug, Error)]
pub enum CandidateGenerationEvidenceBundleError {
    /// A selected path could not be made absolute.
    #[error("generation evidence-bundle path is invalid")]
    InvalidPath(#[source] io::Error),
    /// A selected root, parent, name, link, or filesystem object is unsafe.
    #[error("generation evidence-bundle boundary is unsafe")]
    UnsafeBoundary,
    /// A caller, plan, tree, path, or aggregate ceiling was exceeded.
    #[error("generation evidence-bundle limit was exceeded")]
    LimitExceeded,
    /// An exact typed relationship, entry, role, digest, or size did not match.
    #[error("generation evidence-bundle publication plan does not match")]
    PlanMismatch,
    /// The final destination was already occupied and was not replaced.
    #[error("generation evidence-bundle destination already exists")]
    DestinationExists,
    /// A retained or named filesystem object changed during the operation.
    #[error("generation evidence-bundle storage changed during the operation")]
    Changed,
    /// The filesystem tree was not exactly the declared bundle closure.
    #[error("generation evidence-bundle tree does not match its manifest")]
    TreeMismatch,
    /// A selected or retained filesystem object could not be read or written.
    #[error("generation evidence-bundle storage operation failed")]
    StorageIo(#[source] io::Error),
    /// The model-owned portable contract rejected a record or relationship.
    #[error("generation evidence-bundle portable contract is invalid")]
    Contract(#[source] GenerationQualificationContractError),
    /// The exact retained response artifact failed reconstruction.
    #[error("generation evidence-bundle response artifact is invalid")]
    ResponseArtifact(#[source] RetainedStructuredResponseArtifactError),
    /// The retained response did not contain the one bounded candidate envelope.
    #[error("generation evidence-bundle candidate output is invalid")]
    CandidateOutput(#[source] CandidateOutputError),
    /// Cooperative cancellation was observed.
    #[error("generation evidence-bundle operation was cancelled")]
    Cancelled,
    /// Precommit cleanup failed after an earlier primary failure.
    #[error("generation evidence-bundle operation and staging cleanup both failed")]
    CleanupAfterFailure {
        /// The primary operation failure.
        #[source]
        primary: Box<Self>,
        /// The subsequent exact-ledger cleanup failure.
        cleanup: Box<Self>,
    },
    /// Publication committed, but fresh readback or finalization then failed.
    #[error("generation evidence bundle was published but readback failed")]
    PublishedButReadbackFailed {
        /// The post-commit failure. The committed root was not removed.
        #[source]
        source: Box<Self>,
    },
}

impl CandidateGenerationEvidenceBundleError {
    pub(super) fn cleanup_after(primary: Self, cleanup: Self) -> Self {
        Self::CleanupAfterFailure {
            primary: Box::new(primary),
            cleanup: Box::new(cleanup),
        }
    }

    pub(super) fn published_but_failed(source: Self) -> Self {
        Self::PublishedButReadbackFailed {
            source: Box::new(source),
        }
    }
}

pub(super) fn ensure_active(
    cancellation: &CancellationToken,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    if cancellation.is_cancelled() {
        Err(CandidateGenerationEvidenceBundleError::Cancelled)
    } else {
        Ok(())
    }
}

pub(super) fn digest_bytes(
    bytes: &[u8],
    cancellation: &CancellationToken,
) -> Result<Digest, CandidateGenerationEvidenceBundleError> {
    let mut hasher = Sha256::new();
    for chunk in bytes.chunks(64 * 1_024) {
        ensure_active(cancellation)?;
        hasher.update(chunk);
    }
    ensure_active(cancellation)?;
    Digest::from_sha256_hex(format!("{:x}", hasher.finalize()))
        .map_err(|_| CandidateGenerationEvidenceBundleError::PlanMismatch)
}

pub(super) fn reject_indirect_directory(
    path: &std::path::Path,
) -> Result<(), CandidateGenerationEvidenceBundleError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(CandidateGenerationEvidenceBundleError::StorageIo)?;
    if is_indirect(&metadata) || !metadata.is_dir() {
        Err(CandidateGenerationEvidenceBundleError::UnsafeBoundary)
    } else {
        Ok(())
    }
}

pub(super) fn map_storage(error: ArtifactInventoryError) -> CandidateGenerationEvidenceBundleError {
    match error {
        ArtifactInventoryError::StorageIo(error) => {
            CandidateGenerationEvidenceBundleError::StorageIo(error)
        }
        ArtifactInventoryError::Cancelled => CandidateGenerationEvidenceBundleError::Cancelled,
        ArtifactInventoryError::StorageEntryLimitExceeded
        | ArtifactInventoryError::TotalVerificationLimitExceeded
        | ArtifactInventoryError::InvalidLimits => {
            CandidateGenerationEvidenceBundleError::LimitExceeded
        }
        ArtifactInventoryError::ConcurrentModification => {
            CandidateGenerationEvidenceBundleError::Changed
        }
        ArtifactInventoryError::StorageNotInitialized
        | ArtifactInventoryError::UnsafeStorageLayout
        | ArtifactInventoryError::StorageInUse
        | ArtifactInventoryError::StateEntryLimitExceeded
        | ArtifactInventoryError::State(_) => {
            CandidateGenerationEvidenceBundleError::UnsafeBoundary
        }
    }
}
