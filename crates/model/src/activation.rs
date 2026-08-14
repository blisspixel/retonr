use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use rewrite_types::Digest;

use crate::{ArtifactId, ArtifactRole, QualificationId, QualificationRecord};

/// Stable caller-supplied idempotency identifier for an activation decision.
#[derive(Clone, Debug, Eq, Hash, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActivationId(Digest);

impl ActivationId {
    /// Creates an identifier from a caller-controlled idempotency digest.
    #[must_use]
    pub fn from_digest(digest: Digest) -> Self {
        Self(digest)
    }

    /// Returns the digest that defines this idempotency identifier.
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.0
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

impl InstalledArtifact {
    /// Validates the installed artifact's immutable identity and storage key.
    ///
    /// # Errors
    ///
    /// Returns [`InstallationError`] when the identity, size, or storage key is
    /// invalid.
    pub fn validate(&self) -> Result<(), InstallationError> {
        if self.byte_size == 0 || !valid_storage_key(&self.storage_key) {
            return Err(InstallationError::InvalidState);
        }
        if self.artifact_id.digest() != &self.artifact_digest {
            return Err(InstallationError::IdentityMismatch);
        }
        Ok(())
    }
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

impl QualificationInvalidation {
    /// Validates the stable redacted reason code.
    ///
    /// # Errors
    ///
    /// Returns [`QualificationInvalidationError`] when the reason is empty,
    /// oversized, or not a lowercase machine identifier.
    pub fn validate(&self) -> Result<(), QualificationInvalidationError> {
        let valid = !self.reason_code.is_empty()
            && self.reason_code.len() <= 64
            && self
                .reason_code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
        if valid {
            Ok(())
        } else {
            Err(QualificationInvalidationError)
        }
    }
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

impl ActivationDecision {
    /// Validates that the action and optional identities form one complete state
    /// transition.
    ///
    /// # Errors
    ///
    /// Returns [`ActivationDecisionError`] when an activation omits an identity or
    /// a deactivation carries one.
    pub fn validate(&self) -> Result<(), ActivationDecisionError> {
        let valid = match self.action {
            ActivationAction::Activate => {
                self.artifact_id.is_some() && self.qualification_id.is_some()
            }
            ActivationAction::Deactivate => {
                self.artifact_id.is_none() && self.qualification_id.is_none()
            }
        };
        if valid {
            Ok(())
        } else {
            Err(ActivationDecisionError)
        }
    }
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
    match installed.validate() {
        Ok(()) => {}
        Err(InstallationError::InvalidState) => {
            return Err(ActivationError::InvalidInstallation);
        }
        Err(InstallationError::IdentityMismatch) => {
            return Err(ActivationError::ArtifactMismatch);
        }
    }
    if installed.artifact_id.digest() != &installed.artifact_digest
        || installed.artifact_id != qualification.artifact_id
        || installed.artifact_digest != qualification.artifact_digest
    {
        return Err(ActivationError::ArtifactMismatch);
    }
    let qualification_id = qualification
        .qualification_id()
        .map_err(|_| ActivationError::UnqualifiedRole)?;
    if invalidations
        .iter()
        .any(|item| item.qualification_id == qualification_id)
    {
        return Err(ActivationError::InvalidatedQualification);
    }
    if !qualification.authorizes(&installed.artifact_id, role) {
        return Err(ActivationError::UnqualifiedRole);
    }
    Ok(ActiveArtifactBinding {
        role,
        artifact_id: installed.artifact_id.clone(),
        qualification_id,
        artifact_digest: installed.artifact_digest.clone(),
    })
}

/// Installed artifact validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum InstallationError {
    /// The byte size or opaque storage key is invalid.
    #[error("installed artifact state is invalid")]
    InvalidState,
    /// The artifact identifier and verified digest differ.
    #[error("installed artifact identity does not match its digest")]
    IdentityMismatch,
}

/// Qualification invalidation validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("qualification invalidation reason is invalid")]
pub struct QualificationInvalidationError;

/// Activation decision validation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("activation decision is incomplete or inconsistent")]
pub struct ActivationDecisionError;

fn valid_storage_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && !matches!(segment, "." | "..")
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
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

    use super::{
        ActivationAction, ActivationDecision, ActivationError, ActivationId, InstalledArtifact,
        QualificationInvalidation, activate,
    };
    use crate::{
        ArtifactId, ArtifactRole, HardwareTier, LicenseDecision, QUALIFICATION_SCHEMA_VERSION,
        QualificationRecord, QualificationStatus, RuntimeIdentity,
    };

    fn fixture() -> (InstalledArtifact, QualificationRecord) {
        let digest = Digest::sha256(b"artifact");
        let artifact_id = ArtifactId::from_digest(digest.clone());
        let qualification = QualificationRecord {
            schema_version: QUALIFICATION_SCHEMA_VERSION,
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
    fn generation_qualification_cannot_activate_claim_extraction() {
        let (installed, qualification) = fixture();
        assert_eq!(
            activate(
                &installed,
                &qualification,
                &[],
                ArtifactRole::ClaimExtraction,
            ),
            Err(ActivationError::UnqualifiedRole)
        );
    }

    #[test]
    fn rejects_invalidated_or_mismatched_state() {
        let (installed, qualification) = fixture();
        let invalidation = QualificationInvalidation {
            qualification_id: qualification
                .qualification_id()
                .expect("valid qualification"),
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

    #[test]
    fn rejects_inconsistent_decisions_and_unredacted_reason_codes() {
        let (_, qualification) = fixture();
        let decision = ActivationDecision {
            activation_id: ActivationId::from_digest(Digest::sha256(b"decision")),
            action: ActivationAction::Activate,
            role: ArtifactRole::Generation,
            artifact_id: None,
            qualification_id: Some(
                qualification
                    .qualification_id()
                    .expect("valid qualification"),
            ),
        };
        assert!(decision.validate().is_err());

        let invalidation = QualificationInvalidation {
            qualification_id: qualification
                .qualification_id()
                .expect("valid qualification"),
            reason_code: "Runtime drift: local path".to_owned(),
        };
        assert!(invalidation.validate().is_err());
    }

    #[test]
    fn validates_portable_opaque_storage_keys() {
        let (mut installed, _) = fixture();
        for invalid in ["", ".", "..", "/artifact", "artifacts/", "artifacts//model"] {
            installed.storage_key = invalid.to_owned();
            assert_eq!(
                installed.validate(),
                Err(super::InstallationError::InvalidState)
            );
        }

        installed.storage_key = "artifacts/model..v1.gguf".to_owned();
        installed.validate().expect("portable key is valid");
    }
}
