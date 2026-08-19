use std::io;

mod error_kind;

pub(crate) use error_kind::store_error_kind;
pub(crate) use error_kind::{map_import_error, map_reconciliation_error};

use rewrite_model::{ArtifactId, ArtifactSetId};
use rewrite_model_store::{
    StoreError, StoredArtifactInstallation, StoredArtifactSetInstallation, WriteDisposition,
};
use thiserror::Error;

use crate::{
    ArtifactImportError, ArtifactImportResult, ArtifactInventoryError,
    ArtifactOrphanReconciliationResult, ArtifactReconciliationDisposition,
    ArtifactReconciliationError, ArtifactRemovalDisposition, ArtifactRemovalError,
    ArtifactRemovalResult, ArtifactSetImportDisposition, ArtifactSetImportError,
    ArtifactSetImportResult, ArtifactSetInventoryError, ArtifactSetLeaseError,
    ArtifactSetReconciliationError, ArtifactSetRemovalError, ArtifactSetRemovalResult,
    runtime_artifact_set_lease::set_lease_error_kind,
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

/// Caller-owned ceilings for an explicit repository state migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactRepositoryMigrationLimits {
    /// Maximum logical bytes admitted for the source database and its backup.
    pub maximum_state_bytes: u64,
    /// Maximum direct repository entries admitted before reserving a backup.
    pub maximum_repository_entries: usize,
}

/// Opaque repository-relative identity of one retained pre-migration backup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositoryBackupKey(String);

impl ArtifactRepositoryBackupKey {
    pub(crate) fn from_file_name(file_name: String) -> Self {
        Self(file_name)
    }

    /// Returns the content-free repository-relative backup token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Outcome of an explicit repository state migration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactRepositoryMigrationDisposition {
    /// The repository already used the current exact schema; no backup was made.
    AlreadyCurrent,
    /// A verified backup was retained and a supported migration committed.
    Migrated,
}

/// Successful explicit repository state migration result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositoryMigrationResult {
    /// Exact validated schema observed before the operation.
    pub from_schema: u32,
    /// Exact schema required by this build after the operation.
    pub to_schema: u32,
    /// Whether this call changed durable schema state.
    pub disposition: ArtifactRepositoryMigrationDisposition,
    /// Retained pre-migration backup identity, present only after migration.
    pub backup_key: Option<ArtifactRepositoryBackupKey>,
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

/// Persistence-neutral identity for one exact installed artifact-set generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactSetInstallationKey {
    /// Content-derived artifact-set identity.
    artifact_set_id: ArtifactSetId,
    /// Positive generation preventing a stale operation from targeting a reinstall.
    installation_generation: u64,
}

