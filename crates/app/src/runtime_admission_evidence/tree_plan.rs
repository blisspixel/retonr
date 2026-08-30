use rewrite_model::{ArtifactSetRelativePath, MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES};
use rewrite_types::Digest;
use serde::{Deserialize, Serialize};

use super::{
    RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH, RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_PATH,
    RuntimeAdmissionEvidenceBundleLimits,
    contract::{RuntimeAdmissionEvidenceContractError, domain_separated_digest},
};

/// Current canonical fixed-tree-plan contract version.
pub const RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_SCHEMA_VERSION: u32 = 2;
/// Exact number of regular evidence members before the complete manifest.
pub const RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT: usize = 12;
/// Maximum canonical JSON bytes accepted for one fixed-tree plan.
pub const MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_JSON_BYTES: usize = 16_384;

pub(super) const RUNTIME_ADMISSION_EVIDENCE_DIRECTORY_COUNT: usize = 4;
const TREE_PLAN_ID_DOMAIN: &[u8] = b"retonr:runtime-admission-evidence-tree-plan:v2";

/// One fixed purpose in the future durable admission-evidence closure.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RuntimeAdmissionEvidenceMember {
    /// Inert immutable subject binding.
    Foundation,
    /// Schema-2 runtime-package review.
    RuntimePackageReviewV2,
    /// Static source-lineage control evidence.
    SourceLineageControl,
    /// Static transformation control evidence.
    TransformationControl,
    /// Static license control evidence.
    LicenseControl,
    /// Native dependency discovery evidence.
    NativeDiscovery,
    /// Reviewer-owned native evidence.
    NativeReviewerEvidence,
    /// Native dependency adjudication.
    NativeReview,
    /// Frozen native dependency set.
    NativeFrozenSet,
    /// Ready-process native-load evidence.
    NativeLoadExecution,
    /// Managed final runtime evidence.
    ManagedFinalExecution,
    /// Review-derived cloud-disable policy material.
    CloudDisablePolicyMaterial,
}

impl RuntimeAdmissionEvidenceMember {
    /// All fixed members in canonical member order.
    pub const ALL: [Self; RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT] = [
        Self::Foundation,
        Self::RuntimePackageReviewV2,
        Self::SourceLineageControl,
        Self::TransformationControl,
        Self::LicenseControl,
        Self::NativeDiscovery,
        Self::NativeReviewerEvidence,
        Self::NativeReview,
        Self::NativeFrozenSet,
        Self::NativeLoadExecution,
        Self::ManagedFinalExecution,
        Self::CloudDisablePolicyMaterial,
    ];

    /// Returns the fixed portable path for this purpose.
    #[must_use]
    pub const fn relative_path(self) -> &'static str {
        match self {
            Self::Foundation => RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_PATH,
            Self::RuntimePackageReviewV2 => "runtime-package-review-v2.json",
            Self::SourceLineageControl => "controls/source-lineage-v1.json",
            Self::TransformationControl => "controls/transformation-v1.json",
            Self::LicenseControl => "controls/license-v2.json",
            Self::NativeDiscovery => "native/discovery-v1.json",
            Self::NativeReviewerEvidence => "native/reviewer-evidence-v1.json",
            Self::NativeReview => "native/review-v1.json",
            Self::NativeFrozenSet => "native/frozen-set-v1.json",
            Self::NativeLoadExecution => "execution/native-load-v1.json",
            Self::ManagedFinalExecution => "execution/managed-final-v1.json",
            Self::CloudDisablePolicyMaterial => "policy/cloud-disable-entry-v1.json",
        }
    }

    /// Returns the hard byte ceiling for this purpose.
    #[must_use]
    pub const fn maximum_bytes(self) -> u64 {
        match self {
            Self::Foundation | Self::CloudDisablePolicyMaterial => 65_536,
            Self::RuntimePackageReviewV2
            | Self::SourceLineageControl
            | Self::TransformationControl
            | Self::LicenseControl
            | Self::ManagedFinalExecution => 262_144,
            Self::NativeReviewerEvidence => 4_194_304,
            Self::NativeDiscovery
            | Self::NativeReview
            | Self::NativeFrozenSet
            | Self::NativeLoadExecution => 1_048_576,
        }
    }
}

/// One validated fixed member and its exact future byte size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionEvidencePlannedMember {
    member: RuntimeAdmissionEvidenceMember,
    byte_size: u64,
}

impl RuntimeAdmissionEvidencePlannedMember {
    /// Returns the fixed semantic purpose.
    #[must_use]
    pub const fn member(&self) -> RuntimeAdmissionEvidenceMember {
        self.member
    }

    /// Returns the fixed portable path.
    #[must_use]
    pub const fn relative_path(&self) -> &'static str {
        self.member.relative_path()
    }

    /// Returns the exact declared byte size.
    #[must_use]
    pub const fn byte_size(&self) -> u64 {
        self.byte_size
    }
}

/// Domain-separated identity of one exact canonical fixed-tree plan.
#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RuntimeAdmissionEvidenceTreePlanId(Digest);

impl RuntimeAdmissionEvidenceTreePlanId {
    /// Returns the digest defining this plan identity.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
    }
}

/// Canonical bounded plan for all members of a future evidence closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAdmissionEvidenceTreePlan {
    members: Vec<RuntimeAdmissionEvidencePlannedMember>,
    canonical_bytes: Vec<u8>,
    plan_id: RuntimeAdmissionEvidenceTreePlanId,
    total_member_bytes: u64,
}

