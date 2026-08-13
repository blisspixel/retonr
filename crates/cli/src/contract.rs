//! Versioned, content-redacted command-line transport contracts.

use std::{
    error::Error,
    fmt,
    fs::File,
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

/// Maximum accepted encoded artifact manifest size.
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

/// CLI report representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ReportFormat {
    /// Versioned machine-readable JSON.
    Json,
    /// Concise human-readable output that is not a machine contract.
    Text,
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
    /// Offline single-file artifact import.
    #[serde(rename = "model.import")]
    ModelImport,
    /// Read-only managed artifact inventory.
    #[serde(rename = "model.inventory")]
    ModelInventory,
    /// Read-only inspection of operations requiring recovery.
    #[serde(rename = "model.pending_operations")]
    ModelPendingOperations,
    /// Selected exact artifact reconciliation.
    #[serde(rename = "model.reconcile")]
    ModelReconcile,
    /// Selected inactive installation removal.
    #[serde(rename = "model.remove")]
    ModelRemove,
    /// Forward recovery of one exact prepared removal.
    #[serde(rename = "model.recover_removal")]
    ModelRecoverRemoval,
    /// Product and machine-contract version inspection.
    #[serde(rename = "version")]
    Version,
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
        }
    }

    /// Attaches the exact prepared generation required for safe recovery.
    #[must_use]
    pub fn with_recovery_selection(mut self, selection: ArtifactSelectionDto) -> Self {
        self.recovery_selection = Some(selection);
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

    /// Creates a CLI identity from a validated domain artifact identity.
    #[must_use]
    pub fn from_artifact_id(value: &ArtifactId) -> Self {
        Self(value.digest().clone())
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
    let file = File::open(path).map_err(|error| ManifestInputError::Io(error.kind()))?;
    parse_manifest_bounded(file, maximum_bytes)
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
mod tests {
    use std::io::Cursor;

    use serde_json::{Value, json};

    use super::*;

    fn valid_manifest_json() -> Vec<u8> {
        let artifact_digest = Digest::sha256(b"artifact");
        let license_digest = Digest::sha256(b"license");
        serde_json::to_vec(&json!({
            "schema_version": 1,
            "artifact_id": artifact_digest,
            "source": {
                "origin": "fixture/model",
                "revision": "fixture-revision"
            },
            "artifact_digest": Digest::sha256(b"artifact"),
            "byte_size": 8,
            "format": "gguf",
            "family": "fixture",
            "architecture": "transformer",
            "quantization": "q4",
            "tokenizer": null,
            "licenses": [{
                "component": "weights",
                "identifier": "Apache-2.0",
                "text_digest": license_digest
            }],
            "declared_capabilities": {
                "roles": ["generation"],
                "languages": ["en"],
                "context_tokens": 8192
            }
        }))
        .expect("serialize fixture manifest")
    }

    #[test]
    fn success_and_error_envelopes_are_exact_and_content_free() {
        let success =
            SuccessEnvelope::new(CommandName::ModelRemove, json!({"disposition": "removed"}));
        assert_eq!(
            serde_json::to_value(success).expect("serialize success"),
            json!({
                "schema_version": 1,
                "command": "model.remove",
                "status": "ok",
                "result": {"disposition": "removed"}
            })
        );
        let error = ErrorEnvelope::new(
            CommandName::ModelRemove,
            ErrorBody::new(
                ErrorCategory::Recovery,
                ErrorCode::ArtifactRemovalRecoveryRequired,
                true,
            ),
        );
        let encoded = serde_json::to_string(&error).expect("serialize error");
        assert_eq!(
            serde_json::from_str::<Value>(&encoded).expect("parse error envelope"),
            json!({
                "schema_version": 1,
                "command": "model.remove",
                "status": "error",
                "error": {
                    "category": "recovery",
                    "code": "artifact_removal_recovery_required",
                    "retryable": true
                }
            })
        );
        assert!(!encoded.contains("private path or content"));
    }

    #[test]
    fn exact_artifact_selection_round_trips_without_store_types() {
        let digest = Digest::sha256(b"artifact").to_string();
        let selection = ArtifactSelectionDto {
            artifact_id: digest.parse().expect("canonical artifact ID"),
            installation_generation: "7".parse().expect("positive generation"),
        };
        let encoded = serde_json::to_string(&selection).expect("serialize selection");
        assert_eq!(
            encoded,
            format!("{{\"artifact_id\":\"{digest}\",\"installation_generation\":\"7\"}}")
        );
        assert_eq!(
            serde_json::from_str::<ArtifactSelectionDto>(&encoded).expect("deserialize selection"),
            selection
        );
        assert!("0".parse::<InstallationGeneration>().is_err());
        assert!("01".parse::<InstallationGeneration>().is_err());
        assert!("+1".parse::<InstallationGeneration>().is_err());
        assert!("A".repeat(64).parse::<ArtifactIdArgument>().is_err());
    }

    #[test]
    fn manifest_parser_is_bounded_strict_and_domain_validated() {
        let encoded = valid_manifest_json();
        let manifest = parse_manifest_bounded(Cursor::new(&encoded), encoded.len())
            .expect("valid exact-boundary manifest");
        assert_eq!(manifest.byte_size, 8);
        assert_eq!(
            parse_manifest_bounded(Cursor::new(&encoded), encoded.len() - 1),
            Err(ManifestInputError::TooLarge)
        );
        assert_eq!(
            parse_manifest_bounded(Cursor::new(&encoded), 0),
            Err(ManifestInputError::InvalidLimit)
        );

        let mut unknown: Value = serde_json::from_slice(&encoded).expect("fixture JSON");
        unknown["unknown"] = json!(true);
        assert_eq!(
            parse_manifest_bounded(
                Cursor::new(serde_json::to_vec(&unknown).expect("serialize unknown field")),
                MAX_MANIFEST_BYTES,
            ),
            Err(ManifestInputError::InvalidJson)
        );

        let mut invalid: Value = serde_json::from_slice(&encoded).expect("fixture JSON");
        invalid["schema_version"] = json!(0);
        assert_eq!(
            parse_manifest_bounded(
                Cursor::new(serde_json::to_vec(&invalid).expect("serialize invalid manifest")),
                MAX_MANIFEST_BYTES,
            ),
            Err(ManifestInputError::UnsupportedSchema)
        );
    }
}
