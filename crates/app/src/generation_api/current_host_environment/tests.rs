use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::Mutex;

use rewrite_model::{
    ComputeBackend, ExecutionPlacement, HostAcceleratorScopeV1, HostArchitectureV1Input,
    HostEnvironmentV1, HostEnvironmentV1Input, HostExecutionClassV1Input, HostExecutionProfileV1,
    HostHardwareEnvelopeV1Input, HostOperatingSystemV1Input, ObserverBinaryAssertionModeV1,
    RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
};
use rewrite_types::CancellationToken;

use super::cgroup::{
    CgroupControlFile, INITIAL_CGROUP_NAMESPACE_INODE, require_initial_namespace_inode,
    require_returned_mount_id, validate_cgroup2_mount_root, validate_controls,
};
use super::parser::{
    cpu_max_is_unlimited, memory_max_is_unlimited, parse_cpu_info, parse_cpu_list,
    parse_kernel_release, parse_online_cpu_list, parse_total_memory_mib, parse_unified_cgroup_path,
    read_bounded_stream, validate_affinity,
};
use super::*;

pub(super) struct FixtureEnvironmentSource {
    state: Mutex<FixtureState>,
}

struct FixtureState {
    observations: VecDeque<HostEnvironmentV1>,
    final_observation: HostEnvironmentV1,
}

impl FixtureEnvironmentSource {
    pub(super) fn stable(
        input: HostEnvironmentV1Input,
    ) -> Result<Self, CurrentHostEnvironmentError> {
        let environment = HostEnvironmentV1::new(input)?;
        Ok(Self {
            state: Mutex::new(FixtureState {
                observations: VecDeque::new(),
                final_observation: environment,
            }),
        })
    }

    pub(super) fn sequence(
        inputs: Vec<HostEnvironmentV1Input>,
    ) -> Result<Self, CurrentHostEnvironmentError> {
        let observations = inputs
            .into_iter()
            .map(HostEnvironmentV1::new)
            .collect::<Result<VecDeque<_>, _>>()?;
        let final_observation = observations
            .back()
            .cloned()
            .ok_or(CurrentHostEnvironmentError::InvalidObservation)?;
        Ok(Self {
            state: Mutex::new(FixtureState {
                observations,
                final_observation,
            }),
        })
    }
}

impl CurrentHostEnvironmentSource for FixtureEnvironmentSource {
    fn observe(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<HostEnvironmentV1, CurrentHostEnvironmentError> {
        ensure_active(cancellation)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_error| CurrentHostEnvironmentError::ObservationUnavailable)?;
        Ok(state
            .observations
            .pop_front()
            .unwrap_or_else(|| state.final_observation.clone()))
    }
}

pub(crate) fn host_input(assertion_mode: ObserverBinaryAssertionModeV1) -> HostEnvironmentV1Input {
    HostEnvironmentV1Input {
        operating_system: HostOperatingSystemV1Input {
            family: RuntimeOperatingSystem::Linux,
            version: "6.12.10-test".to_owned(),
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
            cpu_model: "Example Native CPU".to_owned(),
            physical_core_count: 4,
            logical_core_count: 8,
            total_system_memory_mib: 16_384,
            memory_rounding_granularity_mib: 1_024,
        },
    }
}

#[test]
fn exact_double_observation_retains_redacted_revalidatable_authority() {
    let input = host_input(ObserverBinaryAssertionModeV1::Disabled);
    let authority = VerifiedCurrentHostEnvironment::exact_test_fixture(input)
        .expect("stable exact observation");
    authority
        .revalidate_current(&CancellationToken::new())
        .expect("fresh exact revalidation");
    let bindings =
        GenerationQualificationPlatformAssessmentPolicyV1Bindings::from_current_host_environment(
            &authority,
        );
    assert_eq!(bindings.runtime_target, authority.runtime_target());
    let digests = authority.digest_set();
    assert_eq!(
        bindings.operating_system_digest,
        *digests.operating_system_digest().digest()
    );
    assert_eq!(
        bindings.architecture_digest,
        *digests.architecture_digest().digest()
    );
    assert_eq!(
        bindings.execution_class_digest,
        *digests.execution_class_digest().digest()
    );
    assert_eq!(
        bindings.hardware_envelope_digest,
        *digests.hardware_envelope_digest().digest()
    );
    let debug = format!("{authority:?}");
    assert_eq!(
        debug,
        "VerifiedCurrentHostEnvironment { schema_version: 1, .. }"
    );
    assert!(!debug.contains("Example Native CPU") && !debug.contains("6.12.10-test"));
}

