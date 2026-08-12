use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

/// Current artifact-manifest contract version.
pub const ARTIFACT_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Content-derived identifier for one immutable artifact.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactId(Digest);

impl ArtifactId {
    /// Creates an identifier from the artifact's complete byte digest.
    #[must_use]
    pub fn from_digest(digest: Digest) -> Self {
        Self(digest)
    }

    /// Returns the digest that defines this identifier.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Product role an artifact may be qualified to perform.
#[derive(
    Clone, Copy, Debug, Eq, Hash, JsonSchema, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRole {
    /// Candidate text generation.
    Generation,
    /// Semantic or style embedding.
    Embedding,
    /// Speech recognition.
    SpeechRecognition,
    /// Voice activity detection.
    VoiceActivityDetection,
    /// Speech synthesis model.
    SpeechSynthesis,
    /// Speech synthesis voice data.
    Voice,
}

/// Immutable upstream origin recorded for an artifact.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSource {
    /// Repository, registry, or approved origin identifier.
    pub origin: String,
    /// Immutable upstream revision, digest, or release identifier.
    pub revision: String,
}

/// Tokenizer identity associated with a text model.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenizerIdentity {
    /// Tokenizer family or format identifier.
    pub family: String,
    /// Digest of the tokenizer files or canonical configuration.
    pub digest: Digest,
}

/// License evidence for one separately licensed component.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseRecord {
    /// Component covered by this record.
    pub component: String,
    /// SPDX expression or reviewed upstream identifier.
    pub identifier: String,
    /// Digest of the exact reviewed license text.
    pub text_digest: Digest,
}

/// Untrusted capabilities declared by an upstream artifact source.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredCapabilities {
    /// Roles claimed by the upstream source.
    pub roles: Vec<ArtifactRole>,
    /// Languages claimed by the upstream source.
    pub languages: Vec<String>,
    /// Context size claimed by the upstream source.
    pub context_tokens: Option<u32>,
}

/// Immutable facts and untrusted upstream metadata for one artifact.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    /// Manifest contract version.
    pub schema_version: u32,
    /// Content-derived artifact identifier.
    pub artifact_id: ArtifactId,
    /// Approved immutable upstream source.
    pub source: ArtifactSource,
    /// Digest of the complete artifact bytes.
    pub artifact_digest: Digest,
    /// Complete artifact size.
    pub byte_size: u64,
    /// Intrinsic artifact format.
    pub format: String,
    /// Intrinsic model or runtime family.
    pub family: String,
    /// Artifact architecture when applicable.
    pub architecture: Option<String>,
    /// Quantization identifier when applicable.
    pub quantization: Option<String>,
    /// Tokenizer identity when applicable.
    pub tokenizer: Option<TokenizerIdentity>,
    /// Separately reviewed component licenses.
    pub licenses: Vec<LicenseRecord>,
    /// Upstream capability claims. These do not grant activation.
    pub declared_capabilities: DeclaredCapabilities,
}

impl ArtifactManifest {
    /// Validates intrinsic manifest invariants without treating declarations as
    /// qualification evidence.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError`] when required facts are missing, inconsistent, or
    /// outside their bounded contract.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != ARTIFACT_MANIFEST_SCHEMA_VERSION {
            return Err(ManifestError::UnsupportedSchema(self.schema_version));
        }
        if self.artifact_id.digest() != &self.artifact_digest {
            return Err(ManifestError::IdentityMismatch);
        }
        if self.byte_size == 0 {
            return Err(ManifestError::EmptyArtifact);
        }
        for value in [
            self.source.origin.as_str(),
            self.source.revision.as_str(),
            self.format.as_str(),
            self.family.as_str(),
        ] {
            if !valid_bounded_text(value, 256) {
                return Err(ManifestError::InvalidMetadata);
            }
        }
        if self.licenses.is_empty() || self.licenses.len() > 32 {
            return Err(ManifestError::InvalidLicenses);
        }
        let mut components = std::collections::BTreeSet::new();
        for license in &self.licenses {
            if !valid_bounded_text(&license.component, 128)
                || !valid_bounded_text(&license.identifier, 256)
                || !components.insert(license.component.as_str())
            {
                return Err(ManifestError::InvalidLicenses);
            }
        }
        validate_declared_capabilities(&self.declared_capabilities)
    }
}

