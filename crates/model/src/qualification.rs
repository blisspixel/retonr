use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use rewrite_types::Digest;

use crate::{ArtifactId, ArtifactRole};

/// Current qualification-record contract version.
pub const QUALIFICATION_SCHEMA_VERSION: u32 = 1;

/// Content-derived identifier for an immutable qualification record.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QualificationId(Digest);

impl QualificationId {
    /// Creates a qualification identifier from canonical record bytes.
    #[must_use]
    pub fn from_digest(digest: Digest) -> Self {
        Self(digest)
    }
}

/// Exact runtime identity used during qualification.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeIdentity {
    /// Backend implementation identifier.
    pub backend: String,
    /// Exact runtime version.
    pub version: String,
    /// Digest of the executable or runtime package when available.
    pub digest: Option<Digest>,
}

/// Hardware class on which qualification evidence was collected.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareTier {
    /// Stable tier identifier.
    pub id: String,
    /// Total system memory in MiB.
    pub memory_mib: u64,
    /// Accelerator description, or `none` for CPU-only qualification.
    pub accelerator: String,
}

/// Reviewed permission to use or redistribute an artifact.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicenseDecision {
    /// Local use is approved, but redistribution is not.
    LocalUseOnly,
    /// Local use and redistribution are approved.
    RedistributionApproved,
    /// License evidence is insufficient or use is rejected.
    Rejected,
}

/// Outcome of applying a predeclared qualification policy.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationStatus {
    /// Every required threshold passed.
    Qualified,
    /// One or more required thresholds failed or had insufficient evidence.
    Rejected,
}

/// Immutable evidence binding one artifact to a tested runtime and policy.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualificationRecord {
    /// Qualification contract version.
    pub schema_version: u32,
    /// Content-derived qualification identifier.
    pub qualification_id: QualificationId,
    /// Artifact that was tested.
    pub artifact_id: ArtifactId,
    /// Digest rechecked during qualification.
    pub artifact_digest: Digest,
    /// Exact runtime under test.
    pub runtime: RuntimeIdentity,
    /// Operating system identifier under test.
    pub operating_system: String,
    /// Hardware class under test.
    pub hardware_tier: HardwareTier,
    /// Roles that passed the qualification policy.
    pub supported_roles: Vec<ArtifactRole>,
    /// Maximum admitted source size for this record.
    pub source_byte_limit: u64,
    /// Effective context limit for this record.
    pub context_token_limit: u32,
    /// Prompt or chat-template digest.
    pub prompt_template_digest: Digest,
    /// Digest of explicit request and generation parameters.
    pub request_policy_digest: Digest,
    /// Digest of predeclared thresholds and evaluation policy.
    pub threshold_policy_digest: Digest,
    /// Reviewed license decision.
    pub license_decision: LicenseDecision,
    /// Qualification outcome.
    pub status: QualificationStatus,
}

impl QualificationRecord {
    /// Returns whether this record can authorize the requested role and artifact.
    #[must_use]
    pub fn authorizes(&self, artifact_id: &ArtifactId, role: ArtifactRole) -> bool {
        self.schema_version == QUALIFICATION_SCHEMA_VERSION
            && self.status == QualificationStatus::Qualified
            && self.license_decision != LicenseDecision::Rejected
            && &self.artifact_id == artifact_id
            && self.artifact_id.digest() == &self.artifact_digest
            && self.source_byte_limit > 0
            && self.context_token_limit > 0
            && self.supported_roles.contains(&role)
    }
}

#[cfg(test)]
mod tests {
    use rewrite_types::Digest;

    use super::{
        HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION, QualificationId,
        QualificationRecord, QualificationStatus, RuntimeIdentity,
    };
    use crate::{ArtifactId, ArtifactRole};

    fn record() -> QualificationRecord {
        let artifact_digest = Digest::sha256(b"artifact");
        QualificationRecord {
            schema_version: QUALIFICATION_SCHEMA_VERSION,
            qualification_id: QualificationId::from_digest(Digest::sha256(b"qualification")),
            artifact_id: ArtifactId::from_digest(artifact_digest.clone()),
            artifact_digest,
            runtime: RuntimeIdentity {
                backend: "fake".to_owned(),
                version: "1.0.0".to_owned(),
                digest: None,
            },
            operating_system: "test".to_owned(),
            hardware_tier: HardwareTier {
                id: "fixture".to_owned(),
                memory_mib: 8_192,
                accelerator: "none".to_owned(),
            },
            supported_roles: vec![ArtifactRole::Generation],
            source_byte_limit: 4_096,
            context_token_limit: 8_192,
            prompt_template_digest: Digest::sha256(b"prompt"),
            request_policy_digest: Digest::sha256(b"request"),
            threshold_policy_digest: Digest::sha256(b"threshold"),
            license_decision: LicenseDecision::LocalUseOnly,
            status: QualificationStatus::Qualified,
        }
    }

    #[test]
    fn authorizes_only_exact_qualified_binding() {
        let record = record();
        assert!(record.authorizes(&record.artifact_id, ArtifactRole::Generation));
        assert!(!record.authorizes(&record.artifact_id, ArtifactRole::Embedding));

        let mut rejected = record.clone();
        rejected.status = QualificationStatus::Rejected;
        assert!(!rejected.authorizes(&rejected.artifact_id, ArtifactRole::Generation));
    }
}
