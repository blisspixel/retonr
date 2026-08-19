use rewrite_model_store::StoreError;

use super::{ArtifactInstallationKey, ArtifactRepositoryError, ArtifactRepositoryErrorKind};
use crate::{
    ArtifactImportError, ArtifactInventoryError, ArtifactReconciliationError, ArtifactRemovalError,
    ArtifactSetImportError,
};

pub(crate) fn store_error_kind(error: &StoreError) -> ArtifactRepositoryErrorKind {
    use ArtifactRepositoryErrorKind as Kind;
    match error {
        StoreError::Database(_) | StoreError::BackupIo(_) | StoreError::BackupIncomplete => {
            Kind::Operational
        }
        StoreError::Serialization(_)
        | StoreError::InvalidManifest(_)
        | StoreError::InvalidInstallation(_)
        | StoreError::InvalidQualification(_)
        | StoreError::InvalidArtifactSet(_)
        | StoreError::InvalidArtifactSetInstallation(_)
        | StoreError::InvalidRuntimeBuild(_)
        | StoreError::InvalidRuntimeState(_)
        | StoreError::InvalidEffectivePackage(_)
        | StoreError::InvalidQualificationV2(_)
        | StoreError::InvalidInvalidation(_)
        | StoreError::InvalidDecision(_)
        | StoreError::CorruptRecord
        | StoreError::InvalidActiveBinding => Kind::CorruptState,
        StoreError::UnsupportedSchema(_) | StoreError::MigrationRequired { .. } => {
            Kind::IncompatibleState
        }
        StoreError::NotInitialized => Kind::NotInitialized,
        StoreError::BackupCancelled => Kind::Cancelled,
        StoreError::RecordTooLarge
        | StoreError::BackupTooLarge
        | StoreError::InvalidLimit
        | StoreError::InventoryLimitExceeded
        | StoreError::InstallationEpochExhausted => Kind::ResourceLimit,
        StoreError::ImmutableConflict
        | StoreError::VerificationFailed
        | StoreError::InvalidBackupDestination
        | StoreError::BackupRequired => Kind::Conflict,
        StoreError::MissingRecord => Kind::NotFound,
        StoreError::ActiveArtifact => Kind::ActiveArtifact,
        StoreError::RemovalPending => Kind::RecoveryRequired,
        StoreError::StaleInstallation => Kind::StaleSelection,
    }
}

pub(super) fn set_import_error_kind(error: &ArtifactSetImportError) -> ArtifactRepositoryErrorKind {
    use ArtifactRepositoryErrorKind as Kind;
    use ArtifactSetImportError as Error;
    match error {
        Error::InvalidLimits | Error::InvalidManifest(_) => Kind::InvalidInput,
        Error::InvalidInstallation(_) | Error::StateStorageMismatch => Kind::CorruptState,
        Error::TooManyMembers { .. }
        | Error::MemberTooLarge { .. }
        | Error::ArtifactSetTooLarge { .. }
        | Error::TreeEntryLimitExceeded
        | Error::StagingEntryLimitExceeded
        | Error::StorageEntryLimitExceeded => Kind::ResourceLimit,
        Error::Cancelled => Kind::Cancelled,
        Error::StorageInUse => Kind::InUse,
        Error::StorageChanged => Kind::ConcurrentModification,
        Error::IndirectSource
        | Error::SourceNotDirectory
        | Error::UnsafeSourceTree
        | Error::SourceTreeMismatch
        | Error::SizeMismatch
        | Error::DigestMismatch
        | Error::UnsafeStorageLayout
        | Error::StorageConflict => Kind::Conflict,
        Error::SourceIo(_) | Error::StorageIo(_) => Kind::Operational,
        Error::State(error) => store_error_kind(error),
    }
}

pub(super) fn import_error_kind(error: &ArtifactImportError) -> ArtifactRepositoryErrorKind {
    use ArtifactImportError as Error;
    use ArtifactRepositoryErrorKind as Kind;
    match error {
        Error::InvalidLimits | Error::InvalidManifest(_) => Kind::InvalidInput,
        Error::ArtifactTooLarge { .. }
        | Error::StagingEntryLimitExceeded
        | Error::StorageEntryLimitExceeded => Kind::ResourceLimit,
        Error::Cancelled => Kind::Cancelled,
        Error::StorageInUse => Kind::InUse,
        Error::StorageChanged => Kind::ConcurrentModification,
        Error::IndirectSource
        | Error::SourceNotRegular
        | Error::SizeMismatch
        | Error::DigestMismatch
        | Error::UnsafeStorageLayout
        | Error::StorageConflict => Kind::Conflict,
        Error::SourceIo(_) | Error::StorageIo(_) => Kind::Operational,
        Error::State(error) => store_error_kind(error),
        Error::RemovalPending { .. } => Kind::RecoveryRequired,
    }
}

