use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use super::{
    EffectivePackageEvidenceMode, EffectivePackageMemberPurpose, PackageTransformationDisposition,
};
use crate::runtime_identity::valid_machine_id;
use crate::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetRelativePath, EffectiveRuntimeState,
    EffectiveRuntimeStateId, MAX_ARTIFACT_SET_MEMBERS, RuntimeBuildId, RuntimeBuildIdentity,
};

mod codec;

/// Schema version for the version 2 effective-package evidence contract.
pub const EFFECTIVE_PACKAGE_EVIDENCE_V2_SCHEMA_VERSION: u32 = 2;
/// Maximum JSON bytes admitted by the version 2 evidence decoder.
pub const MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES: usize = 2_097_152;
/// Maximum canonical identity bytes for one version 2 evidence record.
pub const MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_CANONICAL_BYTES: usize = 524_288;
/// Maximum effective or declared purposes assigned to one version 2 member.
pub const MAX_EFFECTIVE_PACKAGE_MEMBER_V2_PURPOSES: usize = 8;
/// Maximum evidence-only roles assigned to one version 2 member.
pub const MAX_EFFECTIVE_PACKAGE_MEMBER_V2_ROLES: usize = 3;
/// Maximum typed member assignments across one version 2 record.
pub const MAX_EFFECTIVE_PACKAGE_MEMBER_V2_ASSIGNMENTS: usize = 8_192;

/// Content-derived identifier for one version 2 effective-package record.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectivePackageEvidenceV2Id(Digest);

impl EffectivePackageEvidenceV2Id {
    /// Returns the digest that defines this version 2 evidence record.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Closed review-evidence role vocabulary for package members.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectivePackageEvidenceRoleV2 {
    /// Human-readable license text retained with the package.
    LicenseText,
    /// Acquisition, origin, or source provenance retained with the package.
    ProvenanceRecord,
    /// Process, parameters, logs, or comparison evidence for a transformation.
    TransformationEvidence,
}

/// Typed disposition for one exact version 2 package member.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectivePackageMemberUseV2 {
    /// A member whose bytes can affect generated output.
    Effective {
        /// Nonempty purposes in canonical purpose-tag order.
        purposes: Vec<EffectivePackageMemberPurpose>,
    },
    /// A retained member that supports review but cannot affect output.
    EvidenceOnly {
        /// Nonempty evidence roles in canonical role-tag order.
        roles: Vec<EffectivePackageEvidenceRoleV2>,
    },
    /// A member that both affects output and supplies retained review evidence.
    EffectiveAndEvidence {
        /// Nonempty purposes in canonical purpose-tag order.
        purposes: Vec<EffectivePackageMemberPurpose>,
        /// Nonempty evidence roles in canonical role-tag order.
        roles: Vec<EffectivePackageEvidenceRoleV2>,
    },
    /// A present member excluded from the admitted runtime load closure.
    Excluded {
        /// Nonempty purposes the excluded member would otherwise serve.
        declared_purposes: Vec<EffectivePackageMemberPurpose>,
    },
}

impl EffectivePackageMemberUseV2 {
    fn validate(&self) -> Result<usize, EffectivePackageEvidenceV2Error> {
        match self {
            Self::Effective { purposes }
            | Self::Excluded {
                declared_purposes: purposes,
            } => {
                if purposes.is_empty()
                    || purposes.len() > MAX_EFFECTIVE_PACKAGE_MEMBER_V2_PURPOSES
                    || purposes
                        .windows(2)
                        .any(|pair| codec::purpose_byte(pair[0]) >= codec::purpose_byte(pair[1]))
                {
                    return Err(EffectivePackageEvidenceV2Error::InvalidMemberUse);
                }
                Ok(purposes.len())
            }
            Self::EvidenceOnly { roles } => {
                validate_roles(roles)?;
                Ok(roles.len())
            }
            Self::EffectiveAndEvidence { purposes, roles } => {
                validate_purposes(purposes)?;
                validate_roles(roles)?;
                purposes
                    .len()
                    .checked_add(roles.len())
                    .ok_or(EffectivePackageEvidenceV2Error::TooManyMemberAssignments)
            }
        }
    }
}

fn validate_purposes(
    purposes: &[EffectivePackageMemberPurpose],
) -> Result<(), EffectivePackageEvidenceV2Error> {
    if purposes.is_empty()
        || purposes.len() > MAX_EFFECTIVE_PACKAGE_MEMBER_V2_PURPOSES
        || purposes
            .windows(2)
            .any(|pair| codec::purpose_byte(pair[0]) >= codec::purpose_byte(pair[1]))
    {
        return Err(EffectivePackageEvidenceV2Error::InvalidMemberUse);
    }
    Ok(())
}

