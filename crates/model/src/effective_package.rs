use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use crate::runtime_identity::valid_machine_id;
use crate::{
    ArtifactSetId, ArtifactSetManifest, ArtifactSetRelativePath, EffectiveRuntimeState,
    EffectiveRuntimeStateId, MAX_ARTIFACT_SET_MEMBERS, RuntimeBuildId, RuntimeBuildIdentity,
};

mod codec;

/// Current effective-package evidence contract version.
pub const EFFECTIVE_PACKAGE_EVIDENCE_SCHEMA_VERSION: u32 = 1;
/// Maximum JSON bytes admitted by the effective-package decoder.
pub const MAX_EFFECTIVE_PACKAGE_EVIDENCE_JSON_BYTES: usize = 1_048_576;
/// Maximum canonical identity bytes for one effective-package record.
pub const MAX_EFFECTIVE_PACKAGE_CANONICAL_BYTES: usize = 524_288;
/// Maximum distinct purposes assigned to one artifact-set member.
pub const MAX_EFFECTIVE_PACKAGE_MEMBER_PURPOSES: usize = 8;
/// Maximum purpose assignments across one effective-package record.
pub const MAX_EFFECTIVE_PACKAGE_PURPOSE_ASSIGNMENTS: usize = 8_192;

/// Content-derived identifier for one effective-package evidence record.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectivePackageEvidenceId(Digest);

impl EffectivePackageEvidenceId {
    /// Returns the digest that defines this evidence record.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Evidence class used to bind an artifact set to an attested runtime tuple.
///
/// Observed-only packages are deliberately absent. A structurally valid value
/// does not establish that the referenced evidence is true.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectivePackageEvidenceMode {
    /// Retonr owns an immutable package and its managed runtime process.
    ManagedImmutablePackage,
    /// A reviewed local attestor bound an attached package to its runtime.
    AttachedAttestedPackage,
}

/// Closed purpose vocabulary for one output-affecting package member.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectivePackageMemberPurpose {
    /// Primary model weights or a model-weight shard.
    ModelWeights,
    /// Index that resolves a set of model-weight shards.
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
    /// Grammar or structured-output schema.
    GrammarOrSchema,
    /// Model adapter such as a `LoRA`.
    Adapter,
    /// Modality projector.
    Projector,
    /// Draft model used by speculative decoding.
    DraftModel,
    /// Custom model-loading or inference code.
    CustomModelCode,
    /// Custom generation, parsing, or rendering code.
    CustomGenerationCode,
    /// Other declared output-affecting data admitted by this schema.
    AuxiliaryData,
}

/// Exact purpose coverage for one artifact-set member path.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectivePackageMemberEvidence {
    relative_path: ArtifactSetRelativePath,
    purposes: Vec<EffectivePackageMemberPurpose>,
}

impl EffectivePackageMemberEvidence {
    /// Creates canonical purpose evidence for one member.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivePackageEvidenceError::InvalidMemberPurposes`] unless
    /// purposes are nonempty, bounded, strictly ordered, and unique.
    pub fn new(
        relative_path: ArtifactSetRelativePath,
        purposes: Vec<EffectivePackageMemberPurpose>,
    ) -> Result<Self, EffectivePackageEvidenceError> {
        let member = Self {
            relative_path,
            purposes,
        };
        member.validate()?;
        Ok(member)
    }

    /// Returns the exact artifact-set member path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns purposes in canonical tag order.
    #[must_use]
    pub fn purposes(&self) -> &[EffectivePackageMemberPurpose] {
        &self.purposes
    }

    fn validate(&self) -> Result<(), EffectivePackageEvidenceError> {
        if self.purposes.is_empty()
            || self.purposes.len() > MAX_EFFECTIVE_PACKAGE_MEMBER_PURPOSES
            || self
                .purposes
                .windows(2)
                .any(|pair| codec::purpose_byte(pair[0]) >= codec::purpose_byte(pair[1]))
        {
            return Err(EffectivePackageEvidenceError::InvalidMemberPurposes);
        }
        Ok(())
    }
}

