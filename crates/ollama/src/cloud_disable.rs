use std::{fmt, str::FromStr};

use rewrite_model::RuntimePackageManifestId;
use serde::{Serialize, Serializer};
use thiserror::Error;

const MAX_VERSION_BYTES: usize = 32;
const MAX_ENVIRONMENT_DECLARATIONS: usize = 4;
const MAX_ENVIRONMENT_VALUE_BYTES: usize = 16;
const MAX_STARTUP_LOG_BYTES: usize = 64 * 1024;
const CLOUD_DISABLED_MARKER: &str = "Ollama cloud disabled: true";
const CLOUD_ENABLED_MARKER: &str = "Ollama cloud disabled: false";
const CLOUD_DISABLED_LOG_FIELD: &str = "msg=\"Ollama cloud disabled: true\"";
const CLOUD_ENABLED_LOG_FIELD: &str = "msg=\"Ollama cloud disabled: false\"";

/// First Ollama version that documents the cloud-disable feature.
pub const OLLAMA_CLOUD_DISABLE_FEATURE_FLOOR: OllamaVersion = OllamaVersion::new(0, 16, 2);

// Production remains fail closed until an exact runtime artifact has been reviewed.
const REVIEWED_CLOUD_DISABLE_RUNTIMES: &[ReviewedCloudDisableRuntime] = &[];

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReviewedCloudDisableRuntime {
    version: OllamaVersion,
    runtime_package_manifest_id: RuntimePackageManifestId,
}

/// Exact stable Ollama version in `major.minor.patch` form.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OllamaVersion {
    major: u32,
    minor: u32,
    patch: u32,
}

impl OllamaVersion {
    const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Returns the major version component.
    #[must_use]
    pub const fn major(self) -> u32 {
        self.major
    }

    /// Returns the minor version component.
    #[must_use]
    pub const fn minor(self) -> u32 {
        self.minor
    }

    /// Returns the patch version component.
    #[must_use]
    pub const fn patch(self) -> u32 {
        self.patch
    }
}

impl fmt::Display for OllamaVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for OllamaVersion {
    type Err = OllamaVersionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() || value.len() > MAX_VERSION_BYTES || !value.is_ascii() {
            return Err(OllamaVersionParseError);
        }

        let mut components = value.split('.');
        let major = parse_component(components.next())?;
        let minor = parse_component(components.next())?;
        let patch = parse_component(components.next())?;
        if components.next().is_some() {
            return Err(OllamaVersionParseError);
        }

        Ok(Self::new(major, minor, patch))
    }
}

impl Serialize for OllamaVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

/// Error returned when an Ollama version is not an exact stable version.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("invalid exact Ollama version")]
pub struct OllamaVersionParseError;

/// Result of applying the exact-version cloud-disable policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaCloudDisableVersionStatus {
    /// The runtime predates the documented feature.
    FeatureUnavailable,
    /// The runtime has the feature but its exact artifact has not been reviewed.
    Unreviewed,
    /// The exact runtime version is in the production reviewed-version allowlist.
    Reviewed,
}

/// Production policy for the version-gated Ollama cloud-disable feature.
///
/// The reviewed-version allowlist is intentionally empty until an exact canonical runtime
/// artifact is selected and reviewed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OllamaCloudDisableFeaturePolicy;

impl OllamaCloudDisableFeaturePolicy {
    /// Returns the first version that documents the cloud-disable feature.
    #[must_use]
    pub const fn feature_floor() -> OllamaVersion {
        OLLAMA_CLOUD_DISABLE_FEATURE_FLOOR
    }

    /// Returns the number of exact runtime packages reviewed by the production policy.
    #[must_use]
    pub const fn reviewed_runtime_count() -> usize {
        REVIEWED_CLOUD_DISABLE_RUNTIMES.len()
    }

    /// Assesses one exact stable version and runtime package against production policy.
    #[must_use]
    pub fn assess(
        version: OllamaVersion,
        runtime_package_manifest_id: &RuntimePackageManifestId,
    ) -> OllamaCloudDisableVersionStatus {
        assess_runtime(
            version,
            runtime_package_manifest_id,
            REVIEWED_CLOUD_DISABLE_RUNTIMES,
        )
    }
}

