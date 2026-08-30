use rewrite_model::{
    GenerationQualificationPlatformEvidenceV1Input, GenerationQualificationPlatformReasonV1,
    GenerationQualificationPlatformStatusV1, ObserverBinaryAssertionModeV1,
};

use super::{
    GenerationQualificationPlatformAssessmentError,
    GenerationQualificationPlatformPortableRelations,
};
use crate::VerifiedCurrentHostEnvironment;

pub(super) fn validate_current_host_static(
    portable: GenerationQualificationPlatformPortableRelations<'_>,
    current_host: &VerifiedCurrentHostEnvironment,
) -> Result<(), GenerationQualificationPlatformAssessmentError> {
    let system = portable
        .operation_policy_relations
        .target_system
        .generation_system;
    let host_digests = current_host.digest_set();
    if current_host.runtime_target() != portable.target()
        || host_digests.operating_system_digest().digest() != system.operating_system_digest()
        || host_digests.architecture_digest().digest() != system.architecture_digest()
    {
        return Err(GenerationQualificationPlatformAssessmentError::CurrentHostMismatch);
    }
    Ok(())
}

pub(super) fn current_host_input(
    portable: GenerationQualificationPlatformPortableRelations<'_>,
    current_host: &VerifiedCurrentHostEnvironment,
) -> GenerationQualificationPlatformEvidenceV1Input {
    let system = portable
        .operation_policy_relations
        .target_system
        .generation_system;
    let execution = current_host.environment().execution_class();
    let host_digests = current_host.digest_set();
    let reason = if execution.observer_binary_assertion_mode()
        != ObserverBinaryAssertionModeV1::Disabled
        || host_digests.execution_class_digest().digest() != system.execution_class_digest()
    {
        GenerationQualificationPlatformReasonV1::UnsupportedExecutionClass
    } else if host_digests.hardware_envelope_digest().digest() != system.hardware_envelope_digest()
    {
        GenerationQualificationPlatformReasonV1::UnsupportedHardwareEnvelope
    } else {
        GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu
    };
    let status = if reason == GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu
    {
        GenerationQualificationPlatformStatusV1::Supported
    } else {
        GenerationQualificationPlatformStatusV1::Rejected
    };
    GenerationQualificationPlatformEvidenceV1Input { status, reason }
}
