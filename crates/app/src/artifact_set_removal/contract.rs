use std::io;

use rewrite_model::{ArtifactSetManifestError, InstalledArtifactSetError};
use rewrite_model_store::{StoreError, StoredArtifactSetInstallation};
use thiserror::Error;

use crate::ArtifactRemovalDisposition;

/// Caller-owned resource ceilings for one selected managed-set removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetRemovalLimits {
    /// Maximum members admitted from the registered manifest.
    pub maximum_members: usize,
    /// Maximum bytes admitted for any one member.
    pub maximum_member_bytes: u64,
    /// Maximum checked sum of all member bytes.
    pub maximum_total_bytes: u64,
    /// Maximum files and directories admitted in the managed set tree.
    pub maximum_tree_entries: usize,
    /// Maximum raw set-root entries admitted in managed set storage.
    pub maximum_storage_entries: usize,
}

/// Exact installed-set generation authority for one selected removal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetRemovalRequest {
    /// Persisted installation and epoch returned by a current state inspection.
    pub selection: StoredArtifactSetInstallation,
}

/// Complete result for one selected managed-set removal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetRemovalResult {
    /// Exact installed-set generation selected by the request.
    pub selection: StoredArtifactSetInstallation,
    /// Whether the call removed, recovered, or confirmed prior completion.
    pub disposition: ArtifactRemovalDisposition,
}

/// Content-free stage before the destructive set-removal boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactSetRemovalStage {
    /// Validate the exact selection and current durable state.
    InspectingSelection,
    /// Enumerate and hash the selected inactive managed set tree.
    VerifyingInactiveTree,
    /// Last cancellable stage before durable removal preparation.
    PreparingRemoval,
}

/// Content-free progress emitted only before durable set-removal preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetRemovalProgress {
    /// Current pre-removal lifecycle stage.
    pub stage: ArtifactSetRemovalStage,
    /// Bytes processed in the current verification stage.
    pub completed_bytes: u64,
    /// Exact member-byte total from the registered manifest.
    pub total_bytes: u64,
}

/// Post-preparation failure that leaves an exact retryable set-removal journal.
#[derive(Debug, Error)]
pub enum ArtifactSetRemovalRecoveryError {
    /// Managed set storage changed or conflicted after durable preparation.
    #[error("prepared artifact-set removal requires storage recovery")]
    Storage,
    /// A filesystem operation failed after durable preparation.
    #[error("prepared artifact-set removal requires filesystem recovery")]
    StorageIo(#[source] io::Error),
    /// Durable completion failed after tree deletion or absence confirmation.
    #[error("prepared artifact-set removal requires state recovery")]
    State(#[source] StoreError),
}

/// Failure from selected managed-set removal.
#[derive(Debug, Error)]
pub enum ArtifactSetRemovalError {
    /// A caller-owned resource ceiling was zero or not representable.
    #[error("artifact-set removal limits are invalid")]
    InvalidLimits,
    /// The supplied installation selection failed validation.
    #[error("artifact-set removal selection is invalid")]
    InvalidSelection,
    /// The registered set manifest failed domain validation.
    #[error("artifact-set manifest is invalid")]
    InvalidManifest(#[source] ArtifactSetManifestError),
    /// The application-derived installed-set record failed domain validation.
    #[error("artifact-set installation record is invalid")]
    InvalidInstallation(#[source] InstalledArtifactSetError),
    /// The selected set exceeds a caller-owned member, tree, or byte ceiling.
    #[error("artifact-set size {actual} exceeds the configured maximum {maximum}")]
    ArtifactSetTooLarge {
        /// Manifest-declared aggregate size or member count.
        actual: u64,
        /// Configured ceiling that was exceeded.
        maximum: u64,
    },
    /// Existing application-owned artifact storage is not initialized.
    #[error("artifact storage is not initialized")]
    StorageNotInitialized,
    /// An application-owned storage boundary is unsafe.
    #[error("artifact-set storage layout is invalid")]
    UnsafeStorageLayout,
    /// Another process holds the artifact lifecycle lock.
    #[error("artifact storage is already in use")]
    StorageInUse,
    /// Managed set storage exceeded the caller-owned entry ceiling.
    #[error("managed artifact-set storage exceeds the configured entry limit")]
    StorageEntryLimitExceeded,
    /// The exact installed-set generation is absent or no longer current.
    #[error("artifact-set installation selection is stale")]
    StaleSelection,
    /// Current canonical set root was absent before durable preparation.
    #[error("selected managed artifact-set tree is missing")]
    TreeMissing,
    /// Current canonical set tree was stable but disagreed with durable state.
    #[error("selected managed artifact-set tree conflicts with durable state")]
    StorageConflict,
    /// Managed storage changed during pre-removal verification.
    #[error("artifact-set storage changed during removal")]
    StorageChanged,
    /// Cancellation was observed before durable removal preparation.
    #[error("artifact-set removal was cancelled")]
    Cancelled,
    /// An application-owned storage operation failed before preparation.
    #[error("artifact-set storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Existing durable state failed integrity validation.
    #[error("persisted artifact-set state is corrupt")]
    StateCorrupt(#[source] StoreError),
    /// Durable-state inspection or preparation failed operationally.
    #[error("artifact-set removal state operation failed")]
    State(#[source] StoreError),
    /// Durable preparation committed, so an exact retry must resume recovery.
    #[error("artifact-set removal is durably prepared and requires recovery")]
    RecoveryRequired(#[source] ArtifactSetRemovalRecoveryError),
}
