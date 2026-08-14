use std::{io, path::PathBuf};

use rewrite_model::{
    ArtifactSetManifest, ArtifactSetManifestError, InstalledArtifactSet, InstalledArtifactSetError,
};
use rewrite_model_store::{ArtifactSetInstallationWriteDisposition, StoreError};
use thiserror::Error;

/// Explicit request to import one complete local artifact-set directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfflineArtifactSetImportRequest {
    /// Caller-selected source root. The complete tree is opened read-only.
    pub source_root: PathBuf,
    /// Complete expected regular-file manifest in canonical path order.
    pub manifest: ArtifactSetManifest,
}

/// Caller-owned resource ceilings for one offline artifact-set import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetImportLimits {
    /// Maximum members admitted from the manifest.
    pub maximum_members: usize,
    /// Maximum bytes admitted for any one member.
    pub maximum_member_bytes: u64,
    /// Maximum checked sum of all member bytes.
    pub maximum_total_bytes: u64,
    /// Maximum files and directories admitted in the exact source or final tree.
    pub maximum_tree_entries: usize,
    /// Maximum complete set roots admitted in managed set storage after import.
    pub maximum_storage_entries: usize,
    /// Maximum direct entries allowed in managed staging, including the root
    /// reserved by this import.
    ///
    /// Existing entries are counted but never descended into or removed implicitly.
    pub maximum_staging_entries: usize,
}

/// Result of publishing and registering one complete managed artifact set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetImportResult {
    /// Structurally validated application-owned set-root record.
    pub installed: InstalledArtifactSet,
    /// Atomic durable-state write outcome and exact installation epoch.
    pub state: ArtifactSetInstallationWriteDisposition,
    /// Whether this call published, registered, or reconfirmed the exact set.
    pub disposition: ArtifactSetImportDisposition,
}

/// Successful artifact-set import disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactSetImportDisposition {
    /// A new managed tree was published and registered.
    Imported,
    /// An exact published orphan was verified and registered.
    RegisteredExisting,
    /// Exact managed bytes and durable installation state already existed.
    AlreadyPresent,
}

/// Content-free lifecycle stage for an offline artifact-set import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactSetImportStage {
    /// Validate the manifest and exact source-tree shape.
    InspectingSource,
    /// Copy and hash source members into an application-owned staging tree.
    StagingAndVerifying,
    /// Verify source members when an exact final set root already exists.
    VerifyingSource,
    /// Synchronize and prepare the complete staging tree for publication.
    PublishingTree,
    /// Enter the silent publish, final verification, and state-registration tail.
    ///
    /// This is the last callback. Cancellation is checked after it and immediately
    /// before publication; no callback or cancellation check follows publication.
    Finalizing,
}

/// Content-free progress snapshot for one offline artifact-set import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetImportProgress {
    /// Current lifecycle stage.
    pub stage: ArtifactSetImportStage,
    /// Complete members processed in the current byte-processing stage.
    pub completed_members: usize,
    /// Exact member count declared by the validated manifest.
    pub total_members: usize,
    /// Bytes processed in the current byte-processing stage.
    pub completed_bytes: u64,
    /// Exact total bytes declared by the validated manifest.
    pub total_bytes: u64,
}

/// Failure from the offline artifact-set import trust boundary.
#[derive(Debug, Error)]
pub enum ArtifactSetImportError {
    /// One or more configured import ceilings were zero or inconsistent.
    #[error("artifact-set import limits are invalid")]
    InvalidLimits,
    /// The supplied canonical artifact-set manifest failed domain validation.
    #[error("artifact-set manifest is invalid")]
    InvalidManifest(#[source] ArtifactSetManifestError),
    /// The application-derived installed-set record failed domain validation.
    #[error("artifact-set installation record is invalid")]
    InvalidInstallation(#[source] InstalledArtifactSetError),
    /// The manifest member count exceeds the configured ceiling.
    #[error("artifact-set member count {actual} exceeds the configured maximum {maximum}")]
    TooManyMembers {
        /// Manifest-declared member count.
        actual: usize,
        /// Configured member-count ceiling.
        maximum: usize,
    },
    /// A manifest member exceeds the configured per-member byte ceiling.
    #[error("artifact-set member size {actual} exceeds the configured maximum {maximum}")]
    MemberTooLarge {
        /// Manifest-declared member size.
        actual: u64,
        /// Configured per-member ceiling.
        maximum: u64,
    },
    /// The manifest total exceeds the configured aggregate byte ceiling.
    #[error("artifact-set size {actual} exceeds the configured maximum {maximum}")]
    ArtifactSetTooLarge {
        /// Manifest-declared aggregate size.
        actual: u64,
        /// Configured aggregate ceiling.
        maximum: u64,
    },
    /// The source root could not be inspected or read.
    #[error("artifact-set source could not be read")]
    SourceIo(#[source] io::Error),
    /// The source root is a symlink, junction, reparse point, or other indirect path.
    #[error("artifact-set source root must not be an indirect filesystem link")]
    IndirectSource,
    /// The opened source root is not one directory.
    #[error("artifact-set source root must be one directory")]
    SourceNotDirectory,
    /// A source descendant was indirect, special, noncanonical, or otherwise unsafe.
    #[error("artifact-set source tree contains an unsafe entry")]
    UnsafeSourceTree,
    /// Source entries do not exactly match the canonical manifest tree.
    #[error("artifact-set source tree does not match its manifest")]
    SourceTreeMismatch,
    /// A complete source member size did not match the immutable manifest.
    #[error("artifact-set source member size does not match its manifest")]
    SizeMismatch,
    /// A complete source member digest did not match the immutable manifest.
    #[error("artifact-set source member digest does not match its manifest")]
    DigestMismatch,
    /// Cancellation was observed before whole-tree publication.
    #[error("artifact-set import was cancelled")]
    Cancelled,
    /// The application-owned storage layout was missing or unsafe.
    #[error("artifact-set storage layout is invalid")]
    UnsafeStorageLayout,
    /// Another process currently owns the artifact storage lifecycle.
    #[error("artifact storage is already in use")]
    StorageInUse,
    /// Source or managed storage changed during a coherence-sensitive operation.
    #[error("artifact-set storage changed during import")]
    StorageChanged,
    /// Exact tree inspection exceeded the caller-owned entry ceiling.
    #[error("artifact-set tree exceeds the configured entry limit")]
    TreeEntryLimitExceeded,
    /// Direct staging entry count exceeded the caller-owned ceiling.
    #[error("artifact-set staging exceeds the configured entry limit")]
    StagingEntryLimitExceeded,
    /// Managed set storage reached the caller-owned set-root ceiling.
    #[error("managed artifact-set storage exceeds the configured entry limit")]
    StorageEntryLimitExceeded,
    /// Application-owned staging, publication, or durability work failed.
    #[error("artifact-set storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Existing bytes under the content-derived set-root key did not match.
    #[error("artifact-set storage key contains conflicting bytes")]
    StorageConflict,
    /// Durable installation state points to no matching managed set root.
    #[error("artifact-set state and managed storage disagree")]
    StateStorageMismatch,
    /// Durable structural installation registration failed.
    #[error("artifact-set state registration failed")]
    State(#[source] StoreError),
}
