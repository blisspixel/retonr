use rewrite_model::{
    GenerationQualificationLicensePermissionV1, ModelLicenseControlId, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem, RuntimeTarget,
};
use rewrite_types::Digest;

use super::verification::{license_policy_json_for_test, platform_policy_json_for_test};
use super::*;

#[path = "tests/substitutions.rs"]
mod substitutions;

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn platform_bindings() -> GenerationQualificationPlatformAssessmentPolicyV1Bindings {
    GenerationQualificationPlatformAssessmentPolicyV1Bindings {
        runtime_target: RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("reviewed target"),
        operating_system_digest: digest("linux operating system"),
        architecture_digest: digest("x86_64 architecture"),
        execution_class_digest: digest("native cpu execution"),
        hardware_envelope_digest: digest("reviewed hardware envelope"),
    }
}

fn license_control_id(label: &str) -> ModelLicenseControlId {
    ModelLicenseControlId::from_derived_digest(digest(label))
}

#[test]
fn exact_platform_vector_is_stable_and_production_fails_closed() {
    let bindings = platform_bindings();
    let json = platform_policy_json_for_test(&bindings);
    let expected = format!(
        concat!(
            "{{\"authority\":\"none\",",
            "\"policy\":\"generation_qualification_platform_assessment\",",
            "\"decision_rule\":\"exact_reviewed_profile_and_current_host\",",
            "\"procedure_id\":\"retonr:generation-qualification-platform-assessment:procedure\",",
            "\"procedure_version\":1,",
            "\"profile\":\"managed_linux_native_cpu\",",
            "\"runtime_target\":{{\"operating_system\":\"linux\",",
            "\"architecture\":\"x86_64\",\"abi\":\"linux_gnu_libc\"}},",
            "\"operating_system_digest\":\"{}\",",
            "\"architecture_digest\":\"{}\",",
            "\"execution_class_digest\":\"{}\",",
            "\"hardware_envelope_digest\":\"{}\",",
            "\"schema_version\":1}}"
        ),
        bindings.operating_system_digest,
        bindings.architecture_digest,
        bindings.execution_class_digest,
        bindings.hardware_envelope_digest,
    );
    assert_eq!(json, expected.as_bytes());

    let production = ProductionGenerationQualificationPlatformAssessmentPolicySource::new();
    assert_eq!(production.approved_policy_count(), 0);
    let denied = GenerationQualificationAssessmentPolicyVerifier::verify_platform(
        &json,
        &bindings,
        &production,
    )
    .expect("canonical policy remains structurally verifiable when source-denied");
    assert_eq!(
        denied.source_disposition(),
        GenerationQualificationAssessmentPolicySourceDisposition::Denied
    );
    assert!(denied.bindings() == &bindings);
    assert_eq!(
        denied.policy_id().digest().as_str(),
        "b67a326d64abf0fa1a3d8da5cd0e2d71205aa861ec3ac31995976cd68d4f7f7f"
    );

    let approved_source =
        ProductionGenerationQualificationPlatformAssessmentPolicySource::exact_test_source(
            denied.policy_id().clone(),
        );
    let approved = GenerationQualificationAssessmentPolicyVerifier::verify_platform(
        &json,
        &bindings,
        &approved_source,
    )
    .expect("exact test source approves exact platform policy");
    assert_eq!(approved_source.approved_policy_count(), 1);
    assert_eq!(
        approved.source_disposition(),
        GenerationQualificationAssessmentPolicySourceDisposition::Approved
    );
    assert_eq!(approved.policy_id(), denied.policy_id());
}

