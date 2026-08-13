use std::io;

use rewrite_model::{ActiveArtifactBinding, ArtifactId, ArtifactManifest};
use rewrite_model_store::StoreError;
use rewrite_types::Digest;
use thiserror::Error;

use crate::ArtifactInstallationKey;

/// Caller-owned resource ceilings for one read-only artifact inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactInventoryLimits {
    /// Maximum durable manifest rows accepted in one state snapshot.
    pub maximum_state_entries: usize,
    /// Maximum raw entries accepted in the managed artifact directory.
    pub maximum_storage_entries: usize,
    /// Maximum bytes hashed for any one artifact file.
    pub maximum_artifact_bytes: u64,
    /// Maximum aggregate bytes hashed in one inventory.
    pub maximum_total_verification_bytes: u64,
}

/// Point-in-time byte status for one registered artifact installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegisteredArtifactBytes {
    /// Canonical direct bytes matched exact size and SHA-256.
    Verified,
    /// No exact case-sensitive canonical directory entry existed.
    Missing,
    /// The expected name was occupied by an indirect or non-regular entry.
    UnsafeEntry,
    /// The file had another hard-link name outside its canonical managed entry.
    AliasedEntry,
    /// The persisted storage key did not match the application-owned layout.
    StateLayoutConflict,
    /// Direct regular bytes had a different observed size.
    SizeConflict {
        /// Size observed before any hash work.
        observed_bytes: u64,
    },
    /// Stable same-size bytes had a different SHA-256 digest.
    DigestConflict {
        /// Digest observed during this inventory.
        observed_digest: Digest,
    },
    /// Exact-size bytes exceeded the caller's single-file hash ceiling.
    TooLargeToVerify {
        /// Size of the unverified direct regular file.
        observed_bytes: u64,
    },
}

/// Integrity-validated state and current byte observation for one installation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredArtifactInspection {
    /// Immutable artifact facts and source metadata.
    pub manifest: ArtifactManifest,
    /// Persistence-neutral identity of the exact registered installation.
    pub installation: ArtifactInstallationKey,
    /// Validated active bindings that currently reference this installation.
    pub active_bindings: Vec<ActiveArtifactBinding>,
    /// Current point-in-time managed-byte classification.
    pub bytes: RegisteredArtifactBytes,
}

/// Current byte status for one exact durably prepared removal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingArtifactRemovalInspection {
    /// Persistence-neutral identity selected by the pending removal.
    pub selection: ArtifactInstallationKey,
    /// Current point-in-time canonical-byte classification.
    pub bytes: RegisteredArtifactBytes,
}

/// Manifest association for independently verified uninstalled bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrphanManifestAssociation {
    /// No durable manifest names this content identity.
    NoManifest,
    /// A durable manifest names the same digest and byte size.
    MatchingManifest(ArtifactManifest),
    /// A durable manifest names the digest but declares a different byte size.
    ManifestSizeConflict {
        /// Conflicting immutable manifest.
        manifest: ArtifactManifest,
    },
}

/// Direct content-address-consistent bytes with no canonical installation record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedArtifactOrphan {
    /// Content-derived artifact identity proven during this scan.
    pub artifact_id: ArtifactId,
    /// Stable byte size observed during this scan.
    pub byte_size: u64,
    /// Durable manifest association, if any.
    pub manifest: OrphanManifestAssociation,
}

/// An uninstalled canonical name whose stable bytes did not hash to that name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentAddressConflict {
    /// Identity claimed by the canonical directory name.
    pub claimed_artifact_id: ArtifactId,
    /// Digest actually observed from the file bytes.
    pub observed_digest: Digest,
    /// Stable byte size observed during this scan.
    pub byte_size: u64,
}

/// Canonical direct bytes skipped because they exceeded the single-file ceiling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OversizedArtifactFile {
    /// Identity claimed by the canonical directory name.
    pub claimed_artifact_id: ArtifactId,
    /// Unverified observed file size.
    pub byte_size: u64,
}

