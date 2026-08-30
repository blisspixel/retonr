use std::io::Read as _;

use rewrite_model::{ArtifactSetRelativePath, RuntimePackageManifestId, RuntimePackageMemberRole};
use rewrite_ollama_package::{RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputRole};
use rewrite_types::{CancellationToken, Digest};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

use super::super::binding::RuntimeAdmissionFoundationEvidenceView;
use super::super::{
    RuntimeAdmissionFoundationBindingError, VerifiedRuntimeAdmissionFoundationBinding,
};
use crate::{RuntimeSourceBuildEvidenceBundleError, RuntimeSourceBuildEvidenceBundleLease};

/// Hard ceiling for one canonical static-control record.
pub const MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES: usize = 262_144;
/// Hard ceiling for one embedded canonical reviewer record.
pub const MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES: usize = 262_144;

const MAX_RETAINED_STATIC_EVIDENCE_BYTES: u64 = 4 * 1_024 * 1_024;

/// Failure while compiling or independently verifying an inert static control.
#[derive(Debug, Error)]
pub enum RuntimeAdmissionStaticControlError {
    /// The verified foundation no longer matches the selected durable closure.
    #[error(transparent)]
    Foundation(#[from] RuntimeAdmissionFoundationBindingError),
    /// The durable source-build closure failed a read or revalidation.
    #[error(transparent)]
    Evidence(#[from] RuntimeSourceBuildEvidenceBundleError),
    /// Canonical control or reviewer JSON was malformed or noncanonical.
    #[error("runtime admission static-control encoding is invalid")]
    InvalidEncoding,
    /// A procedure, foundation, evidence, or subject identity did not match.
    #[error("runtime admission static-control binding is invalid")]
    InvalidBinding,
    /// A selected static-control input exceeded its hard ceiling.
    #[error("runtime admission static-control limit was exceeded")]
    LimitExceeded,
    /// Retained evidence still requires an explicit review decision.
    #[error("runtime admission static control still requires review")]
    ReviewRequired,
    /// Controlled-build evidence does not support the claimed static result.
    #[error("runtime admission static-control evidence is insufficient")]
    EvidenceInsufficient,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StaticInputComponent {
    pub(super) relative_path: ArtifactSetRelativePath,
    pub(super) byte_size: u64,
    pub(super) digest: Digest,
    pub(super) name: String,
    pub(super) revision: String,
    pub(super) source_locator: String,
    pub(super) roles: Vec<RuntimeSourceBuildInputRole>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StaticLicenseMember {
    pub(super) relative_path: ArtifactSetRelativePath,
    pub(super) byte_size: u64,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StaticEvidenceFacts {
    pub(super) components: Vec<StaticInputComponent>,
    pub(super) build_program_path: ArtifactSetRelativePath,
    pub(super) build_program_digest: Digest,
    pub(super) byte_identical: bool,
    pub(super) runtime_package_manifest_id: RuntimePackageManifestId,
    pub(super) license_members: Vec<StaticLicenseMember>,
}

pub(super) trait RuntimeAdmissionStaticEvidenceView:
    RuntimeAdmissionFoundationEvidenceView
{
    fn facts(&self) -> StaticEvidenceFacts;

    fn read_source_input(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError>;

    fn read_source_input_with_limit(
        &self,
        path: &ArtifactSetRelativePath,
        maximum: u64,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError> {
        let bytes = self.read_source_input(path, cancellation)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
            Err(RuntimeAdmissionStaticControlError::LimitExceeded)
        } else {
            Ok(bytes)
        }
    }

    fn read_evidence(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError>;
}

impl RuntimeAdmissionStaticEvidenceView for RuntimeSourceBuildEvidenceBundleLease {
    fn facts(&self) -> StaticEvidenceFacts {
        let manifest = self.source_inputs().manifest();
        let components = manifest.components().iter().map(component_fact).collect();
        let primary = self.report().primary();
        let license_members = primary
            .layout()
            .members()
            .iter()
            .filter(|member| {
                member
                    .roles()
                    .contains(&RuntimePackageMemberRole::LicenseText)
            })
            .map(|member| StaticLicenseMember {
                relative_path: member.relative_path().clone(),
                byte_size: member.byte_size(),
                digest: member.digest().clone(),
            })
            .collect();
        StaticEvidenceFacts {
            components,
            build_program_path: self.plan().build_program_path().clone(),
            build_program_digest: self.plan().build_program_digest().clone(),
            byte_identical: self.report().is_byte_identical(),
            runtime_package_manifest_id: primary.runtime_package().runtime_package_manifest_id(),
            license_members,
        }
    }

    fn read_source_input(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError> {
        let file = self.open_source_input(path, cancellation)?;
        read_bounded(file, MAX_RETAINED_STATIC_EVIDENCE_BYTES, cancellation)
    }

    fn read_source_input_with_limit(
        &self,
        path: &ArtifactSetRelativePath,
        maximum: u64,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError> {
        let file = self.open_source_input(path, cancellation)?;
        read_bounded(file, maximum, cancellation)
    }

    fn read_evidence(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError> {
        let file = self.open_evidence(path, cancellation)?;
        read_bounded(file, MAX_RETAINED_STATIC_EVIDENCE_BYTES, cancellation)
    }
}

fn component_fact(component: &RuntimeSourceBuildInputComponent) -> StaticInputComponent {
    StaticInputComponent {
        relative_path: component.relative_path().clone(),
        byte_size: component.byte_size(),
        digest: component.digest().clone(),
        name: component.name().to_owned(),
        revision: component.revision().to_owned(),
        source_locator: component.source_locator().to_owned(),
        roles: component.roles().to_vec(),
    }
}

pub(super) fn ensure_same_foundation<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    binding: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    cancellation: &CancellationToken,
) -> Result<StaticEvidenceFacts, RuntimeAdmissionStaticControlError> {
    evidence.revalidate(cancellation)?;
    let snapshot = evidence.snapshot()?;
    if snapshot != binding.snapshot {
        return Err(RuntimeAdmissionStaticControlError::InvalidBinding);
    }
    let facts = evidence.facts();
    if !facts.byte_identical
        || facts.runtime_package_manifest_id != binding.snapshot.runtime_package_manifest_id
    {
        return Err(RuntimeAdmissionStaticControlError::EvidenceInsufficient);
    }
    Ok(facts)
}

pub(super) fn finish<V: RuntimeAdmissionStaticEvidenceView + ?Sized>(
    binding: &VerifiedRuntimeAdmissionFoundationBinding,
    evidence: &V,
    cancellation: &CancellationToken,
) -> Result<(), RuntimeAdmissionStaticControlError> {
    let _facts = ensure_same_foundation(binding, evidence, cancellation)?;
    evidence.revalidate(cancellation)?;
    Ok(())
}

pub(super) fn parse_canonical<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
    maximum: usize,
) -> Result<T, RuntimeAdmissionStaticControlError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(RuntimeAdmissionStaticControlError::LimitExceeded);
    }
    let parsed: T = serde_json::from_slice(bytes)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidEncoding)?;
    let canonical = serde_json::to_vec(&parsed)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidEncoding)?;
    if canonical != bytes {
        return Err(RuntimeAdmissionStaticControlError::InvalidEncoding);
    }
    Ok(parsed)
}

pub(super) fn encode<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidEncoding)?;
    if bytes.len() > MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES {
        return Err(RuntimeAdmissionStaticControlError::LimitExceeded);
    }
    Ok(bytes)
}

pub(super) fn fixed_path(
    path: &str,
) -> Result<ArtifactSetRelativePath, RuntimeAdmissionStaticControlError> {
    ArtifactSetRelativePath::new(path.to_owned())
        .map_err(|_| RuntimeAdmissionStaticControlError::InvalidBinding)
}

pub(super) fn find_role(
    facts: &StaticEvidenceFacts,
    role: RuntimeSourceBuildInputRole,
) -> Result<&StaticInputComponent, RuntimeAdmissionStaticControlError> {
    let mut found = facts
        .components
        .iter()
        .filter(|component| component.roles.contains(&role));
    let first = found
        .next()
        .ok_or(RuntimeAdmissionStaticControlError::EvidenceInsufficient)?;
    if found.next().is_some() {
        return Err(RuntimeAdmissionStaticControlError::EvidenceInsufficient);
    }
    Ok(first)
}

pub(super) fn digest(domain: &[u8], bytes: &[u8]) -> Digest {
    super::super::contract::domain_separated_digest(domain, bytes)
}

fn read_bounded(
    mut input: impl std::io::Read,
    maximum: u64,
    cancellation: &CancellationToken,
) -> Result<Vec<u8>, RuntimeAdmissionStaticControlError> {
    let capacity =
        usize::try_from(maximum).map_err(|_| RuntimeAdmissionStaticControlError::LimitExceeded)?;
    let mut bytes = Vec::with_capacity(capacity.min(64 * 1_024));
    let mut limited = input.by_ref().take(maximum.saturating_add(1));
    limited
        .read_to_end(&mut bytes)
        .map_err(|_| RuntimeAdmissionStaticControlError::EvidenceInsufficient)?;
    if cancellation.is_cancelled() {
        return Err(RuntimeAdmissionStaticControlError::Evidence(
            RuntimeSourceBuildEvidenceBundleError::Cancelled,
        ));
    }
    let length = u64::try_from(bytes.len())
        .map_err(|_| RuntimeAdmissionStaticControlError::LimitExceeded)?;
    if length > maximum {
        return Err(RuntimeAdmissionStaticControlError::LimitExceeded);
    }
    Ok(bytes)
}
