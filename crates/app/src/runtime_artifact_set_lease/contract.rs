use std::io;

use rewrite_model::ArtifactSetManifestError;
use rewrite_model_store::StoreError;
use thiserror::Error;

use crate::{
    ArtifactRepositoryErrorKind, ArtifactSetImportError, artifact_repository::store_error_kind,
    artifact_set_import::ArtifactSetPlanBounds,
};

/// Caller-owned ceilings for acquiring one managed artifact-set lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeArtifactSetLeaseLimits {
    /// Maximum members admitted from the registered canonical manifest.
    pub maximum_members: usize,
    /// Maximum bytes admitted for any one member.
    pub maximum_member_bytes: u64,
    /// Maximum checked sum of all member bytes.
    pub maximum_total_bytes: u64,
    /// Maximum files and directories admitted in the exact managed set tree.
    pub maximum_tree_entries: usize,
    /// Maximum complete set roots admitted in managed set storage.
    pub maximum_storage_entries: usize,
}

impl RuntimeArtifactSetLeaseLimits {
    pub(super) const fn plan_bounds(self) -> ArtifactSetPlanBounds {
        ArtifactSetPlanBounds {
            members: self.maximum_members,
            member_bytes: self.maximum_member_bytes,
            total_bytes: self.maximum_total_bytes,
            tree_entries: self.maximum_tree_entries,
        }
    }

    pub(super) fn validate_storage_ceiling(self) -> Result<(), ArtifactSetLeaseError> {
        if self.maximum_storage_entries == 0
            || self.maximum_storage_entries.checked_add(1).is_none()
        {
            Err(ArtifactSetLeaseError::InvalidLimits)
        } else {
            Ok(())
        }
    }
}

/// Failure from the managed artifact-set lease trust boundary.
#[derive(Debug, Error)]
pub enum ArtifactSetLeaseError {
    /// One or more configured lease ceilings were zero or inconsistent.
    #[error("artifact-set lease limits are invalid")]
    InvalidLimits,
    /// No durable installation state exists for the selected artifact-set identity.
    #[error("artifact set is not currently installed")]
    ArtifactSetNotInstalled,
    /// The registered canonical manifest failed domain validation.
    #[error("artifact-set manifest is invalid")]
    InvalidManifest(#[source] ArtifactSetManifestError),
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
    /// Exact tree inspection exceeded the caller-owned entry ceiling.
    #[error("artifact-set tree exceeds the configured entry limit")]
    TreeEntryLimitExceeded,
    /// Managed set storage reached the caller-owned set-root ceiling.
    #[error("managed artifact-set storage exceeds the configured entry limit")]
    StorageEntryLimitExceeded,
    /// The application-owned storage layout was missing or unsafe.
    #[error("artifact-set storage layout is invalid")]
    UnsafeStorageLayout,
    /// Another operation currently owns the artifact storage lifecycle exclusively.
    #[error("artifact storage is already in use")]
    StorageInUse,
    /// Managed storage or durable state changed during lease acquisition.
    #[error("artifact-set storage changed during lease acquisition")]
    StorageChanged,
    /// Managed member bytes disagree with the registered canonical manifest.
    #[error("managed artifact-set bytes do not match their manifest")]
    StorageConflict,
    /// Durable installation state points to no matching managed set root.
    #[error("artifact-set state and managed storage disagree")]
    StateStorageMismatch,
    /// Cancellation was observed before the lease was established.
    #[error("artifact-set lease acquisition was cancelled")]
    Cancelled,
    /// Managed storage inspection failed.
    #[error("artifact-set storage operation failed")]
    StorageIo(#[source] io::Error),
    /// Durable artifact-set state could not be read.
    #[error("artifact-set state read failed")]
    State(#[source] StoreError),
}

pub(super) fn map_set_lease_error(error: ArtifactSetImportError) -> ArtifactSetLeaseError {
    use ArtifactSetImportError as Source;
    use ArtifactSetLeaseError as Target;
    match error {
        Source::InvalidLimits => Target::InvalidLimits,
        Source::InvalidManifest(error) => Target::InvalidManifest(error),
        Source::TooManyMembers { actual, maximum } => Target::TooManyMembers { actual, maximum },
        Source::MemberTooLarge { actual, maximum } => Target::MemberTooLarge { actual, maximum },
        Source::ArtifactSetTooLarge { actual, maximum } => {
            Target::ArtifactSetTooLarge { actual, maximum }
        }
        Source::TreeEntryLimitExceeded => Target::TreeEntryLimitExceeded,
        Source::StagingEntryLimitExceeded | Source::StorageEntryLimitExceeded => {
            Target::StorageEntryLimitExceeded
        }
        Source::UnsafeStorageLayout => Target::UnsafeStorageLayout,
        Source::StorageInUse => Target::StorageInUse,
        Source::StorageChanged => Target::StorageChanged,
        Source::StateStorageMismatch | Source::InvalidInstallation(_) => {
            Target::StateStorageMismatch
        }
        Source::Cancelled => Target::Cancelled,
        Source::StorageIo(error) => Target::StorageIo(error),
        Source::State(error) => Target::State(error),
        Source::SourceIo(_)
        | Source::IndirectSource
        | Source::SourceNotDirectory
        | Source::UnsafeSourceTree
        | Source::SourceTreeMismatch
        | Source::SizeMismatch
        | Source::DigestMismatch
        | Source::StorageConflict => Target::StorageConflict,
    }
}

pub(crate) fn set_lease_error_kind(error: &ArtifactSetLeaseError) -> ArtifactRepositoryErrorKind {
    use ArtifactRepositoryErrorKind as Kind;
    use ArtifactSetLeaseError as Error;
    match error {
        Error::InvalidLimits | Error::InvalidManifest(_) => Kind::InvalidInput,
        Error::ArtifactSetNotInstalled => Kind::NotFound,
        Error::TooManyMembers { .. }
        | Error::MemberTooLarge { .. }
        | Error::ArtifactSetTooLarge { .. }
        | Error::TreeEntryLimitExceeded
        | Error::StorageEntryLimitExceeded => Kind::ResourceLimit,
        Error::UnsafeStorageLayout | Error::StorageConflict => Kind::Conflict,
        Error::StorageInUse => Kind::InUse,
        Error::StorageChanged => Kind::ConcurrentModification,
        Error::StateStorageMismatch => Kind::CorruptState,
        Error::Cancelled => Kind::Cancelled,
        Error::StorageIo(_) => Kind::Operational,
        Error::State(error) => store_error_kind(error),
    }
}
