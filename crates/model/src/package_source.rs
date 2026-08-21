use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use crate::ArtifactSetId;
use crate::runtime_identity::{append_digest, append_text, append_u32};

/// Current package-source contract version.
pub const PACKAGE_SOURCE_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded JSON bytes accepted for one package source.
pub const MAX_PACKAGE_SOURCE_JSON_BYTES: usize = 4_096;
const MAX_SOURCE_LOCATOR_BYTES: usize = 512;
const MAX_SOURCE_REVISION_BYTES: usize = 128;
const MAX_PACKAGE_SOURCE_CANONICAL_BYTES: usize = 2_048;

/// Stable source class for one acquired package.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageSourceKind {
    /// An immutable upstream release asset.
    UpstreamRelease,
    /// An exact source repository revision.
    RepositoryRevision,
    /// A reviewed local archive with no stronger upstream identity.
    LocalArchive,
}

/// Content-derived identifier for one canonical package source.
#[derive(Clone, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct PackageSourceId(Digest);

impl PackageSourceId {
    /// Returns the digest defining this source identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Exact reviewed origin for a runtime or model package.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackageSource {
    schema_version: u32,
    kind: PackageSourceKind,
    locator: String,
    revision: String,
    provenance_digest: Digest,
}

impl PackageSource {
    /// Creates a validated version 1 package source.
    ///
    /// # Errors
    ///
    /// Returns [`PackageSourceError`] when text is ambiguous, unbounded, or unsafe.
    pub fn new(
        kind: PackageSourceKind,
        locator: impl Into<String>,
        revision: impl Into<String>,
        provenance_digest: Digest,
    ) -> Result<Self, PackageSourceError> {
        Self::from_wire(
            PACKAGE_SOURCE_SCHEMA_VERSION,
            kind,
            locator.into(),
            revision.into(),
            provenance_digest,
        )
    }

    /// Parses bounded JSON and revalidates the source identity.
    ///
    /// # Errors
    ///
    /// Returns [`PackageSourceError`] for oversized, malformed, or invalid input.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, PackageSourceError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            schema_version: u32,
            kind: PackageSourceKind,
            locator: String,
            revision: String,
            provenance_digest: Digest,
        }

        if bytes.len() > MAX_PACKAGE_SOURCE_JSON_BYTES {
            return Err(PackageSourceError::EncodedSourceTooLarge);
        }
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|_| PackageSourceError::InvalidEncoding)?;
        Self::from_wire(
            wire.schema_version,
            wire.kind,
            wire.locator,
            wire.revision,
            wire.provenance_digest,
        )
    }

    fn from_wire(
        schema_version: u32,
        kind: PackageSourceKind,
        locator: String,
        revision: String,
        provenance_digest: Digest,
    ) -> Result<Self, PackageSourceError> {
        if schema_version != PACKAGE_SOURCE_SCHEMA_VERSION {
            return Err(PackageSourceError::UnsupportedSchema(schema_version));
        }
        if !valid_locator(&locator) || !valid_revision(&revision) {
            return Err(PackageSourceError::InvalidMetadata);
        }
        let value = Self {
            schema_version,
            kind,
            locator,
            revision,
            provenance_digest,
        };
        if value.canonical_bytes().len() > MAX_PACKAGE_SOURCE_CANONICAL_BYTES {
            return Err(PackageSourceError::CanonicalEncodingTooLarge);
        }
        Ok(value)
    }

    /// Returns the content-derived source identity.
    #[must_use]
    pub fn package_source_id(&self) -> PackageSourceId {
        PackageSourceId(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the source contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the stable source class.
    #[must_use]
    pub const fn kind(&self) -> PackageSourceKind {
        self.kind
    }

    /// Returns the credential-free source locator.
    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }

    /// Returns the exact source revision.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Returns the retained provenance evidence digest.
    #[must_use]
    pub const fn provenance_digest(&self) -> &Digest {
        &self.provenance_digest
    }

    pub(super) fn append_identity(&self, output: &mut Vec<u8>) {
        append_digest(output, self.package_source_id().digest());
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(b"retonr:package-source:v1\0");
        append_u32(&mut output, self.schema_version);
        output.push(source_kind_byte(self.kind));
        append_text(&mut output, &self.locator);
        append_text(&mut output, &self.revision);
        append_digest(&mut output, &self.provenance_digest);
        output
    }
}

/// Reviewed transformation relationship for a package.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackageTransformation {
    /// Package bytes match the reviewed acquired source.
    Untransformed {
        /// Digest of retained comparison and disposition evidence.
        evidence_digest: Digest,
    },
    /// Package bytes were produced from an exact source artifact set.
    Transformed {
        /// Exact source artifact set.
        source_artifact_set_id: ArtifactSetId,
        /// Exact transformation tool or process evidence.
        tool_evidence_digest: Digest,
        /// Canonical transformation parameters.
        parameters_digest: Digest,
        /// Bounded retained transformation log.
        log_digest: Digest,
    },
}

impl PackageTransformation {
    pub(super) fn append_canonical(&self, output: &mut Vec<u8>) {
        match self {
            Self::Untransformed { evidence_digest } => {
                output.push(0);
                append_digest(output, evidence_digest);
            }
            Self::Transformed {
                source_artifact_set_id,
                tool_evidence_digest,
                parameters_digest,
                log_digest,
            } => {
                output.push(1);
                append_digest(output, source_artifact_set_id.digest());
                append_digest(output, tool_evidence_digest);
                append_digest(output, parameters_digest);
                append_digest(output, log_digest);
            }
        }
    }

    pub(super) const fn requires_transformation_record(&self) -> bool {
        matches!(self, Self::Transformed { .. })
    }
}

fn valid_locator(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SOURCE_LOCATOR_BYTES
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
        && !value.contains(['\\', '@', '?', '#'])
}

fn valid_revision(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SOURCE_REVISION_BYTES
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

pub(super) const fn source_kind_byte(value: PackageSourceKind) -> u8 {
    match value {
        PackageSourceKind::UpstreamRelease => 0,
        PackageSourceKind::RepositoryRevision => 1,
        PackageSourceKind::LocalArchive => 2,
    }
}

/// Package-source validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PackageSourceError {
    /// Encoded input exceeds its fixed ceiling.
    #[error("encoded package source exceeds its limit")]
    EncodedSourceTooLarge,
    /// JSON is malformed or contains unknown fields.
    #[error("package source encoding is invalid")]
    InvalidEncoding,
    /// The source schema version is unsupported.
    #[error("unsupported package source schema {0}")]
    UnsupportedSchema(u32),
    /// Locator or revision metadata is invalid.
    #[error("package source metadata is invalid")]
    InvalidMetadata,
    /// Canonical identity bytes exceed the fixed ceiling.
    #[error("package source canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
}

#[cfg(test)]
#[path = "package_source/tests.rs"]
mod tests;