fn validate_roles(
    roles: &[EffectivePackageEvidenceRoleV2],
) -> Result<(), EffectivePackageEvidenceV2Error> {
    if roles.is_empty()
        || roles.len() > MAX_EFFECTIVE_PACKAGE_MEMBER_V2_ROLES
        || roles
            .windows(2)
            .any(|pair| codec::evidence_role_byte(pair[0]) >= codec::evidence_role_byte(pair[1]))
    {
        return Err(EffectivePackageEvidenceV2Error::InvalidMemberUse);
    }
    Ok(())
}

/// Artifact-bound typed use for one package member.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectivePackageMemberEvidenceV2 {
    relative_path: ArtifactSetRelativePath,
    artifact_id: ArtifactId,
    byte_size: u64,
    member_use: EffectivePackageMemberUseV2,
}

impl EffectivePackageMemberEvidenceV2 {
    /// Creates a validated artifact-bound member disposition.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivePackageEvidenceV2Error::InvalidMemberUse`] unless the
    /// selected purposes or roles are nonempty, bounded, ordered, and unique.
    pub fn new(
        relative_path: ArtifactSetRelativePath,
        artifact_id: ArtifactId,
        byte_size: u64,
        member_use: EffectivePackageMemberUseV2,
    ) -> Result<Self, EffectivePackageEvidenceV2Error> {
        member_use.validate()?;
        Ok(Self {
            relative_path,
            artifact_id,
            byte_size,
            member_use,
        })
    }

    /// Returns the exact package member path.
    #[must_use]
    pub const fn relative_path(&self) -> &ArtifactSetRelativePath {
        &self.relative_path
    }

    /// Returns the exact member byte identity.
    #[must_use]
    pub const fn artifact_id(&self) -> &ArtifactId {
        &self.artifact_id
    }

    /// Returns the exact member byte length.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }

    /// Returns the typed effective, evidence, dual-use, or excluded disposition.
    #[must_use]
    pub const fn member_use(&self) -> &EffectivePackageMemberUseV2 {
        &self.member_use
    }
}

/// Inputs needed to bind a version 2 package to exact runtime records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectivePackageEvidenceV2Input {
    /// Whether the package is managed or locally attested.
    pub evidence_mode: EffectivePackageEvidenceMode,
    /// Stable identifier for the evidence-producing contract.
    pub evidence_contract_id: String,
    /// Version of the evidence-producing contract.
    pub evidence_contract_schema_version: u32,
    /// Exact typed disposition for every artifact-set member.
    pub member_evidence: Vec<EffectivePackageMemberEvidenceV2>,
    /// Digest of retained evidence that the artifact set is complete.
    pub artifact_set_completeness_evidence_digest: Digest,
    /// Digest of retained immutable acquisition and origin evidence.
    pub acquisition_evidence_digest: Digest,
    /// Digest of the retained license-review decision and inputs.
    pub license_review_evidence_digest: Digest,
    /// Exact transformation disposition and retained evidence bindings.
    pub transformation: PackageTransformationDisposition,
    /// Digest of retained runtime resolution and load-closure evidence.
    pub runtime_load_closure_evidence_digest: Digest,
    /// Digest of retained exclusion, isolation, and resolution evidence.
    pub exclusion_isolation_evidence_digest: Digest,
}

/// Artifact-bound typed evidence joining a complete package and runtime tuple.
///
/// Every artifact-set member must appear exactly once in canonical path order
/// with its exact artifact identity and byte length. Distinct logical paths may
/// share one artifact identity because the artifact store is content-addressed.
///
/// This record is inert. Structural validity and digest equality do not prove
/// the truth or completeness of referenced evidence or grant runtime authority.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectivePackageEvidenceV2 {
    schema_version: u32,
    evidence_mode: EffectivePackageEvidenceMode,
    artifact_set_id: ArtifactSetId,
    runtime_build_id: RuntimeBuildId,
    effective_runtime_state_id: EffectiveRuntimeStateId,
    evidence_contract_id: String,
    evidence_contract_schema_version: u32,
    member_evidence: Vec<EffectivePackageMemberEvidenceV2>,
    artifact_set_completeness_evidence_digest: Digest,
    acquisition_evidence_digest: Digest,
    license_review_evidence_digest: Digest,
    transformation: PackageTransformationDisposition,
    runtime_load_closure_evidence_digest: Digest,
    exclusion_isolation_evidence_digest: Digest,
}

