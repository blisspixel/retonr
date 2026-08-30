use rewrite_model::{
    ComputeBackend, ExecutionPlacement, GenerationQualificationPlatformReasonV1 as Reason,
    GenerationQualificationPlatformStatusV1 as Status, HostAcceleratorScopeV1,
    HostArchitectureV1Input, HostEnvironmentV1Input, HostExecutionClassV1Input,
    HostExecutionProfileV1, HostHardwareEnvelopeV1Input, HostOperatingSystemV1Input,
    ObserverBinaryAssertionModeV1, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
    RuntimeTarget,
};
use rewrite_types::CancellationToken;

use super::*;
use crate::candidate_attempt_precursor::tests::support::frozen_with_label;

#[path = "tests/support.rs"]
mod support;

use support::{
    AssessmentFixture, foreign_generation_policy, foreign_platform_policy, reviewed_target,
};

fn authorities<'a>(
    fixture: &'a AssessmentFixture,
    policy: &'a VerifiedGenerationQualificationPlatformAssessmentPolicy,
) -> GenerationQualificationReviewedPlatformAuthorities<'a> {
    GenerationQualificationReviewedPlatformAuthorities {
        admitted_runtime: &fixture.fixture.runtime.admitted,
        generation_path: &fixture.fixture.runtime.path,
        frozen_components: &fixture.fixture.runtime.frozen,
        generation_system_policy: &fixture.qualification.generation_policy,
        assessment_policy: policy,
    }
}

fn host_input(assertion_mode: ObserverBinaryAssertionModeV1) -> HostEnvironmentV1Input {
    HostEnvironmentV1Input {
        operating_system: HostOperatingSystemV1Input {
            family: RuntimeOperatingSystem::Linux,
            version: "6.12.10-platform-test".to_owned(),
        },
        architecture: HostArchitectureV1Input {
            instruction_set: RuntimeArchitecture::X86_64,
            abi: RuntimeAbi::LinuxGnuLibc,
        },
        execution_class: HostExecutionClassV1Input {
            profile: HostExecutionProfileV1::ManagedLinuxNativeCpu,
            compute_backend: ComputeBackend::NativeCpu,
            placement: ExecutionPlacement::CpuOnly,
            observer_binary_assertion_mode: assertion_mode,
            accelerator_scope: HostAcceleratorScopeV1::NotAssessedForManagedNativeCpu,
        },
        hardware_envelope: HostHardwareEnvelopeV1Input {
            cpu_model: "Platform Test CPU".to_owned(),
            physical_core_count: 4,
            logical_core_count: 8,
            total_system_memory_mib: 16_384,
            memory_rounding_granularity_mib: 1_024,
        },
    }
}

fn current_host(input: HostEnvironmentV1Input) -> VerifiedCurrentHostEnvironment {
    VerifiedCurrentHostEnvironment::exact_test_fixture(input).expect("exact current host")
}

#[test]
fn intrinsic_tuple_rejections_are_deterministic_and_precede_managed_owners() {
    let cases = [
        (
            RuntimeTarget::new(
                RuntimeOperatingSystem::Windows,
                RuntimeArchitecture::Aarch64,
                RuntimeAbi::WindowsMsvc,
            )
            .expect("Windows target"),
            Reason::UnsupportedOperatingSystem,
        ),
        (
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::Aarch64,
                RuntimeAbi::LinuxMusl,
            )
            .expect("Linux Arm target"),
            Reason::UnsupportedArchitecture,
        ),
        (
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::X86_64,
                RuntimeAbi::LinuxMusl,
            )
            .expect("Linux musl target"),
            Reason::UnsupportedAbi,
        ),
    ];
    for (target, expected_reason) in cases {
        let fixture = AssessmentFixture::new(target);
        let assessment =
            GenerationQualificationPlatformAssessmentCompiler::assess_intrinsic(fixture.portable())
                .expect("intrinsic assessment");
        assert_eq!(assessment.evidence().status(), Status::Rejected);
        assert_eq!(assessment.evidence().reason(), expected_reason);
        assessment
            .revalidate(fixture.portable(), &CancellationToken::new())
            .expect("intrinsic assessment revalidates");
    }
}

