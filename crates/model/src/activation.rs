use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use crate::{ArtifactId, ArtifactRole, QualificationId, QualificationRecord};

/// Content-derived identifier for an activation decision.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActivationId(Digest);

impl ActivationId {
    /// Creates an identifier from canonical decision bytes.
    #[must_use]
    pub fn from_digest(digest: Digest) -> Self {
        Self(digest)
    }
}

/// Verified local installation state for one immutable artifact.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledArtifact {
    /// Installed artifact identity.
    pub artifact_id: ArtifactId,
    /// Digest verified after staging completed.
    pub artifact_digest: Digest,
    /// Verified artifact byte size.
    pub byte_size: u64,
    /// Opaque application-owned storage key, not a user-supplied path.
    pub storage_key: String,
}

/// Append-only reason a qualification record stopped being eligible for activation.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualificationInvalidation {
    /// Invalidated qualification record.
    pub qualification_id: QualificationId,
    /// Stable, redacted invalidation category.
    pub reason_code: String,
}

/// Activation state transition recorded by the artifact repository.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationAction {
    /// Bind an artifact and qualification to a role.
    Activate,
    /// Remove the active binding for a role.
    Deactivate,
}

/// Immutable activation decision appended before the active pointer changes.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivationDecision {
    /// Decision identifier.
    pub activation_id: ActivationId,
    /// Requested transition.
    pub action: ActivationAction,
    /// Product role affected by the transition.
    pub role: ArtifactRole,
    /// Installed artifact selected by an activation.
    pub artifact_id: Option<ArtifactId>,
    /// Qualification selected by an activation.
    pub qualification_id: Option<QualificationId>,
}

/// Currently active, fully qualified artifact binding for one role.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveArtifactBinding {
    /// Role served by this binding.
    pub role: ArtifactRole,
    /// Installed artifact used for the role.
    pub artifact_id: ArtifactId,
    /// Qualification that authorizes this exact binding.
    pub qualification_id: QualificationId,
    /// Artifact digest revalidated at activation.
    pub artifact_digest: Digest,
}

/// Verifies the domain conditions required before an activation transaction commits.
///
/// # Errors
///
/// Returns [`ActivationError`] for mismatched installation state, invalidated or
/// insufficient qualification evidence, or malformed local storage state.
pub fn activate(
    installed: &InstalledArtifact,
    qualification: &QualificationRecord,
    invalidations: &[QualificationInvalidation],
    role: ArtifactRole,
) -> Result<ActiveArtifactBinding, ActivationError> {
    if installed.byte_size == 0 || !valid_storage_key(&installed.storage_key) {
        return Err(ActivationError::InvalidInstallation);
    }
    if installed.artifact_id.digest() != &installed.artifact_digest
        || installed.artifact_id != qualification.artifact_id
        || installed.artifact_digest != qualification.artifact_digest
    {
        return Err(ActivationError::ArtifactMismatch);
    }
    if invalidations
        .iter()
        .any(|item| item.qualification_id == qualification.qualification_id)
    {
        return Err(ActivationError::InvalidatedQualification);
    }
    if !qualification.authorizes(&installed.artifact_id, role) {
        return Err(ActivationError::UnqualifiedRole);
    }
    Ok(ActiveArtifactBinding {
        role,
        artifact_id: installed.artifact_id.clone(),
        qualification_id: qualification.qualification_id.clone(),
        artifact_digest: installed.artifact_digest.clone(),
    })
}

fn valid_storage_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'/' | b'.'))
        && !value.starts_with('/')
        && !value.contains("..")
}

/// Activation precondition failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ActivationError {
    /// Installed artifact state is empty or has an invalid opaque storage key.
    #[error("installed artifact state is invalid")]
    InvalidInstallation,
    /// Installed, manifest, and qualification artifact identities differ.
    #[error("installed artifact does not match qualification")]
    ArtifactMismatch,
    /// Qualification has an append-only invalidation record.
    #[error("qualification has been invalidated")]
    InvalidatedQualification,
    /// Qualification does not authorize the requested role.
    #[error("qualification does not authorize the requested role")]
    UnqualifiedRole,
}

#[cfg(test)]
mod tests {
    use rewrite_types::Digest;

    use super::{ActivationError, InstalledArtifact, QualificationInvalidation, activate};
    use crate::{
        ArtifactId, ArtifactRole, HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION,
        QualificationId, QualificationRecord, QualificationStatus, RuntimeIdentity,
    };

    fn fixture() -> (InstalledArtifact, QualificationRecord) {
        let digest = Digest::sha256(b"artifact");
        let artifact_id = ArtifactId::from_digest(digest.clone());
        let qualification = QualificationRecord {
            schema_version: QUALIFICATION_SCHEMA_VERSION,
            qualification_id: QualificationId::from_digest(Digest::sha256(b"qualification")),
            artifact_id: artifact_id.clone(),
            artifact_digest: digest.clone(),
            runtime: RuntimeIdentity {
                backend: "fake".to_owned(),
                version: "1".to_owned(),
                digest: None,
            },
            operating_system: "test".to_owned(),
            hardware_tier: HardwareTier {
                id: "test".to_owned(),
                memory_mib: 4_096,
                accelerator: "none".to_owned(),
            },
            supported_roles: vec![ArtifactRole::Generation],
            source_byte_limit: 1_024,
            context_token_limit: 2_048,
            prompt_template_digest: Digest::sha256(b"prompt"),
            request_policy_digest: Digest::sha256(b"request"),
            threshold_policy_digest: Digest::sha256(b"threshold"),
            license_decision: LicenseDecision::LocalUseOnly,
            status: QualificationStatus::Qualified,
        };
        (
            InstalledArtifact {
                artifact_id,
                artifact_digest: digest,
                byte_size: 8,
                storage_key: "artifacts/fixture.gguf".to_owned(),
            },
            qualification,
        )
    }

    #[test]
    fn binds_exact_installed_artifact_and_qualification() {
        let (installed, qualification) = fixture();
        let binding = activate(&installed, &qualification, &[], ArtifactRole::Generation)
            .expect("exact qualified artifact activates");
        assert_eq!(binding.artifact_digest, installed.artifact_digest);
    }

    #[test]
    fn rejects_invalidated_or_mismatched_state() {
        let (installed, qualification) = fixture();
        let invalidation = QualificationInvalidation {
            qualification_id: qualification.qualification_id.clone(),
            reason_code: "runtime_drift".to_owned(),
        };
        assert_eq!(
            activate(
                &installed,
                &qualification,
                &[invalidation],
                ArtifactRole::Generation,
            ),
            Err(ActivationError::InvalidatedQualification)
        );

        let mut wrong = installed;
        wrong.artifact_digest = Digest::sha256(b"wrong");
        assert_eq!(
            activate(&wrong, &qualification, &[], ArtifactRole::Generation),
            Err(ActivationError::ArtifactMismatch)
        );
    }
}