/// Evidence describing whether package bytes were transformed after acquisition.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PackageTransformationDisposition {
    /// Package bytes match the reviewed acquired source without transformation.
    Untransformed {
        /// Digest of the retained comparison and disposition evidence.
        evidence_digest: Digest,
    },
    /// Package bytes were produced by a declared transformation.
    Transformed {
        /// Exact source artifact set used by the transformation.
        source_artifact_set_id: ArtifactSetId,
        /// Digest of the retained transformation process or tool evidence.
        process_evidence_digest: Digest,
        /// Digest of exact normalized transformation parameters.
        parameters_digest: Digest,
        /// Digest of bounded retained transformation logs.
        log_digest: Digest,
    },
}

/// Caller-supplied evidence needed to bind a package to exact runtime records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectivePackageEvidenceInput {
    /// Whether the package is managed or locally attested.
    pub evidence_mode: EffectivePackageEvidenceMode,
    /// Stable identifier for the evidence-producing contract.
    pub evidence_contract_id: String,
    /// Version of the evidence-producing contract.
    pub evidence_contract_schema_version: u32,
    /// Exact purpose coverage for every artifact-set member.
    pub member_evidence: Vec<EffectivePackageMemberEvidence>,
    /// Digest of retained evidence that the artifact set is complete.
    pub artifact_set_completeness_evidence_digest: Digest,
    /// Digest of retained immutable acquisition and origin evidence.
    pub acquisition_evidence_digest: Digest,
    /// Digest of the retained license-review decision and inputs.
    pub license_review_evidence_digest: Digest,
    /// Exact transformation disposition and its retained evidence bindings.
    pub transformation: PackageTransformationDisposition,
    /// Digest of retained runtime resolution and load-closure evidence.
    pub runtime_load_closure_evidence_digest: Digest,
    /// Digest of retained exclusion, isolation, and remote-resolution evidence.
    pub exclusion_isolation_evidence_digest: Digest,
}

/// Validated, content-addressed evidence joining package and runtime identities.
///
/// This record is inert. Structural validity and digest equality do not prove the
/// truth or completeness of referenced evidence and grant no runtime authority.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectivePackageEvidence {
    schema_version: u32,
    evidence_mode: EffectivePackageEvidenceMode,
    artifact_set_id: ArtifactSetId,
    runtime_build_id: RuntimeBuildId,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    evidence_contract_id: String,
    evidence_contract_schema_version: u32,
    member_evidence: Vec<EffectivePackageMemberEvidence>,
    artifact_set_completeness_evidence_digest: Digest,
    acquisition_evidence_digest: Digest,
    license_review_evidence_digest: Digest,
    transformation: PackageTransformationDisposition,
    runtime_load_closure_evidence_digest: Digest,
    exclusion_isolation_evidence_digest: Digest,
}

impl EffectivePackageEvidence {
    /// Creates a version 1 record from exact referenced objects and evidence.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivePackageEvidenceError`] when metadata, member coverage,
    /// runtime relationships, evidence mode, or canonical bounds are invalid.
    pub fn new(
        artifact_set: &ArtifactSetManifest,
        runtime_build: &RuntimeBuildIdentity,
        runtime_state: &EffectiveRuntimeState,
        input: EffectivePackageEvidenceInput,
    ) -> Result<Self, EffectivePackageEvidenceError> {
        let evidence = Self::from_wire(
            EFFECTIVE_PACKAGE_EVIDENCE_SCHEMA_VERSION,
            artifact_set.artifact_set_id(),
            runtime_build.runtime_build_id(),
            runtime_state.effective_runtime_state_id(),
            input,
        )?;
        evidence.validate_against(artifact_set, runtime_build, runtime_state)?;
        Ok(evidence)
    }