impl EffectivePackageEvidenceV2 {
    /// Creates a version 2 record from exact referenced objects and evidence.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivePackageEvidenceV2Error`] for invalid structure,
    /// artifact coverage, runtime relationships, mode, or canonical bounds.
    pub fn new(
        artifact_set: &ArtifactSetManifest,
        runtime_build: &RuntimeBuildIdentity,
        runtime_state: &EffectiveRuntimeState,
        input: EffectivePackageEvidenceV2Input,
    ) -> Result<Self, EffectivePackageEvidenceV2Error> {
        let evidence = Self::from_wire(
            EFFECTIVE_PACKAGE_EVIDENCE_V2_SCHEMA_VERSION,
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
        input: EffectivePackageEvidenceV2Input,
    ) -> Result<Self, EffectivePackageEvidenceV2Error> {
        if schema_version != EFFECTIVE_PACKAGE_EVIDENCE_V2_SCHEMA_VERSION {
            return Err(EffectivePackageEvidenceV2Error::UnsupportedSchema(
                schema_version,
            ));
        }
        if !valid_machine_id(&input.evidence_contract_id)
            || input.evidence_contract_schema_version == 0
        {
            return Err(EffectivePackageEvidenceV2Error::InvalidMetadata);
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
        if evidence.canonical_bytes().len() > MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_CANONICAL_BYTES {
            return Err(EffectivePackageEvidenceV2Error::CanonicalEncodingTooLarge);
        }
        Ok(evidence)
    }

    /// Rechecks exact identities, member bytes, and cross-record relationships.
    ///
    /// # Errors
    ///
    /// Returns [`EffectivePackageEvidenceV2Error`] when a reference, member,
    /// build-state relationship, or evidence mode differs.
    pub fn validate_against(
        &self,
        artifact_set: &ArtifactSetManifest,
        runtime_build: &RuntimeBuildIdentity,
        runtime_state: &EffectiveRuntimeState,
    ) -> Result<(), EffectivePackageEvidenceV2Error> {
        if self.artifact_set_id != artifact_set.artifact_set_id() {
            return Err(EffectivePackageEvidenceV2Error::ArtifactSetMismatch);
        }
        if self.runtime_build_id != runtime_build.runtime_build_id() {
            return Err(EffectivePackageEvidenceV2Error::RuntimeBuildMismatch);
        }
        if self.effective_runtime_state_id != runtime_state.effective_runtime_state_id() {
            return Err(EffectivePackageEvidenceV2Error::RuntimeStateMismatch);
        }
        if runtime_state.runtime_build_id() != &self.runtime_build_id {
            return Err(EffectivePackageEvidenceV2Error::RuntimeStateBuildMismatch);
        }
        if !codec::mode_matches_build(self.evidence_mode, runtime_build.mode()) {
            return Err(EffectivePackageEvidenceV2Error::EvidenceModeMismatch);
        }
        if self.member_evidence.len() != artifact_set.members().len()
            || self
                .member_evidence
                .iter()
                .zip(artifact_set.members())
                .any(|(evidence, member)| {
                    evidence.relative_path() != member.relative_path()
                        || evidence.artifact_id() != member.artifact_id()
                        || evidence.byte_size() != member.byte_size()
                })
        {
            return Err(EffectivePackageEvidenceV2Error::MemberCoverageMismatch);
        }
        Ok(())
    }

    /// Returns the content-derived identity of this complete version 2 record.
    #[must_use]
    pub fn effective_package_evidence_v2_id(&self) -> EffectivePackageEvidenceV2Id {
        EffectivePackageEvidenceV2Id(Digest::sha256(&self.canonical_bytes()))
    }

    /// Returns the version 2 evidence schema version.
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

    /// Returns typed member evidence in canonical artifact-set path order.
    #[must_use]
    pub fn member_evidence(&self) -> &[EffectivePackageMemberEvidenceV2] {
        &self.member_evidence
    }
}

fn validate_member_evidence(
    members: &[EffectivePackageMemberEvidenceV2],
) -> Result<(), EffectivePackageEvidenceV2Error> {
    if members.is_empty() {
        return Err(EffectivePackageEvidenceV2Error::EmptyMemberCoverage);
    }
    if members.len() > MAX_ARTIFACT_SET_MEMBERS {
        return Err(EffectivePackageEvidenceV2Error::TooManyMembers);
    }
    let mut assignments = 0usize;
    let mut prior_path: Option<&str> = None;
    for member in members {
        let path = member.relative_path.as_str();
        if prior_path == Some(path) {
            return Err(EffectivePackageEvidenceV2Error::DuplicateMemberPath);
        }
        if prior_path.is_some_and(|prior| prior.as_bytes() > path.as_bytes()) {
            return Err(EffectivePackageEvidenceV2Error::NoncanonicalMemberOrder);
        }
        prior_path = Some(path);
        assignments = assignments
            .checked_add(member.member_use.validate()?)
            .ok_or(EffectivePackageEvidenceV2Error::TooManyMemberAssignments)?;
        if assignments > MAX_EFFECTIVE_PACKAGE_MEMBER_V2_ASSIGNMENTS {
            return Err(EffectivePackageEvidenceV2Error::TooManyMemberAssignments);
        }
    }
    validate_shared_artifact_uses(members)?;
    Ok(())
}

fn validate_shared_artifact_uses(
    members: &[EffectivePackageMemberEvidenceV2],
) -> Result<(), EffectivePackageEvidenceV2Error> {
    let mut effective_purposes = HashMap::<&ArtifactId, Vec<EffectivePackageMemberPurpose>>::new();
    for member in members {
        let purposes = match &member.member_use {
            EffectivePackageMemberUseV2::Effective { purposes }
            | EffectivePackageMemberUseV2::EffectiveAndEvidence { purposes, .. } => purposes,
            EffectivePackageMemberUseV2::EvidenceOnly { .. }
            | EffectivePackageMemberUseV2::Excluded { .. } => continue,
        };
        let union = effective_purposes.entry(&member.artifact_id).or_default();
        for purpose in purposes {
            if !union.contains(purpose) {
                union.push(*purpose);
            }
        }
        union.sort_unstable_by_key(|purpose| codec::purpose_byte(*purpose));
    }

    for member in members {
        let Some(expected) = effective_purposes.get(&member.artifact_id) else {
            continue;
        };
        let actual = match &member.member_use {
            EffectivePackageMemberUseV2::Effective { purposes }
            | EffectivePackageMemberUseV2::EffectiveAndEvidence { purposes, .. } => purposes,
            EffectivePackageMemberUseV2::EvidenceOnly { .. }
            | EffectivePackageMemberUseV2::Excluded { .. } => {
                return Err(EffectivePackageEvidenceV2Error::InconsistentSharedArtifactUse);
            }
        };
        if actual != expected {
            return Err(EffectivePackageEvidenceV2Error::InconsistentSharedArtifactUse);
        }
    }
    Ok(())
}

/// Version 2 effective-package evidence validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum EffectivePackageEvidenceV2Error {
    /// Encoded input exceeds the fixed pre-decode byte ceiling.
    #[error("encoded version 2 effective-package evidence exceeds its limit")]
    EncodedEvidenceTooLarge,
    /// Encoded JSON is malformed or contains an unknown field.
    #[error("version 2 effective-package evidence encoding is invalid")]
    InvalidEncoding,
    /// A decoded member path violates the portable path contract.
    #[error("version 2 effective-package member path is invalid")]
    InvalidMemberPath,
    /// The evidence schema version is unsupported.
    #[error("unsupported version 2 effective-package evidence schema {0}")]
    UnsupportedSchema(u32),
    /// Evidence contract identity or version metadata is invalid.
    #[error("version 2 effective-package evidence metadata is invalid")]
    InvalidMetadata,
    /// Member coverage must include at least one member.
    #[error("version 2 effective-package member coverage is empty")]
    EmptyMemberCoverage,
    /// Member coverage exceeds the artifact-set member ceiling.
    #[error("version 2 effective-package member limit exceeded")]
    TooManyMembers,
    /// Member entries are not in strict canonical path order.
    #[error("version 2 effective-package members are not in canonical order")]
    NoncanonicalMemberOrder,
    /// More than one member entry uses the same path.
    #[error("version 2 effective-package member path is duplicated")]
    DuplicateMemberPath,
    /// A typed use is empty, unbounded, duplicated, or unordered.
    #[error("version 2 effective-package member use is invalid")]
    InvalidMemberUse,
    /// Aggregate member assignments exceed the fixed ceiling.
    #[error("version 2 effective-package member assignment limit exceeded")]
    TooManyMemberAssignments,
    /// Logical paths sharing output-effective bytes disagree on effective purposes.
    #[error("version 2 effective-package shared artifact use is inconsistent")]
    InconsistentSharedArtifactUse,
    /// Canonical identity bytes exceed the fixed contract ceiling.
    #[error("version 2 effective-package canonical identity exceeds its limit")]
    CanonicalEncodingTooLarge,
    /// The referenced artifact set differs from the record.
    #[error("version 2 effective-package artifact-set identity does not match")]
    ArtifactSetMismatch,
    /// The referenced runtime build differs from the record.
    #[error("version 2 effective-package runtime-build identity does not match")]
    RuntimeBuildMismatch,
    /// The referenced effective runtime state differs from the record.
    #[error("version 2 effective-package runtime-state identity does not match")]
    RuntimeStateMismatch,
    /// The effective runtime state is not bound to the referenced build.
    #[error("version 2 effective-package runtime state and build do not match")]
    RuntimeStateBuildMismatch,
    /// The package evidence mode is incompatible with the runtime-build mode.
    #[error("version 2 effective-package evidence mode does not match runtime-build mode")]
    EvidenceModeMismatch,
    /// Typed member evidence does not cover the artifact set exactly.
    #[error("version 2 effective-package member coverage does not match the artifact set")]
    MemberCoverageMismatch,
}

#[cfg(test)]
#[path = "v2/tests.rs"]
mod tests;
