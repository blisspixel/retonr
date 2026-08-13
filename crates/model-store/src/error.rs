use thiserror::Error;

use rewrite_model::{
    ActivationDecisionError, InstallationError, ManifestError, QualificationInvalidationError,
    QualificationRecordError,
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
    /// The database schema is newer than this adapter understands.
    #[error("unsupported artifact state schema {0}")]
    UnsupportedSchema(i64),
    /// An immutable identifier already names different record bytes.
    #[error("immutable artifact state conflicts with an existing record")]
    ImmutableConflict,
    /// A required manifest, installation, qualification, or binding does not exist.
    #[error("required artifact state record is missing")]
    MissingRecord,
    /// An installed artifact still has an active role binding.
    #[error("active artifact installation cannot be removed")]
    ActiveArtifact,
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
