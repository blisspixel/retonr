use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use rewrite_types::Digest;

use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetRelativePath, PackageSource, PackageTransformation,
};

mod codec;
mod validation;

/// Current model-package manifest contract version.
pub const MODEL_PACKAGE_MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Maximum encoded JSON bytes accepted for one model package.
pub const MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES: usize = 1_048_576;
/// Maximum embedded components in one model package.
pub const EMBEDDED_MODEL_COMPONENT_LIMIT: usize = 64;
const MAX_MODEL_PACKAGE_CANONICAL_BYTES: usize = 1_048_576;
const MAX_MODEL_MEMBER_ROLES: usize = 8;
const MAX_MODEL_ROLE_ASSIGNMENTS: usize = 8_192;
const MAX_EXTRACTION_CONTRACT_BYTES: usize = 64;
const MAX_EXTRACTION_SELECTOR_BYTES: usize = 256;
const MAX_FORMAT_CONTRACT_BYTES: usize = 64;

/// Content-derived identifier for one model-package manifest.
#[derive(Clone, Debug, serde::Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ModelPackageManifestId(Digest);

impl ModelPackageManifestId {
    /// Returns the digest defining this model package.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Closed purpose vocabulary for one model-package member.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPackageMemberRole {
    /// One complete, unsharded model-weight container.
    ModelWeights,
    /// One member of a sharded model-weight set.
    ModelWeightShard,
    /// Index resolving a complete set of model-weight shards.
    ModelShardIndex,
    /// Model architecture or load configuration.
    ModelConfiguration,
    /// Default generation configuration.
    GenerationConfiguration,
    /// Tokenizer model data.
    TokenizerModel,
    /// Tokenizer vocabulary data.
    TokenizerVocabulary,
    /// Tokenizer merge rules.
    TokenizerMerges,
    /// Tokenizer configuration.
    TokenizerConfiguration,
    /// Prompt or chat template.
    PromptTemplate,
    /// Fixed system-prompt material.
    SystemPrompt,
    /// Model adapter.
    Adapter,
    /// Modality projector.
    Projector,
    /// Draft model for speculative decoding.
    DraftModel,
    /// Grammar or structured-output schema.
    GrammarOrSchema,
    /// Custom model-loading code.
    CustomModelCode,
    /// Custom generation, parsing, or rendering code.
    CustomGenerationCode,
    /// Exact reviewed license text.
    LicenseText,
    /// Exact source or acquisition provenance.
    ProvenanceRecord,
    /// Exact transformation evidence.
    TransformationEvidence,
    /// Other explicitly retained package data.
    AuxiliaryData,
}

impl ModelPackageMemberRole {
    /// Returns whether this role is only review evidence and not a runtime-use claim.
    #[must_use]
    pub const fn is_evidence_only(self) -> bool {
        matches!(
            self,
            Self::LicenseText | Self::ProvenanceRecord | Self::TransformationEvidence
        )
    }
}

/// One exact byte member and its declared model-package meaning.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPackageMember {
    artifact_id: ArtifactId,
    byte_size: u64,
    relative_path: ArtifactSetRelativePath,
    roles: Vec<ModelPackageMemberRole>,
}

impl ModelPackageMember {
    /// Creates one model member. The enclosing manifest validates relationships.
    #[must_use]
    pub const fn new(
        artifact_id: ArtifactId,
        byte_size: u64,
        relative_path: ArtifactSetRelativePath,
        roles: Vec<ModelPackageMemberRole>,
    ) -> Self {
        Self {
            artifact_id,
            byte_size,
            relative_path,
            roles,
        }
    }

    /// Returns the complete member byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact member byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the portable package path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns roles in stable canonical order.
    #[must_use]
    pub fn roles(&self) -> &[ModelPackageMemberRole] {
        &self.roles
    }
}

/// Foundational model data extracted from an exact container member.
#[derive(Clone, Copy, Debug, serde::Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedModelComponentPurpose {
    /// Embedded model architecture or load configuration.
    ModelConfiguration,
    /// Embedded default generation configuration.
    GenerationConfiguration,
    /// Embedded tokenizer data and configuration.
    Tokenizer,
    /// Embedded prompt or chat template.
    PromptTemplate,
}