/// Validated declaration that a managed launch sets the cloud-disable environment value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaManagedCloudDisableEnvironment(());

impl OllamaManagedCloudDisableEnvironment {
    /// Validates all values assigned to the managed `OLLAMA_NO_CLOUD` key.
    ///
    /// This function does not read the host environment. The caller must pass the values in the
    /// exact environment block prepared for the managed process.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, duplicate, conflicting, or oversized declarations.
    pub fn parse(values: &[&str]) -> Result<Self, OllamaCloudDisableEvidenceError> {
        if values.len() > MAX_ENVIRONMENT_DECLARATIONS
            || values
                .iter()
                .any(|value| value.len() > MAX_ENVIRONMENT_VALUE_BYTES)
        {
            return Err(OllamaCloudDisableEvidenceError::OversizedEnvironmentDeclaration);
        }
        let Some((first, remaining)) = values.split_first() else {
            return Err(OllamaCloudDisableEvidenceError::MissingEnvironmentDeclaration);
        };
        if remaining.iter().any(|value| value != first) || *first != "1" {
            return Err(OllamaCloudDisableEvidenceError::ConflictingEnvironmentDeclaration);
        }
        if !remaining.is_empty() {
            return Err(OllamaCloudDisableEvidenceError::DuplicateEnvironmentDeclaration);
        }
        Ok(Self(()))
    }
}

/// Validated observation of the exact cloud-disabled startup marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OllamaCloudDisableStartupMarker(());

impl OllamaCloudDisableStartupMarker {
    /// Parses bounded managed-process startup output for the exact cloud-disabled marker.
    ///
    /// # Errors
    ///
    /// Returns an error when the output is oversized or has a missing, duplicate, or conflicting
    /// marker.
    pub fn parse(startup_log: &str) -> Result<Self, OllamaCloudDisableEvidenceError> {
        if startup_log.len() > MAX_STARTUP_LOG_BYTES {
            return Err(OllamaCloudDisableEvidenceError::OversizedStartupLog);
        }

        parse_marker_texts([startup_log])
    }

    /// Parses the separately captured standard-output and standard-error streams.
    ///
    /// The aggregate byte length is bounded before UTF-8 decoding, and marker
    /// cardinality is evaluated across both streams without concatenating them.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized or non-UTF-8 output, or for a missing,
    /// duplicate, or conflicting marker across the two streams.
    pub fn parse_streams(
        standard_output: &[u8],
        standard_error: &[u8],
    ) -> Result<Self, OllamaCloudDisableEvidenceError> {
        let total = standard_output
            .len()
            .checked_add(standard_error.len())
            .ok_or(OllamaCloudDisableEvidenceError::OversizedStartupLog)?;
        if total > MAX_STARTUP_LOG_BYTES {
            return Err(OllamaCloudDisableEvidenceError::OversizedStartupLog);
        }
        let standard_output = std::str::from_utf8(standard_output)
            .map_err(|_error| OllamaCloudDisableEvidenceError::InvalidStartupLogEncoding)?;
        let standard_error = std::str::from_utf8(standard_error)
            .map_err(|_error| OllamaCloudDisableEvidenceError::InvalidStartupLogEncoding)?;
        parse_marker_texts([standard_output, standard_error])
    }
}

fn parse_marker_texts<'a>(
    texts: impl IntoIterator<Item = &'a str>,
) -> Result<OllamaCloudDisableStartupMarker, OllamaCloudDisableEvidenceError> {
    let mut disabled_markers = 0_u8;
    let mut enabled_markers = 0_u8;
    for text in texts {
        for line in text.lines() {
            disabled_markers = disabled_markers.saturating_add(u8::from(line_has_marker(
                line,
                CLOUD_DISABLED_MARKER,
                CLOUD_DISABLED_LOG_FIELD,
            )));
            enabled_markers = enabled_markers.saturating_add(u8::from(line_has_marker(
                line,
                CLOUD_ENABLED_MARKER,
                CLOUD_ENABLED_LOG_FIELD,
            )));
        }
    }

    if enabled_markers != 0 {
        return Err(OllamaCloudDisableEvidenceError::ConflictingStartupMarker);
    }
    match disabled_markers {
        0 => Err(OllamaCloudDisableEvidenceError::MissingStartupMarker),
        1 => Ok(OllamaCloudDisableStartupMarker(())),
        _ => Err(OllamaCloudDisableEvidenceError::DuplicateStartupMarker),
    }
}

