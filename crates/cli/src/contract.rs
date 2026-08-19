//! Versioned, content-redacted command-line transport contracts.

use std::{
    error::Error,
    fmt,
    fs::{self, File},
    io::{self, Read},
    num::NonZeroU64,
    path::Path,
    str::FromStr,
};

use clap::ValueEnum;
use rewrite_app::ArtifactInstallationKey;
use rewrite_model::{ArtifactId, ArtifactManifest};
use rewrite_types::Digest;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

mod set;

pub use set::{ArtifactSetSelectionDto, parse_set_manifest_bounded, read_set_manifest_bounded};

/// Current major version of the CLI JSON envelope.
pub const CLI_SCHEMA_VERSION: u32 = 1;

/// Successful command completion.
pub const EXIT_SUCCESS: u8 = 0;
/// Filesystem, storage, backend, or internal operational failure.
pub const EXIT_OPERATIONAL: u8 = 1;
/// Invalid command, option, input syntax, or configuration.
pub const EXIT_USAGE: u8 = 2;
/// Policy refusal or an explicitly fatal domain outcome.
pub const EXIT_POLICY: u8 = 3;
/// Unsupported schema, format, capability, runtime, or protocol.
pub const EXIT_COMPATIBILITY: u8 = 4;
/// Durable state requires an exact recovery operation.
pub const EXIT_RECOVERY_REQUIRED: u8 = 5;
/// Cancellation observed before an irreversible boundary.
pub const EXIT_CANCELLED: u8 = 130;

/// Maximum accepted encoded artifact or artifact-set manifest size.
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

/// CLI report representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReportFormat {
    /// Versioned machine-readable JSON.
    Json,
    /// Concise human-readable output that is not a machine contract.
    Text,
}

impl ReportFormat {
    /// Resolves an explicit `--format` flag, or a terminal versus pipe default.
    ///
    /// A terminal defaults to text so everyday commands stay short. A pipe or
    /// file defaults to JSON so scripts keep a stable machine envelope.
    pub(crate) const fn from_invocation(explicit: Option<Self>, stdout_is_terminal: bool) -> Self {
        match explicit {
            Some(format) => format,
            None if stdout_is_terminal => Self::Text,
            None => Self::Json,
        }
    }
}

/// Stable command identity carried by a JSON envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum CommandName {
    /// Command-line parsing before a specific operation is known.
    #[serde(rename = "cli")]
    Cli,
    /// Deterministic candidate validation.
    #[serde(rename = "check")]
    Check,
    /// Grounded rewrite of one source document.
    #[serde(rename = "rewrite")]
    Rewrite,
    /// Pre-model inventory of one source document.
    #[serde(rename = "inspect")]
    Inspect,
    /// Offline single-file artifact import.
    #[serde(rename = "model.import")]
    ModelImport,
    /// Offline exact artifact-set folder import.
    #[serde(rename = "model.import_set")]
    ModelImportSet,
    /// Read-only catalog of registered single-file installations.
    #[serde(rename = "model.list")]
    ModelList,
    /// Read-only inspection of one registered artifact.
    #[serde(rename = "model.inspect")]
    ModelInspect,
    /// Read-only managed artifact inventory.
    #[serde(rename = "model.inventory")]
    ModelInventory,
    /// Read-only managed artifact-set inventory.
    #[serde(rename = "model.inventory_set")]
    ModelInventorySet,
    /// Read-only inspection of operations requiring recovery.
    #[serde(rename = "model.pending_operations")]
    ModelPendingOperations,
    /// Explicit forward migration of one existing model repository.
    #[serde(rename = "model.migrate")]
    ModelMigrate,
    /// Selected exact artifact reconciliation.
    #[serde(rename = "model.reconcile")]
    ModelReconcile,
    /// Selected exact artifact-set reconciliation.
    #[serde(rename = "model.reconcile_set")]
    ModelReconcileSet,
    /// Selected inactive installation removal.
    #[serde(rename = "model.remove")]
    ModelRemove,
    /// Forward recovery of one exact prepared removal.
    #[serde(rename = "model.recover_removal")]
    ModelRecoverRemoval,
    /// Selected inactive artifact-set installation removal.
    #[serde(rename = "model.remove_set")]
    ModelRemoveSet,
    /// Forward recovery of one exact prepared artifact-set removal.
    #[serde(rename = "model.recover_set_removal")]
    ModelRecoverSetRemoval,
    /// Product and machine-contract version inspection.
    #[serde(rename = "version")]
    Version,
    /// Read-only local recovery inspection.
    #[serde(rename = "doctor")]
    Doctor,
    /// Generated shell-completion script.
    #[serde(rename = "completions")]
    Completions,
    /// Generated section-1 manual page.
    #[serde(rename = "man")]
    Man,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum EnvelopeStatus {
    Ok,
    Error,
}

