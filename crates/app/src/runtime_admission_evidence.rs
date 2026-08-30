//! Inert contracts for a future durable runtime-admission evidence closure.
//!
//! These types bind subjects and plan a bounded tree. They confer no admission
//! or policy authority and intentionally provide no publication API.

mod binding;
mod contract;
mod foundation;
mod static_control;
mod tree_plan;

pub use contract::{
    MAX_RUNTIME_ADMISSION_EVIDENCE_BUNDLE_BYTES,
    MAX_RUNTIME_ADMISSION_EVIDENCE_DESTINATION_ENTRIES,
    MAX_RUNTIME_ADMISSION_EVIDENCE_STAGING_ROOTS, MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_ENTRIES,
    RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH, RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_PATH,
    RuntimeAdmissionEvidenceBundleDestination, RuntimeAdmissionEvidenceBundleLimits,
    RuntimeAdmissionEvidenceBundleSource, RuntimeAdmissionEvidenceContractError,
};
pub use foundation::{
    MAX_RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_JSON_BYTES,
    RUNTIME_ADMISSION_EVIDENCE_FOUNDATION_SCHEMA_VERSION, RuntimeAdmissionEvidenceFoundation,
    RuntimeAdmissionEvidenceFoundationId, RuntimeAdmissionEvidenceFoundationInput,
};
pub use static_control::{
    CompiledRuntimeAdmissionLicenseControl, CompiledRuntimeAdmissionSourceLineageControl,
    CompiledRuntimeAdmissionTransformationControl, MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES,
    MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES, RUNTIME_ADMISSION_LICENSE_PROCEDURE_ID,
    RUNTIME_ADMISSION_LICENSE_PROCEDURE_VERSION, RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_ID,
    RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_VERSION,
    RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_ID,
    RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_VERSION, RuntimeAdmissionLicenseControlCompiler,
    RuntimeAdmissionLicenseControlId, RuntimeAdmissionLicenseControlVerifier,
    RuntimeAdmissionSourceLineageControlCompiler, RuntimeAdmissionSourceLineageControlId,
    RuntimeAdmissionSourceLineageControlVerifier, RuntimeAdmissionStaticControlError,
    RuntimeAdmissionTransformationControlCompiler, RuntimeAdmissionTransformationControlId,
    RuntimeAdmissionTransformationControlVerifier, VerifiedPassedRuntimeAdmissionLicenseControl,
    VerifiedPassedRuntimeAdmissionSourceLineageControl,
    VerifiedPassedRuntimeAdmissionTransformationControl,
};
pub use tree_plan::{
    MAX_RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_JSON_BYTES, RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT,
    RUNTIME_ADMISSION_EVIDENCE_TREE_PLAN_SCHEMA_VERSION, RuntimeAdmissionEvidenceMember,
    RuntimeAdmissionEvidencePlannedMember, RuntimeAdmissionEvidenceTreePlan,
    RuntimeAdmissionEvidenceTreePlanId,
};

#[cfg(test)]
mod tests;
pub use binding::{
    RuntimeAdmissionFoundationBindingError, VerifiedRuntimeAdmissionFoundationBinding,
};