/// Exact canonical value extracted from one package container.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedModelComponent {
    container_path: ArtifactSetRelativePath,
    purpose: EmbeddedModelComponentPurpose,
    extraction_contract_id: String,
    extraction_contract_schema_version: u32,
    selector: String,
    value_digest: Digest,
}

impl EmbeddedModelComponent {
    /// Creates a bounded embedded-component descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`ModelPackageManifestError`] for invalid contract or selector text.
    pub fn new(
        container_path: ArtifactSetRelativePath,
        purpose: EmbeddedModelComponentPurpose,
        extraction_contract_id: impl Into<String>,
        extraction_contract_schema_version: u32,
        selector: impl Into<String>,
        value_digest: Digest,
    ) -> Result<Self, ModelPackageManifestError> {
        let value = Self {
            container_path,
            purpose,
            extraction_contract_id: extraction_contract_id.into(),
            extraction_contract_schema_version,
            selector: selector.into(),
            value_digest,
        };
        value.validate()?;
        Ok(value)
    }

    /// Returns the exact container path.
    #[must_use]
    pub const fn container_path(&self) -> &ArtifactSetRelativePath {
        &self.container_path
    }

    /// Returns the extracted component purpose.
    #[must_use]
    pub const fn purpose(&self) -> EmbeddedModelComponentPurpose {
        self.purpose
    }

    /// Returns the extraction contract identifier.
    #[must_use]
    pub fn extraction_contract_id(&self) -> &str {
        &self.extraction_contract_id
    }

    /// Returns the extraction contract version.
    #[must_use]
    pub const fn extraction_contract_schema_version(&self) -> u32 {
        self.extraction_contract_schema_version
    }

    /// Returns the exact bounded selector.
    #[must_use]
    pub fn selector(&self) -> &str {
        &self.selector
    }

    /// Returns the canonical extracted value digest.
    #[must_use]
    pub const fn value_digest(&self) -> &Digest {
        &self.value_digest
    }

    fn validate(&self) -> Result<(), ModelPackageManifestError> {
        if !valid_machine_id(&self.extraction_contract_id, MAX_EXTRACTION_CONTRACT_BYTES)
            || self.extraction_contract_schema_version == 0
            || !valid_selector(&self.selector)
        {
            return Err(ModelPackageManifestError::InvalidEmbeddedComponent);
        }
        Ok(())
    }
}

/// Canonical model-weight layout.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelWeightLayout {
    /// One complete unsharded weight member.
    Single {
        /// Exact weight member path.
        member: ArtifactSetRelativePath,
    },
    /// Multiple exact shards resolved by one exact index.
    Sharded {
        /// Exact shard paths in canonical byte order.
        shards: Vec<ArtifactSetRelativePath>,
        /// Exact shard-index member path.
        index: ArtifactSetRelativePath,
    },
}

/// Canonical static contents and meaning of one model package.
///
/// Output-affecting roles are candidates only. This record does not prove runtime use.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPackageManifest {
    schema_version: u32,
    artifact_set_id: crate::ArtifactSetId,
    format_contract_id: String,
    format_contract_schema_version: u32,
    source: PackageSource,
    transformation: PackageTransformation,
    members: Vec<ModelPackageMember>,
    weight_layout: ModelWeightLayout,
    embedded_components: Vec<EmbeddedModelComponent>,
}