    fn from_wire(
        schema_version: u32,
        artifact_set_id: ArtifactSetId,
        runtime_build_id: RuntimeBuildId,
        effective_runtime_state_id: EffectiveRuntimeStateId,
        input: EffectivePackageEvidenceInput,
    ) -> Result<Self, EffectivePackageEvidenceError> {
        if schema_version != EFFECTIVE_PACKAGE_EVIDENCE_SCHEMA_VERSION {
            return Err(EffectivePackageEvidenceError::UnsupportedSchema(
                schema_version,
            ));
        }
        if !valid_machine_id(&input.evidence_contract_id)
            || input.evidence_contract_schema_version == 0
        {
            return Err(EffectivePackageEvidenceError::InvalidMetadata);
        }
        validate_member_evidence(&input.member_evidence)?;
        let evidence = Self {
            schema_version,
            evidence_mode: input.evidence_mode,
            artifact_set_id,
            runtime_build_id,
            effective_runtime_state_id,
            evidence_contract_id: input.evidence_contract_id,
            evidence_contract_schema_version: input.evidence_contract_schema_version,
            member_evidence: input.member_evidence,
            artifact_set_completeness_evidence_digest: input
                .artifact_set_completeness_evidence_digest,
            acquisition_evidence_digest: input.acquisition_evidence_digest,
            license_review_evidence_digest: input.license_review_evidence_digest,
            transformation: input.transformation,
            runtime_load_closure_evidence_digest: input.runtime_load_closure_evidence_digest,
            exclusion_isolation_evidence_digest: input.exclusion_isolation_evidence_digest,
        };
        if evidence.canonical_bytes().len() > MAX_EFFECTIVE_PACKAGE_CANONICAL_BYTES {
            return Err(EffectivePackageEvidenceError::CanonicalEncodingTooLarge);
        }
        Ok(evidence)
    }

    /// Rechecks all content identities and cross-record relationships.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivePackageEvidenceError`] when any supplied reference,
    /// member path, build-state relationship, or evidence mode differs.
    pub fn validate_against(
        &self,
        artifact_set: &ArtifactSetManifest,
        runtime_build: &RuntimeBuildIdentity,
        runtime_state: &EffectiveRuntimeState,
    ) -> Result<(), EffectivePackageEvidenceError> {
        if self.artifact_set_id != artifact_set.artifact_set_id() {
            return Err(EffectivePackageEvidenceError::ArtifactSetMismatch);
        }
        if self.runtime_build_id != runtime_build.runtime_build_id() {
            return Err(EffectivePackageEvidenceError::RuntimeBuildMismatch);
        }
        if self.effective_runtime_state_id != runtime_state.effective_runtime_state_id() {
            return Err(EffectivePackageEvidenceError::RuntimeStateMismatch);
        }
        if runtime_state.runtime_build_id() != &self.runtime_build_id {
            return Err(EffectivePackageEvidenceError::RuntimeStateBuildMismatch);
        }
        if !codec::mode_matches_build(self.evidence_mode, runtime_build.mode()) {
            return Err(EffectivePackageEvidenceError::EvidenceModeMismatch);
        }
        if self.member_evidence.len() != artifact_set.members().len()
            || self
                .member_evidence
                .iter()
                .zip(artifact_set.members())
                .any(|(evidence, member)| evidence.relative_path() != member.relative_path())
        {
            return Err(EffectivePackageEvidenceError::MemberCoverageMismatch);
        }
        Ok(())
    }

