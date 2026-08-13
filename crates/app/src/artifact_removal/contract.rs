use std::io;

use rewrite_model_store::{StoreError, StoredArtifactInstallation};
use thiserror::Error;

/// Caller-owned resource ceilings for one selected managed-artifact removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactRemovalLimits {
    /// Maximum bytes accepted for the selected artifact.
    pub maximum_artifact_bytes: u64,
    /// Maximum entries inspected in managed artifact storage.
    pub maximum_storage_entries: usize,
}

/// Exact installed generation authority for one selected removal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRemovalRequest {
    /// Persisted installation and epoch returned by a current state inspection.
    pub selection: StoredArtifactInstallation,
}

/// Successful outcome for one selected managed-artifact removal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactRemovalDisposition {
    /// This call prepared, deleted, synchronized, and completed the removal.
    Removed,
    /// This call resumed an earlier prepared removal and completed it.
    Recovered,
    /// The exact installation generation was already durably removed.
    AlreadyRemoved,
}

/// Complete result for one selected managed-artifact removal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRemovalResult {
    /// Exact installed generation selected by the request.
    pub selection: StoredArtifactInstallation,
    /// Whether the call removed, recovered, or confirmed prior completion.
    pub disposition: ArtifactRemovalDisposition,
}

/// Content-free stage before the destructive removal boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactRemovalStage {
    /// Validate the exact selection and current durable state.
    InspectingSelection,
    /// Hash and reverify the selected inactive managed bytes.
    VerifyingInactiveBytes,
    /// Last cancellable stage before durable removal preparation.
    PreparingRemoval,
}

/// Content-free progress emitted only before durable removal preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactRemovalProgress {
    /// Current pre-removal lifecycle stage.
    pub stage: ArtifactRemovalStage,
    /// Bytes processed in the current verification stage.
    pub completed_bytes: u64,
    /// Exact byte size from the selected installed record.
    pub total_bytes: u64,
}

/// Post-preparation failure that leaves an exact retryable removal journal.
#[derive(Debug, Error)]
pub enum ArtifactRemovalRecoveryError {
    /// Managed storage changed or conflicted after durable preparation.
    #[error("prepared artifact removal requires storage recovery")]
    Storage,
    /// A filesystem operation failed after durable preparation.
    #[error("prepared artifact removal requires filesystem recovery")]
    StorageIo(#[source] io::Error),
    /// Durable completion failed after byte deletion or absence confirmation.
    #[error("prepared artifact removal requires state recovery")]
    State(#[source] StoreError),
}

/// Failure from selected managed-artifact removal.
#[derive(Debug, Error)]
pub enum ArtifactRemovalError {
    /// A caller-owned resource ceiling was zero or not representable.
    #[error("artifact removal limits are invalid")]
    InvalidLimits,
    /// The supplied installation selection failed validation.
    #[error("artifact removal selection is invalid")]
    InvalidSelection,
    /// The selected artifact exceeds the caller-owned byte ceiling.
    #[error("artifact size {actual} exceeds the configured maximum {maximum}")]
    ArtifactTooLarge {
        /// Selected artifact size.
        actual: u64,
        /// Configured single-artifact ceiling.
        maximum: u64,
    },
    /// Existing application-owned artifact storage is not initialized.
    #[error("artifact storage is not initialized")]
    StorageNotInitialized,
    /// An application-owned storage boundary is unsafe.
    #[error("artifact storage layout is invalid")]
    UnsafeStorageLayout,
    /// Another process holds the artifact lifecycle lock.
    #[error("artifact storage is already in use")]
    StorageInUse,
    /// Managed artifact storage exceeded the caller-owned entry ceiling.
    #[error("managed artifact storage exceeds the configured entry limit")]
    StorageEntryLimitExceeded,
    /// The exact installed generation is absent or no longer current.
    #[error("artifact installation selection is stale")]
    StaleSelection,
    /// The exact installation still has an active role binding.
    #[error("active artifact installation cannot be removed")]
    ActiveArtifact,
    /// Current canonical bytes were absent before durable preparation.
    #[error("selected managed artifact bytes are missing")]
    BytesMissing,
    /// Current canonical bytes were stable but disagreed with durable state.
    #[error("selected managed artifact bytes conflict with durable state")]
    StorageConflict,
    /// Managed storage changed during pre-removal verification.
    #[error("artifact storage changed during removal")]
    StorageChanged,
    /// Cancellation was observed before durable removal preparation.
    #[error("artifact removal was cancelled")]
    Cancelled,
    /// An application-owned storage operation failed before preparation.
    #[error("artifact storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Existing durable state failed integrity validation.
    #[error("persisted artifact state is corrupt")]
    StateCorrupt(#[source] StoreError),
    /// Durable-state inspection or preparation failed operationally.
    #[error("artifact removal state operation failed")]
    State(#[source] StoreError),
    /// Durable preparation committed, so an exact retry must resume recovery.
    #[error("artifact removal is durably prepared and requires recovery")]
    RecoveryRequired(#[source] ArtifactRemovalRecoveryError),
}
