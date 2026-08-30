use super::*;

fn replace_once(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    let text = std::str::from_utf8(bytes).expect("policy JSON is UTF-8");
    assert_eq!(text.matches(from).count(), 1, "unique replacement source");
    text.replacen(from, to, 1).into_bytes()
}

fn assert_platform_error(
    bytes: &[u8],
    bindings: &GenerationQualificationPlatformAssessmentPolicyV1Bindings,
    expected: GenerationQualificationAssessmentPolicyError,
) {
    assert_eq!(
        GenerationQualificationAssessmentPolicyVerifier::verify_platform(
            bytes,
            bindings,
            &ProductionGenerationQualificationPlatformAssessmentPolicySource::new(),
        )
        .expect_err("substituted platform policy must fail"),
        expected
    );
}

fn assert_license_error(
    bytes: &[u8],
    control_id: &ModelLicenseControlId,
    expected: GenerationQualificationAssessmentPolicyError,
) {
    assert_eq!(
        GenerationQualificationAssessmentPolicyVerifier::verify_license(
            bytes,
            control_id,
            &ProductionGenerationQualificationLicenseAssessmentPolicySource::new(),
        )
        .expect_err("substituted license policy must fail"),
        expected
    );
}

#[test]
fn empty_malformed_unknown_and_oversized_inputs_are_rejected() {
    let bindings = platform_bindings();
    let control_id = license_control_id("input limits control");
    for malformed in [b"".as_slice(), b"{".as_slice(), b"null".as_slice()] {
        assert_platform_error(
            malformed,
            &bindings,
            GenerationQualificationAssessmentPolicyError::InvalidEncoding,
        );
        assert_license_error(
            malformed,
            &control_id,
            GenerationQualificationAssessmentPolicyError::InvalidEncoding,
        );
    }

    let mut platform_unknown = platform_policy_json_for_test(&bindings);
    platform_unknown.pop();
    platform_unknown.extend_from_slice(b",\"unknown\":true}");
    assert_platform_error(
        &platform_unknown,
        &bindings,
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
    );
    let mut license_unknown = license_policy_json_for_test(&control_id);
    license_unknown.pop();
    license_unknown.extend_from_slice(b",\"unknown\":true}");
    assert_license_error(
        &license_unknown,
        &control_id,
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
    );

    let platform = platform_policy_json_for_test(&bindings);
    let platform_duplicate = replace_once(
        &platform,
        "{\"authority\":\"none\"",
        "{\"authority\":\"none\",\"authority\":\"none\"",
    );
    assert_platform_error(
        &platform_duplicate,
        &bindings,
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
    );
    let platform_missing = replace_once(&platform, "\"authority\":\"none\",", "");
    assert_platform_error(
        &platform_missing,
        &bindings,
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
    );

    let license = license_policy_json_for_test(&control_id);
    let license_duplicate = replace_once(
        &license,
        "{\"authority\":\"none\"",
        "{\"authority\":\"none\",\"authority\":\"none\"",
    );
    assert_license_error(
        &license_duplicate,
        &control_id,
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
    );
    let license_missing = replace_once(&license, "\"authority\":\"none\",", "");
    assert_license_error(
        &license_missing,
        &control_id,
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
    );

    let oversized = vec![b' '; MAX_GENERATION_QUALIFICATION_ASSESSMENT_POLICY_JSON_BYTES + 1];
    assert_platform_error(
        &oversized,
        &bindings,
        GenerationQualificationAssessmentPolicyError::LimitExceeded,
    );
    assert_license_error(
        &oversized,
        &control_id,
        GenerationQualificationAssessmentPolicyError::LimitExceeded,
    );
}

