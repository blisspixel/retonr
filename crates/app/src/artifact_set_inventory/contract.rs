use std::io;

use rewrite_model::{ArtifactSetId, ArtifactSetManifest};
use rewrite_model_store::StoreError;
use thiserror::Error;

use crate::ArtifactSetInstallationKey;

/// Caller-owned resource ceilings for one read-only artifact-set inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetInventoryLimits {
    /// Maximum durable set-manifest rows accepted in one state snapshot.
    pub maximum_state_entries: usize,
    /// Maximum raw entries accepted in the managed set-root directory.
    pub maximum_storage_entries: usize,
    /// Maximum members admitted from one durable set manifest.
    pub maximum_members: usize,
    /// Maximum bytes hashed for any one set member.
    pub maximum_member_bytes: u64,
    /// Maximum files and directories admitted in one inspected set tree.
    pub maximum_tree_entries: usize,
    /// Maximum aggregate member bytes hashed in one inventory.
    pub maximum_total_verification_bytes: u64,
}

/// Point-in-time tree status for one registered artifact-set installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegisteredArtifactSetBytes {
    /// The complete managed tree matched the registered manifest.
    Verified,
    /// No exact case-sensitive canonical set-root directory existed.
    Missing,
    /// The expected name was occupied by an indirect or non-directory entry.
    UnsafeEntry,
    /// The persisted storage key did not match the application-owned layout.
    StateLayoutConflict,
    /// The tree shape, member sizes, or link counts disagreed with the manifest.
    TreeMismatch,
    /// A manifest member had a different observed size.
    MemberSizeConflict {
        /// Size observed before any hash work on that member.
        observed_bytes: u64,
    },
    /// Stable same-size member bytes had a different SHA-256 digest.
    MemberDigestConflict,
    /// The planned tree exceeded a caller-owned per-set ceiling.
    TooLargeToVerify {
        /// Planned member-byte total that was not hashed.
        observed_bytes: u64,
    },
}

/// Integrity-validated state and current tree observation for one set installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredArtifactSetInspection {
    /// Immutable set membership facts.
    pub manifest: ArtifactSetManifest,
    /// Persistence-neutral identity of the exact registered set installation.
    pub installation: ArtifactSetInstallationKey,
    /// Current point-in-time managed-tree classification.
    pub bytes: RegisteredArtifactSetBytes,
}

/// A verified uninstalled set root whose tree matched a durable manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedArtifactSetOrphan {
    /// Content-derived set identity proven against the durable manifest.
    pub artifact_set_id: ArtifactSetId,
    /// Stable planned member-byte total observed during this scan.
    pub byte_size: u64,
}

/// An uninstalled canonical set root whose tree did not match its durable manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetTreeConflict {
    /// Identity claimed by the canonical set-root name and durable manifest.
    pub artifact_set_id: ArtifactSetId,
    /// Planned member-byte total from the durable manifest.
    pub byte_size: u64,
}

/// A canonical set root skipped because its planned tree exceeded a per-set ceiling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OversizedArtifactSet {
    /// Identity claimed by the canonical set-root name.
    pub artifact_set_id: ArtifactSetId,
    /// Unverified planned member-byte total.
    pub byte_size: u64,
}

/// Aggregate counts for set-root entries that are never emitted by raw name.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UnexpectedArtifactSetEntryCounts {
    /// Entries whose raw name was not a canonical `set-v1-` SHA-256 key.
    pub malformed_names: u64,
    /// Uninstalled canonical names occupied by a link or reparse point.
    pub indirect_entries: u64,
    /// Uninstalled canonical names occupied by a non-directory entry.
    pub non_directory_entries: u64,
    /// Canonical set roots with no matching durable manifest. These are not verified.
    pub unregistered_roots: u64,
}

/// Complete read-only artifact-set evidence from one coherent operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetInventoryReport {
    /// Persisted set installations and their current tree classifications.
    pub registered: Vec<RegisteredArtifactSetInspection>,
    /// Valid set manifests that currently have no installed-set record.
    pub manifest_only: Vec<ArtifactSetManifest>,
    /// Verified set roots with no installation under the canonical storage key.
    pub verified_orphans: Vec<VerifiedArtifactSetOrphan>,
    /// Canonical names whose trees disagreed with a matching durable manifest.
    pub tree_conflicts: Vec<ArtifactSetTreeConflict>,
    /// Canonical set roots skipped under a per-set ceiling.
    pub oversized_sets: Vec<OversizedArtifactSet>,
    /// Safe aggregate counts for all other unexpected set-root entries.
    pub unexpected_entries: UnexpectedArtifactSetEntryCounts,
    /// Exact raw set-root directory-entry count in the frozen snapshot.
    pub storage_entry_count: u64,
    /// Aggregate member bytes hashed during this operation.
    pub verified_bytes: u64,
}

/// Content-free lifecycle stage for read-only artifact-set inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactSetInventoryStage {
    /// Validate existing storage and acquire the shared lifecycle lock.
    OpeningStorage,
    /// Load one bounded, integrity-validated durable-state snapshot.
    LoadingState,
    /// Freeze exact managed set-root directory names.
    FreezingStorage,
    /// Inspect one durable set record and hash installed members when eligible.
    InspectingState,
    /// Inspect and when eligible hash one uninstalled canonical set root.
    VerifyingUninstalled,
    /// Recheck storage boundaries, directory entries, and durable state.
    RecheckingStorageAndState,
}

/// Content-free progress snapshot for artifact-set inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactSetInventoryProgress {
    /// Current lifecycle stage.
    pub stage: ArtifactSetInventoryStage,
    /// Durable state entries plus uninstalled set roots inspected so far.
    pub completed_entries: u64,
    /// Aggregate member bytes hashed so far.
    pub verified_bytes: u64,
}

/// Failure from the read-only artifact-set inventory boundary.
#[derive(Debug, Error)]
pub enum ArtifactSetInventoryError {
    /// One or more caller-owned ceilings were zero or not representable.
    #[error("artifact-set inventory limits are invalid")]
    InvalidLimits,
    /// Existing application-owned artifact storage has not been initialized.
    #[error("artifact storage is not initialized")]
    StorageNotInitialized,
    /// A managed root, lock, directory, or opened entry had an unsafe type.
    #[error("artifact-set storage layout is invalid")]
    UnsafeStorageLayout,
    /// An exclusive artifact lifecycle operation currently owns the storage lock.
    #[error("artifact storage is already in use")]
    StorageInUse,
    /// Durable set state exceeded its caller-owned snapshot entry ceiling.
    #[error("artifact-set state exceeds the configured inventory limit")]
    StateEntryLimitExceeded,
    /// The raw set-root directory exceeded its caller-owned entry ceiling.
    #[error("artifact-set storage exceeds the configured inventory entry limit")]
    StorageEntryLimitExceeded,
    /// Hashing the next eligible set would exceed the aggregate byte ceiling.
    #[error("artifact-set verification exceeds the configured total byte limit")]
    TotalVerificationLimitExceeded,
    /// Managed storage or durable state changed while the report was being built.
    #[error("artifact-set storage or state changed during inventory")]
    ConcurrentModification,
    /// Cancellation was observed before the report completed.
    #[error("artifact-set inventory was cancelled")]
    Cancelled,
    /// Existing application-owned storage could not be read completely.
    #[error("artifact-set storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Durable artifact-set lifecycle state could not be read and validated.
    #[error("artifact-set state inventory failed")]
    State(#[source] StoreError),
}
