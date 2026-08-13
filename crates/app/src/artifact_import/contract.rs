use std::{io, path::PathBuf};

use rewrite_model::{ArtifactManifest, InstalledArtifact, ManifestError};
use rewrite_model_store::{InstallationWriteDisposition, StoreError};
use thiserror::Error;

/// Explicit request to import one immutable local artifact file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineArtifactImportRequest {
    /// Caller-selected source path. The source is opened read-only and never changed.
    pub source: PathBuf,
    /// Complete expected manifest, including exact byte size and digest.
    pub manifest: ArtifactManifest,
}

/// Caller-owned resource ceiling for offline artifact import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactImportLimits {
    /// Maximum bytes accepted for one imported artifact file.
    pub maximum_artifact_bytes: u64,
}

/// Result of durably staging and registering one local artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactImportResult {
    /// Verified application-owned installation state.
    pub installed: InstalledArtifact,
    /// Atomic durable-state write outcome.
    pub state: InstallationWriteDisposition,
}

/// Content-free lifecycle stage for an offline artifact import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactImportStage {
    /// Validate the manifest and inspect the source entry.
    InspectingSource,
    /// Copy and hash source bytes into application-owned staging.
    StagingAndVerifying,
    /// Hash source bytes without staging because exact final bytes already exist.
    VerifyingSource,
    /// Commit verified staged bytes under their content-derived key.
    CommittingFile,
    /// Reverify a final file that another exact import already committed.
    VerifyingExistingFile,
    /// Atomically register the manifest and installed-artifact state.
    RegisteringState,
    /// The file and durable state both committed successfully.
    Complete,
}

/// Content-free progress snapshot for one offline artifact import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactImportProgress {
    /// Current lifecycle stage.
    pub stage: ArtifactImportStage,
    /// Bytes processed in the current byte-processing stage.
    pub completed_bytes: u64,
    /// Exact total bytes declared by the validated manifest.
    pub total_bytes: u64,
}

/// Failure from the offline artifact import trust boundary.
#[derive(Debug, Error)]
pub enum ArtifactImportError {
    /// The configured import resource ceiling was zero.
    #[error("artifact import limits are invalid")]
    InvalidLimits,
    /// The supplied manifest failed domain validation.
    #[error("artifact manifest is invalid")]
    InvalidManifest(#[source] ManifestError),
    /// The source could not be inspected or read.
    #[error("artifact source could not be read")]
    SourceIo(#[source] io::Error),
    /// The source is a symlink, junction, reparse point, or other indirect path.
    #[error("artifact source must not be an indirect filesystem link")]
    IndirectSource,
    /// The opened source is not one regular file.
    #[error("artifact source must be one regular file")]
    SourceNotRegular,
    /// The manifest exceeds the configured single-artifact byte ceiling.
    #[error("artifact size {actual} exceeds the configured maximum {maximum}")]
    ArtifactTooLarge {
        /// Manifest-declared artifact size.
        actual: u64,
        /// Configured single-artifact ceiling.
        maximum: u64,
    },
    /// The complete source size did not match the immutable manifest.
    #[error("artifact source size does not match its manifest")]
    SizeMismatch,
    /// The complete source digest did not match the immutable manifest.
    #[error("artifact source digest does not match its manifest")]
    DigestMismatch,
    /// Cancellation was observed during a cancellable import or verification stage.
    #[error("artifact import was cancelled")]
    Cancelled,
    /// The application-owned storage layout was missing or unsafe.
    #[error("artifact storage layout is invalid")]
    UnsafeStorageLayout,
    /// Another process currently owns the artifact import service.
    #[error("artifact storage is already in use")]
    StorageInUse,
    /// Application-owned staging or persistence failed.
    #[error("artifact storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Existing bytes under the content-derived storage key did not match.
    #[error("artifact storage key contains conflicting bytes")]
    StorageConflict,
    /// Durable lifecycle-state registration failed.
    #[error("artifact state registration failed")]
    State(#[source] StoreError),
}
