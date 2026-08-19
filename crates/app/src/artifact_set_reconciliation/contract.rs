use std::io;

use rewrite_model::{
    ArtifactSetManifest, ArtifactSetManifestError, InstalledArtifactSet, InstalledArtifactSetError,
};
use rewrite_model_store::{StoreError, StoredArtifactSetInstallation};
use thiserror::Error;

use crate::ArtifactReconciliationDisposition;

/// Caller-owned resource ceilings for one selected set-root reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetReconciliationLimits {
    /// Maximum members admitted from the supplied manifest.
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

/// Exact set-manifest authority for one selected set-root reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetReconciliationRequest {
    /// Complete expected set manifest used to derive and verify the set root.
    pub manifest: ArtifactSetManifest,
}

/// Result of registering one independently reverified managed set root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetReconciliationResult {
    /// Exact installed-set record derived from the supplied manifest.
    pub installed: InstalledArtifactSet,
    /// Exact durable installation generation inserted or confirmed.
    pub installation: StoredArtifactSetInstallation,
    /// Whether this call inserted or confirmed exact durable state.
    pub disposition: ArtifactReconciliationDisposition,
}

/// Content-free stage for selected set-root reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactSetReconciliationStage {
    /// Validate the selection and inspect its canonical managed set root.
    InspectingSelection,
    /// Enumerate and hash the current managed set tree.
    VerifyingOrphan,
}

/// Content-free progress for selected set-root reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetReconciliationProgress {
    /// Current lifecycle stage.
    pub stage: ArtifactSetReconciliationStage,
    /// Bytes processed in the current byte-processing stage.
    pub completed_bytes: u64,
    /// Exact total bytes declared by the validated manifest.
    pub total_bytes: u64,
}

/// Failure from selected set-root reconciliation.
#[derive(Debug, Error)]
pub enum ArtifactSetReconciliationError {
    /// A caller-owned resource ceiling was zero or not representable.
    #[error("artifact-set reconciliation limits are invalid")]
    InvalidLimits,
    /// The supplied set manifest failed domain validation.
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
    /// No exact canonical set root exists for the supplied manifest.
    #[error("selected artifact-set orphan was not found")]
    OrphanNotFound,
    /// Managed storage changed during a coherence-sensitive operation.
    #[error("artifact-set storage changed during reconciliation")]
    StorageChanged,
    /// Stable managed tree bytes disagree with the supplied manifest.
    #[error("selected artifact-set tree conflicts with the manifest")]
    StorageConflict,
    /// Cancellation was observed before durable registration.
    #[error("artifact-set reconciliation was cancelled")]
    Cancelled,
    /// An application-owned storage operation failed.
    #[error("artifact-set storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Valid immutable durable state disagrees with the exact request.
    #[error("artifact-set state conflicts with the reconciliation request")]
    StateConflict,
    /// Existing durable state failed integrity validation.
    #[error("persisted artifact-set state is corrupt")]
    StateCorrupt(#[source] StoreError),
    /// Durable-state registration failed operationally.
    #[error("artifact-set state registration failed")]
    State(#[source] StoreError),
}