#[test]
fn confirmation_and_fresh_revalidation_reject_drift() {
    let initial = host_input(ObserverBinaryAssertionModeV1::Disabled);
    let mut changed = initial.clone();
    changed.hardware_envelope.total_system_memory_mib = 32_768;
    assert_eq!(
        VerifiedCurrentHostEnvironment::observe_from_source(
            Box::new(
                FixtureEnvironmentSource::sequence(vec![initial.clone(), changed.clone()])
                    .expect("fixture source"),
            ),
            &CancellationToken::new(),
        )
        .expect_err("initial confirmation must be stable"),
        CurrentHostEnvironmentError::ObservationDrift
    );

    let authority = VerifiedCurrentHostEnvironment::exact_test_fixture_with_revalidations(vec![
        initial.clone(),
        initial,
        changed.clone(),
        changed,
    ])
    .expect("stable initial confirmation");
    assert_eq!(
        authority
            .revalidate_current(&CancellationToken::new())
            .expect_err("fresh observation drift"),
        CurrentHostEnvironmentError::ObservationDrift
    );
}

#[test]
fn cancelled_construction_and_revalidation_fail_at_checkpoints() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert_eq!(
        VerifiedCurrentHostEnvironment::observe_from_source(
            Box::new(
                FixtureEnvironmentSource::stable(host_input(
                    ObserverBinaryAssertionModeV1::Disabled,
                ))
                .expect("fixture source"),
            ),
            &cancellation,
        )
        .expect_err("cancelled observation"),
        CurrentHostEnvironmentError::Cancelled
    );
    let authority = VerifiedCurrentHostEnvironment::exact_test_fixture(host_input(
        ObserverBinaryAssertionModeV1::Disabled,
    ))
    .expect("authority");
    assert_eq!(
        authority
            .revalidate_current(&cancellation)
            .expect_err("cancelled revalidation"),
        CurrentHostEnvironmentError::Cancelled
    );
}

#[cfg(not(target_os = "linux"))]
#[test]
fn production_observer_fails_closed_outside_the_linux_profile() {
    assert_eq!(
        CurrentHostEnvironmentObserver::observe(&CancellationToken::new())
            .expect_err("non-Linux production observation must be unavailable"),
        CurrentHostEnvironmentError::UnsupportedPlatform
    );
}

#[test]
fn pure_linux_parsers_accept_one_exact_privacy_bounded_fixture() {
    assert_eq!(
        parse_kernel_release(b"6.12.10-test\n").expect("kernel release"),
        "6.12.10-test"
    );
    assert_eq!(
        parse_kernel_release(b"6.12.10-test\r\n").expect("CRLF kernel release"),
        "6.12.10-test"
    );
    let online = parse_online_cpu_list(b"0-1\n").expect("online CPU set");
    let cpu = parse_cpu_info(
        concat!(
            "processor : 0\nphysical id : 0\ncore id : 0\n",
            "model name : Example   Native CPU\n\n",
            "processor : 1\nphysical id : 0\ncore id : 0\n",
            "model name : Example Native CPU\n\n"
        )
        .as_bytes(),
    )
    .expect("CPU inventory");
    assert_eq!(cpu.logical_cpus, online);
    assert_eq!(cpu.physical_core_count, 1);
    assert_eq!(cpu.cpu_model, "Example Native CPU");
    assert_eq!(
        parse_total_memory_mib(b"MemTotal:       65987654 kB\nMemFree: 1 kB\n")
            .expect("rounded memory"),
        63_488
    );
    assert!(cpu_max_is_unlimited(b"max 100000\n").expect("unlimited CPU"));
    assert!(memory_max_is_unlimited(b"max\n").expect("unlimited memory"));
    assert_eq!(
        parse_unified_cgroup_path(b"0::/user.slice/session.scope\n").expect("unified cgroup"),
        vec!["user.slice", "session.scope"]
    );
}