#[test]
fn valid_but_noncanonical_encodings_are_rejected() {
    let bindings = platform_bindings();
    let platform = platform_policy_json_for_test(&bindings);
    let mut platform_whitespace = platform.clone();
    platform_whitespace.push(b'\n');
    assert_platform_error(
        &platform_whitespace,
        &bindings,
        GenerationQualificationAssessmentPolicyError::NonCanonicalEncoding,
    );
    let platform_reordered = replace_once(
        &platform,
        "{\"authority\":\"none\",\"policy\":\"generation_qualification_platform_assessment\"",
        "{\"policy\":\"generation_qualification_platform_assessment\",\"authority\":\"none\"",
    );
    assert_platform_error(
        &platform_reordered,
        &bindings,
        GenerationQualificationAssessmentPolicyError::NonCanonicalEncoding,
    );

    let control_id = license_control_id("noncanonical control");
    let license = license_policy_json_for_test(&control_id);
    let mut license_whitespace = license.clone();
    license_whitespace.push(b' ');
    assert_license_error(
        &license_whitespace,
        &control_id,
        GenerationQualificationAssessmentPolicyError::NonCanonicalEncoding,
    );
    let license_reordered = replace_once(
        &license,
        "{\"authority\":\"none\",\"policy\":\"generation_qualification_license_assessment\"",
        "{\"policy\":\"generation_qualification_license_assessment\",\"authority\":\"none\"",
    );
    assert_license_error(
        &license_reordered,
        &control_id,
        GenerationQualificationAssessmentPolicyError::NonCanonicalEncoding,
    );
}

#[test]
fn every_platform_field_is_closed_or_exactly_bound() {
    let bindings = platform_bindings();
    let policy = platform_policy_json_for_test(&bindings);
    let invalid_enums = [
        ("\"authority\":\"none\"", "\"authority\":\"caller\""),
        (
            "\"policy\":\"generation_qualification_platform_assessment\"",
            "\"policy\":\"other\"",
        ),
        (
            "\"decision_rule\":\"exact_reviewed_profile_and_current_host\"",
            "\"decision_rule\":\"caller_claim\"",
        ),
        (
            "\"profile\":\"managed_linux_native_cpu\"",
            "\"profile\":\"other\"",
        ),
    ];
    for (from, to) in invalid_enums {
        assert_platform_error(
            &replace_once(&policy, from, to),
            &bindings,
            GenerationQualificationAssessmentPolicyError::InvalidEncoding,
        );
    }

    let binding_substitutions = [
        (
            "generation-qualification-platform-assessment:procedure",
            "generation-qualification-platform-assessment:foreign",
        ),
        ("\"procedure_version\":1", "\"procedure_version\":2"),
        (
            "\"architecture\":\"x86_64\"",
            "\"architecture\":\"aarch64\"",
        ),
        ("\"abi\":\"linux_gnu_libc\"", "\"abi\":\"linux_musl\""),
        ("\"schema_version\":1", "\"schema_version\":2"),
    ];
    for (from, to) in binding_substitutions {
        assert_platform_error(
            &replace_once(&policy, from, to),
            &bindings,
            GenerationQualificationAssessmentPolicyError::InvalidPlatformBinding,
        );
    }
    let windows = replace_once(
        &replace_once(
            &policy,
            "\"operating_system\":\"linux\"",
            "\"operating_system\":\"windows\"",
        ),
        "\"abi\":\"linux_gnu_libc\"",
        "\"abi\":\"windows_msvc\"",
    );
    assert_platform_error(
        &windows,
        &bindings,
        GenerationQualificationAssessmentPolicyError::InvalidPlatformBinding,
    );

    for original in [
        bindings.operating_system_digest.as_str(),
        bindings.architecture_digest.as_str(),
        bindings.execution_class_digest.as_str(),
        bindings.hardware_envelope_digest.as_str(),
    ] {
        let foreign = digest(&format!("foreign {original}"));
        assert_platform_error(
            &replace_once(&policy, original, foreign.as_str()),
            &bindings,
            GenerationQualificationAssessmentPolicyError::InvalidPlatformBinding,
        );
    }
    assert_platform_error(
        &replace_once(
            &policy,
            bindings.operating_system_digest.as_str(),
            "not-a-digest",
        ),
        &bindings,
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
    );
}