#[test]
fn denied_policy_has_a_distinct_path_that_acquires_no_current_host() {
    let fixture = AssessmentFixture::new(reviewed_target());
    let assessment = GenerationQualificationPlatformAssessmentCompiler::assess_policy_denied(
        fixture.portable(),
        authorities(&fixture, &fixture.denied_platform_policy),
        &CancellationToken::new(),
    )
    .expect("policy denial derives negative evidence without a host authority");
    assert_eq!(assessment.evidence().status(), Status::Rejected);
    assert_eq!(
        assessment.evidence().reason(),
        Reason::AssessmentPolicyDenied
    );
    assessment
        .revalidate(fixture.portable(), &CancellationToken::new())
        .expect("policy denial revalidates its retained owners");
    assert_eq!(
        GenerationQualificationPlatformAssessmentCompiler::assess_policy_denied(
            fixture.portable(),
            authorities(&fixture, &fixture.approved_platform_policy),
            &CancellationToken::new(),
        )
        .expect_err("approved policy cannot use denial path"),
        GenerationQualificationPlatformAssessmentError::AssessmentPolicyDispositionMismatch
    );
}

#[test]
fn exact_release_host_and_policy_support_the_reviewed_profile() {
    let host = current_host(host_input(ObserverBinaryAssertionModeV1::Disabled));
    let fixture = AssessmentFixture::with_current_host(&host);
    let assessment = GenerationQualificationPlatformAssessmentCompiler::assess_reviewed(
        fixture.portable(),
        authorities(&fixture, &fixture.approved_platform_policy),
        &host,
        &CancellationToken::new(),
    )
    .expect("exact retained current host");
    assert_eq!(assessment.evidence().status(), Status::Supported);
    assert_eq!(
        assessment.evidence().reason(),
        Reason::ReviewedManagedLinuxNativeCpu
    );
    assessment
        .revalidate(fixture.portable(), &CancellationToken::new())
        .expect("supported assessment re-observes its retained host");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        assessment
            .revalidate(fixture.portable(), &cancellation)
            .expect_err("cancelled revalidation fails closed"),
        GenerationQualificationPlatformAssessmentError::Cancelled
    );
    let debug = format!("{assessment:?}");
    assert!(debug.contains("VerifiedGenerationQualificationPlatformAssessment"));
    assert!(!debug.contains("Platform Test CPU"));
}

#[test]
fn execution_rejection_precedes_hardware_and_debug_cannot_support() {
    let release_host = current_host(host_input(ObserverBinaryAssertionModeV1::Disabled));
    let fixture = AssessmentFixture::with_current_host(&release_host);
    let mut unsupported = host_input(ObserverBinaryAssertionModeV1::Enabled);
    unsupported.hardware_envelope.total_system_memory_mib = 32_768;
    let unsupported_host = current_host(unsupported);
    let assessment = GenerationQualificationPlatformAssessmentCompiler::assess_reviewed(
        fixture.portable(),
        authorities(&fixture, &fixture.approved_platform_policy),
        &unsupported_host,
        &CancellationToken::new(),
    )
    .expect("deterministic execution rejection");
    assert_eq!(assessment.evidence().status(), Status::Rejected);
    assert_eq!(
        assessment.evidence().reason(),
        Reason::UnsupportedExecutionClass
    );

    let exact_enabled_host = current_host(host_input(ObserverBinaryAssertionModeV1::Enabled));
    let exact_enabled_fixture = AssessmentFixture::with_current_host(&exact_enabled_host);
    let exact_enabled_assessment =
        GenerationQualificationPlatformAssessmentCompiler::assess_reviewed(
            exact_enabled_fixture.portable(),
            authorities(
                &exact_enabled_fixture,
                &exact_enabled_fixture.approved_platform_policy,
            ),
            &exact_enabled_host,
            &CancellationToken::new(),
        )
        .expect("debug assertion mode is a portable rejection");
    assert_eq!(
        exact_enabled_assessment.evidence().status(),
        Status::Rejected
    );
    assert_eq!(
        exact_enabled_assessment.evidence().reason(),
        Reason::UnsupportedExecutionClass
    );
}

