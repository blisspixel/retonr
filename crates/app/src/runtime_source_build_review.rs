use rewrite_model::ArtifactSetRelativePath;
use rewrite_ollama_package::{
    MemberOpenError, RuntimePackageReviewCheck, RuntimePackageReviewCheckStatus,
    RuntimePackageReviewEvidenceClass, RuntimePackageReviewEvidenceOpenError,
    RuntimePackageReviewV2CheckInput, RuntimePackageReviewV2CompilationInput,
    RuntimePackageReviewV2Error, RuntimePackageReviewV2EvidenceInput, RuntimePackageReviewV2Limits,
    RuntimeSourceBuildInputOpenError, VerifiedRuntimePackageReviewV2,
    compile_runtime_package_review_v2, verify_runtime_package_review_v2,
};
use rewrite_types::CancellationToken;
use thiserror::Error;

use crate::{
    RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH, RUNTIME_SOURCE_BUILD_REPORT_PATH,
    RuntimeSourceBuildEvidenceBundleError, RuntimeSourceBuildEvidenceBundleLease,
};

const EXECUTION_RECEIPT_PATH: &str = "attempts/primary/execution-receipt.json";
const PROVENANCE_PATH: &str = "attempts/primary/provenance.json";
const RUNTIME_LAYOUT_PATH: &str = "attempts/primary/runtime-layout.json";
const SBOM_PATH: &str = "attempts/primary/sbom.json";
const TRANSFORMATION_PATH: &str = "attempts/primary/transformation.json";

/// Canonical blocked schema-2 review derived from a durable controlled build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSourceBuildReviewCompilation {
    canonical_review_bytes: Vec<u8>,
    verified: VerifiedRuntimePackageReviewV2,
}

impl RuntimeSourceBuildReviewCompilation {
    /// Returns the canonical content-free schema-2 review bytes.
    #[must_use]
    pub fn canonical_review_bytes(&self) -> &[u8] {
        &self.canonical_review_bytes
    }

    /// Returns the independently verified blocked review.
    #[must_use]
    pub const fn verified(&self) -> &VerifiedRuntimePackageReviewV2 {
        &self.verified
    }
}

