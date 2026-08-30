use rewrite_model::{
    GenerationQualificationLicenseAssessmentPolicyId, GenerationQualificationLicensePermissionV1,
    GenerationQualificationPlatformAssessmentPolicyId, ModelLicenseControlId, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_types::Digest;

use super::wire::{
    AuthorityWire, LicenseDecisionRuleWire, LicensePermissionWire, LicensePolicyKindWire,
    LicensePolicyWire, PlatformDecisionRuleWire, PlatformPolicyKindWire, PlatformPolicyWire,
    PlatformProfileWire, parse_license, parse_platform,
};
use super::{
    GENERATION_QUALIFICATION_ASSESSMENT_POLICY_SCHEMA_VERSION,
    GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_PROCEDURE_ID,
    GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_PROCEDURE_VERSION,
    GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_PROCEDURE_ID,
    GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_PROCEDURE_VERSION,
    GenerationQualificationAssessmentPolicyError,
    GenerationQualificationPlatformAssessmentPolicyV1Bindings,
    ProductionGenerationQualificationLicenseAssessmentPolicySource,
    ProductionGenerationQualificationPlatformAssessmentPolicySource,
    VerifiedGenerationQualificationLicenseAssessmentPolicy,
    VerifiedGenerationQualificationPlatformAssessmentPolicy,
};

pub(super) fn verify_platform(
    policy_json: &[u8],
    expected_bindings: &GenerationQualificationPlatformAssessmentPolicyV1Bindings,
    source: &ProductionGenerationQualificationPlatformAssessmentPolicySource,
) -> Result<
    VerifiedGenerationQualificationPlatformAssessmentPolicy,
    GenerationQualificationAssessmentPolicyError,
> {
    let observed = parse_platform(policy_json)?;
    validate_platform_digest_encodings(&observed)?;
    let reviewed_target = reviewed_runtime_target();
    if expected_bindings.runtime_target != reviewed_target
        || observed != expected_platform_policy(expected_bindings, reviewed_target)
    {
        return Err(GenerationQualificationAssessmentPolicyError::InvalidPlatformBinding);
    }
    let policy_id =
        GenerationQualificationPlatformAssessmentPolicyId::from_canonical_policy_bytes(policy_json)
            .map_err(|_error| GenerationQualificationAssessmentPolicyError::InvalidEncoding)?;
    let source_disposition = source.disposition(&policy_id);
    Ok(VerifiedGenerationQualificationPlatformAssessmentPolicy {
        policy_id,
        source_disposition,
        bindings: expected_bindings.clone(),
    })
}

pub(super) fn verify_license(
    policy_json: &[u8],
    expected_control_id: &ModelLicenseControlId,
    source: &ProductionGenerationQualificationLicenseAssessmentPolicySource,
) -> Result<
    VerifiedGenerationQualificationLicenseAssessmentPolicy,
    GenerationQualificationAssessmentPolicyError,
> {
    let observed = parse_license(policy_json)?;
    validate_digest(&observed.model_license_control_id)?;
    if observed != expected_license_policy(expected_control_id) {
        return Err(GenerationQualificationAssessmentPolicyError::InvalidLicenseBinding);
    }
    let policy_id =
        GenerationQualificationLicenseAssessmentPolicyId::from_canonical_policy_bytes(policy_json)
            .map_err(|_error| GenerationQualificationAssessmentPolicyError::InvalidEncoding)?;
    let source_disposition = source.disposition(&policy_id);
    Ok(VerifiedGenerationQualificationLicenseAssessmentPolicy {
        policy_id,
        source_disposition,
        model_license_control_id: expected_control_id.clone(),
        permission: GenerationQualificationLicensePermissionV1::LocalGeneration,
    })
}

fn expected_platform_policy(
    bindings: &GenerationQualificationPlatformAssessmentPolicyV1Bindings,
    reviewed_target: RuntimeTarget,
) -> PlatformPolicyWire {
    PlatformPolicyWire {
        authority: AuthorityWire::None,
        policy: PlatformPolicyKindWire::GenerationQualificationPlatformAssessment,
        decision_rule: PlatformDecisionRuleWire::ExactReviewedProfileAndCurrentHost,
        procedure_id: GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_PROCEDURE_ID.to_owned(),
        procedure_version: GENERATION_QUALIFICATION_PLATFORM_ASSESSMENT_PROCEDURE_VERSION,
        profile: PlatformProfileWire::ManagedLinuxNativeCpu,
        runtime_target: reviewed_target,
        operating_system_digest: bindings.operating_system_digest.as_str().to_owned(),
        architecture_digest: bindings.architecture_digest.as_str().to_owned(),
        execution_class_digest: bindings.execution_class_digest.as_str().to_owned(),
        hardware_envelope_digest: bindings.hardware_envelope_digest.as_str().to_owned(),
        schema_version: GENERATION_QUALIFICATION_ASSESSMENT_POLICY_SCHEMA_VERSION,
    }
}

fn expected_license_policy(expected_control_id: &ModelLicenseControlId) -> LicensePolicyWire {
    LicensePolicyWire {
        authority: AuthorityWire::None,
        policy: LicensePolicyKindWire::GenerationQualificationLicenseAssessment,
        decision_rule: LicenseDecisionRuleWire::ExactControlAndProductionApproval,
        model_license_control_id: expected_control_id.digest().as_str().to_owned(),
        permission: LicensePermissionWire::LocalGeneration,
        procedure_id: GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_PROCEDURE_ID.to_owned(),
        procedure_version: GENERATION_QUALIFICATION_LICENSE_ASSESSMENT_PROCEDURE_VERSION,
        schema_version: GENERATION_QUALIFICATION_ASSESSMENT_POLICY_SCHEMA_VERSION,
    }
}

fn reviewed_runtime_target() -> RuntimeTarget {
    RuntimeTarget::new(
        RuntimeOperatingSystem::Linux,
        RuntimeArchitecture::X86_64,
        RuntimeAbi::LinuxGnuLibc,
    )
    .expect("fixed reviewed platform target must remain valid")
}

fn validate_platform_digest_encodings(
    policy: &PlatformPolicyWire,
) -> Result<(), GenerationQualificationAssessmentPolicyError> {
    validate_digest(&policy.operating_system_digest)?;
    validate_digest(&policy.architecture_digest)?;
    validate_digest(&policy.execution_class_digest)?;
    validate_digest(&policy.hardware_envelope_digest)
}

fn validate_digest(value: &str) -> Result<(), GenerationQualificationAssessmentPolicyError> {
    Digest::from_sha256_hex(value)
        .map(|_digest| ())
        .map_err(|_error| GenerationQualificationAssessmentPolicyError::InvalidEncoding)
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn platform_policy_json_for_test(
    bindings: &GenerationQualificationPlatformAssessmentPolicyV1Bindings,
) -> Vec<u8> {
    super::wire::encode_platform(&expected_platform_policy(
        bindings,
        reviewed_runtime_target(),
    ))
    .expect("bounded exact platform policy")
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn license_policy_json_for_test(control_id: &ModelLicenseControlId) -> Vec<u8> {
    super::wire::encode_license(&expected_license_policy(control_id))
        .expect("bounded exact license policy")
}