pub(super) fn inventory_error_kind(error: &ArtifactInventoryError) -> ArtifactRepositoryErrorKind {
    use ArtifactInventoryError as Error;
    use ArtifactRepositoryErrorKind as Kind;
    match error {
        Error::InvalidLimits => Kind::InvalidInput,
        Error::StorageNotInitialized => Kind::NotInitialized,
        Error::StorageInUse => Kind::InUse,
        Error::StateEntryLimitExceeded
        | Error::StorageEntryLimitExceeded
        | Error::TotalVerificationLimitExceeded => Kind::ResourceLimit,
        Error::ConcurrentModification => Kind::ConcurrentModification,
        Error::Cancelled => Kind::Cancelled,
        Error::UnsafeStorageLayout => Kind::Conflict,
        Error::StorageIo(_) => Kind::Operational,
        Error::State(error) => store_error_kind(error),
    }
}

pub(super) fn reconciliation_error_kind(
    error: &ArtifactReconciliationError,
) -> ArtifactRepositoryErrorKind {
    use ArtifactReconciliationError as Error;
    use ArtifactRepositoryErrorKind as Kind;
    match error {
        Error::InvalidLimits | Error::InvalidManifest(_) => Kind::InvalidInput,
        Error::ArtifactTooLarge { .. } | Error::StorageEntryLimitExceeded => Kind::ResourceLimit,
        Error::StorageNotInitialized => Kind::NotInitialized,
        Error::StorageInUse => Kind::InUse,
        Error::OrphanNotFound => Kind::NotFound,
        Error::StorageChanged => Kind::ConcurrentModification,
        Error::Cancelled => Kind::Cancelled,
        Error::UnsafeStorageLayout | Error::StorageConflict | Error::StateConflict => {
            Kind::Conflict
        }
        Error::StorageIo(_) | Error::State(_) => Kind::Operational,
        Error::StateCorrupt(_) => Kind::CorruptState,
        Error::RemovalPending { .. } => Kind::RecoveryRequired,
    }
}

pub(super) fn removal_error_kind(error: &ArtifactRemovalError) -> ArtifactRepositoryErrorKind {
    use ArtifactRemovalError as Error;
    use ArtifactRepositoryErrorKind as Kind;
    match error {
        Error::InvalidLimits | Error::InvalidSelection => Kind::InvalidInput,
        Error::ArtifactTooLarge { .. } | Error::StorageEntryLimitExceeded => Kind::ResourceLimit,
        Error::StorageNotInitialized => Kind::NotInitialized,
        Error::StorageInUse => Kind::InUse,
        Error::StaleSelection => Kind::StaleSelection,
        Error::ActiveArtifact => Kind::ActiveArtifact,
        Error::BytesMissing => Kind::NotFound,
        Error::StorageChanged => Kind::ConcurrentModification,
        Error::Cancelled => Kind::Cancelled,
        Error::UnsafeStorageLayout | Error::StorageConflict => Kind::Conflict,
        Error::StorageIo(_) | Error::State(_) => Kind::Operational,
        Error::StateCorrupt(_) => Kind::CorruptState,
        Error::RecoveryRequired(_) => Kind::RecoveryRequired,
    }
}

pub(crate) fn map_import_error(error: crate::ArtifactImportError) -> ArtifactRepositoryError {
    match error {
        crate::ArtifactImportError::RemovalPending {
            selection: Some(selection),
        } => ArtifactRepositoryError::RemovalRecoveryPending {
            key: ArtifactInstallationKey::from_stored(&selection),
        },
        other => ArtifactRepositoryError::Import(other),
    }
}

pub(crate) fn map_reconciliation_error(
    error: crate::ArtifactReconciliationError,
) -> ArtifactRepositoryError {
    match error {
        crate::ArtifactReconciliationError::RemovalPending {
            selection: Some(selection),
        } => ArtifactRepositoryError::RemovalRecoveryPending {
            key: ArtifactInstallationKey::from_stored(&selection),
        },
        other => ArtifactRepositoryError::Reconciliation(other),
    }
}