impl ArtifactSetInstallationKey {
    /// Constructs one validated exact artifact-set installation key.
    ///
    /// # Errors
    ///
    /// Returns [`ArtifactRepositoryError::InvalidInstallationGeneration`] when the
    /// generation is zero or outside the durable store range.
    pub fn new(
        artifact_set_id: ArtifactSetId,
        installation_generation: u64,
    ) -> Result<Self, ArtifactRepositoryError> {
        if installation_generation == 0 || i64::try_from(installation_generation).is_err() {
            Err(ArtifactRepositoryError::InvalidInstallationGeneration)
        } else {
            Ok(Self {
                artifact_set_id,
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

    /// Returns the content-derived artifact-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact positive installation generation.
    #[must_use]
    pub const fn installation_generation(&self) -> u64 {
        self.installation_generation
    }

    pub(crate) fn from_stored(value: &StoredArtifactSetInstallation) -> Self {
        Self {
            artifact_set_id: value.installed.artifact_set_id().clone(),
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

/// Successful repository-level offline artifact-set import result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositorySetImportResult {
    /// Exact store-issued artifact-set installation key.
    pub key: ArtifactSetInstallationKey,
    /// Whether this call imported or confirmed exact prior state.
    pub disposition: ArtifactSetImportDisposition,
}

impl From<ArtifactSetImportResult> for ArtifactRepositorySetImportResult {
    fn from(value: ArtifactSetImportResult) -> Self {
        Self {
            key: ArtifactSetInstallationKey::from_stored(&value.state.installation),
            disposition: value.disposition,
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
    /// Exact prepared artifact-set-removal generations in set-identity order.
    pub artifact_set_removals: Vec<ArtifactSetInstallationKey>,
}

/// Successful repository-level set removal or recovery result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRepositorySetRemovalResult {
    /// Exact set installation key affected or confirmed by the operation.
    pub key: ArtifactSetInstallationKey,
    /// Whether the tree was removed, recovered, or already absent for this generation.
    pub disposition: ArtifactRemovalDisposition,
}

impl From<ArtifactSetRemovalResult> for ArtifactRepositorySetRemovalResult {
    fn from(value: ArtifactSetRemovalResult) -> Self {
        Self {
            key: ArtifactSetInstallationKey::from_stored(&value.selection),
            disposition: value.disposition,
        }
    }
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
    /// Existing state or its backup exceeds the configured migration byte ceiling.
    #[error("artifact repository migration byte limit was exceeded")]
    MigrationLimitExceeded,
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
    /// Migration failed after a verified backup was retained.
    #[error("artifact repository migration failed after retaining a verified backup")]
    MigrationFailed {
        /// Opaque repository-relative identity of the retained backup.
        backup_key: ArtifactRepositoryBackupKey,
        /// Underlying repository failure.
        #[source]
        source: Box<ArtifactRepositoryError>,
    },
    /// The fixed artifact state database could not be opened or validated.
    #[error("artifact repository state operation failed")]
    State(#[from] StoreError),
    /// Offline artifact import failed.
    #[error(transparent)]
    Import(#[from] ArtifactImportError),
    /// Offline artifact-set import failed.
    #[error(transparent)]
    SetImport(#[from] ArtifactSetImportError),
    /// Shared managed artifact-set lease acquisition failed.
    #[error(transparent)]
    SetLease(#[from] ArtifactSetLeaseError),
    /// Read-only artifact inventory failed.
    #[error(transparent)]
    Inventory(#[from] ArtifactInventoryError),
    /// Read-only artifact-set inventory failed.
    #[error(transparent)]
    SetInventory(#[from] ArtifactSetInventoryError),
    /// Selected orphan reconciliation failed.
    #[error(transparent)]
    Reconciliation(#[from] ArtifactReconciliationError),
    /// Selected set-root reconciliation failed.
    #[error(transparent)]
    SetReconciliation(#[from] ArtifactSetReconciliationError),
    /// Selected inactive artifact removal failed.
    #[error(transparent)]
    Removal(#[from] ArtifactRemovalError),
    /// Selected inactive artifact-set removal failed.
    #[error(transparent)]
    SetRemoval(#[from] ArtifactSetRemovalError),
    /// No current installation exists for the selected artifact identity.
    #[error("artifact is not currently installed")]
    ArtifactNotInstalled,
    /// No current installation exists for the selected artifact-set identity.
    #[error("artifact set is not currently installed")]
    ArtifactSetNotInstalled,
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
    /// Fresh set removal cannot continue until the prepared operation is recovered.
    #[error("artifact set has a prepared removal that requires explicit recovery")]
    SetRemovalRecoveryPending {
        /// Exact prepared set generation that must be recovered.
        key: ArtifactSetInstallationKey,
    },
    /// Set removal crossed its durable preparation boundary and must recover exactly.
    #[error("artifact-set removal requires exact recovery")]
    SetRemovalRecoveryRequired {
        /// Exact prepared set generation that must be recovered.
        key: ArtifactSetInstallationKey,
        /// Lower-level post-preparation failure.
        #[source]
        source: ArtifactSetRemovalError,
    },
    /// No exact prepared set removal exists for the selected identity.
    #[error("artifact set has no prepared removal to recover")]
    SetRemovalRecoveryNotPending,
    /// The selected installation generation is no longer current.
    #[error("artifact installation selection is stale")]
    StaleInstallation,
    /// The selected set installation generation is no longer current.
    #[error("artifact-set installation selection is stale")]
    StaleSetInstallation,
}

impl ArtifactRepositoryError {
    /// Returns the stable, persistence-neutral failure classification.
    #[must_use]
    pub fn kind(&self) -> ArtifactRepositoryErrorKind {
        match self {
            Self::InvalidInstallationGeneration | Self::InvalidLimits => {
                ArtifactRepositoryErrorKind::InvalidInput
            }
            Self::MigrationLimitExceeded => ArtifactRepositoryErrorKind::ResourceLimit,
            Self::Cancelled => ArtifactRepositoryErrorKind::Cancelled,
            Self::NotInitialized => ArtifactRepositoryErrorKind::NotInitialized,
            Self::UnsafeDataDirectory => ArtifactRepositoryErrorKind::Conflict,
            Self::RepositoryInUse => ArtifactRepositoryErrorKind::InUse,
            Self::DataDirectoryIo(_) => ArtifactRepositoryErrorKind::Operational,
            Self::MigrationFailed { source, .. } => source.kind(),
            Self::State(error) => error_kind::store_error_kind(error),
            Self::Import(error) => error_kind::import_error_kind(error),
            Self::SetImport(error) => error_kind::set_import_error_kind(error),
            Self::SetLease(error) => set_lease_error_kind(error),
            Self::Inventory(error) => error_kind::inventory_error_kind(error),
            Self::SetInventory(error) => super::set_inventory::set_inventory_error_kind(error),
            Self::Reconciliation(error) => error_kind::reconciliation_error_kind(error),
            Self::SetReconciliation(error) => {
                super::set_reconciliation::set_reconciliation_error_kind(error)
            }
            Self::Removal(error) => error_kind::removal_error_kind(error),
            Self::SetRemoval(error) => super::set_removal::set_removal_error_kind(error),
            Self::ArtifactNotInstalled
            | Self::ArtifactSetNotInstalled
            | Self::RemovalRecoveryNotPending
            | Self::SetRemovalRecoveryNotPending => ArtifactRepositoryErrorKind::NotFound,
            Self::RemovalRecoveryPending { .. }
            | Self::RemovalRecoveryRequired { .. }
            | Self::SetRemovalRecoveryPending { .. }
            | Self::SetRemovalRecoveryRequired { .. } => {
                ArtifactRepositoryErrorKind::RecoveryRequired
            }
            Self::StaleInstallation | Self::StaleSetInstallation => {
                ArtifactRepositoryErrorKind::StaleSelection
            }
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

    /// Returns the exact set generation required for recovery, when available.
    #[must_use]
    pub fn set_recovery_key(&self) -> Option<&ArtifactSetInstallationKey> {
        match self {
            Self::SetRemovalRecoveryPending { key }
            | Self::SetRemovalRecoveryRequired { key, .. } => Some(key),
            _ => None,
        }
    }

    /// Returns the retained pre-migration backup identity, when available.
    #[must_use]
    pub fn migration_backup_key(&self) -> Option<&ArtifactRepositoryBackupKey> {
        match self {
            Self::MigrationFailed { backup_key, .. } => Some(backup_key),
            _ => None,
        }
    }
}