/// Versioned successful JSON command envelope.
#[derive(Debug, Serialize)]
pub struct SuccessEnvelope<T> {
    schema_version: u32,
    command: CommandName,
    status: EnvelopeStatus,
    result: T,
}

impl<T> SuccessEnvelope<T> {
    /// Wraps one complete, content-redacted command result.
    pub const fn new(command: CommandName, result: T) -> Self {
        Self {
            schema_version: CLI_SCHEMA_VERSION,
            command,
            status: EnvelopeStatus::Ok,
            result,
        }
    }
}

/// Stable high-level category for a command error.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Invalid invocation, input syntax, or configuration.
    Usage,
    /// Valid input that current policy or state refuses.
    Policy,
    /// Unsupported schema, format, capability, runtime, or protocol.
    Compatibility,
    /// Filesystem, storage, backend, or internal failure.
    Operational,
    /// Durable state requires an exact recovery operation.
    Recovery,
    /// Cancellation observed before an irreversible boundary.
    Cancelled,
}

/// Stable content-free CLI error code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Command-line arguments did not form a valid invocation.
    InvalidInvocation,
    /// An artifact manifest was malformed or failed validation.
    InvalidManifest,
    /// A selected input could not be opened or read.
    InputUnreadable,
    /// The fixed artifact repository has not been initialized.
    RepositoryNotInitialized,
    /// Another operation currently owns an incompatible repository lock.
    RepositoryInUse,
    /// A destructive operation lacked explicit confirmation.
    ConfirmationRequired,
    /// The selected artifact or installation does not exist.
    ArtifactNotFound,
    /// No exact prepared removal exists for the selected generation.
    RemovalRecoveryNotPending,
    /// The exact installation generation is no longer current.
    StaleInstallation,
    /// The selected artifact is active and cannot be removed.
    ArtifactActive,
    /// Current bytes, manifest, or immutable state disagree.
    ArtifactConflict,
    /// A caller-owned resource ceiling was reached.
    ResourceLimitExceeded,
    /// Storage or state changed during a coherence-sensitive operation.
    ConcurrentModification,
    /// Persisted state failed integrity validation.
    CorruptState,
    /// Existing state requires another supported schema version.
    IncompatibleState,
    /// A valid request was refused by current policy or durable state.
    PolicyRefusal,
    /// The selected output destination already exists and is never replaced.
    OutputExists,
    /// The requested contract or capability is not supported.
    Unsupported,
    /// An operation failed without a safe domain result.
    OperationalFailure,
    /// Prepared artifact removal must be resumed exactly.
    ArtifactRemovalRecoveryRequired,
    /// The operation observed cancellation.
    OperationCancelled,
}

/// Content-free error details for a failed command.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ErrorBody {
    category: ErrorCategory,
    code: ErrorCode,
    retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_selection: Option<ArtifactSelectionDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    set_recovery_selection: Option<set::ArtifactSetSelectionDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    migration_backup_key: Option<String>,
}

impl ErrorBody {
    /// Creates stable error details without raw content or an error source chain.
    #[must_use]
    pub const fn new(category: ErrorCategory, code: ErrorCode, retryable: bool) -> Self {
        Self {
            category,
            code,
            retryable,
            recovery_selection: None,
            set_recovery_selection: None,
            migration_backup_key: None,
        }
    }

    /// Attaches the exact prepared generation required for safe recovery.
    #[must_use]
    pub fn with_recovery_selection(mut self, selection: ArtifactSelectionDto) -> Self {
        self.recovery_selection = Some(selection);
        self
    }