#[test]
fn hardware_rejection_and_host_preimage_mismatch_are_distinct() {
    let exact_input = host_input(ObserverBinaryAssertionModeV1::Disabled);
    let exact_host = current_host(exact_input.clone());
    let fixture = AssessmentFixture::with_current_host(&exact_host);

    let mut hardware_input = exact_input.clone();
    hardware_input.hardware_envelope.total_system_memory_mib = 32_768;
    let hardware_host = current_host(hardware_input);
    let assessment = GenerationQualificationPlatformAssessmentCompiler::assess_reviewed(
        fixture.portable(),
        authorities(&fixture, &fixture.approved_platform_policy),
        &hardware_host,
        &CancellationToken::new(),
    )
    .expect("deterministic hardware rejection");
    assert_eq!(
        assessment.evidence().reason(),
        Reason::UnsupportedHardwareEnvelope
    );

    let mut mismatched_input = exact_input;
    mismatched_input.operating_system.version = "6.12.11-platform-test".to_owned();
    let mismatched_host = current_host(mismatched_input);
    assert_eq!(
        GenerationQualificationPlatformAssessmentCompiler::assess_reviewed(
            fixture.portable(),
            authorities(&fixture, &fixture.approved_platform_policy),
            &mismatched_host,
            &CancellationToken::new(),
        )
        .expect_err("OS preimage mismatch is operational"),
        GenerationQualificationPlatformAssessmentError::CurrentHostMismatch
    );
}

#[test]
fn second_fresh_revalidation_detects_drift_after_derivation() {
    let initial = host_input(ObserverBinaryAssertionModeV1::Disabled);
    let stable_host = current_host(initial.clone());
    let fixture = AssessmentFixture::with_current_host(&stable_host);
    let mut changed = initial.clone();
    changed.hardware_envelope.total_system_memory_mib = 32_768;
    let drifting_host =
        VerifiedCurrentHostEnvironment::exact_test_fixture_with_revalidations(vec![
            initial.clone(),
            initial.clone(),
            initial.clone(),
            initial,
            changed.clone(),
            changed,
        ])
        .expect("initial and first fresh brackets are stable");
    assert_eq!(
        GenerationQualificationPlatformAssessmentCompiler::assess_reviewed(
            fixture.portable(),
            authorities(&fixture, &fixture.approved_platform_policy),
            &drifting_host,
            &CancellationToken::new(),
        )
        .expect_err("second fresh bracket detects drift"),
        GenerationQualificationPlatformAssessmentError::CurrentHostDrift
    );
}