/// Source of the provider cloud-disable declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaCloudDisableDeclarationSource {
    /// The exact value was supplied in a managed process environment.
    ManagedEnvironment,
}

/// Source of the confirming cloud-disabled marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaCloudDisableMarkerSource {
    /// The marker was parsed from bounded output captured during managed startup.
    ManagedStartupOutput,
}

/// Provider declaration state represented by provider-only evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaProviderDeclarationStatus {
    /// The reviewed declaration and its startup marker were both observed.
    Observed,
}

/// Network-isolation state represented by provider-only evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaNetworkIsolationStatus {
    /// No OS-enforced network isolation is represented by this evidence.
    NotEnforced,
}

/// Qualification state represented by provider-only evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OllamaProviderQualificationStatus {
    /// Provider declaration alone cannot qualify the runtime.
    NotQualified,
}

/// Redacted provider declaration that deliberately makes no network-isolation claim.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OllamaCloudDisableEvidence {
    schema_version: u8,
    runtime_version: OllamaVersion,
    runtime_package_manifest_id: RuntimePackageManifestId,
    feature_floor: OllamaVersion,
    version_reviewed: bool,
    declaration_source: OllamaCloudDisableDeclarationSource,
    marker_source: OllamaCloudDisableMarkerSource,
    provider_declaration: OllamaProviderDeclarationStatus,
    network_isolation: OllamaNetworkIsolationStatus,
    qualification: OllamaProviderQualificationStatus,
}

impl OllamaCloudDisableEvidence {
    /// Observes the declaration using the production exact-version policy.
    ///
    /// The production reviewed-version allowlist is empty until a canonical runtime artifact is
    /// selected and reviewed. Consequently, this constructor currently fails closed for every
    /// runtime version.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or drifting versions, an unavailable or unreviewed feature,
    /// or invalid managed-environment and startup-marker observations.
    pub fn observe(
        runtime_package_manifest_id: &RuntimePackageManifestId,
        version_before: &str,
        version_after: &str,
        managed_environment_values: &[&str],
        startup_log: &str,
    ) -> Result<Self, OllamaCloudDisableEvidenceError> {
        observe_with_policy(
            version_before,
            version_after,
            managed_environment_values,
            startup_log,
            runtime_package_manifest_id,
            REVIEWED_CLOUD_DISABLE_RUNTIMES,
        )
    }

    /// Observes the declaration using separately captured managed startup streams.
    ///
    /// This is equivalent to [`Self::observe`] but preserves the standard-output
    /// and standard-error boundary while checking marker cardinality across both.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid or drifting versions, an unavailable or
    /// unreviewed package, an invalid managed environment, or invalid startup output.
    pub fn observe_startup_streams(
        runtime_package_manifest_id: &RuntimePackageManifestId,
        version_before: &str,
        version_after: &str,
        managed_environment_values: &[&str],
        standard_output: &[u8],
        standard_error: &[u8],
    ) -> Result<Self, OllamaCloudDisableEvidenceError> {
        let version = validate_version_policy(
            version_before,
            version_after,
            runtime_package_manifest_id,
            REVIEWED_CLOUD_DISABLE_RUNTIMES,
        )?;
        OllamaManagedCloudDisableEnvironment::parse(managed_environment_values)?;
        OllamaCloudDisableStartupMarker::parse_streams(standard_output, standard_error)?;
        Ok(observed_evidence(version, runtime_package_manifest_id))
    }