    /// Attaches the exact prepared set generation required for safe recovery.
    #[must_use]
    pub fn with_set_recovery_selection(mut self, selection: set::ArtifactSetSelectionDto) -> Self {
        self.set_recovery_selection = Some(selection);
        self
    }

    /// Attaches the repository-owned backup key retained after a migration failure.
    #[must_use]
    pub fn with_migration_backup_key(mut self, backup_key: String) -> Self {
        self.migration_backup_key = Some(backup_key);
        self
    }
}

/// Versioned failed JSON command envelope.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    schema_version: u32,
    command: CommandName,
    status: EnvelopeStatus,
    error: ErrorBody,
}

impl ErrorEnvelope {
    /// Wraps one stable, content-free command error.
    #[must_use]
    pub const fn new(command: CommandName, error: ErrorBody) -> Self {
        Self {
            schema_version: CLI_SCHEMA_VERSION,
            command,
            status: EnvelopeStatus::Error,
            error,
        }
    }
}

/// Canonical CLI artifact identity parsed without persistence-layer types.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ArtifactIdArgument(Digest);

impl ArtifactIdArgument {
    /// Returns the canonical lowercase SHA-256 artifact identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Converts the validated digest into the domain artifact identity.
    #[must_use]
    pub fn to_artifact_id(&self) -> ArtifactId {
        ArtifactId::from_digest(self.0.clone())
    }

    /// Converts the validated digest into the domain artifact-set identity.
    #[must_use]
    pub fn to_artifact_set_id(&self) -> rewrite_model::ArtifactSetId {
        rewrite_model::ArtifactSetId::from_digest(self.0.clone())
    }

    /// Creates a CLI identity from a validated domain artifact identity.
    #[must_use]
    pub fn from_artifact_id(value: &ArtifactId) -> Self {
        Self(value.digest().clone())
    }

    /// Creates a CLI identity from a validated SHA-256 digest.
    #[must_use]
    pub fn from_digest(value: &Digest) -> Self {
        Self(value.clone())
    }
}

impl FromStr for ArtifactIdArgument {
    type Err = ArtifactIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Digest::from_sha256_hex(value.to_owned())
            .map(Self)
            .map_err(|_| ArtifactIdParseError)
    }
}

impl fmt::Display for ArtifactIdArgument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Error returned for a noncanonical CLI artifact identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactIdParseError;

impl fmt::Display for ArtifactIdParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("artifact ID must be 64 lowercase hexadecimal characters")
    }
}

impl Error for ArtifactIdParseError {}

/// Positive installation generation with exact decimal JSON representation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct InstallationGeneration(NonZeroU64);

impl InstallationGeneration {
    /// Returns the positive generation value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }

    /// Creates a CLI generation from a validated positive value.
    #[must_use]
    pub fn new(value: u64) -> Option<Self> {
        NonZeroU64::new(value).map(Self)
    }
}

impl FromStr for InstallationGeneration {
    type Err = InstallationGenerationParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(InstallationGenerationParseError);
        }
        value
            .parse::<u64>()
            .ok()
            .filter(|value| i64::try_from(*value).is_ok())
            .and_then(NonZeroU64::new)
            .map(Self)
            .ok_or(InstallationGenerationParseError)
    }
}

impl fmt::Display for InstallationGeneration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.get())
    }
}

impl Serialize for InstallationGeneration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for InstallationGeneration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

/// Error returned for a noncanonical installation generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstallationGenerationParseError;

impl fmt::Display for InstallationGenerationParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("installation generation must be a canonical positive decimal")
    }
}

impl Error for InstallationGenerationParseError {}

/// Persistence-neutral selector for one exact installed artifact generation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArtifactSelectionDto {
    /// Canonical content-derived artifact identity.
    pub artifact_id: ArtifactIdArgument,
    /// Exact positive installation generation represented as a JSON string.
    pub installation_generation: InstallationGeneration,
}

impl From<&ArtifactInstallationKey> for ArtifactSelectionDto {
    fn from(value: &ArtifactInstallationKey) -> Self {
        Self {
            artifact_id: ArtifactIdArgument::from_artifact_id(value.artifact_id()),
            installation_generation: InstallationGeneration::new(value.installation_generation())
                .expect("application installation keys always contain a positive generation"),
        }
    }
}

