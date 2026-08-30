//! Inert, byte-bound static review controls for a verified foundation.

mod common;
mod license;
mod lineage;
mod transformation;

pub use common::{
    MAX_RUNTIME_ADMISSION_STATIC_CONTROL_JSON_BYTES,
    MAX_RUNTIME_ADMISSION_STATIC_REVIEW_JSON_BYTES, RuntimeAdmissionStaticControlError,
};
pub use license::{
    CompiledRuntimeAdmissionLicenseControl, RUNTIME_ADMISSION_LICENSE_PROCEDURE_ID,
    RUNTIME_ADMISSION_LICENSE_PROCEDURE_VERSION, RuntimeAdmissionLicenseControlCompiler,
    RuntimeAdmissionLicenseControlId, RuntimeAdmissionLicenseControlVerifier,
    VerifiedPassedRuntimeAdmissionLicenseControl,
};
pub use lineage::{
    CompiledRuntimeAdmissionSourceLineageControl, RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_ID,
    RUNTIME_ADMISSION_SOURCE_LINEAGE_PROCEDURE_VERSION,
    RuntimeAdmissionSourceLineageControlCompiler, RuntimeAdmissionSourceLineageControlId,
    RuntimeAdmissionSourceLineageControlVerifier,
    VerifiedPassedRuntimeAdmissionSourceLineageControl,
};
pub use transformation::{
    CompiledRuntimeAdmissionTransformationControl, RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_ID,
    RUNTIME_ADMISSION_TRANSFORMATION_PROCEDURE_VERSION,
    RuntimeAdmissionTransformationControlCompiler, RuntimeAdmissionTransformationControlId,
    RuntimeAdmissionTransformationControlVerifier,
    VerifiedPassedRuntimeAdmissionTransformationControl,
};

#[cfg(test)]
mod tests;