    /// Returns the exact stable runtime version.
    #[must_use]
    pub const fn runtime_version(&self) -> OllamaVersion {
        self.runtime_version
    }

    /// Returns the exact reviewed runtime-package manifest identity.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.runtime_package_manifest_id
    }

    /// Returns whether the exact version was reviewed.
    #[must_use]
    pub const fn version_reviewed(&self) -> bool {
        self.version_reviewed
    }

    /// Returns whether the provider declaration was observed.
    #[must_use]
    pub const fn provider_declared(&self) -> bool {
        matches!(
            self.provider_declaration,
            OllamaProviderDeclarationStatus::Observed
        )
    }

    /// Returns the typed provider-declaration status.
    #[must_use]
    pub const fn provider_declaration_status(&self) -> OllamaProviderDeclarationStatus {
        self.provider_declaration
    }

    /// Returns whether OS network isolation was enforced.
    #[must_use]
    pub const fn network_isolation_enforced(&self) -> bool {
        match self.network_isolation {
            OllamaNetworkIsolationStatus::NotEnforced => false,
        }
    }

    /// Returns the typed network-isolation status.
    #[must_use]
    pub const fn network_isolation_status(&self) -> OllamaNetworkIsolationStatus {
        self.network_isolation
    }

    /// Returns whether this provider-only evidence qualifies the runtime.
    #[must_use]
    pub const fn qualified(&self) -> bool {
        match self.qualification {
            OllamaProviderQualificationStatus::NotQualified => false,
        }
    }

    /// Returns the typed provider-only qualification status.
    #[must_use]
    pub const fn qualification_status(&self) -> OllamaProviderQualificationStatus {
        self.qualification
    }
}

/// Failure to establish a bounded, reviewed provider cloud-disable declaration.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum OllamaCloudDisableEvidenceError {
    /// A version was not exact stable `major.minor.patch` syntax.
    #[error("invalid exact Ollama version")]
    InvalidVersion,
    /// The runtime version changed between bound observations.
    #[error("Ollama version drifted during observation")]
    VersionDrift,
    /// The runtime predates the documented cloud-disable feature.
    #[error("Ollama cloud-disable feature is unavailable")]
    FeatureUnavailable,
    /// The exact runtime version and package are not in the reviewed allowlist.
    #[error("Ollama runtime package is not reviewed for cloud disablement")]
    UnreviewedRuntimePackage,
    /// The managed environment omitted the declaration.
    #[error("managed cloud-disable environment declaration is missing")]
    MissingEnvironmentDeclaration,
    /// The managed environment repeated the declaration.
    #[error("managed cloud-disable environment declaration is duplicated")]
    DuplicateEnvironmentDeclaration,
    /// The managed environment used a value other than the exact required value.
    #[error("managed cloud-disable environment declaration conflicts with policy")]
    ConflictingEnvironmentDeclaration,
    /// The managed environment declaration exceeded its bounds.
    #[error("managed cloud-disable environment declaration is oversized")]
    OversizedEnvironmentDeclaration,
    /// Startup output did not contain the exact cloud-disabled marker.
    #[error("cloud-disabled startup marker is missing")]
    MissingStartupMarker,
    /// Startup output contained the cloud-disabled marker more than once.
    #[error("cloud-disabled startup marker is duplicated")]
    DuplicateStartupMarker,
    /// Startup output contained a cloud-enabled marker.
    #[error("cloud-disabled startup marker conflicts with startup output")]
    ConflictingStartupMarker,
    /// Startup output exceeded its byte bound.
    #[error("startup output is oversized")]
    OversizedStartupLog,
    /// Startup output was not valid UTF-8.
    #[error("startup output encoding is invalid")]
    InvalidStartupLogEncoding,
}

fn parse_component(component: Option<&str>) -> Result<u32, OllamaVersionParseError> {
    let component = component.ok_or(OllamaVersionParseError)?;
    if component.is_empty()
        || component.len() > 1 && component.starts_with('0')
        || !component.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(OllamaVersionParseError);
    }
    component.parse().map_err(|_| OllamaVersionParseError)
}

