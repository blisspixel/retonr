use std::io;

use thiserror::Error;

use rewrite_model::{
    ActivationDecisionError, ArtifactSetManifestError, EffectivePackageEvidenceError,
    EffectiveRuntimeStateError, InstallationError, InstalledArtifactSetError, ManifestError,
    QualificationInvalidationError, QualificationRecordError, QualificationRecordV2Error,
    RuntimeBuildIdentityError,
};

/// Result returned by the durable artifact state adapter.
pub type StoreResult<T> = Result<T, StoreError>;

/// Durable artifact state failure with no document or model content in its display
/// representation.
#[derive(Debug, Error)]
pub enum StoreError {
    /// `SQLite` could not complete a bounded state operation.
    #[error("artifact state database operation failed")]
    Database(#[source] rusqlite::Error),
    /// A stored or incoming record could not be encoded or decoded.
    #[error("artifact state record serialization failed")]
    Serialization(#[source] serde_json::Error),
    /// An artifact manifest failed domain validation.
    #[error("artifact manifest is invalid")]
    InvalidManifest(#[source] ManifestError),
    /// Installed artifact state failed domain validation.
    #[error("installed artifact state is invalid")]
    InvalidInstallation(#[source] InstallationError),
    /// A qualification record failed domain validation.
    #[error("qualification record is invalid")]
    InvalidQualification(#[source] QualificationRecordError),
    /// An artifact-set manifest failed domain validation.
    #[error("artifact-set manifest is invalid")]
    InvalidArtifactSet(#[source] ArtifactSetManifestError),
    /// Installed artifact-set state failed domain validation.
    #[error("installed artifact-set state is invalid")]
    InvalidArtifactSetInstallation(#[source] InstalledArtifactSetError),
    /// A runtime-build identity failed domain validation.
    #[error("runtime-build identity is invalid")]
    InvalidRuntimeBuild(#[source] RuntimeBuildIdentityError),
    /// An effective runtime-state record failed domain validation.
    #[error("effective runtime state is invalid")]
    InvalidRuntimeState(#[source] EffectiveRuntimeStateError),
    /// Effective-package evidence failed domain validation.
    #[error("effective package evidence is invalid")]
    InvalidEffectivePackage(#[source] EffectivePackageEvidenceError),
    /// A qualification-v2 evidence record failed domain validation.
    #[error("qualification v2 record is invalid")]
    InvalidQualificationV2(#[source] QualificationRecordV2Error),
    /// A qualification invalidation failed domain validation.
    #[error("qualification invalidation is invalid")]
    InvalidInvalidation(#[source] QualificationInvalidationError),
    /// An activation decision failed domain validation.
    #[error("activation decision is invalid")]
    InvalidDecision(#[source] ActivationDecisionError),
    /// A serialized record exceeded the adapter's fixed bound.
    #[error("artifact state record exceeds the storage bound")]
    RecordTooLarge,
    /// A caller-supplied state inventory ceiling was zero or not representable.
    #[error("artifact state inventory limit is invalid")]
    InvalidLimit,
    /// Durable manifest state exceeded the caller-owned inventory ceiling.
    #[error("artifact state inventory exceeds the configured entry limit")]
    InventoryLimitExceeded,
    /// A durable record disagreed with its indexed identity or columns.
    #[error("persisted artifact state record failed integrity validation")]
    CorruptRecord,
    /// The database schema is outside the versions this adapter understands.
    #[error("unsupported artifact state schema {0}")]
    UnsupportedSchema(i64),
    /// An existing state database was required but no filesystem entry existed.
    #[error("artifact state database is not initialized")]
    NotInitialized,
    /// A read-only open found an older schema that requires an explicit migration.
    #[error("artifact state schema {found} requires migration to {current}")]
    MigrationRequired {
        /// Schema version found in the existing database.
        found: i64,
        /// Exact schema version required by this adapter.
        current: i64,
    },
    /// A backup destination handle was nonempty, aliased, or not a regular file.
    #[error("artifact state backup destination is invalid")]
    InvalidBackupDestination,
    /// A state backup could not fit within the caller-owned byte ceiling.
    #[error("artifact state backup exceeds its configured byte limit")]
    BackupTooLarge,
    /// Cooperative cancellation stopped a state backup before completion.
    #[error("artifact state backup was cancelled")]
    BackupCancelled,
    /// A bounded state snapshot could not make progress to completion.
    #[error("artifact state backup could not complete within its step limit")]
    BackupIncomplete,
    /// The caller-held backup file could not be read, written, or synchronized.
    #[error("artifact state backup file operation failed")]
    BackupIo(#[source] io::Error),
    /// Migration was requested before this session completed a verified backup.
    #[error("artifact state migration requires a completed verified backup")]
    BackupRequired,
    /// An immutable identifier already names different record bytes.
    #[error("immutable artifact state conflicts with an existing record")]
    ImmutableConflict,
    /// A required manifest, installation, qualification, or binding does not exist.
    #[error("required artifact state record is missing")]
    MissingRecord,
    /// An installed artifact still has an active role binding.
    #[error("active artifact installation cannot be removed")]
    ActiveArtifact,
    /// A prepared removal blocks installation or activation until resumed.
    #[error("artifact removal is pending")]
    RemovalPending,
    /// The selected installation generation is no longer current.
    #[error("artifact installation selection is stale")]
    StaleInstallation,
    /// The per-artifact installation epoch cannot be incremented safely.
    #[error("artifact installation epoch is exhausted")]
    InstallationEpochExhausted,
    /// Persisted active state did not revalidate against its durable evidence.
    #[error("persisted active artifact binding failed recovery validation")]
    InvalidActiveBinding,
    /// Current artifact bytes did not match the durable installation snapshot.
    #[error("installed artifact bytes failed current verification")]
    VerificationFailed,
}

impl From<rusqlite::Error> for StoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value)
    }
}
