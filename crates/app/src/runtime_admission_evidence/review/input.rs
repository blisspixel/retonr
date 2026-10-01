use rewrite_model::{ArtifactSetRelativePath, RuntimePackageManifest};
use rewrite_types::CancellationToken;
use std::io::{Cursor, Read};

use rewrite_ollama_package::{
    RuntimePackageReviewCheck as Check, RuntimePackageReviewCheckStatus as Status,
    RuntimePackageReviewEvidenceClass as Class, RuntimePackageReviewV2CheckInput,
    RuntimePackageReviewV2CompilationInput, RuntimePackageReviewV2EvidenceInput,
};

use super::super::{
    MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES, RuntimeAdmissionEvidenceMember as Member,
};
use super::{
    ReviewSource, RuntimeAdmissionAllPassReviewError, RuntimeAdmissionAllPassReviewRequest,
};
use crate::{RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH, RUNTIME_SOURCE_BUILD_REPORT_PATH};
use crate::{
    RuntimeAdmissionFinalOperation, VerifiedPassedRuntimeAdmissionLicenseControl,
    VerifiedPassedRuntimeAdmissionSourceLineageControl,
    VerifiedPassedRuntimeAdmissionTransformationControl, VerifiedRuntimeAdmissionFoundationBinding,
};
use rewrite_ollama_package::VerifiedRuntimePackageReviewV2;

const PROVENANCE: &str = "attempts/primary/provenance.json";
const LAYOUT: &str = "attempts/primary/runtime-layout.json";

pub(super) struct ReviewMaterial<'a> {
    pub(super) foundation: &'a VerifiedRuntimeAdmissionFoundationBinding,
    pub(super) source_lineage_bytes: &'a [u8],
    pub(super) source_lineage: &'a VerifiedPassedRuntimeAdmissionSourceLineageControl,
    pub(super) transformation_bytes: &'a [u8],
    pub(super) transformation: &'a VerifiedPassedRuntimeAdmissionTransformationControl,
    pub(super) license_bytes: &'a [u8],
    pub(super) license: &'a VerifiedPassedRuntimeAdmissionLicenseControl,
    pub(super) execution: &'a RuntimeAdmissionFinalOperation,
}

impl<'a> ReviewMaterial<'a> {
    pub(super) fn from_request(request: &RuntimeAdmissionAllPassReviewRequest<'a>) -> Self {
        Self {
            foundation: request.foundation,
            source_lineage_bytes: request.source_lineage_bytes,
            source_lineage: request.source_lineage,
            transformation_bytes: request.transformation_bytes,
            transformation: request.transformation,
            license_bytes: request.license_bytes,
            license: request.license,
            execution: request.execution,
        }
    }

    pub(super) fn validate_limits(&self) -> Result<(), RuntimeAdmissionAllPassReviewError> {
        if [
            self.source_lineage_bytes,
            self.transformation_bytes,
            self.license_bytes,
        ]
        .into_iter()
        .any(|bytes| {
            bytes.is_empty() || bytes.len() > MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES
        }) || self.execution.native_load_record().canonical_bytes().len() as u64
            > Member::NativeLoadExecution.maximum_bytes()
            || self
                .execution
                .managed_final_record()
                .canonical_bytes()
                .len() as u64
                > Member::ManagedFinalExecution.maximum_bytes()
        {
            return Err(RuntimeAdmissionAllPassReviewError::InvalidBinding);
        }
        Ok(())
    }

    pub(super) fn verify_authorities(
        &self,
        package: &RuntimePackageManifest,
    ) -> Result<(), RuntimeAdmissionAllPassReviewError> {
        if [
            self.source_lineage.foundation_id(),
            self.transformation.foundation_id(),
            self.license.foundation_id(),
        ]
        .into_iter()
        .any(|id| id != self.foundation.foundation_id())
        {
            return Err(RuntimeAdmissionAllPassReviewError::InvalidBinding);
        }
        self.execution
            .verify_records_for_review(self.foundation, package)
            .map_err(RuntimeAdmissionAllPassReviewError::Execution)
    }

