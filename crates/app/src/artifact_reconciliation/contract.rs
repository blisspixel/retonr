use std::io;

use rewrite_model::{ArtifactManifest, InstalledArtifact, ManifestError};
use rewrite_model_store::{StoreError, StoredArtifactInstallation};
use thiserror::Error;

/// Caller-owned resource ceilings for one selected orphan reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactReconciliationLimits {
    /// Maximum bytes accepted for the selected artifact.
    pub maximum_artifact_bytes: u64,
    /// Maximum entries inspected in managed artifact storage.
    pub maximum_storage_entries: usize,
}

/// Exact manifest authority for one selected orphan reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactOrphanReconciliationRequest {
    /// Complete expected manifest used to derive and verify the canonical file.
    pub manifest: ArtifactManifest,
}

/// Durable-state outcome for selected orphan reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactReconciliationDisposition {
    /// Exact manifest and installation state were registered.
    Registered,
    /// Exact manifest and installation state were already registered.
    AlreadyRegistered,
}

/// Result of registering one independently reverified canonical artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactOrphanReconciliationResult {
    /// Exact installation state derived from the supplied manifest.
    pub installed: InstalledArtifact,
    /// Exact durable installation generation inserted or confirmed by the state store.
    pub installation: StoredArtifactInstallation,
    /// Whether this call inserted or confirmed exact durable state.
    pub disposition: ArtifactReconciliationDisposition,
}

/// Content-free stage for selected orphan reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactOrphanReconciliationStage {
    /// Validate the selection and inspect its canonical managed entry.
    InspectingSelection,
    /// Hash and verify the current canonical managed bytes.
    VerifyingOrphan,
}

/// Content-free progress for selected orphan reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactOrphanReconciliationProgress {
    /// Current lifecycle stage.
    pub stage: ArtifactOrphanReconciliationStage,
    /// Bytes processed in the current byte-processing stage.
    pub completed_bytes: u64,
    /// Exact total bytes declared by the validated manifest.
    pub total_bytes: u64,
}

/// Failure from selected orphan reconciliation.
#[derive(Debug, Error)]
pub enum ArtifactReconciliationError {
    /// A caller-owned resource ceiling was zero or not representable.
    #[error("artifact reconciliation limits are invalid")]
    InvalidLimits,
    /// The supplied manifest failed domain validation.
    #[error("artifact manifest is invalid")]
    InvalidManifest(#[source] ManifestError),
    /// The selected artifact exceeds the caller-owned byte ceiling.
    #[error("artifact size {actual} exceeds the configured maximum {maximum}")]
    ArtifactTooLarge {
        /// Manifest-declared artifact size.
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
    /// No exact canonical entry exists for the supplied manifest.
    #[error("selected artifact orphan was not found")]
    OrphanNotFound,
    /// Managed storage changed during a coherence-sensitive operation.
    #[error("artifact storage changed during reconciliation")]
    StorageChanged,
    /// Stable canonical bytes disagree with the supplied manifest.
    #[error("selected artifact bytes conflict with the manifest")]
    StorageConflict,
    /// Cancellation was observed before durable registration.
    #[error("artifact reconciliation was cancelled")]
    Cancelled,
    /// An application-owned storage operation failed.
    #[error("artifact storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Valid immutable durable state disagrees with the exact request.
    #[error("artifact state conflicts with the reconciliation request")]
    StateConflict,
    /// Existing durable state failed integrity validation.
    #[error("persisted artifact state is corrupt")]
    StateCorrupt(#[source] StoreError),
    /// Durable-state registration failed operationally.
    #[error("artifact state registration failed")]
    State(#[source] StoreError),
    /// The selected identity has a durably prepared removal to recover first.
    #[error("artifact removal is pending recovery")]
    RemovalPending {
        /// Exact prepared generation when it was observed before this failure.
        selection: Option<StoredArtifactInstallation>,
    },
}