impl RuntimeAdmissionEvidenceTreePlan {
    /// Compiles exact member sizes in [`RuntimeAdmissionEvidenceMember::ALL`]
    /// order into a canonical bounded plan.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionEvidenceContractError`] if limits are invalid
    /// or any member or aggregate size exceeds its ceiling.
    pub fn compile(
        member_byte_sizes: [u64; RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT],
        limits: RuntimeAdmissionEvidenceBundleLimits,
    ) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        let limits = limits.validate()?;
        let members = RuntimeAdmissionEvidenceMember::ALL
            .into_iter()
            .zip(member_byte_sizes)
            .map(|(member, byte_size)| RuntimeAdmissionEvidencePlannedMember { member, byte_size })
            .collect::<Vec<_>>();
        Self::from_members(members, limits)
    }

    /// Parses an exact canonical fixed-tree plan under caller-owned ceilings.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionEvidenceContractError`] for malformed,
    /// noncanonical, reordered, overlong, or over-limit input.
    pub fn from_canonical_bytes(
        bytes: &[u8],
        limits: RuntimeAdmissionEvidenceBundleLimits,
    ) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        if bytes.is_empty() || bytes.len() > MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_JSON_BYTES {
            return Err(RuntimeAdmissionEvidenceContractError::LimitExceeded);
        }
        let limits = limits.validate()?;
        let wire: TreePlanWire = serde_json::from_slice(bytes)
            .map_err(|_| RuntimeAdmissionEvidenceContractError::InvalidEncoding)?;
        if wire.schema_version != RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_SCHEMA_VERSION
            || wire.members.len() != RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT
        {
            return Err(RuntimeAdmissionEvidenceContractError::InvalidBinding);
        }
        let mut members = Vec::with_capacity(RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT);
        for (wire_member, expected) in wire.members.iter().zip(RuntimeAdmissionEvidenceMember::ALL)
        {
            ArtifactSetRelativePath::new(wire_member.path.clone())
                .map_err(|_| RuntimeAdmissionEvidenceContractError::InvalidBinding)?;
            if wire_member.path != expected.relative_path() {
                return Err(RuntimeAdmissionEvidenceContractError::InvalidBinding);
            }
            members.push(RuntimeAdmissionEvidencePlannedMember {
                member: expected,
                byte_size: wire_member.byte_size,
            });
        }
        let compiled = Self::from_members(members, limits)?;
        if compiled.canonical_bytes != bytes {
            return Err(RuntimeAdmissionEvidenceContractError::InvalidEncoding);
        }
        Ok(compiled)
    }

    /// Returns fixed members in canonical path order.
    #[must_use]
    pub fn members(&self) -> &[RuntimeAdmissionEvidencePlannedMember] {
        &self.members
    }

    /// Returns the aggregate bytes of all fixed members, excluding the complete
    /// artifact-set manifest.
    #[must_use]
    pub const fn total_member_bytes(&self) -> u64 {
        self.total_member_bytes
    }

    /// Returns the exact canonical JSON bytes defining this plan.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated identity of the canonical plan.
    #[must_use]
    pub const fn plan_id(&self) -> &RuntimeAdmissionEvidenceTreePlanId {
        &self.plan_id
    }

    fn from_members(
        members: Vec<RuntimeAdmissionEvidencePlannedMember>,
        limits: RuntimeAdmissionEvidenceBundleLimits,
    ) -> Result<Self, RuntimeAdmissionEvidenceContractError> {
        if members.len() != RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT {
            return Err(RuntimeAdmissionEvidenceContractError::InvalidBinding);
        }
        let mut total_member_bytes = 0_u64;
        for (planned, expected) in members.iter().zip(RuntimeAdmissionEvidenceMember::ALL) {
            if planned.member != expected
                || planned.byte_size == 0
                || planned.byte_size > planned.member.maximum_bytes()
            {
                return Err(RuntimeAdmissionEvidenceContractError::LimitExceeded);
            }
            ArtifactSetRelativePath::new(planned.relative_path().to_owned())
                .map_err(|_| RuntimeAdmissionEvidenceContractError::InvalidBinding)?;
            total_member_bytes = total_member_bytes
                .checked_add(planned.byte_size)
                .ok_or(RuntimeAdmissionEvidenceContractError::LimitExceeded)?;
        }
        let reserved_manifest_bytes = u64::try_from(MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES)
            .map_err(|_| RuntimeAdmissionEvidenceContractError::LimitExceeded)?;
        if total_member_bytes
            .checked_add(reserved_manifest_bytes)
            .is_none_or(|total| total > limits.maximum_total_bytes)
        {
            return Err(RuntimeAdmissionEvidenceContractError::LimitExceeded);
        }
        ArtifactSetRelativePath::new(RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH.to_owned())
            .map_err(|_| RuntimeAdmissionEvidenceContractError::InvalidBinding)?;
        let wire = TreePlanWire {
            members: members
                .iter()
                .map(|planned| TreePlanMemberWire {
                    byte_size: planned.byte_size,
                    path: planned.relative_path().to_owned(),
                })
                .collect(),
            schema_version: RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_SCHEMA_VERSION,
        };
        let canonical_bytes = serde_json::to_vec(&wire)
            .map_err(|_| RuntimeAdmissionEvidenceContractError::InvalidEncoding)?;
        if canonical_bytes.len() > MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_JSON_BYTES {
            return Err(RuntimeAdmissionEvidenceContractError::LimitExceeded);
        }
        let plan_id = RuntimeAdmissionEvidenceTreePlanId(domain_separated_digest(
            TREE_PLAN_ID_DOMAIN,
            &canonical_bytes,
        ));
        Ok(Self {
            members,
            canonical_bytes,
            plan_id,
            total_member_bytes,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TreePlanWire {
    members: Vec<TreePlanMemberWire>,
    schema_version: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TreePlanMemberWire {
    byte_size: u64,
    path: String,
}