#[test]
fn pure_linux_parsers_reject_ambiguous_or_unbounded_fixtures() {
    for invalid in ["", "00", "1,0", "0-2,2-3", "0-8192", "0--1"] {
        assert!(parse_cpu_list(invalid).is_err(), "accepted {invalid:?}");
    }
    for invalid in [
        b"0-1\n\n".as_slice(),
        b"0-1\r\n\r\n".as_slice(),
        b"0-1\r".as_slice(),
    ] {
        assert!(parse_online_cpu_list(invalid).is_err());
    }
    for invalid in [
        b"6.12.10 test\n".as_slice(),
        b"6.12.10\n\n".as_slice(),
        b"6.12.10\r\n\r\n".as_slice(),
        b"6.12.10\nextra\n".as_slice(),
        b"\xff".as_slice(),
    ] {
        assert!(parse_kernel_release(invalid).is_err());
    }
    assert!(
        parse_cpu_info(
            b"processor:0\nphysical id:0\ncore id:0\nmodel name: first\n\n\
              processor:1\nphysical id:0\ncore id:1\nmodel name: second\n"
        )
        .is_err()
    );
    assert!(parse_total_memory_mib(b"MemTotal: 512 kB\n").is_err());
    assert!(!cpu_max_is_unlimited(b"20000 100000\n").expect("bounded CPU"));
    for invalid in [
        b"invalid 100000\n".as_slice(),
        b"0 100000\n".as_slice(),
        b"max  100000\n".as_slice(),
        b" max 100000\n".as_slice(),
        b"max 100000 \n".as_slice(),
        b"max\t100000\n".as_slice(),
        b"max 100000\n\n".as_slice(),
    ] {
        assert!(cpu_max_is_unlimited(invalid).is_err());
    }
    assert!(!memory_max_is_unlimited(b"1073741824\n").expect("bounded memory"));
    assert!(memory_max_is_unlimited(b"max\n\n").is_err());
    for invalid in [
        b"1:cpu:/legacy\n".as_slice(),
        b"0::/../../escape\n".as_slice(),
        b"0::/a//b\n".as_slice(),
        b"0::/a\n0::/b\n".as_slice(),
    ] {
        assert!(parse_unified_cgroup_path(invalid).is_err());
    }
}

#[test]
fn cgroup_namespace_and_mount_scope_fail_closed() {
    assert_eq!(
        require_initial_namespace_inode(Some(INITIAL_CGROUP_NAMESPACE_INODE)),
        Ok(())
    );
    assert_eq!(
        require_initial_namespace_inode(Some(INITIAL_CGROUP_NAMESPACE_INODE + 1)),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );
    assert_eq!(
        require_initial_namespace_inode(None),
        Err(CurrentHostEnvironmentError::ObservationUnavailable)
    );
    assert_eq!(require_returned_mount_id(true, 21, None), Ok(21));
    assert_eq!(require_returned_mount_id(true, 21, Some(21)), Ok(21));
    assert_eq!(
        require_returned_mount_id(false, 21, None),
        Err(CurrentHostEnvironmentError::ObservationUnavailable)
    );
    assert_eq!(
        require_returned_mount_id(true, 0, None),
        Err(CurrentHostEnvironmentError::ObservationUnavailable)
    );
    assert_eq!(
        require_returned_mount_id(true, 21, Some(22)),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );

    let exact = b"21 20 0:19 / /sys/fs/cgroup rw,nosuid,nodev - cgroup2 cgroup2 rw\n";
    assert_eq!(validate_cgroup2_mount_root(exact, 21), Ok(()));
    let hidden = b"21 20 0:19 /hidden /sys/fs/cgroup rw - cgroup2 cgroup2 rw\n";
    assert_eq!(
        validate_cgroup2_mount_root(hidden, 21),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );
    let wrong_filesystem = b"21 20 8:1 / /sys/fs/cgroup rw - ext4 /dev/root rw\n";
    assert_eq!(
        validate_cgroup2_mount_root(wrong_filesystem, 21),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );
    let descendant = concat!(
        "21 20 0:19 / /sys/fs/cgroup rw - cgroup2 cgroup2 rw\n",
        "22 21 8:1 /fake /sys/fs/cgroup/parent rw - ext4 /dev/root rw\n",
    );
    assert_eq!(
        validate_cgroup2_mount_root(descendant.as_bytes(), 21),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );
    assert_eq!(
        validate_cgroup2_mount_root(b"20 1 8:1 / / rw - ext4 /dev/root rw\n", 21),
        Err(CurrentHostEnvironmentError::ObservationUnavailable)
    );
    assert_eq!(
        validate_cgroup2_mount_root(exact, 22),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );
}