fn validate_declared_capabilities(value: &DeclaredCapabilities) -> Result<(), ManifestError> {
    if value.roles.is_empty() || value.roles.len() > 16 || value.languages.len() > 128 {
        return Err(ManifestError::InvalidCapabilities);
    }
    let unique_roles: std::collections::BTreeSet<_> = value.roles.iter().copied().collect();
    if unique_roles.len() != value.roles.len()
        || value
            .languages
            .iter()
            .any(|language| !valid_bounded_text(language, 64))
    {
        return Err(ManifestError::InvalidCapabilities);
    }
    Ok(())
}

fn valid_bounded_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

/// Artifact-manifest validation failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ManifestError {
    /// The manifest schema is not supported.
    #[error("unsupported artifact manifest schema {0}")]
    UnsupportedSchema(u32),
    /// The content-derived ID does not match the artifact digest.
    #[error("artifact identifier does not match artifact digest")]
    IdentityMismatch,
    /// An empty artifact cannot be activated.
    #[error("artifact byte size must be greater than zero")]
    EmptyArtifact,
    /// Required source or intrinsic metadata is invalid.
    #[error("artifact metadata is empty, oversized, or contains controls")]
    InvalidMetadata,
    /// License evidence is missing, duplicated, or invalid.
    #[error("artifact license records are invalid")]
    InvalidLicenses,
    /// Declared capability metadata is invalid.
    #[error("declared artifact capabilities are invalid")]
    InvalidCapabilities,
}

#[cfg(test)]
mod tests {
    use rewrite_types::Digest;

    use super::{
        ARTIFACT_MANIFEST_SCHEMA_VERSION, ArtifactId, ArtifactManifest, ArtifactRole,
        ArtifactSource, DeclaredCapabilities, LicenseRecord, ManifestError,
    };

    fn manifest() -> ArtifactManifest {
        let digest = Digest::sha256(b"artifact");
        ArtifactManifest {
            schema_version: ARTIFACT_MANIFEST_SCHEMA_VERSION,
            artifact_id: ArtifactId::from_digest(digest.clone()),
            source: ArtifactSource {
                origin: "approved-registry/model".to_owned(),
                revision: "sha256:revision".to_owned(),
            },
            artifact_digest: digest,
            byte_size: 8,
            format: "gguf".to_owned(),
            family: "fixture".to_owned(),
            architecture: Some("transformer".to_owned()),
            quantization: Some("q4".to_owned()),
            tokenizer: None,
            licenses: vec![LicenseRecord {
                component: "weights".to_owned(),
                identifier: "Apache-2.0".to_owned(),
                text_digest: Digest::sha256(b"license"),
            }],
            declared_capabilities: DeclaredCapabilities {
                roles: vec![ArtifactRole::Generation],
                languages: vec!["en".to_owned()],
                context_tokens: Some(8_192),
            },
        }
    }

    #[test]
    fn validates_complete_manifest_and_round_trip() {
        let manifest = manifest();
        manifest.validate().expect("manifest is valid");
        let encoded = serde_json::to_string(&manifest).expect("serialize manifest");
        let decoded: ArtifactManifest = serde_json::from_str(&encoded).expect("parse manifest");
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn rejects_identity_and_capability_conflicts() {
        let mut value = manifest();
        value.artifact_id = ArtifactId::from_digest(Digest::sha256(b"other"));
        assert_eq!(value.validate(), Err(ManifestError::IdentityMismatch));

        let mut value = manifest();
        value
            .declared_capabilities
            .roles
            .push(ArtifactRole::Generation);
        assert_eq!(value.validate(), Err(ManifestError::InvalidCapabilities));
    }
}