#[test]
fn exact_license_vector_is_stable_and_production_fails_closed() {
    let control_id = license_control_id("reviewed model license control");
    let json = license_policy_json_for_test(&control_id);
    let expected = format!(
        concat!(
            "{{\"authority\":\"none\",",
            "\"policy\":\"generation_qualification_license_assessment\",",
            "\"decision_rule\":\"exact_control_and_production_approval\",",
            "\"model_license_control_id\":\"{}\",",
            "\"permission\":\"local_generation\",",
            "\"procedure_id\":\"retonr:generation-qualification-license-assessment:procedure\",",
            "\"procedure_version\":1,\"schema_version\":1}}"
        ),
        control_id.digest(),
    );
    assert_eq!(json, expected.as_bytes());

    let production = ProductionGenerationQualificationLicenseAssessmentPolicySource::new();
    assert_eq!(production.approved_policy_count(), 0);
    let denied = GenerationQualificationAssessmentPolicyVerifier::verify_license(
        &json,
        &control_id,
        &production,
    )
    .expect("canonical policy remains structurally verifiable when source-denied");
    assert_eq!(
        denied.source_disposition(),
        GenerationQualificationAssessmentPolicySourceDisposition::Denied
    );
    assert_eq!(denied.model_license_control_id(), &control_id);
    assert_eq!(
        denied.permission(),
        GenerationQualificationLicensePermissionV1::LocalGeneration
    );
    assert_eq!(
        denied.policy_id().digest().as_str(),
        "a06d5a31f17d10ab4cf03e59cc46ef21d1735a4d2e925f3c2bf1a606cf0ab02a"
    );

    let approved_source =
        ProductionGenerationQualificationLicenseAssessmentPolicySource::exact_test_source(
            denied.policy_id().clone(),
        );
    let approved = GenerationQualificationAssessmentPolicyVerifier::verify_license(
        &json,
        &control_id,
        &approved_source,
    )
    .expect("exact test source approves exact license policy");
    assert_eq!(approved_source.approved_policy_count(), 1);
    assert_eq!(
        approved.source_disposition(),
        GenerationQualificationAssessmentPolicySourceDisposition::Approved
    );
    assert_eq!(approved.policy_id(), denied.policy_id());
}

#[test]
fn debug_and_errors_are_redacted() {
    let bindings = platform_bindings();
    let platform_json = platform_policy_json_for_test(&bindings);
    let platform = GenerationQualificationAssessmentPolicyVerifier::verify_platform(
        &platform_json,
        &bindings,
        &ProductionGenerationQualificationPlatformAssessmentPolicySource::default(),
    )
    .expect("platform policy");
    let control_id = license_control_id("redaction control");
    let license_json = license_policy_json_for_test(&control_id);
    let license = GenerationQualificationAssessmentPolicyVerifier::verify_license(
        &license_json,
        &control_id,
        &ProductionGenerationQualificationLicenseAssessmentPolicySource::default(),
    )
    .expect("license policy");

    let platform_debug = format!("{platform:?}");
    assert!(platform_debug.contains("source_disposition: Denied"));
    assert!(!platform_debug.contains(platform.policy_id().digest().as_str()));
    assert!(!platform_debug.contains(bindings.hardware_envelope_digest.as_str()));
    let license_debug = format!("{license:?}");
    assert!(license_debug.contains("source_disposition: Denied"));
    assert!(!license_debug.contains(license.policy_id().digest().as_str()));
    assert!(!license_debug.contains(control_id.digest().as_str()));

    let platform_source_debug = format!(
        "{:?}",
        ProductionGenerationQualificationPlatformAssessmentPolicySource::exact_test_source(
            platform.policy_id().clone(),
        )
    );
    assert!(platform_source_debug.contains("approved_policy_count: 1"));
    assert!(!platform_source_debug.contains(platform.policy_id().digest().as_str()));
    let license_source_debug = format!(
        "{:?}",
        ProductionGenerationQualificationLicenseAssessmentPolicySource::exact_test_source(
            license.policy_id().clone(),
        )
    );
    assert!(license_source_debug.contains("approved_policy_count: 1"));
    assert!(!license_source_debug.contains(license.policy_id().digest().as_str()));

    for error in [
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
        GenerationQualificationAssessmentPolicyError::NonCanonicalEncoding,
        GenerationQualificationAssessmentPolicyError::LimitExceeded,
        GenerationQualificationAssessmentPolicyError::InvalidPlatformBinding,
        GenerationQualificationAssessmentPolicyError::InvalidLicenseBinding,
    ] {
        assert!(!error.to_string().contains(control_id.digest().as_str()));
        assert!(!format!("{error:?}").contains(control_id.digest().as_str()));
    }
}
