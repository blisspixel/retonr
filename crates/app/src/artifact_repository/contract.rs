use std::io;

use rewrite_model::ArtifactId;
use rewrite_model_store::{StoreError, StoredArtifactInstallation, WriteDisposition};
use thiserror::Error;

use crate::{
    ArtifactImportError, ArtifactImportResult, ArtifactInventoryError,
    ArtifactOrphanReconciliationResult, ArtifactReconciliationDisposition,
    ArtifactReconciliationError, ArtifactRemovalDisposition, ArtifactRemovalError,
    ArtifactRemovalResult,
};

/// Stable application-level classification for repository failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactRepositoryErrorKind {
    /// Caller input or a configured limit is invalid.
    InvalidInput,
    /// The fixed repository has not been initialized.
    NotInitialized,
    /// Another operation owns an incompatible repository or storage lock.
    InUse,
    /// A caller-owned resource ceiling was reached.
    ResourceLimit,
    /// The selected artifact or installation does not exist.
    NotFound,
    /// The selected installation generation is no longer current.
    StaleSelection,
    /// An active artifact cannot be removed.
    ActiveArtifact,
    /// Stable bytes, immutable state, or an exact manifest disagree.
    Conflict,
    /// State or storage changed during a coherence-sensitive operation.
    ConcurrentModification,
    /// Existing durable state failed integrity validation.
    CorruptState,
    /// Existing durable state uses an unsupported or migration-required schema.
    IncompatibleState,
    /// A prepared removal requires an explicit exact recovery operation.
    RecoveryRequired,
    /// Cancellation was observed before an irreversible boundary.
    Cancelled,
    /// A filesystem, database, or other operational action failed.
    Operational,
}

/// Persistence-neutral identity for one exact installed artifact generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactInstallationKey {
    /// Content-derived artifact identity.
    artifact_id: ArtifactId,
    /// Positive generation preventing a stale operation from targeting a reinstall.
    installation_generation: u64,
}

impl ArtifactInstallationKey {
    /// Constructs one validated exact installation key.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError::InvalidInstallationGeneration`] when the
    /// generation is zero or outside the durable store range.
    pub fn new(
        artifact_id: ArtifactId,
        installation_generation: u64,
    ) -> Result<Self, ArtifactRepositoryError> {
        if installation_generation == 0 || i64::try_from(installation_generation).is_err() {
            Err(ArtifactRepositoryError::InvalidInstallationGeneration)
        } else {
            Ok(Self {
                artifact_id,
                installation_generation,
            })
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ArtifactRepositoryError> {
        if self.installation_generation == 0 || i64::try_from(self.installation_generation).is_err()
        {
            Err(ArtifactRepositoryError::InvalidInstallationGeneration)
        } else {
            Ok(())
        }
    }

    /// Returns the content-derived artifact identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact positive installation generation.
    #[must_use]
    pub const fn installation_generation(&self) -> u64 {
        self.installation_generation
    }

    pub(crate) fn from_stored(value: &StoredArtifactInstallation) -> Self {
        Self {
            artifact_id: value.installed.artifact_id.clone(),
            installation_generation: value.epoch.get(),
        }
    }
}

/// Repository-level disposition for one offline import.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactRepositoryImportDisposition {
    /// New managed bytes or durable installation state were created.
    Imported,
    /// Exact managed bytes and durable installation state already existed.
    AlreadyPresent,
}

/// Successful repository-level offline import result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositoryImportResult {
    /// Exact store-issued installation key.
    pub key: ArtifactInstallationKey,
    /// Whether this call imported or confirmed exact prior state.
    pub disposition: ArtifactRepositoryImportDisposition,
}

impl From<ArtifactImportResult> for ArtifactRepositoryImportResult {
    fn from(value: ArtifactImportResult) -> Self {
        let disposition = if value.state.installed == WriteDisposition::AlreadyPresent {
            ArtifactRepositoryImportDisposition::AlreadyPresent
        } else {
            ArtifactRepositoryImportDisposition::Imported
        };
        Self {
            key: ArtifactInstallationKey::from_stored(&value.state.installation),
            disposition,
        }
    }
}

