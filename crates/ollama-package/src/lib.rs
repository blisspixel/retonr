//! Bounded offline reconstruction of admitted Ollama model and runtime packages.
//!
//! This crate reads caller-supplied bytes only. It does not discover installed
//! models or runtimes, open paths, access a registry, execute members, or grant
//! runtime authority.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod error;
mod gguf;
mod json;
mod manifest;
mod model_foundation;
mod reconstruct;
mod review;
mod review_v2;
mod runtime;
mod source_build;
mod source_build_report;

pub use error::{BlobOpenError, ReconstructionError, ReconstructionResult};
pub use gguf::{GgufComponentDigests, GgufLimits, GgufObservation, inspect_gguf_v3};
pub use manifest::{
    BlobDescriptor, CONFIG_MEDIA_TYPE, LICENSE_MEDIA_TYPE, MANIFEST_MEDIA_TYPE, MODEL_MEDIA_TYPE,
    OllamaManifestPlan, PARAMS_MEDIA_TYPE, ReconstructionLimits, TEMPLATE_MEDIA_TYPE,
    parse_manifest_v2,
};
pub use model_foundation::{
    OllamaLocalArchiveFoundationError, OllamaLocalArchiveFoundationEvidence,
    OllamaLocalArchiveMemberBinding, verify_ollama_local_archive_foundation,
    verify_ollama_local_archive_foundation_bindings,
};
pub use reconstruct::{
    ReconstructedModelPackage, RootfsDescriptorComparison, ollama_logical_binding_digest,
    reconstruct_model_package, reconstruct_model_package_with_limits,
};
pub use review::{
    RUNTIME_PACKAGE_REVIEW_SCHEMA_VERSION, RuntimePackageReview, RuntimePackageReviewCheck,
    RuntimePackageReviewCheckStatus, RuntimePackageReviewDisposition, RuntimePackageReviewError,
};
pub use review_v2::{
    CompiledRuntimePackageReviewV2, RUNTIME_PACKAGE_REVIEW_V2_SCHEMA_VERSION,
    RuntimePackageReviewDispositionV2, RuntimePackageReviewEvidenceClass,
    RuntimePackageReviewEvidenceOpenError, RuntimePackageReviewV2,
    RuntimePackageReviewV2CheckInput, RuntimePackageReviewV2CompilationInput,
    RuntimePackageReviewV2Error, RuntimePackageReviewV2EvidenceInput, RuntimePackageReviewV2Limits,
    VerifiedRuntimePackageReviewV2, compile_runtime_package_review_v2,
    verify_runtime_package_review_v2,
};
pub use runtime::{
    ADMITTED_RUNTIME_FAMILY, MemberOpenError, RUNTIME_LAYOUT_SCHEMA_VERSION,
    ReconstructedRuntimePackage, RuntimeLayoutLimits, RuntimePackageLayout,
    RuntimePackageLayoutMember, RuntimeReconstructionError, RuntimeReconstructionResult,
    reconstruct_runtime_package, reconstruct_runtime_package_with_limits,
};
#[cfg(feature = "retained-program-closure")]
pub use source_build::{
    CARGO_SOURCE_CLOSURE_PROCEDURE_ID, CARGO_SOURCE_CLOSURE_PROCEDURE_VERSION,
    CargoSourceClosureError, CargoSourceClosureLimits, CargoSourceClosureReviewerFacts,
    RETAINED_PROGRAM_LICENSE_EVIDENCE_PROCEDURE_ID,
    RETAINED_PROGRAM_LICENSE_EVIDENCE_PROCEDURE_VERSION, RetainedProgramLicenseEvidenceClosureId,
    RetainedProgramLicenseEvidenceError, RetainedProgramLicenseEvidenceLimits,
    RetainedProgramLicenseEvidenceReviewerFacts, RetainedProgramUpstreamClosureError,
    RetainedProgramUpstreamClosureId, VerifiedAlpineReleaseUpstream, VerifiedCargoSourceClosure,
    VerifiedRetainedProgramLicenseEvidenceClosure, VerifiedRetainedProgramUpstreamClosure,
    VerifiedRustReleaseUpstream, verify_cargo_source_closure,
    verify_retained_program_license_evidence_closure, verify_retained_program_upstream_closure,
};
pub use source_build::{
    RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_ID, RETAINED_PROGRAM_BUILD_RECIPE_PROCEDURE_VERSION,
    RETAINED_PROGRAM_LINEAGE_PROCEDURE_ID, RETAINED_PROGRAM_LINEAGE_PROCEDURE_VERSION,
    RUNTIME_SOURCE_BUILD_INPUT_SCHEMA_VERSION, RetainedProgramBuildRecipeId,
    RetainedProgramLineageId, RuntimeSourceBuildAcceleratorPolicy,
    RuntimeSourceBuildEnvironmentVariable, RuntimeSourceBuildExecutionPolicy,
    RuntimeSourceBuildInputComponent, RuntimeSourceBuildInputError, RuntimeSourceBuildInputLimits,
    RuntimeSourceBuildInputManifest, RuntimeSourceBuildInputOpenError, RuntimeSourceBuildInputRole,
    RuntimeSourceBuildNetworkPolicy, RuntimeSourceBuildPlan, RuntimeSourceBuildPolicy,
    SelfContainedLinuxExecutable, VerifiedRetainedProgramLineage, VerifiedRuntimeSourceBuildInputs,
    verify_runtime_source_build_inputs,
};
pub use source_build_report::{
    CompiledRuntimeSourceBuildReport, RUNTIME_SOURCE_BUILD_OUTPUT_TREE_SCHEMA_VERSION,
    RUNTIME_SOURCE_BUILD_REPORT_SCHEMA_VERSION, RuntimeSourceBuildAttempt,
    RuntimeSourceBuildComparison, RuntimeSourceBuildEvidenceKind, RuntimeSourceBuildEvidenceRecord,
    RuntimeSourceBuildOutputTree, RuntimeSourceBuildOutputTreeEntry,
    RuntimeSourceBuildOutputTreeEntryKind, RuntimeSourceBuildReport, RuntimeSourceBuildReportError,
    RuntimeSourceBuildReportLimits, RuntimeSourceBuildReportOpenError,
    VerifiedRuntimeSourceBuildExecutionReceipt, VerifiedRuntimeSourceBuildReport,
    compile_runtime_source_build_report, verify_runtime_source_build_execution_receipt,
    verify_runtime_source_build_report,
};