#[test]
fn every_reviewed_owner_substitution_fails_before_host_use() {
    let host = current_host(host_input(ObserverBinaryAssertionModeV1::Disabled));
    let fixture = AssessmentFixture::with_current_host(&host);
    let foreign_frozen = frozen_with_label(
        &fixture.fixture.runtime.runtime_manifest,
        "foreign platform assessment components",
    );
    let foreign_admitted = VerifiedAdmittedRuntime::exact_candidate_precursor_test_fixture(
        &fixture.fixture.runtime.runtime_manifest,
        foreign_frozen.frozen_set_id().clone(),
    );
    let foreign_path = VerifiedManagedGenerationPath::exact_candidate_precursor_test_fixture(
        &foreign_admitted,
        &fixture.fixture.runtime.runtime_manifest,
    );
    let foreign_generation_policy = foreign_generation_policy();
    let foreign_platform_policy = foreign_platform_policy(&fixture);
    let cases = [
        GenerationQualificationReviewedPlatformAuthorities {
            admitted_runtime: &foreign_admitted,
            ..authorities(&fixture, &fixture.approved_platform_policy)
        },
        GenerationQualificationReviewedPlatformAuthorities {
            generation_path: &foreign_path,
            ..authorities(&fixture, &fixture.approved_platform_policy)
        },
        GenerationQualificationReviewedPlatformAuthorities {
            frozen_components: &foreign_frozen,
            ..authorities(&fixture, &fixture.approved_platform_policy)
        },
        GenerationQualificationReviewedPlatformAuthorities {
            generation_system_policy: &foreign_generation_policy,
            ..authorities(&fixture, &fixture.approved_platform_policy)
        },
        GenerationQualificationReviewedPlatformAuthorities {
            assessment_policy: &foreign_platform_policy,
            ..authorities(&fixture, &fixture.approved_platform_policy)
        },
    ];
    for substituted in cases {
        assert_eq!(
            GenerationQualificationPlatformAssessmentCompiler::assess_reviewed(
                fixture.portable(),
                substituted,
                &host,
                &CancellationToken::new(),
            )
            .expect_err("substituted owner must fail"),
            GenerationQualificationPlatformAssessmentError::InvalidReviewedAuthorityBinding
        );
    }
}

#[test]
fn portable_substitutions_and_cancellation_fail_closed() {
    let fixture = AssessmentFixture::new(reviewed_target());
    let foreign = AssessmentFixture::with_operation_resource_policy(
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxMusl,
        )
        .expect("foreign target"),
        "foreign operation resource policy",
    );
    let mut wrong_operation = fixture.portable();
    wrong_operation.operation_policy = &foreign.operation;
    let fixture_portable = fixture.portable();
    let mut wrong_operation_input = fixture.portable();
    wrong_operation_input.operation_policy_input = &foreign.operation_input;
    let mut wrong_projection = fixture.portable();
    wrong_projection.request_projection = &foreign.projection;
    let foreign_portable = foreign.portable();
    let mut wrong_projection_relations = fixture.portable();
    wrong_projection_relations.request_projection_relations =
        foreign_portable.request_projection_relations;
    let mut foreign_projection_inputs = fixture.projection_inputs.clone();
    foreign_projection_inputs[0].complete_input_byte_count += 1;
    let _foreign_projection = rewrite_model::GenerationQualificationRequestProjectionV1::new(
        fixture_portable.request_projection_relations,
        &foreign_projection_inputs,
    )
    .expect("foreign projection entry inputs are valid");
    let mut wrong_projection_inputs = fixture.portable();
    wrong_projection_inputs.request_projection_entry_inputs = &foreign_projection_inputs;
    let mut wrong_target = fixture.portable();
    wrong_target
        .operation_policy_relations
        .target_system
        .generation_system = foreign.target_system();
    for (label, portable) in [
        ("operation policy", wrong_operation),
        ("operation policy input", wrong_operation_input),
        ("request projection", wrong_projection),
        ("request projection relations", wrong_projection_relations),
        ("request projection inputs", wrong_projection_inputs),
        ("target system", wrong_target),
    ] {
        let result = GenerationQualificationPlatformAssessmentCompiler::assess_policy_denied(
            portable,
            authorities(&fixture, &fixture.denied_platform_policy),
            &CancellationToken::new(),
        );
        let Err(error) = result else {
            panic!("accepted substituted {label}");
        };
        assert_eq!(
            error,
            GenerationQualificationPlatformAssessmentError::InvalidPortableClosure,
            "wrong error for substituted {label}"
        );
    }

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        GenerationQualificationPlatformAssessmentCompiler::assess_policy_denied(
            fixture.portable(),
            authorities(&fixture, &fixture.denied_platform_policy),
            &cancellation,
        )
        .expect_err("cancelled assessment"),
        GenerationQualificationPlatformAssessmentError::Cancelled
    );
}