#[test]
fn cgroup_controls_reject_finite_ancestry_missing_files_and_cpuset_mismatch() {
    let online = parse_cpu_list("0-1").expect("online CPUs");
    let exact = |_: &[String], control| {
        Ok(match control {
            CgroupControlFile::CpuMax => b"max 100000\n".to_vec(),
            CgroupControlFile::MemoryMax | CgroupControlFile::MemoryHigh => b"max\n".to_vec(),
            CgroupControlFile::CgroupType => b"domain\n".to_vec(),
            CgroupControlFile::EffectiveCpuSet => b"0-1\n".to_vec(),
        })
    };
    assert_eq!(
        validate_controls(b"0::/parent/leaf\n", &online, exact),
        Ok(())
    );

    let finite_parent = |components: &[String], control| {
        Ok(
            if components.len() == 1 && control == CgroupControlFile::CpuMax {
                b"50000 100000\n".to_vec()
            } else {
                match control {
                    CgroupControlFile::CpuMax => b"max 100000\n".to_vec(),
                    CgroupControlFile::MemoryMax | CgroupControlFile::MemoryHigh => {
                        b"max\n".to_vec()
                    }
                    CgroupControlFile::CgroupType => b"domain\n".to_vec(),
                    CgroupControlFile::EffectiveCpuSet => b"0-1\n".to_vec(),
                }
            },
        )
    };
    assert_eq!(
        validate_controls(b"0::/parent/leaf\n", &online, finite_parent),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );

    assert_eq!(
        validate_controls(b"0::/parent\n", &online, |_, control| {
            if control == CgroupControlFile::MemoryHigh {
                Err(CurrentHostEnvironmentError::ObservationUnavailable)
            } else {
                exact(&[], control)
            }
        }),
        Err(CurrentHostEnvironmentError::ObservationUnavailable)
    );
    assert_eq!(
        validate_controls(b"0::/parent\n", &online, |components, control| {
            if control == CgroupControlFile::EffectiveCpuSet {
                Ok(b"0\n".to_vec())
            } else {
                exact(components, control)
            }
        }),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );
    for cgroup_type in [
        b"threaded\n".as_slice(),
        b"domain threaded\n",
        b"domain invalid\n",
    ] {
        assert_eq!(
            validate_controls(b"0::/parent\n", &online, |components, control| {
                if control == CgroupControlFile::CgroupType {
                    Ok(cgroup_type.to_vec())
                } else {
                    exact(components, control)
                }
            }),
            Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
        );
    }
}

#[test]
fn affinity_and_bounded_reads_reject_partial_host_views() {
    let online = parse_cpu_list("0-1").expect("online CPUs");
    assert_eq!(validate_affinity(&online, 4, |cpu| cpu < 2), Ok(()));
    assert_eq!(
        validate_affinity(&online, 4, |cpu| cpu == 0),
        Err(CurrentHostEnvironmentError::ConstrainedEnvironment)
    );
    assert_eq!(
        read_bounded_stream(Cursor::new(*b"abcd"), 4),
        Ok(b"abcd".to_vec())
    );
    assert_eq!(
        read_bounded_stream(Cursor::new(*b"abcde"), 4),
        Err(CurrentHostEnvironmentError::ObservationUnavailable)
    );
}
