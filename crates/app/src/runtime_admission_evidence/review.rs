use std::{fmt, io::Read};

use rewrite_model::{ArtifactSetRelativePath, RuntimePackageManifest};
use rewrite_ollama_package::{
    RuntimePackageReviewV2Limits, VerifiedRuntimePackageReviewV2,
    compile_runtime_package_review_v2, verify_runtime_package_review_v2,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use super::{
    RuntimeAdmissionFoundationBindingError, RuntimeAdmissionStaticControlError,
    VerifiedPassedRuntimeAdmissionLicenseControl,
    VerifiedPassedRuntimeAdmissionSourceLineageControl,
    VerifiedPassedRuntimeAdmissionTransformationControl, VerifiedRuntimeAdmissionFoundationBinding,
};
use crate::{
    RuntimeAdmissionFinalOperation, RuntimeAdmissionRunnerError,
    RuntimeSourceBuildEvidenceBundleLease,
};

mod input;
mod source;
#[cfg(test)]
mod tests;

/// Exact retained material and opaque passed controls for an inert all-pass review.
///
/// No caller supplies a review status. The static bytes are independently verified
/// again and the final operation must already have completed mandatory cleanup.
pub struct RuntimeAdmissionAllPassReviewRequest<'a> {
    /// Foundation independently bound to the durable source-build closure.
    pub foundation: &'a VerifiedRuntimeAdmissionFoundationBinding,
    /// Retained byte-identical controlled build and its exact source inputs.
    pub source: &'a RuntimeSourceBuildEvidenceBundleLease,
    /// Exact canonical source-lineage publication bytes.
    pub source_lineage_bytes: &'a [u8],
    /// Independently verified passed source-lineage control.
    pub source_lineage: &'a VerifiedPassedRuntimeAdmissionSourceLineageControl,
    /// Exact canonical transformation publication bytes.
    pub transformation_bytes: &'a [u8],
    /// Independently verified passed transformation control.
    pub transformation: &'a VerifiedPassedRuntimeAdmissionTransformationControl,
    /// Exact canonical license publication bytes.
    pub license_bytes: &'a [u8],
    /// Independently verified passed license control.
    pub license: &'a VerifiedPassedRuntimeAdmissionLicenseControl,
    /// Cleanup-gated canonical execution records and three passed live controls.
    pub execution: &'a RuntimeAdmissionFinalOperation,
    /// Caller-selected schema-2 ceilings, bounded by that contract's hard limits.
    pub limits: RuntimePackageReviewV2Limits,
}

/// Canonical all-pass review material with independent structural readback.
///
/// This material grants no production policy, admitted-runtime capability, model
/// qualification, or generation authority.
pub struct CompiledRuntimeAdmissionAllPassReview {
    canonical_bytes: Vec<u8>,
    verified: VerifiedRuntimePackageReviewV2,
}

impl CompiledRuntimeAdmissionAllPassReview {
    /// Returns exact canonical schema-2 publication bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the independently verified inert structural review.
    #[must_use]
    pub const fn verified_review(&self) -> &VerifiedRuntimePackageReviewV2 {
        &self.verified
    }
}