    /// Returns the content-derived identity of this complete record.
    #[must_use]
    pub fn effective_package_evidence_id(&self) -> EffectivePackageEvidenceId {
        EffectivePackageEvidenceId(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the evidence contract version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the managed or attached-attested evidence mode.
    #[must_use]
    pub const fn evidence_mode(&self) -> EffectivePackageEvidenceMode {
        self.evidence_mode
    }

    /// Returns the exact artifact-set identity.
    #[must_use]
    pub const fn artifact_set_id(&self) -> &ArtifactSetId {
        &self.artifact_set_id
    }

    /// Returns the exact runtime-build identity.
    #[must_use]
    pub const fn runtime_build_id(&self) -> &RuntimeBuildId {
        &self.runtime_build_id
    }

    /// Returns the exact effective-runtime-state identity.
    #[must_use]
    pub const fn effective_runtime_state_id(&self) -> &EffectiveRuntimeStateId {
        &self.effective_runtime_state_id
    }

    /// Returns member purpose evidence in canonical artifact-set path order.
    #[must_use]
    pub fn member_evidence(&self) -> &[EffectivePackageMemberEvidence] {
        &self.member_evidence
    }
}

fn validate_member_evidence(
    members: &[EffectivePackageMemberEvidence],
) -> Result<(), EffectivePackageEvidenceError> {
    if members.is_empty() {
        return Err(EffectivePackageEvidenceError::EmptyMemberCoverage);
    }
    if members.len() > MAX_ARTIFACT_SET_MEMBERS {
        return Err(EffectivePackageEvidenceError::TooManyMembers);
    }
    let mut assignments = 0usize;
    let mut prior_path: Option<&str> = None;
    for member in members {
        member.validate()?;
        let path = member.relative_path.as_str();
        if prior_path.is_some_and(|prior| prior.as_bytes() >= path.as_bytes()) {
            return Err(EffectivePackageEvidenceError::NoncanonicalMemberOrder);
        }
        prior_path = Some(path);
        assignments = assignments
            .checked_add(member.purposes.len())
            .ok_or(EffectivePackageEvidenceError::TooManyPurposeAssignments)?;
        if assignments > MAX_EFFECTIVE_PACKAGE_PURPOSE_ASSIGNMENTS {
            return Err(EffectivePackageEvidenceError::TooManyPurposeAssignments);
        }
    }
    Ok(())
}

/// Effective-package evidence validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum EffectivePackageEvidenceError {
    /// Encoded input exceeds the fixed pre-decode byte ceiling.
    #[error("encoded effective-package evidence exceeds its limit")]
    EncodedEvidenceTooLarge,
    /// Encoded JSON is malformed or contains an unknown field.
    #[error("effective-package evidence encoding is invalid")]
    InvalidEncoding,
    /// A decoded member path violates the portable path contract.
    #[error("effective-package member path is invalid")]
    InvalidMemberPath,
    /// The evidence schema version is unsupported.
    #[error("unsupported effective-package evidence schema {0}")]
    UnsupportedSchema(u32),
    /// Evidence contract identity or version metadata is invalid.
    #[error("effective-package evidence metadata is invalid")]
    InvalidMetadata,
    /// Purpose coverage must include at least one member.
    #[error("effective-package member coverage is empty")]
    EmptyMemberCoverage,
    /// Member coverage exceeds the artifact-set member ceiling.
    #[error("effective-package member limit exceeded")]
    TooManyMembers,
    /// Member entries are not in strict canonical path order.
    #[error("effective-package members are not in canonical order")]
    NoncanonicalMemberOrder,
    /// A member purpose set is empty, unbounded, duplicated, or unordered.
    #[error("effective-package member purposes are invalid")]
    InvalidMemberPurposes,
    /// Aggregate purpose assignments exceed the fixed ceiling.
    #[error("effective-package purpose assignment limit exceeded")]
    TooManyPurposeAssignments,
    /// Canonical identity bytes exceed the fixed contract ceiling.
    #[error("effective-package canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
    /// The referenced artifact set differs from the record.
    #[error("effective-package artifact-set identity does not match")]
    ArtifactSetMismatch,
    /// The referenced runtime build differs from the record.
    #[error("effective-package runtime-build identity does not match")]
    RuntimeBuildMismatch,
    /// The referenced effective runtime state differs from the record.
    #[error("effective-package runtime-state identity does not match")]
    RuntimeStateMismatch,
    /// The effective runtime state is not bound to the referenced build.
    #[error("effective-package runtime state and build do not match")]
    RuntimeStateBuildMismatch,
    /// The package evidence mode is incompatible with the runtime-build mode.
    #[error("effective-package evidence mode does not match runtime-build mode")]
    EvidenceModeMismatch,
    /// Member purpose evidence does not cover the artifact set exactly.
    #[error("effective-package member coverage does not match the artifact set")]
    MemberCoverageMismatch,
}

#[cfg(test)]
#[path = "effective_package/tests.rs"]
mod tests;