/// Content-redacted bounded manifest input failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestInputError {
    /// The caller supplied a zero byte ceiling.
    InvalidLimit,
    /// The manifest file could not be opened or read.
    Io(io::ErrorKind),
    /// Encoded manifest bytes exceeded the caller-owned ceiling.
    TooLarge,
    /// Encoded bytes were not one valid strict artifact-manifest JSON value.
    InvalidJson,
    /// The decoded manifest uses another schema version.
    UnsupportedSchema,
    /// The decoded artifact manifest failed its domain invariants.
    InvalidManifest,
}

impl fmt::Display for ManifestInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimit => "artifact manifest byte limit is invalid",
            Self::Io(_) => "artifact manifest could not be read",
            Self::TooLarge => "artifact manifest exceeds the configured byte limit",
            Self::InvalidJson => "artifact manifest JSON is invalid",
            Self::UnsupportedSchema => "artifact manifest schema is unsupported",
            Self::InvalidManifest => "artifact manifest is invalid",
        })
    }
}

impl Error for ManifestInputError {}

/// Opens and parses one strict artifact manifest under a byte ceiling.
///
/// Errors disclose no input path, manifest content, or parser source chain.
///
/// # Errors
///
/// Returns [`ManifestInputError`] when the byte ceiling is zero, the file cannot be
/// read, the encoded input exceeds the ceiling, JSON is malformed, or the decoded
/// manifest violates its domain contract.
pub fn read_manifest_bounded(
    path: &Path,
    maximum_bytes: usize,
) -> Result<ArtifactManifest, ManifestInputError> {
    let file = open_regular_file(path).map_err(|error| ManifestInputError::Io(error.kind()))?;
    parse_manifest_bounded(file, maximum_bytes)
}

/// Path value that selects the standard stream instead of a filesystem entry.
pub(crate) const STANDARD_STREAM_PATH: &str = "-";

/// Reads one bounded document from a file or from standard input.
///
/// Standard input is read to end of file without trimming, so blank lines,
/// leading and trailing whitespace, a byte order mark, the newline kind, and an
/// absent final newline all remain exactly as supplied.
pub(crate) fn read_input_bounded(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    if path.as_os_str() == STANDARD_STREAM_PATH {
        read_bounded(io::stdin().lock(), limit)
    } else {
        read_bounded(open_regular_file(path)?, limit)
    }
}

/// Reads at most `limit` bytes and rejects anything longer.
pub(crate) fn read_bounded(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    reader.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "input exceeds the supported byte limit",
        ));
    }
    Ok(bytes)
}

pub(crate) fn open_regular_file(path: &Path) -> io::Result<File> {
    let listed = fs::symlink_metadata(path)?;
    let metadata = if listed.file_type().is_symlink() {
        fs::metadata(path)?
    } else {
        listed
    };
    if metadata.is_file() {
        File::open(path)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input must be a regular file",
        ))
    }
}

/// Parses one strict artifact manifest from a bounded byte stream.
///
/// Errors disclose no manifest content or parser source chain.
///
/// # Errors
///
/// Returns [`ManifestInputError`] when the byte ceiling is zero, the stream cannot
/// be read, the encoded input exceeds the ceiling, JSON is malformed, or the
/// decoded manifest violates its domain contract.
pub fn parse_manifest_bounded(
    reader: impl Read,
    maximum_bytes: usize,
) -> Result<ArtifactManifest, ManifestInputError> {
    if maximum_bytes == 0 {
        return Err(ManifestInputError::InvalidLimit);
    }
    let read_limit = u64::try_from(maximum_bytes)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::with_capacity(maximum_bytes.min(64 * 1024));
    reader
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| ManifestInputError::Io(error.kind()))?;
    if bytes.len() > maximum_bytes {
        return Err(ManifestInputError::TooLarge);
    }
    let manifest: ArtifactManifest =
        serde_json::from_slice(&bytes).map_err(|_| ManifestInputError::InvalidJson)?;
    manifest.validate().map_err(|error| match error {
        rewrite_model::ManifestError::UnsupportedSchema(_) => ManifestInputError::UnsupportedSchema,
        _ => ManifestInputError::InvalidManifest,
    })?;
    Ok(manifest)
}

#[cfg(test)]
mod tests;