    pub(super) fn open_evidence(
        &self,
        source: &impl ReviewSource,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::RuntimePackageReviewEvidenceOpenError> {
        let bytes = if path.as_str() == Member::SourceLineageControl.relative_path() {
            Some(self.source_lineage_bytes)
        } else if path.as_str() == Member::TransformationControl.relative_path() {
            Some(self.transformation_bytes)
        } else if path.as_str() == Member::LicenseControl.relative_path() {
            Some(self.license_bytes)
        } else if path.as_str() == Member::NativeLoadExecution.relative_path() {
            Some(self.execution.native_load_record().canonical_bytes())
        } else if path.as_str() == Member::ManagedFinalExecution.relative_path() {
            Some(self.execution.managed_final_record().canonical_bytes())
        } else {
            None
        };
        if let Some(bytes) = bytes {
            Ok(Box::new(Cursor::new(bytes.to_vec())))
        } else {
            source.open_evidence(path, cancellation)
        }
    }
}

pub(super) fn compilation_input() -> Result<
    RuntimePackageReviewV2CompilationInput,
    rewrite_ollama_package::RuntimePackageReviewV2Error,
> {
    let declarations = [
        (PROVENANCE, Class::BuildTool),
        (LAYOUT, Class::BuildOutput),
        (Member::LicenseControl.relative_path(), Class::FetchedInput),
        (
            Member::SourceLineageControl.relative_path(),
            Class::FetchedInput,
        ),
        (
            Member::TransformationControl.relative_path(),
            Class::BuildOutput,
        ),
        (
            Member::ManagedFinalExecution.relative_path(),
            Class::Execution,
        ),
        (
            Member::NativeLoadExecution.relative_path(),
            Class::Execution,
        ),
        (
            RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH,
            Class::FetchedInput,
        ),
        (RUNTIME_SOURCE_BUILD_REPORT_PATH, Class::BuildOutput),
    ];
    let evidence = declarations
        .into_iter()
        .map(|(name, class)| Ok(RuntimePackageReviewV2EvidenceInput::new(class, path(name)?)))
        .collect::<Result<Vec<_>, rewrite_ollama_package::RuntimePackageReviewV2Error>>()?;
    let checks = [
        (
            Check::SourceLineage,
            vec![
                PROVENANCE,
                Member::SourceLineageControl.relative_path(),
                RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH,
            ],
        ),
        (
            Check::Transformation,
            vec![
                PROVENANCE,
                LAYOUT,
                Member::TransformationControl.relative_path(),
                RUNTIME_SOURCE_BUILD_REPORT_PATH,
            ],
        ),
        (
            Check::License,
            vec![
                Member::LicenseControl.relative_path(),
                RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH,
            ],
        ),
        (
            Check::NativeClosure,
            vec![LAYOUT, Member::NativeLoadExecution.relative_path()],
        ),
        (
            Check::ManagedStartup,
            vec![
                Member::ManagedFinalExecution.relative_path(),
                Member::NativeLoadExecution.relative_path(),
            ],
        ),
        (
            Check::CloudDisable,
            vec![Member::ManagedFinalExecution.relative_path()],
        ),
    ]
    .into_iter()
    .map(|(check, names)| {
        let paths = names.into_iter().map(path).collect::<Result<Vec<_>, _>>()?;
        Ok(RuntimePackageReviewV2CheckInput::new(
            check,
            Status::Passed,
            paths,
        ))
    })
    .collect::<Result<Vec<_>, rewrite_ollama_package::RuntimePackageReviewV2Error>>()?;
    Ok(RuntimePackageReviewV2CompilationInput::new(
        path(RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH)?,
        path(LAYOUT)?,
        evidence,
        checks,
    ))
}

pub(super) fn validate_review(
    review: &VerifiedRuntimePackageReviewV2,
    foundation: &VerifiedRuntimeAdmissionFoundationBinding,
    package: &RuntimePackageManifest,
) -> Result<(), RuntimeAdmissionAllPassReviewError> {
    let expected = package.runtime_package_manifest_id();
    let reconstructed = review
        .reconstructed_runtime()
        .ok_or(RuntimeAdmissionAllPassReviewError::InvalidBinding)?;
    if foundation.runtime_package_manifest_id() != &expected
        || reconstructed.runtime_package() != package
        || &review
            .source_build_inputs()
            .artifact_set()
            .artifact_set_id()
            != foundation.source_build_inputs_id()
        || !matches!(review.review().disposition(), rewrite_ollama_package::RuntimePackageReviewDispositionV2::Admitted {
            runtime_package_manifest_id, ..
        } if runtime_package_manifest_id == &expected)
        || [
            Check::SourceLineage,
            Check::Transformation,
            Check::License,
            Check::NativeClosure,
            Check::ManagedStartup,
            Check::CloudDisable,
        ]
        .into_iter()
        .any(|check| review.review().check_status(check) != Status::Passed)
    {
        return Err(RuntimeAdmissionAllPassReviewError::InvalidBinding);
    }
    Ok(())
}

fn path(
    value: &str,
) -> Result<ArtifactSetRelativePath, rewrite_ollama_package::RuntimePackageReviewV2Error> {
    ArtifactSetRelativePath::new(value.to_owned())
        .map_err(|_| rewrite_ollama_package::RuntimePackageReviewV2Error::InvalidEvidence)
}