/// Failure while compiling or independently verifying a build-stage review.
#[derive(Debug, Error)]
pub enum RuntimeSourceBuildReviewCompilationError {
    /// The durable controlled-build evidence boundary failed revalidation.
    #[error(transparent)]
    Evidence(#[from] RuntimeSourceBuildEvidenceBundleError),
    /// The schema-2 review contract rejected the selected evidence or result.
    #[error(transparent)]
    Review(#[from] RuntimePackageReviewV2Error),
    /// Compilation and independent verification derived different typed results.
    #[error("compiled and independently verified runtime-package reviews differ")]
    VerificationMismatch,
}

/// Deterministic compiler for the review justified by controlled-build evidence.
#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeSourceBuildReviewCompiler;

impl RuntimeSourceBuildReviewCompiler {
    /// Compiles and independently verifies a blocked schema-2 review.
    ///
    /// The durable build retains evidence relevant to all six controls, but this
    /// structural compiler does not decide any semantic review outcome. Every
    /// control remains `not_run`, so this operation cannot grant runtime admission
    /// authority or be reused as a partial approval.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeSourceBuildReviewCompilationError`] for cancellation,
    /// durable evidence drift, unavailable bytes, invalid review limits, a review
    /// contract failure, or disagreement between compilation and verification.
    pub fn compile_build_stage(
        evidence: &RuntimeSourceBuildEvidenceBundleLease,
        limits: &RuntimePackageReviewV2Limits,
        cancellation: &CancellationToken,
    ) -> Result<RuntimeSourceBuildReviewCompilation, RuntimeSourceBuildReviewCompilationError> {
        evidence.revalidate(cancellation)?;
        let input = build_stage_input()?;
        let compiled = compile_runtime_package_review_v2(
            &input,
            limits,
            |path| {
                evidence
                    .open_evidence(path, cancellation)
                    .map_err(|_| RuntimePackageReviewEvidenceOpenError)
            },
            |path| {
                evidence
                    .open_source_input(path, cancellation)
                    .map_err(|_| RuntimeSourceBuildInputOpenError)
            },
            |path| {
                evidence
                    .open_primary_member(path, cancellation)
                    .map_err(|_| MemberOpenError)
            },
            || cancellation.is_cancelled(),
        )?;
        evidence.revalidate(cancellation)?;
        let verified = verify_runtime_package_review_v2(
            compiled.canonical_bytes(),
            limits,
            |path| {
                evidence
                    .open_evidence(path, cancellation)
                    .map_err(|_| RuntimePackageReviewEvidenceOpenError)
            },
            |path| {
                evidence
                    .open_source_input(path, cancellation)
                    .map_err(|_| RuntimeSourceBuildInputOpenError)
            },
            |path| {
                evidence
                    .open_primary_member(path, cancellation)
                    .map_err(|_| MemberOpenError)
            },
            || cancellation.is_cancelled(),
        )?;
        evidence.revalidate(cancellation)?;
        if compiled.verified() != &verified {
            return Err(RuntimeSourceBuildReviewCompilationError::VerificationMismatch);
        }
        Ok(RuntimeSourceBuildReviewCompilation {
            canonical_review_bytes: compiled.canonical_bytes().to_vec(),
            verified,
        })
    }
}

fn build_stage_input() -> Result<RuntimePackageReviewV2CompilationInput, RuntimePackageReviewV2Error>
{
    let execution_receipt = path(EXECUTION_RECEIPT_PATH)?;
    let provenance = path(PROVENANCE_PATH)?;
    let runtime_layout = path(RUNTIME_LAYOUT_PATH)?;
    let sbom = path(SBOM_PATH)?;
    let source_inputs = path(RUNTIME_SOURCE_BUILD_INPUT_MANIFEST_PATH)?;
    let source_report = path(RUNTIME_SOURCE_BUILD_REPORT_PATH)?;
    let transformation = path(TRANSFORMATION_PATH)?;
    let evidence = vec![
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::Execution,
            execution_receipt.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::BuildTool,
            provenance.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::BuildOutput,
            runtime_layout.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::BuildOutput,
            sbom.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::BuildOutput,
            transformation.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::FetchedInput,
            source_inputs.clone(),
        ),
        RuntimePackageReviewV2EvidenceInput::new(
            RuntimePackageReviewEvidenceClass::BuildOutput,
            source_report.clone(),
        ),
    ];
    let checks = vec![
        check(
            RuntimePackageReviewCheck::SourceLineage,
            RuntimePackageReviewCheckStatus::NotRun,
            vec![
                provenance.clone(),
                source_inputs.clone(),
                source_report.clone(),
            ],
        ),
        check(
            RuntimePackageReviewCheck::Transformation,
            RuntimePackageReviewCheckStatus::NotRun,
            vec![
                provenance,
                runtime_layout.clone(),
                transformation,
                source_report,
            ],
        ),
        check(
            RuntimePackageReviewCheck::License,
            RuntimePackageReviewCheckStatus::NotRun,
            vec![sbom, source_inputs.clone()],
        ),
        check(
            RuntimePackageReviewCheck::NativeClosure,
            RuntimePackageReviewCheckStatus::NotRun,
            vec![execution_receipt.clone(), runtime_layout.clone()],
        ),
        check(
            RuntimePackageReviewCheck::ManagedStartup,
            RuntimePackageReviewCheckStatus::NotRun,
            vec![execution_receipt.clone()],
        ),
        check(
            RuntimePackageReviewCheck::CloudDisable,
            RuntimePackageReviewCheckStatus::NotRun,
            vec![execution_receipt],
        ),
    ];
    Ok(RuntimePackageReviewV2CompilationInput::new(
        source_inputs,
        runtime_layout,
        evidence,
        checks,
    ))
}

fn check(
    check: RuntimePackageReviewCheck,
    status: RuntimePackageReviewCheckStatus,
    evidence: Vec<ArtifactSetRelativePath>,
) -> RuntimePackageReviewV2CheckInput {
    RuntimePackageReviewV2CheckInput::new(check, status, evidence)
}

fn path(value: &str) -> Result<ArtifactSetRelativePath, RuntimePackageReviewV2Error> {
    ArtifactSetRelativePath::new(value.to_owned())
        .map_err(|_| RuntimePackageReviewV2Error::InvalidEvidence)
}
