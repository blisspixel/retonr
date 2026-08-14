use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ArtifactSetId, ArtifactSetManifest, artifact_set::reserved_windows_basename};

/// Current installed artifact-set contract version.
pub const INSTALLED_ARTIFACT_SET_SCHEMA_VERSION: u32 = 1;
/// Maximum JSON bytes admitted by the installed artifact-set decoder.
pub const MAX_INSTALLED_ARTIFACT_SET_JSON_BYTES: usize = 512;
const MAX_INSTALLED_ARTIFACT_SET_STORAGE_KEY_BYTES: usize = 128;

/// Structurally validated installation record joined to one exact artifact-set manifest.
///
/// The application-owned storage key names a set root, not a user-supplied path.
/// The referenced manifest supplies the complete ordered member paths, identities,
/// and sizes. This record grants no activation, qualification, runtime, lease, or
/// semantic authority, and does not prove that the root or member bytes exist.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledArtifactSet {
    artifact_set_id: ArtifactSetId,
    schema_version: u32,
    storage_key: String,
}

impl InstalledArtifactSet {
    /// Creates version 1 installed state joined to an exact manifest.
    ///
    /// # Errors
    ///
    /// Returns [`InstalledArtifactSetError`] when the application-owned set-root
    /// storage key is invalid.
    pub fn new(
        manifest: &ArtifactSetManifest,
        storage_key: impl Into<String>,
    ) -> Result<Self, InstalledArtifactSetError> {
        Self::from_wire(
            manifest.artifact_set_id(),
            INSTALLED_ARTIFACT_SET_SCHEMA_VERSION,
            storage_key.into(),
            manifest,
        )
    }

    /// Parses bounded JSON and rejoins the record to an exact manifest.
    ///
    /// # Errors
    ///
    /// Returns [`InstalledArtifactSetError`] before decoding when input exceeds
    /// the byte ceiling, or for any encoding, schema, key, or manifest mismatch.
    pub fn from_json_bytes(
        bytes: &[u8],
        manifest: &ArtifactSetManifest,
    ) -> Result<Self, InstalledArtifactSetError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            artifact_set_id: ArtifactSetId,
            schema_version: u32,
            storage_key: String,
        }

        if bytes.len() > MAX_INSTALLED_ARTIFACT_SET_JSON_BYTES {
            return Err(InstalledArtifactSetError::EncodedInstallationTooLarge);
        }
        let wire: Wire = serde_json::from_slice(bytes)
            .map_err(|_| InstalledArtifactSetError::InvalidEncoding)?;
        Self::from_wire(
            wire.artifact_set_id,
            wire.schema_version,
            wire.storage_key,
            manifest,
        )
    }

    fn from_wire(
        artifact_set_id: ArtifactSetId,
        schema_version: u32,
        storage_key: String,
        manifest: &ArtifactSetManifest,
    ) -> Result<Self, InstalledArtifactSetError> {
        let installed = Self {
            artifact_set_id,
            schema_version,
            storage_key,
        };
        installed.validate_against(manifest)?;
        Ok(installed)
    }

    /// Revalidates the complete record against its exact manifest.
    ///
    /// # Errors
    ///
    /// Returns [`InstalledArtifactSetError`] for an unsupported schema, invalid
    /// set-root key, or manifest identity mismatch.
    pub fn validate_against(
        &self,
        manifest: &ArtifactSetManifest,
    ) -> Result<(), InstalledArtifactSetError> {
        if self.schema_version != INSTALLED_ARTIFACT_SET_SCHEMA_VERSION {
            return Err(InstalledArtifactSetError::UnsupportedSchema(
                self.schema_version,
            ));
        }
        if !valid_set_storage_key(&self.storage_key) {
            return Err(InstalledArtifactSetError::InvalidStorageKey);
        }
        if self.artifact_set_id != manifest.artifact_set_id() {
            return Err(InstalledArtifactSetError::ArtifactSetMismatch);
        }
        Ok(())
    }

    /// Returns the installed-state contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact manifest identity joined by this record.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the opaque application-owned set-root storage key.
    #[must_use]
    pub fn storage_key(&self) -> &str {
        &self.storage_key
    }

    /// Returns deterministic compact JSON for durable storage.
    #[must_use]
    pub fn canonical_json(&self) -> String {
        format!(
            "{{\"artifact_set_id\":\"{}\",\"schema_version\":{},\"storage_key\":\"{}\"}}",
            self.artifact_set_id.digest().as_str(),
            self.schema_version,
            self.storage_key
        )
    }
}

fn valid_set_storage_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_INSTALLED_ARTIFACT_SET_STORAGE_KEY_BYTES
        && !matches!(value, "." | "..")
        && !value.ends_with('.')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
        && !reserved_windows_basename(value.split('.').next().unwrap_or(value))
}

/// Installed artifact-set validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum InstalledArtifactSetError {
    /// Encoded input exceeds the fixed pre-decode byte ceiling.
    #[error("encoded installed artifact set exceeds its limit")]
    EncodedInstallationTooLarge,
    /// Encoded JSON is malformed or contains an unknown field.
    #[error("installed artifact-set encoding is invalid")]
    InvalidEncoding,
    /// The installed-state schema is unsupported.
    #[error("unsupported installed artifact-set schema {0}")]
    UnsupportedSchema(u32),
    /// The referenced artifact-set identity differs from the supplied manifest.
    #[error("installed artifact set does not match the supplied manifest")]
    ArtifactSetMismatch,
    /// The opaque application-owned set-root storage key is invalid.
    #[error("installed artifact-set storage key is invalid")]
    InvalidStorageKey,
}

#[cfg(test)]
#[path = "installed_artifact_set/tests.rs"]
mod tests;