impl ModelPackageManifest {
    /// Creates and relationship-checks a version 1 model package.
    ///
    /// # Errors
    ///
    /// Returns [`ModelPackageManifestError`] for incomplete or noncanonical input.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor makes every package input explicit"
    )]
    pub fn new(
        artifact_set: &ArtifactSetManifest,
        format_contract_id: impl Into<String>,
        format_contract_schema_version: u32,
        source: PackageSource,
        transformation: PackageTransformation,
        members: Vec<ModelPackageMember>,
        weight_layout: ModelWeightLayout,
        embedded_components: Vec<EmbeddedModelComponent>,
    ) -> Result<Self, ModelPackageManifestError> {
        Self::from_wire(
            MODEL_PACKAGE_MANIFEST_SCHEMA_VERSION,
            artifact_set,
            format_contract_id.into(),
            format_contract_schema_version,
            source,
            transformation,
            members,
            weight_layout,
            embedded_components,
        )
    }

    /// Returns the content-derived model package identity.
    #[must_use]
    pub fn model_package_manifest_id(&self) -> ModelPackageManifestId {
        ModelPackageManifestId(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the model-package contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the referenced byte-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &crate::ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact package-format parser contract.
    #[must_use]
    pub fn format_contract_id(&self) -> &str {
        &self.format_contract_id
    }

    /// Returns the package-format contract version.
    #[must_use]
    pub const fn format_contract_schema_version(&self) -> u32 {
        self.format_contract_schema_version
    }

    /// Returns the exact package source.
    #[must_use]
    pub const fn source(&self) -> &PackageSource {
        &self.source
    }

    /// Returns the transformation disposition.
    #[must_use]
    pub const fn transformation(&self) -> &PackageTransformation {
        &self.transformation
    }

    /// Returns semantic members in byte-manifest path order.
    #[must_use]
    pub fn members(&self) -> &[ModelPackageMember] {
        &self.members
    }

    /// Returns the canonical model-weight layout.
    #[must_use]
    pub const fn weight_layout(&self) -> &ModelWeightLayout {
        &self.weight_layout
    }

    /// Returns embedded component descriptors in canonical purpose order.
    #[must_use]
    pub fn embedded_components(&self) -> &[EmbeddedModelComponent] {
        &self.embedded_components
    }
}

fn valid_machine_id(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !value.starts_with('-')
        && !value.ends_with('-')
}

fn valid_selector(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_EXTRACTION_SELECTOR_BYTES
        && value.is_ascii()
        && value.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
}

/// Model-package manifest validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ModelPackageManifestError {
    /// Encoded input exceeds its fixed ceiling.
    #[error("encoded model-package manifest exceeds its limit")]
    EncodedManifestTooLarge,
    /// JSON is malformed or contains unknown fields.
    #[error("model-package manifest encoding is invalid")]
    InvalidEncoding,
    /// The schema version is unsupported.
    #[error("unsupported model-package manifest schema {0}")]
    UnsupportedSchema(u32),
    /// Format contract metadata is invalid.
    #[error("model-package format contract is invalid")]
    InvalidFormatContract,
    /// The referenced artifact set differs.
    #[error("model-package artifact set does not match")]
    ArtifactSetMismatch,
    /// Semantic members do not exactly cover byte members.
    #[error("model-package member coverage does not match")]
    MemberCoverageMismatch,
    /// A member has empty, duplicated, unordered, or excessive roles.
    #[error("model-package member roles are invalid")]
    InvalidMemberRoles,
    /// Aggregate semantic role assignments exceed their ceiling.
    #[error("model-package role assignment limit exceeded")]
    TooManyRoleAssignments,
    /// The weight layout is incomplete or inconsistent.
    #[error("model-package weight layout is invalid")]
    InvalidWeightLayout,
    /// An embedded component is invalid, duplicated, or unordered.
    #[error("model-package embedded component is invalid")]
    InvalidEmbeddedComponent,
    /// Tokenizer, template, or model configuration evidence is incomplete.
    #[error("model-package foundational component evidence is incomplete")]
    MissingFoundationalComponent,
    /// License or provenance evidence is absent.
    #[error("model-package required evidence is missing")]
    MissingEvidence,
    /// A transformed package lacks a transformation-evidence member.
    #[error("model-package transformation evidence is missing")]
    MissingTransformationEvidence,
    /// A decoded path is invalid.
    #[error("model-package member path is invalid")]
    InvalidMemberPath,
    /// A nested package source is invalid.
    #[error("model-package source is invalid")]
    InvalidSource,
    /// Canonical identity bytes exceed their ceiling.
    #[error("model-package canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
}

#[cfg(test)]
#[path = "model_package/tests.rs"]
mod tests;
