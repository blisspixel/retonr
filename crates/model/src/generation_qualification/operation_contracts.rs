//! Portable preregistration and terminal evidence for one qualification operation.

mod common;
mod error;
mod ids;
mod license_evidence;
mod operation_policy;
mod operation_receipt;
mod phase_interruption;
mod platform_evidence;
mod request_projection;

pub use common::{
    GenerationQualificationLicensePermissionV1, MAX_GENERATION_QUALIFICATION_POLICY_BYTES,
};
pub use error::GenerationQualificationOperationContractError;
pub use ids::{
    GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_POLICY_ID_DOMAIN,
    GENERATION_QUALIFICATION_LICENSE_EVIDENCE_ID_DOMAIN,
    GENERATION_QUALIFICATION_OPERATION_POLICY_ID_DOMAIN,
    GENERATION_QUALIFICATION_OPERATION_RECEIPT_ID_DOMAIN,
    GENERATION_QUALIFICATION_PHASE_INTERRUPTION_RECORD_ID_DOMAIN,
    GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_POLICY_ID_DOMAIN,
    GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_ID_DOMAIN,
    GENERATION_QUALIFICATION_REQUEST_PROJECTION_ID_DOMAIN,
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicenseEvidenceId,
    GenerationQualificationOperationPolicyId, GenerationQualificationOperationReceiptId,
    GenerationQualificationPhaseInterruptionRecordId,
    GenerationQualificationPlatformAssessmentPolicyId, GenerationQualificationPlatformEvidenceId,
    GenerationQualificationRequestProjectionId,
};
pub use license_evidence::*;
pub use operation_policy::*;
pub use operation_receipt::*;
pub use phase_interruption::*;
pub use platform_evidence::*;
pub use request_projection::*;

#[cfg(test)]
pub(crate) use operation_policy::test_support as operation_policy_test_support;