#[test]
fn platform_expected_binding_and_source_substitution_cannot_approve() {
    let bindings = platform_bindings();
    let policy = platform_policy_json_for_test(&bindings);
    let mut substituted_expected = platform_bindings();
    substituted_expected.hardware_envelope_digest = digest("substituted expected hardware");
    assert_platform_error(
        &policy,
        &substituted_expected,
        GenerationQualificationAssessmentPolicyError::InvalidPlatformBinding,
    );
    substituted_expected = platform_bindings();
    substituted_expected.runtime_target = RuntimeTarget::new(
        RuntimeOperatingSystem::Linux,
        RuntimeArchitecture::Aarch64,
        RuntimeAbi::LinuxGnuLibc,
    )
    .expect("valid foreign target");
    assert_platform_error(
        &policy,
        &substituted_expected,
        GenerationQualificationAssessmentPolicyError::InvalidPlatformBinding,
    );

    let other_bindings = GenerationQualificationPlatformAssessmentPolicyV1Bindings {
        hardware_envelope_digest: digest("other approved policy"),
        ..platform_bindings()
    };
    let other_json = platform_policy_json_for_test(&other_bindings);
    let other_denied = GenerationQualificationAssessmentPolicyVerifier::verify_platform(
        &other_json,
        &other_bindings,
        &ProductionGenerationQualificationPlatformAssessmentPolicySource::new(),
    )
    .expect("other policy structure");
    let other_source =
        ProductionGenerationQualificationPlatformAssessmentPolicySource::exact_test_source(
            other_denied.policy_id().clone(),
        );
    let observed = GenerationQualificationAssessmentPolicyVerifier::verify_platform(
        &policy,
        &bindings,
        &other_source,
    )
    .expect("original policy structure");
    assert_eq!(
        observed.source_disposition(),
        GenerationQualificationAssessmentPolicySourceDisposition::Denied
    );
}

#[test]
fn every_license_field_is_closed_or_exactly_bound() {
    let control_id = license_control_id("license field control");
    let policy = license_policy_json_for_test(&control_id);
    let invalid_enums = [
        ("\"authority\":\"none\"", "\"authority\":\"caller\""),
        (
            "\"policy\":\"generation_qualification_license_assessment\"",
            "\"policy\":\"other\"",
        ),
        (
            "\"decision_rule\":\"exact_control_and_production_approval\"",
            "\"decision_rule\":\"caller_claim\"",
        ),
        (
            "\"permission\":\"local_generation\"",
            "\"permission\":\"redistribution\"",
        ),
    ];
    for (from, to) in invalid_enums {
        assert_license_error(
            &replace_once(&policy, from, to),
            &control_id,
            GenerationQualificationAssessmentPolicyError::InvalidEncoding,
        );
    }

    let foreign_control = license_control_id("foreign license field control");
    let binding_substitutions = [
        (
            control_id.digest().as_str(),
            foreign_control.digest().as_str(),
        ),
        (
            "generation-qualification-license-assessment:procedure",
            "generation-qualification-license-assessment:foreign",
        ),
        ("\"procedure_version\":1", "\"procedure_version\":2"),
        ("\"schema_version\":1", "\"schema_version\":2"),
    ];
    for (from, to) in binding_substitutions {
        assert_license_error(
            &replace_once(&policy, from, to),
            &control_id,
            GenerationQualificationAssessmentPolicyError::InvalidLicenseBinding,
        );
    }
    assert_license_error(
        &replace_once(&policy, control_id.digest().as_str(), "not-a-digest"),
        &control_id,
        GenerationQualificationAssessmentPolicyError::InvalidEncoding,
    );
}

#[test]
fn license_expected_control_and_source_substitution_cannot_approve() {
    let control_id = license_control_id("expected license control");
    let policy = license_policy_json_for_test(&control_id);
    let foreign_control = license_control_id("foreign expected license control");
    assert_license_error(
        &policy,
        &foreign_control,
        GenerationQualificationAssessmentPolicyError::InvalidLicenseBinding,
    );

    let foreign_json = license_policy_json_for_test(&foreign_control);
    let foreign_denied = GenerationQualificationAssessmentPolicyVerifier::verify_license(
        &foreign_json,
        &foreign_control,
        &ProductionGenerationQualificationLicenseAssessmentPolicySource::new(),
    )
    .expect("foreign policy structure");
    let foreign_source =
        ProductionGenerationQualificationLicenseAssessmentPolicySource::exact_test_source(
            foreign_denied.policy_id().clone(),
        );
    let observed = GenerationQualificationAssessmentPolicyVerifier::verify_license(
        &policy,
        &control_id,
        &foreign_source,
    )
    .expect("original policy structure");
    assert_eq!(
        observed.source_disposition(),
        GenerationQualificationAssessmentPolicySourceDisposition::Denied
    );
}
