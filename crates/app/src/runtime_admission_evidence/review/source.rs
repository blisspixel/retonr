use super::super::binding::RuntimeAdmissionFoundationEvidenceView;
use rewrite_model::{ArtifactSetRelativePath, RuntimePackageManifest};
use rewrite_types::CancellationToken;
use std::io::Read;

use super::{ReviewSource, RuntimeAdmissionAllPassReviewError, input};
use crate::{
    RuntimeAdmissionLicenseControlVerifier, RuntimeAdmissionSourceLineageControlVerifier,
    RuntimeAdmissionTransformationControlVerifier,
};
use crate::{RuntimeSourceBuildEvidenceBundleLease, VerifiedRuntimeAdmissionFoundationBinding};

impl ReviewSource for RuntimeSourceBuildEvidenceBundleLease {
    fn validate(
        &self,
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionAllPassReviewError> {
        RuntimeAdmissionFoundationEvidenceView::revalidate(self, cancellation)
            .map_err(RuntimeAdmissionAllPassReviewError::Source)?;
        let snapshot = RuntimeAdmissionFoundationEvidenceView::snapshot(self)
            .map_err(RuntimeAdmissionAllPassReviewError::Source)?;
        if snapshot != foundation.snapshot {
            return Err(RuntimeAdmissionAllPassReviewError::InvalidBinding);
        }
        RuntimeAdmissionFoundationEvidenceView::revalidate(self, cancellation)
            .map_err(RuntimeAdmissionAllPassReviewError::Source)
    }

    fn verify_static(
        &self,
        material: &input::ReviewMaterial<'_>,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionAllPassReviewError> {
        let lineage = RuntimeAdmissionSourceLineageControlVerifier::verify(
            material.source_lineage_bytes,
            material.foundation,
            self,
            cancellation,
        )
        .map_err(RuntimeAdmissionAllPassReviewError::Static)?;
        let transformation = RuntimeAdmissionTransformationControlVerifier::verify(
            material.transformation_bytes,
            material.foundation,
            self,
            cancellation,
        )
        .map_err(RuntimeAdmissionAllPassReviewError::Static)?;
        let license = RuntimeAdmissionLicenseControlVerifier::verify(
            material.license_bytes,
            material.foundation,
            self,
            cancellation,
        )
        .map_err(RuntimeAdmissionAllPassReviewError::Static)?;
        if &lineage != material.source_lineage
            || &transformation != material.transformation
            || &license != material.license
        {
            return Err(RuntimeAdmissionAllPassReviewError::InvalidBinding);
        }
        Ok(())
    }

    fn package(&self) -> &RuntimePackageManifest {
        self.report().primary().runtime_package()
    }
    fn open_evidence(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::RuntimePackageReviewEvidenceOpenError> {
        RuntimeSourceBuildEvidenceBundleLease::open_evidence(self, path, cancellation)
            .map(|file| Box::new(file) as Box<dyn Read>)
            .map_err(|_| rewrite_ollama_package::RuntimePackageReviewEvidenceOpenError)
    }
    fn open_source(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::RuntimeSourceBuildInputOpenError> {
        self.open_source_input(path, cancellation)
            .map(|file| Box::new(file) as Box<dyn Read>)
            .map_err(|_| rewrite_ollama_package::RuntimeSourceBuildInputOpenError)
    }
    fn open_member(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::MemberOpenError> {
        self.open_primary_member(path, cancellation)
            .map(|file| Box::new(file) as Box<dyn Read>)
            .map_err(|_| rewrite_ollama_package::MemberOpenError)
    }
}