fn assess_runtime(
    version: OllamaVersion,
    runtime_package_manifest_id: &RuntimePackageManifestId,
    reviewed_runtimes: &[ReviewedCloudDisableRuntime],
) -> OllamaCloudDisableVersionStatus {
    if version < OLLAMA_CLOUD_DISABLE_FEATURE_FLOOR {
        OllamaCloudDisableVersionStatus::FeatureUnavailable
    } else if reviewed_runtimes.iter().any(|reviewed| {
        reviewed.version == version
            && &reviewed.runtime_package_manifest_id == runtime_package_manifest_id
    }) {
        OllamaCloudDisableVersionStatus::Reviewed
    } else {
        OllamaCloudDisableVersionStatus::Unreviewed
    }
}

fn line_has_marker(line: &str, bare_marker: &str, structured_field: &str) -> bool {
    let line = line.trim();
    if line == bare_marker {
        return true;
    }
    line.strip_suffix(structured_field)
        .is_some_and(|prefix| prefix.ends_with(char::is_whitespace))
}

fn observe_with_policy(
    version_before: &str,
    version_after: &str,
    managed_environment_values: &[&str],
    startup_log: &str,
    runtime_package_manifest_id: &RuntimePackageManifestId,
    reviewed_runtimes: &[ReviewedCloudDisableRuntime],
) -> Result<OllamaCloudDisableEvidence, OllamaCloudDisableEvidenceError> {
    let version_before = validate_version_policy(
        version_before,
        version_after,
        runtime_package_manifest_id,
        reviewed_runtimes,
    )?;

    OllamaManagedCloudDisableEnvironment::parse(managed_environment_values)?;
    OllamaCloudDisableStartupMarker::parse(startup_log)?;

    Ok(observed_evidence(
        version_before,
        runtime_package_manifest_id,
    ))
}

fn validate_version_policy(
    version_before: &str,
    version_after: &str,
    runtime_package_manifest_id: &RuntimePackageManifestId,
    reviewed_runtimes: &[ReviewedCloudDisableRuntime],
) -> Result<OllamaVersion, OllamaCloudDisableEvidenceError> {
    let version_before = version_before
        .parse::<OllamaVersion>()
        .map_err(|_| OllamaCloudDisableEvidenceError::InvalidVersion)?;
    let version_after = version_after
        .parse::<OllamaVersion>()
        .map_err(|_| OllamaCloudDisableEvidenceError::InvalidVersion)?;
    if version_before != version_after {
        return Err(OllamaCloudDisableEvidenceError::VersionDrift);
    }

    match assess_runtime(
        version_before,
        runtime_package_manifest_id,
        reviewed_runtimes,
    ) {
        OllamaCloudDisableVersionStatus::FeatureUnavailable => {
            return Err(OllamaCloudDisableEvidenceError::FeatureUnavailable);
        }
        OllamaCloudDisableVersionStatus::Unreviewed => {
            return Err(OllamaCloudDisableEvidenceError::UnreviewedRuntimePackage);
        }
        OllamaCloudDisableVersionStatus::Reviewed => {}
    }

    Ok(version_before)
}

fn observed_evidence(
    version: OllamaVersion,
    runtime_package_manifest_id: &RuntimePackageManifestId,
) -> OllamaCloudDisableEvidence {
    OllamaCloudDisableEvidence {
        schema_version: 1,
        runtime_version: version,
        runtime_package_manifest_id: runtime_package_manifest_id.clone(),
        feature_floor: OLLAMA_CLOUD_DISABLE_FEATURE_FLOOR,
        version_reviewed: true,
        declaration_source: OllamaCloudDisableDeclarationSource::ManagedEnvironment,
        marker_source: OllamaCloudDisableMarkerSource::ManagedStartupOutput,
        provider_declaration: OllamaProviderDeclarationStatus::Observed,
        network_isolation: OllamaNetworkIsolationStatus::NotEnforced,
        qualification: OllamaProviderQualificationStatus::NotQualified,
    }
}

#[cfg(test)]
#[path = "cloud_disable/tests.rs"]
mod tests;