/// Successful repository-level selected reconciliation result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositoryReconciliationResult {
    /// Exact store-issued installation key.
    pub key: ArtifactInstallationKey,
    /// Whether this call registered or confirmed exact prior state.
    pub disposition: ArtifactReconciliationDisposition,
}

impl From<ArtifactOrphanReconciliationResult> for ArtifactRepositoryReconciliationResult {
    fn from(value: ArtifactOrphanReconciliationResult) -> Self {
        Self {
            key: ArtifactInstallationKey::from_stored(&value.installation),
            disposition: value.disposition,
        }
    }
}

/// Successful repository-level removal or recovery result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositoryRemovalResult {
    /// Exact installation key affected or confirmed by the operation.
    pub key: ArtifactInstallationKey,
    /// Whether bytes were removed, recovered, or already absent for this generation.
    pub disposition: ArtifactRemovalDisposition,
}

/// Bounded read-only view of operations requiring explicit recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositoryPendingOperations {
    /// Exact prepared artifact-removal generations in artifact-identity order.
    pub artifact_removals: Vec<ArtifactInstallationKey>,
}

impl From<ArtifactRemovalResult> for ArtifactRepositoryRemovalResult {
    fn from(value: ArtifactRemovalResult) -> Self {
        Self {
            key: ArtifactInstallationKey::from_stored(&value.selection),
            disposition: value.disposition,
        }
    }
}

