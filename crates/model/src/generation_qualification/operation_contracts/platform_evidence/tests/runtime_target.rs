use super::*;

#[test]
fn runtime_target_and_reason_truth_table_is_exhaustive() {
    use GenerationQualificationPlatformReasonV1 as Reason;
    use GenerationQualificationPlatformStatusV1 as Status;

    let reasons = [
        Reason::ReviewedManagedLinuxNativeCpu,
        Reason::UnsupportedOperatingSystem,
        Reason::UnsupportedArchitecture,
        Reason::UnsupportedAbi,
        Reason::UnsupportedExecutionClass,
        Reason::UnsupportedHardwareEnvelope,
        Reason::AssessmentPolicyDenied,
    ];
    let targets_and_allowed = [
        (
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::X86_64,
                RuntimeAbi::LinuxGnuLibc,
            )
            .expect("reviewed target"),
            vec![
                (Status::Supported, Reason::ReviewedManagedLinuxNativeCpu),
                (Status::Rejected, Reason::UnsupportedExecutionClass),
                (Status::Rejected, Reason::UnsupportedHardwareEnvelope),
                (Status::Rejected, Reason::AssessmentPolicyDenied),
            ],
        ),
        (
            RuntimeTarget::new(
                RuntimeOperatingSystem::Windows,
                RuntimeArchitecture::X86_64,
                RuntimeAbi::WindowsMsvc,
            )
            .expect("Windows target"),
            vec![(Status::Rejected, Reason::UnsupportedOperatingSystem)],
        ),
        (
            RuntimeTarget::new(
                RuntimeOperatingSystem::MacOs,
                RuntimeArchitecture::Aarch64,
                RuntimeAbi::Darwin,
            )
            .expect("macOS target"),
            vec![(Status::Rejected, Reason::UnsupportedOperatingSystem)],
        ),
        (
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::Aarch64,
                RuntimeAbi::LinuxGnuLibc,
            )
            .expect("Linux Arm target"),
            vec![(Status::Rejected, Reason::UnsupportedArchitecture)],
        ),
        (
            RuntimeTarget::new(
                RuntimeOperatingSystem::Linux,
                RuntimeArchitecture::X86_64,
                RuntimeAbi::LinuxMusl,
            )
            .expect("Linux musl target"),
            vec![(Status::Rejected, Reason::UnsupportedAbi)],
        ),
    ];

    for (target, allowed) in targets_and_allowed {
        for status in [Status::Supported, Status::Rejected] {
            for reason in reasons {
                let actual = validate_status_reason(status, reason, target);
                assert_eq!(
                    actual.is_ok(),
                    allowed.contains(&(status, reason)),
                    "unexpected closure for {target:?}, {status:?}, {reason:?}"
                );
                if actual.is_err() {
                    assert_eq!(
                        actual.expect_err("invalid tuple"),
                        GenerationQualificationOperationContractError::InvalidPlatformClosure
                    );
                }
            }
        }
    }
}

#[test]
fn canonical_runtime_target_tags_are_closed_and_stable() {
    assert_eq!(
        [
            RuntimeOperatingSystem::Windows,
            RuntimeOperatingSystem::MacOs,
            RuntimeOperatingSystem::Linux,
        ]
        .map(operating_system_tag),
        [0, 1, 2]
    );
    assert_eq!(
        [RuntimeArchitecture::X86_64, RuntimeArchitecture::Aarch64].map(architecture_tag),
        [0, 1]
    );
    assert_eq!(
        [
            RuntimeAbi::WindowsMsvc,
            RuntimeAbi::WindowsGnu,
            RuntimeAbi::LinuxGnuLibc,
            RuntimeAbi::LinuxMusl,
            RuntimeAbi::Darwin,
        ]
        .map(abi_tag),
        [0, 1, 2, 3, 4]
    );
}
