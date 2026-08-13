use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

    /// Returns the digest that defines this qualification identifier.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
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
    /// Computes the content-derived identifier for this complete record.
    ///
    /// The identity material uses a versioned, length-delimited binary encoding
    /// with fixed field order. The identifier is intentionally not embedded in the
    /// record it identifies.
    ///
    /// # Errors
    ///
    /// Returns [`QualificationRecordError`] instead of assigning an identity to a
    /// malformed or noncanonical record.
    pub fn qualification_id(&self) -> Result<QualificationId, QualificationRecordError> {
        self.validate()?;
        let mut material = Vec::new();
        material.extend_from_slice(b"retonr:qualification-record:v1\0");
        append_u32(&mut material, self.schema_version);
        append_digest(&mut material, self.artifact_id.digest());
        append_digest(&mut material, &self.artifact_digest);
        append_text(&mut material, &self.runtime.backend);
        append_text(&mut material, &self.runtime.version);
        append_optional_digest(&mut material, self.runtime.digest.as_ref());
        append_text(&mut material, &self.operating_system);
        append_text(&mut material, &self.hardware_tier.id);
        append_u64(&mut material, self.hardware_tier.memory_mib);
        append_text(&mut material, &self.hardware_tier.accelerator);
        append_u64(&mut material, self.supported_roles.len() as u64);
        for role in &self.supported_roles {
            material.push(role_identity_byte(*role));
        }
        append_u64(&mut material, self.source_byte_limit);
        append_u32(&mut material, self.context_token_limit);
        append_digest(&mut material, &self.prompt_template_digest);
        append_digest(&mut material, &self.request_policy_digest);
        append_digest(&mut material, &self.threshold_policy_digest);
        material.push(license_identity_byte(self.license_decision));
        material.push(status_identity_byte(self.status));
        Ok(QualificationId::from_digest(Digest::sha256(&material)))
    }

    /// Validates the intrinsic structure of a qualification record.
    ///
    /// This does not grant support or activate the artifact.
    ///
    /// # Errors
    ///
    /// Returns [`QualificationRecordError`] when the record is unsupported,
    /// inconsistent, unbounded, or contains invalid machine metadata.
    pub fn validate(&self) -> Result<(), QualificationRecordError> {
        if self.schema_version != QUALIFICATION_SCHEMA_VERSION {
            return Err(QualificationRecordError::UnsupportedSchema(
                self.schema_version,
            ));
        }
        if self.artifact_id.digest() != &self.artifact_digest {
            return Err(QualificationRecordError::ArtifactMismatch);
        }
        for value in [
            self.runtime.backend.as_str(),
            self.runtime.version.as_str(),
            self.operating_system.as_str(),
            self.hardware_tier.id.as_str(),
            self.hardware_tier.accelerator.as_str(),
        ] {
            if !valid_bounded_text(value, 128) {
                return Err(QualificationRecordError::InvalidMetadata);
            }
        }
        if self.supported_roles.len() > 16
            || !self
                .supported_roles
                .windows(2)
                .all(|roles| role_identity_byte(roles[0]) < role_identity_byte(roles[1]))
            || (self.status == QualificationStatus::Qualified && self.supported_roles.is_empty())
        {
            return Err(QualificationRecordError::InvalidRoles);
        }
        if self.source_byte_limit == 0
            || self.context_token_limit == 0
            || self.hardware_tier.memory_mib == 0
            || (self.status == QualificationStatus::Qualified
                && self.license_decision == LicenseDecision::Rejected)
        {
            return Err(QualificationRecordError::InvalidPolicy);
        }
        Ok(())
    }

    /// Returns whether this record can authorize the requested role and artifact.
    #[must_use]
    pub fn authorizes(&self, artifact_id: &ArtifactId, role: ArtifactRole) -> bool {
        self.validate().is_ok()
            && self.status == QualificationStatus::Qualified
            && self.license_decision != LicenseDecision::Rejected
            && &self.artifact_id == artifact_id
            && self.artifact_id.digest() == &self.artifact_digest
            && self.source_byte_limit > 0
            && self.context_token_limit > 0
            && self.supported_roles.contains(&role)
    }
}