/// Failure from the application-owned administrative artifact repository.
#[derive(Debug, Error)]
pub enum ArtifactRepositoryError {
    /// The installation generation was zero or outside the durable store range.
    #[error("artifact installation generation is invalid")]
    InvalidInstallationGeneration,
    /// One or more caller-owned repository limits are invalid.
    #[error("artifact repository limits are invalid")]
    InvalidLimits,
    /// Cancellation was observed during a read-only repository operation.
    #[error("artifact repository operation was cancelled")]
    Cancelled,
    /// No fixed application data directory has been initialized.
    #[error("artifact repository is not initialized")]
    NotInitialized,
    /// The selected application data directory is indirect or not a directory.
    #[error("artifact repository data directory is unsafe")]
    UnsafeDataDirectory,
    /// Another repository operation currently holds an incompatible lifecycle lock.
    #[error("artifact repository is in use")]
    RepositoryInUse,
    /// The fixed application data directory could not be inspected or initialized.
    #[error("artifact repository data directory operation failed")]
    DataDirectoryIo(#[source] io::Error),
    /// The fixed artifact state database could not be opened or validated.
    #[error("artifact repository state operation failed")]
    State(#[from] StoreError),
    /// Offline artifact import failed.
    #[error(transparent)]
    Import(#[from] ArtifactImportError),
    /// Read-only artifact inventory failed.
    #[error(transparent)]
    Inventory(#[from] ArtifactInventoryError),
    /// Selected orphan reconciliation failed.
    #[error(transparent)]
    Reconciliation(#[from] ArtifactReconciliationError),
    /// Selected inactive artifact removal failed.
    #[error(transparent)]
    Removal(#[from] ArtifactRemovalError),
    /// No current installation exists for the selected artifact identity.
    #[error("artifact is not currently installed")]
    ArtifactNotInstalled,
    /// Fresh removal cannot continue until the prepared operation is recovered.
    #[error("artifact has a prepared removal that requires explicit recovery")]
    RemovalRecoveryPending {
        /// Exact prepared generation that must be recovered.
        key: ArtifactInstallationKey,
    },
    /// Removal crossed its durable preparation boundary and must recover exactly.
    #[error("artifact removal requires exact recovery")]
    RemovalRecoveryRequired {
        /// Exact prepared generation that must be recovered.
        key: ArtifactInstallationKey,
        /// Lower-level post-preparation failure.
        #[source]
        source: ArtifactRemovalError,
    },
    /// No exact prepared removal exists for the selected artifact identity.
    #[error("artifact has no prepared removal to recover")]
    RemovalRecoveryNotPending,
    /// The selected installation generation is no longer current.
    #[error("artifact installation selection is stale")]
    StaleInstallation,
}

impl ArtifactRepositoryError {
    /// Returns the stable, persistence-neutral failure classification.
    #[must_use]
    pub fn kind(&self) -> ArtifactRepositoryErrorKind {
        match self {
            Self::InvalidInstallationGeneration | Self::InvalidLimits => {
                ArtifactRepositoryErrorKind::InvalidInput
            }
            Self::Cancelled => ArtifactRepositoryErrorKind::Cancelled,
            Self::NotInitialized => ArtifactRepositoryErrorKind::NotInitialized,
            Self::UnsafeDataDirectory => ArtifactRepositoryErrorKind::Conflict,
            Self::RepositoryInUse => ArtifactRepositoryErrorKind::InUse,
            Self::DataDirectoryIo(_) => ArtifactRepositoryErrorKind::Operational,
            Self::State(error) => store_error_kind(error),
            Self::Import(error) => import_error_kind(error),
            Self::Inventory(error) => inventory_error_kind(error),
            Self::Reconciliation(error) => reconciliation_error_kind(error),
            Self::Removal(error) => removal_error_kind(error),
            Self::ArtifactNotInstalled | Self::RemovalRecoveryNotPending => {
                ArtifactRepositoryErrorKind::NotFound
            }
            Self::RemovalRecoveryPending { .. } | Self::RemovalRecoveryRequired { .. } => {
                ArtifactRepositoryErrorKind::RecoveryRequired
            }
            Self::StaleInstallation => ArtifactRepositoryErrorKind::StaleSelection,
        }
    }

    /// Returns the exact generation required for recovery, when available.
    #[must_use]
    pub fn recovery_key(&self) -> Option<&ArtifactInstallationKey> {
        match self {
            Self::RemovalRecoveryPending { key } | Self::RemovalRecoveryRequired { key, .. } => {
                Some(key)
            }
            _ => None,
        }
    }
}

fn store_error_kind(error: &StoreError) -> ArtifactRepositoryErrorKind {
    use ArtifactRepositoryErrorKind as Kind;
    match error {
        StoreError::Database(_) => Kind::Operational,
        StoreError::Serialization(_)
        | StoreError::InvalidManifest(_)
        | StoreError::InvalidInstallation(_)
        | StoreError::InvalidQualification(_)
        | StoreError::InvalidInvalidation(_)
        | StoreError::InvalidDecision(_)
        | StoreError::CorruptRecord
        | StoreError::InvalidActiveBinding => Kind::CorruptState,
        StoreError::UnsupportedSchema(_) | StoreError::MigrationRequired { .. } => {
            Kind::IncompatibleState
        }
        StoreError::NotInitialized => Kind::NotInitialized,
        StoreError::RecordTooLarge
        | StoreError::InvalidLimit
        | StoreError::InventoryLimitExceeded
        | StoreError::InstallationEpochExhausted => Kind::ResourceLimit,
        StoreError::ImmutableConflict | StoreError::VerificationFailed => Kind::Conflict,
        StoreError::MissingRecord => Kind::NotFound,
        StoreError::ActiveArtifact => Kind::ActiveArtifact,
        StoreError::RemovalPending => Kind::RecoveryRequired,
        StoreError::StaleInstallation => Kind::StaleSelection,
    }
}

fn import_error_kind(error: &ArtifactImportError) -> ArtifactRepositoryErrorKind {
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
        Error::RemovalPending => Kind::RecoveryRequired,
    }
}

fn inventory_error_kind(error: &ArtifactInventoryError) -> ArtifactRepositoryErrorKind {
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

fn reconciliation_error_kind(error: &ArtifactReconciliationError) -> ArtifactRepositoryErrorKind {
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
        Error::RemovalPending => Kind::RecoveryRequired,
    }
}

fn removal_error_kind(error: &ArtifactRemovalError) -> ArtifactRepositoryErrorKind {
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
