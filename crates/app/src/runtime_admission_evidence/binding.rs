use rewrite_model::{ArtifactSetId, ArtifactSetRelativePath, RuntimePackageManifestId};
use rewrite_types::{CancellationToken, Digest};
use thiserror::Error;

use super::RuntimeAdmissionEvidenceFoundation;
use crate::{
    RUNTIME_SOURCE_BUILD_REPORT_PATH, RuntimeSourceBuildEvidenceBundleError,
    RuntimeSourceBuildEvidenceBundleLease,
};

/// A foundation whose complete subject binding was re-derived from one retained
/// durable source-build evidence lease.
///
/// This value is inert. It proves only that the foundation names the retained
/// source-build closure; it grants no review, admission, or policy authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedRuntimeAdmissionFoundationBinding {
    foundation_id: super::RuntimeAdmissionEvidenceFoundationId,
    pub(super) snapshot: RuntimeAdmissionFoundationSnapshot,
}

impl VerifiedRuntimeAdmissionFoundationBinding {
    #[cfg(test)]
    pub(crate) fn test_fixture(foundation: &RuntimeAdmissionEvidenceFoundation) -> Self {
        Self {
            foundation_id: foundation.foundation_id().clone(),
            snapshot: RuntimeAdmissionFoundationSnapshot {
                source_build_evidence_bundle_id: foundation
                    .source_build_evidence_bundle_id()
                    .clone(),
                source_build_inputs_id: foundation.source_build_inputs_id().clone(),
                source_manifest_digest: foundation.source_manifest_digest().clone(),
                build_plan_digest: foundation.build_plan_digest().clone(),
                source_report_digest: foundation.source_report_digest().clone(),
                runtime_package_manifest_id: foundation.runtime_package_manifest_id().clone(),
            },
        }
    }

    /// Revalidates a durable source-build closure and verifies every foundation
    /// subject against identities independently derived from that closure.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeAdmissionFoundationBindingError`] for durable evidence
    /// failure, a nonidentical build, or any mismatched foundation subject.
    pub fn verify(
        foundation: &RuntimeAdmissionEvidenceFoundation,
        evidence: &RuntimeSourceBuildEvidenceBundleLease,
        cancellation: &CancellationToken,
    ) -> Result<Self, RuntimeAdmissionFoundationBindingError> {
        verify_view(foundation, evidence, cancellation)
    }

    /// Returns the exact inert foundation identity that was verified.
    #[must_use]
    pub const fn foundation_id(&self) -> &super::RuntimeAdmissionEvidenceFoundationId {
        &self.foundation_id
    }

    /// Returns the exact runtime-package identity bound by the foundation.
    #[must_use]
    pub const fn runtime_package_manifest_id(&self) -> &RuntimePackageManifestId {
        &self.snapshot.runtime_package_manifest_id
    }

    /// Returns the exact reviewed source-build input-set identity.
    #[must_use]
    pub const fn source_build_inputs_id(&self) -> &ArtifactSetId {
        &self.snapshot.source_build_inputs_id
    }
}

/// Failure while binding an inert foundation to durable source-build evidence.
#[derive(Debug, Error)]
pub enum RuntimeAdmissionFoundationBindingError {
    /// The durable source-build closure failed revalidation.
    #[error(transparent)]
    Evidence(#[from] RuntimeSourceBuildEvidenceBundleError),
    /// The two controlled-build attempts did not produce one exact package.
    #[error("runtime admission foundation requires a byte-identical controlled build")]
    NonidenticalBuild,
    /// A foundation subject did not match the durable closure.
    #[error("runtime admission foundation does not bind the durable source-build closure")]
    InvalidBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RuntimeAdmissionFoundationSnapshot {
    pub(super) source_build_evidence_bundle_id: ArtifactSetId,
    pub(super) source_build_inputs_id: ArtifactSetId,
    pub(super) source_manifest_digest: Digest,
    pub(super) build_plan_digest: Digest,
    pub(super) source_report_digest: Digest,
    pub(super) runtime_package_manifest_id: RuntimePackageManifestId,
}

pub(super) trait RuntimeAdmissionFoundationEvidenceView {
    fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionFoundationBindingError>;

    fn snapshot(
        &self,
    ) -> Result<RuntimeAdmissionFoundationSnapshot, RuntimeAdmissionFoundationBindingError>;
}

pub(super) fn verify_view<V: RuntimeAdmissionFoundationEvidenceView + ?Sized>(
    foundation: &RuntimeAdmissionEvidenceFoundation,
    evidence: &V,
    cancellation: &CancellationToken,
) -> Result<VerifiedRuntimeAdmissionFoundationBinding, RuntimeAdmissionFoundationBindingError> {
    evidence.revalidate(cancellation)?;
    let snapshot = evidence.snapshot()?;
    if foundation.source_build_evidence_bundle_id() != &snapshot.source_build_evidence_bundle_id
        || foundation.source_build_inputs_id() != &snapshot.source_build_inputs_id
        || foundation.source_manifest_digest() != &snapshot.source_manifest_digest
        || foundation.build_plan_digest() != &snapshot.build_plan_digest
        || foundation.source_report_digest() != &snapshot.source_report_digest
        || foundation.runtime_package_manifest_id() != &snapshot.runtime_package_manifest_id
    {
        return Err(RuntimeAdmissionFoundationBindingError::InvalidBinding);
    }
    evidence.revalidate(cancellation)?;
    Ok(VerifiedRuntimeAdmissionFoundationBinding {
        foundation_id: foundation.foundation_id().clone(),
        snapshot,
    })
}

impl RuntimeAdmissionFoundationEvidenceView for RuntimeSourceBuildEvidenceBundleLease {
    fn revalidate(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionFoundationBindingError> {
        self.revalidate(cancellation).map_err(Into::into)
    }

    fn snapshot(
        &self,
    ) -> Result<RuntimeAdmissionFoundationSnapshot, RuntimeAdmissionFoundationBindingError> {
        if !self.report().is_byte_identical() {
            return Err(RuntimeAdmissionFoundationBindingError::NonidenticalBuild);
        }
        let report_path = ArtifactSetRelativePath::new(RUNTIME_SOURCE_BUILD_REPORT_PATH.to_owned())
            .map_err(|_| RuntimeAdmissionFoundationBindingError::InvalidBinding)?;
        let report_member = self
            .manifest()
            .members()
            .iter()
            .find(|member| member.relative_path() == &report_path)
            .ok_or(RuntimeAdmissionFoundationBindingError::InvalidBinding)?;
        let primary_id = self
            .report()
            .primary()
            .runtime_package()
            .runtime_package_manifest_id();
        let rebuild_id = self
            .report()
            .rebuild()
            .runtime_package()
            .runtime_package_manifest_id();
        if primary_id != rebuild_id {
            return Err(RuntimeAdmissionFoundationBindingError::NonidenticalBuild);
        }
        Ok(RuntimeAdmissionFoundationSnapshot {
            source_build_evidence_bundle_id: self.manifest().artifact_set_id(),
            source_build_inputs_id: self
                .source_inputs()
                .manifest()
                .artifact_set()
                .artifact_set_id(),
            source_manifest_digest: self.source_inputs().manifest().manifest_digest().clone(),
            build_plan_digest: self.plan().plan_digest().clone(),
            source_report_digest: report_member.artifact_id().digest().clone(),
            runtime_package_manifest_id: primary_id,
        })
    }
}