fn append_u32(material: &mut Vec<u8>, value: u32) {
    material.extend_from_slice(&value.to_be_bytes());
}

fn append_u64(material: &mut Vec<u8>, value: u64) {
    material.extend_from_slice(&value.to_be_bytes());
}

fn append_text(material: &mut Vec<u8>, value: &str) {
    append_u64(material, value.len() as u64);
    material.extend_from_slice(value.as_bytes());
}

fn append_digest(material: &mut Vec<u8>, value: &Digest) {
    material.extend_from_slice(value.as_str().as_bytes());
}

fn append_optional_digest(material: &mut Vec<u8>, value: Option<&Digest>) {
    match value {
        Some(value) => {
            material.push(1);
            append_digest(material, value);
        }
        None => material.push(0),
    }
}

const fn role_identity_byte(role: ArtifactRole) -> u8 {
    match role {
        ArtifactRole::Generation => 0,
        ArtifactRole::Embedding => 1,
        ArtifactRole::SpeechRecognition => 2,
        ArtifactRole::VoiceActivityDetection => 3,
        ArtifactRole::SpeechSynthesis => 4,
        ArtifactRole::Voice => 5,
    }
}

const fn license_identity_byte(decision: LicenseDecision) -> u8 {
    match decision {
        LicenseDecision::LocalUseOnly => 0,
        LicenseDecision::RedistributionApproved => 1,
        LicenseDecision::Rejected => 2,
    }
}

const fn status_identity_byte(status: QualificationStatus) -> u8 {
    match status {
        QualificationStatus::Qualified => 0,
        QualificationStatus::Rejected => 1,
    }
}

fn valid_bounded_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

/// Qualification record validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum QualificationRecordError {
    /// The qualification schema version is unsupported.
    #[error("unsupported qualification schema {0}")]
    UnsupportedSchema(u32),
    /// The artifact identifier and verified digest differ.
    #[error("qualification artifact identity does not match its digest")]
    ArtifactMismatch,
    /// Runtime, platform, or hardware metadata is invalid.
    #[error("qualification metadata is empty, oversized, or contains controls")]
    InvalidMetadata,
    /// Supported roles are unordered, duplicated, oversized, or absent for a
    /// qualified record.
    #[error("qualification roles are invalid")]
    InvalidRoles,
    /// Resource bounds or the license and outcome combination are invalid.
    #[error("qualification policy is invalid")]
    InvalidPolicy,
}

#[cfg(test)]
mod tests {
    use rewrite_types::Digest;

    use super::{
        HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION, QualificationRecord,
        QualificationStatus, RuntimeIdentity,
    };
    use crate::{ArtifactId, ArtifactRole};

    fn record() -> QualificationRecord {
        let artifact_digest = Digest::sha256(b"artifact");
        QualificationRecord {
            schema_version: QUALIFICATION_SCHEMA_VERSION,
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

    #[test]
    fn rejects_inconsistent_qualification_structure() {
        let mut duplicate_roles = record();
        duplicate_roles
            .supported_roles
            .push(ArtifactRole::Generation);
        assert!(duplicate_roles.validate().is_err());

        let mut missing_backend = record();
        missing_backend.runtime.backend.clear();
        assert!(missing_backend.validate().is_err());

        let mut zero_context = record();
        zero_context.context_token_limit = 0;
        assert!(zero_context.validate().is_err());

        let mut unordered_roles = record();
        unordered_roles.supported_roles = vec![ArtifactRole::Embedding, ArtifactRole::Generation];
        assert!(unordered_roles.validate().is_err());
    }

    #[test]
    fn identity_is_stable_and_covers_complete_qualification_content() {
        let record = record();
        assert_eq!(
            record
                .qualification_id()
                .expect("valid record has an identity")
                .digest()
                .as_str(),
            "aa156c8224aa6a6dacc7bd3351b3ebd67fab2c345ce340c1bc7294d49193d4dd"
        );

        let mut changed = record.clone();
        changed.context_token_limit += 1;
        assert_ne!(
            record.qualification_id().expect("valid record"),
            changed.qualification_id().expect("valid changed record")
        );
    }
}