/// Failure to compose or freshly revalidate an inert all-pass review.
#[derive(Error)]
pub enum RuntimeAdmissionAllPassReviewError {
    /// Original caller cancellation prevents releasing review material.
    #[error("runtime admission review compilation was cancelled")]
    Cancelled,
    /// An exact foundation, control, source, package, or record join disagreed.
    #[error("runtime admission review material has an invalid subject binding")]
    InvalidBinding,
    /// Mandatory source/foundation validation failed.
    #[error("runtime admission review source validation failed")]
    Source(#[source] RuntimeAdmissionFoundationBindingError),
    /// Independent verification of selected static control bytes failed.
    #[error("runtime admission review static control validation failed")]
    Static(#[source] RuntimeAdmissionStaticControlError),
    /// Canonical execution record reparse or authority comparison failed.
    #[error("runtime admission review execution validation failed")]
    Execution(#[source] RuntimeAdmissionRunnerError),
    /// Schema-2 compilation, reconstruction, or independent verification failed.
    #[error("runtime admission canonical review validation failed")]
    Review(#[source] rewrite_ollama_package::RuntimePackageReviewV2Error),
    /// Independent primary and fresh terminal source validation both failed.
    #[error("runtime admission review primary and terminal validation failed")]
    FinalizationAfterFailure {
        /// Original compilation failure.
        primary: Box<Self>,
        /// Independently attempted fresh terminal validation failure.
        finalization: Box<Self>,
    },
}

impl fmt::Debug for RuntimeAdmissionAllPassReviewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("Cancelled"),
            Self::InvalidBinding => f.write_str("InvalidBinding"),
            Self::Source(_) => f.write_str("SourceValidation"),
            Self::Static(_) => f.write_str("StaticControlValidation"),
            Self::Execution(_) => f.write_str("ExecutionValidation"),
            Self::Review(_) => f.write_str("ReviewValidation"),
            Self::FinalizationAfterFailure {
                primary,
                finalization,
            } => f
                .debug_struct("FinalizationAfterFailure")
                .field("primary", primary)
                .field("finalization", finalization)
                .finish(),
        }
    }
}

/// Compiler deriving all six passed statuses from existing opaque authorities.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeAdmissionAllPassReviewCompiler;

impl RuntimeAdmissionAllPassReviewCompiler {
    /// Compiles canonical material, independently reparses it, and always performs
    /// fresh terminal source validation before sampling original cancellation.
    ///
    /// # Errors
    /// Returns [`RuntimeAdmissionAllPassReviewError`] for invalid controls or
    /// material, source drift, cancellation, limits, or final validation failures.
    pub fn compile(
        request: &RuntimeAdmissionAllPassReviewRequest<'_>,
        cancellation: &CancellationToken,
    ) -> Result<CompiledRuntimeAdmissionAllPassReview, RuntimeAdmissionAllPassReviewError> {
        let material = input::ReviewMaterial::from_request(request);
        compile_view(&material, request.source, request.limits, cancellation)
    }
}

trait ReviewSource {
    fn validate(
        &self,
        foundation: &VerifiedRuntimeAdmissionFoundationBinding,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionAllPassReviewError>;
    fn verify_static(
        &self,
        material: &input::ReviewMaterial<'_>,
        cancellation: &CancellationToken,
    ) -> Result<(), RuntimeAdmissionAllPassReviewError>;
    fn package(&self) -> &RuntimePackageManifest;
    fn open_evidence(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::RuntimePackageReviewEvidenceOpenError>;
    fn open_source(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::RuntimeSourceBuildInputOpenError>;
    fn open_member(
        &self,
        path: &ArtifactSetRelativePath,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn Read>, rewrite_ollama_package::MemberOpenError>;
}

fn compile_view(
    material: &input::ReviewMaterial<'_>,
    source: &impl ReviewSource,
    limits: RuntimePackageReviewV2Limits,
    cancellation: &CancellationToken,
) -> Result<CompiledRuntimeAdmissionAllPassReview, RuntimeAdmissionAllPassReviewError> {
    let primary = compile_primary(material, source, limits, cancellation);
    let terminal = source.validate(material.foundation, &CancellationToken::new());
    match (primary, terminal) {
        (Err(primary), Err(finalization)) => Err(
            RuntimeAdmissionAllPassReviewError::FinalizationAfterFailure {
                primary: Box::new(primary),
                finalization: Box::new(finalization),
            },
        ),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(_), Err(finalization)) => Err(finalization),
        (Ok(result), Ok(())) if !cancellation.is_cancelled() => Ok(result),
        (Ok(_), Ok(())) => Err(RuntimeAdmissionAllPassReviewError::Cancelled),
    }
}

fn compile_primary(
    material: &input::ReviewMaterial<'_>,
    source: &impl ReviewSource,
    limits: RuntimePackageReviewV2Limits,
    cancellation: &CancellationToken,
) -> Result<CompiledRuntimeAdmissionAllPassReview, RuntimeAdmissionAllPassReviewError> {
    if cancellation.is_cancelled() {
        return Err(RuntimeAdmissionAllPassReviewError::Cancelled);
    }
    source.validate(material.foundation, cancellation)?;
    material.validate_limits()?;
    source.verify_static(material, cancellation)?;
    material.verify_authorities(source.package())?;
    let input = input::compilation_input().map_err(RuntimeAdmissionAllPassReviewError::Review)?;
    let compiled = compile_runtime_package_review_v2(
        &input,
        &limits,
        |path| material.open_evidence(source, path, cancellation),
        |path| source.open_source(path, cancellation),
        |path| source.open_member(path, cancellation),
        || cancellation.is_cancelled(),
    )
    .map_err(RuntimeAdmissionAllPassReviewError::Review)?;
    source.validate(material.foundation, cancellation)?;
    let verified = verify_runtime_package_review_v2(
        compiled.canonical_bytes(),
        &limits,
        |path| material.open_evidence(source, path, cancellation),
        |path| source.open_source(path, cancellation),
        |path| source.open_member(path, cancellation),
        || cancellation.is_cancelled(),
    )
    .map_err(RuntimeAdmissionAllPassReviewError::Review)?;
    if compiled.verified() != &verified {
        return Err(RuntimeAdmissionAllPassReviewError::InvalidBinding);
    }
    input::validate_review(&verified, material.foundation, source.package())?;
    Ok(CompiledRuntimeAdmissionAllPassReview {
        canonical_bytes: compiled.canonical_bytes().to_vec(),
        verified,
    })
}