/// Aggregate counts for entries that are never emitted by raw name.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UnexpectedArtifactEntryCounts {
    /// Entries whose raw name was not canonical lowercase SHA-256.
    pub malformed_names: u64,
    /// Uninstalled canonical names occupied by a link or reparse point.
    pub indirect_entries: u64,
    /// Uninstalled canonical names occupied by another non-regular entry type.
    pub non_regular_entries: u64,
    /// Uninstalled canonical direct files that were empty.
    pub empty_files: u64,
    /// Uninstalled canonical direct files with another hard-link name.
    pub aliased_files: u64,
}

/// Complete read-only reconciliation evidence from one coherent operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactInventoryReport {
    /// Persisted installations and their current canonical-byte classifications.
    pub registered: Vec<RegisteredArtifactInspection>,
    /// Prepared removals, disjoint from registered installations and orphans.
    pub pending_removals: Vec<PendingArtifactRemovalInspection>,
    /// Valid manifests that currently have no installed-artifact record.
    pub manifest_only: Vec<ArtifactManifest>,
    /// Verified files with no installation under the canonical storage key.
    pub verified_orphans: Vec<VerifiedArtifactOrphan>,
    /// Canonical names that contained different stable bytes.
    pub content_address_conflicts: Vec<ContentAddressConflict>,
    /// Canonical direct files skipped under the single-file ceiling.
    pub oversized_files: Vec<OversizedArtifactFile>,
    /// Safe aggregate counts for all other unexpected entries.
    pub unexpected_entries: UnexpectedArtifactEntryCounts,
    /// Exact raw directory-entry count in the frozen snapshot.
    pub storage_entry_count: u64,
    /// Aggregate bytes hashed during this operation.
    pub verified_bytes: u64,
}

/// Content-free lifecycle stage for read-only artifact inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactInventoryStage {
    /// Validate existing storage and acquire the shared lifecycle lock.
    OpeningStorage,
    /// Load one bounded, integrity-validated durable-state snapshot.
    LoadingState,
    /// Freeze exact managed artifact directory names.
    FreezingStorage,
    /// Inspect one durable state record and hash installed bytes when eligible.
    InspectingState,
    /// Inspect and when eligible hash one uninstalled canonical file.
    VerifyingUninstalled,
    /// Recheck storage boundaries, directory entries, and durable state.
    RecheckingStorageAndState,
}

/// Content-free progress snapshot for artifact inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactInventoryProgress {
    /// Current lifecycle stage.
    pub stage: ArtifactInventoryStage,
    /// Durable state entries plus uninstalled storage entries inspected so far.
    pub completed_entries: u64,
    /// Aggregate bytes hashed so far.
    pub verified_bytes: u64,
}

/// Failure from the read-only artifact inventory boundary.
#[derive(Debug, Error)]
pub enum ArtifactInventoryError {
    /// One or more caller-owned ceilings were zero or not representable.
    #[error("artifact inventory limits are invalid")]
    InvalidLimits,
    /// Existing application-owned artifact storage has not been initialized.
    #[error("artifact storage is not initialized")]
    StorageNotInitialized,
    /// A managed root, lock, directory, or opened entry had an unsafe type.
    #[error("artifact storage layout is invalid")]
    UnsafeStorageLayout,
    /// An exclusive artifact lifecycle operation currently owns the storage lock.
    #[error("artifact storage is already in use")]
    StorageInUse,
    /// Durable state exceeded its caller-owned snapshot entry ceiling.
    #[error("artifact state exceeds the configured inventory limit")]
    StateEntryLimitExceeded,
    /// The raw artifact directory exceeded its caller-owned entry ceiling.
    #[error("artifact storage exceeds the configured inventory entry limit")]
    StorageEntryLimitExceeded,
    /// Hashing the next eligible file would exceed the aggregate byte ceiling.
    #[error("artifact verification exceeds the configured total byte limit")]
    TotalVerificationLimitExceeded,
    /// Managed storage or durable state changed while the report was being built.
    #[error("artifact storage or state changed during inventory")]
    ConcurrentModification,
    /// Cancellation was observed before the report completed.
    #[error("artifact inventory was cancelled")]
    Cancelled,
    /// Existing application-owned storage could not be read completely.
    #[error("artifact storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Durable artifact lifecycle state could not be read and validated.
    #[error("artifact state inventory failed")]
    State(#[source] StoreError),
}
