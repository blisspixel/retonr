#[cfg(not(target_os = "linux"))]
use std::sync::Arc;
use std::{ffi::OsString, path::PathBuf, time::Duration};

use rewrite_types::{CancellationToken, Digest};

use super::{
    IsolationEvidence, IsolationPolicy, IsolationPreparationEvidence, LaunchSpec,
    ManagedDeviceBoundaryEvidence, ManagedDeviceVisibilityPolicy, ManagedRuntimeInputEvidence,
    NamespaceIdentity, PreparedIsolation, TargetProcessEvidence,
};
#[cfg(not(target_os = "linux"))]
use super::{PreparedIsolationSubjectToken, RetainedIsolationLease};
use crate::IsolationError;

#[test]
fn policy_rejects_zero_and_unbounded_values() {
    assert_eq!(
        IsolationPolicy::new(Duration::ZERO, Duration::from_secs(1), 1, 1, 1, 64, 8,),
        Err(IsolationError::InvalidPolicy("startup timeout"))
    );
    assert_eq!(
        IsolationPolicy::new(
            Duration::from_secs(1),
            Duration::from_secs(1),
            4_097,
            1,
            1,
            64,
            8,
        ),
        Err(IsolationError::InvalidPolicy("argument count"))
    );
}

#[test]
fn launch_validation_rejects_relative_paths_and_internal_environment() {
    let token = CancellationToken::new();
    let result = PreparedIsolation::prepare(
        "relative-helper",
        &Digest::sha256(b"helper"),
        6,
        IsolationPolicy::default(),
        &token,
    );
    assert!(matches!(
        result,
        Err(IsolationError::UnsupportedPlatform | IsolationError::InvalidHelper)
    ));

    let mut spec = LaunchSpec::new(PathBuf::from("relative-target"));
    spec.insert_environment("REWRITE_ISOLATION_INTERNAL_BAD", "value");
    assert_eq!(
        spec.validate(IsolationPolicy::default()),
        Err(IsolationError::InvalidLaunch("executable path"))
    );
}

#[test]
fn every_policy_bound_is_validated_and_exposed() {
    let valid = IsolationPolicy::new(
        Duration::from_secs(2),
        Duration::from_secs(3),
        2,
        2,
        16,
        64,
        8,
    )
    .expect("valid policy");
    assert_eq!(valid.startup_timeout(), Duration::from_secs(2));
    assert_eq!(valid.shutdown_timeout(), Duration::from_secs(3));
    assert_ne!(
        valid.redacted_digest(),
        IsolationPolicy::default().redacted_digest()
    );

    let cases = [
        (
            IsolationPolicy::new(
                Duration::from_secs(31),
                Duration::from_secs(1),
                1,
                1,
                1,
                64,
                8,
            ),
            "startup timeout",
        ),
        (
            IsolationPolicy::new(Duration::from_secs(1), Duration::ZERO, 1, 1, 1, 64, 8),
            "shutdown timeout",
        ),
        (
            IsolationPolicy::new(
                Duration::from_secs(1),
                Duration::from_secs(1),
                1,
                1_025,
                1,
                64,
                8,
            ),
            "environment count",
        ),
        (
            IsolationPolicy::new(
                Duration::from_secs(1),
                Duration::from_secs(1),
                1,
                1,
                0,
                64,
                8,
            ),
            "value bytes",
        ),
        (
            IsolationPolicy::new(
                Duration::from_secs(1),
                Duration::from_secs(1),
                1,
                1,
                1,
                63,
                8,
            ),
            "open-file limit",
        ),
        (
            IsolationPolicy::new(
                Duration::from_secs(1),
                Duration::from_secs(1),
                1,
                1,
                1,
                64,
                7,
            ),
            "process limit",
        ),
    ];
    for (result, field) in cases {
        assert_eq!(result, Err(IsolationError::InvalidPolicy(field)));
    }
}

#[test]
fn launch_builder_enforces_counts_values_keys_and_directories() {
    let executable = std::env::current_exe().expect("current executable");
    let policy = IsolationPolicy::new(
        Duration::from_secs(1),
        Duration::from_secs(1),
        1,
        1,
        4,
        64,
        8,
    )
    .expect("valid policy");

    let mut valid = LaunchSpec::new(&executable);
    valid.push_argument("a");
    valid.insert_environment("K", "V");
    valid.set_current_directory(std::env::current_dir().expect("current directory"));
    assert_eq!(
        valid.environment_value(std::ffi::OsStr::new("K")),
        Some(std::ffi::OsStr::new("V"))
    );
    assert_eq!(
        valid.environment_value(std::ffi::OsStr::new("MISSING")),
        None
    );
    assert!(valid.validate(policy).is_ok());
    let original_digest = valid.redacted_digest();
    let mut changed = valid.clone();
    changed.insert_environment("K", "W");
    assert_ne!(changed.redacted_digest(), original_digest);

    let mut too_many_arguments = valid.clone();
    too_many_arguments.push_argument("b");
    assert_eq!(
        too_many_arguments.validate(policy),
        Err(IsolationError::InvalidLaunch("argument count"))
    );

    let mut too_many_environment = valid.clone();
    too_many_environment.insert_environment("X", "Y");
    assert_eq!(
        too_many_environment.validate(policy),
        Err(IsolationError::InvalidLaunch("environment count"))
    );

    let mut oversized = LaunchSpec::new(&executable);
    oversized.push_argument("12345");
    assert_eq!(
        oversized.validate(policy),
        Err(IsolationError::InvalidLaunch("value bytes"))
    );

    let mut invalid_key = LaunchSpec::new(&executable);
    invalid_key.insert_environment("A=B", "V");
    assert_eq!(
        invalid_key.validate(policy),
        Err(IsolationError::InvalidLaunch("environment key"))
    );

    let mut nul = LaunchSpec::new(&executable);
    nul.push_argument(OsString::from("a\0b"));
    assert_eq!(
        nul.validate(policy),
        Err(IsolationError::InvalidLaunch("value bytes"))
    );

    let mut relative_directory = LaunchSpec::new(executable);
    relative_directory.set_current_directory("relative");
    assert_eq!(
        relative_directory.validate(policy),
        Err(IsolationError::InvalidLaunch("current directory"))
    );
}

#[test]
fn preparation_evidence_requires_exact_canaries_and_exposes_measurements() {
    let preparation = IsolationPreparationEvidence {
        loopback_interface_index: 1,
        canary_protocol_version: 1,
        device_canary_protocol_version: 1,
        runtime_input_canary_protocol_version: 1,
        managed_device_visibility: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
        helper_digest: Digest::sha256(b"helper"),
        helper_bytes: 6,
    };
    assert!(preparation.all_canaries_passed());
    assert_eq!(preparation.loopback_interface_index(), 1);
    assert_eq!(preparation.canary_protocol_version(), 1);
    assert_eq!(preparation.device_canary_protocol_version(), 1);
    assert_eq!(preparation.runtime_input_canary_protocol_version(), 1);
    assert_eq!(
        preparation.managed_device_visibility(),
        ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1
    );
    assert_eq!(preparation.helper_digest(), &Digest::sha256(b"helper"));
    assert_eq!(preparation.helper_bytes(), 6);
    assert!(
        !IsolationPreparationEvidence {
            loopback_interface_index: 0,
            canary_protocol_version: 1,
            device_canary_protocol_version: 1,
            runtime_input_canary_protocol_version: 1,
            managed_device_visibility: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
            helper_digest: Digest::sha256(b"helper"),
            helper_bytes: 6,
        }
        .all_canaries_passed()
    );
}

#[test]
fn managed_device_boundary_getters_preserve_exact_observations() {
    let boundary = ManagedDeviceBoundaryEvidence {
        policy: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
        device_mount: NamespaceIdentity {
            device: 9,
            inode: 10,
        },
        proc_mount: NamespaceIdentity {
            device: 11,
            inode: 12,
        },
        null_device: NamespaceIdentity {
            device: 13,
            inode: 14,
        },
        null_major: 1,
        null_minor: 3,
        visible_device_entries: 1,
        postconditions: ManagedDeviceBoundaryEvidence::REQUIRED_POSTCONDITIONS,
    };
    assert!(boundary.all_visibility_canaries_passed());
    assert_eq!(
        boundary.policy(),
        ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1
    );
    assert_eq!(
        boundary.device_mount(),
        NamespaceIdentity {
            device: 9,
            inode: 10
        }
    );
    assert_eq!(
        boundary.proc_mount(),
        NamespaceIdentity {
            device: 11,
            inode: 12
        }
    );
    assert_eq!(
        boundary.null_device(),
        NamespaceIdentity {
            device: 13,
            inode: 14
        }
    );
    assert_eq!(boundary.null_device_numbers(), (1, 3));
    assert_eq!(boundary.visible_device_entries(), 1);
}

#[test]
fn evidence_getters_preserve_exact_native_identity() {
    let preparation = IsolationPreparationEvidence {
        loopback_interface_index: 1,
        canary_protocol_version: 1,
        device_canary_protocol_version: 1,
        runtime_input_canary_protocol_version: 1,
        managed_device_visibility: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
        helper_digest: Digest::sha256(b"helper"),
        helper_bytes: 6,
    };
    let network = NamespaceIdentity {
        device: 1,
        inode: 2,
    };
    let user = NamespaceIdentity {
        device: 3,
        inode: 4,
    };
    let process = NamespaceIdentity {
        device: 5,
        inode: 6,
    };
    let mount = NamespaceIdentity {
        device: 7,
        inode: 8,
    };
    let device_boundary = ManagedDeviceBoundaryEvidence {
        policy: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
        device_mount: NamespaceIdentity {
            device: 9,
            inode: 10,
        },
        proc_mount: NamespaceIdentity {
            device: 11,
            inode: 12,
        },
        null_device: NamespaceIdentity {
            device: 13,
            inode: 14,
        },
        null_major: 1,
        null_minor: 3,
        visible_device_entries: 1,
        postconditions: ManagedDeviceBoundaryEvidence::REQUIRED_POSTCONDITIONS,
    };
    assert_eq!(network.device(), 1);
    assert_eq!(network.inode(), 2);
    let target = TargetProcessEvidence {
        outer_pid: 8,
        namespace_pid: 2,
        process_start_token: 9,
        namespace_user_id: 0,
        executable_device: 10,
        executable_inode: 11,
        executable_bytes: 12,
    };
    let evidence = IsolationEvidence {
        guardian_pid: 7,
        network_namespace: network,
        user_namespace: user,
        process_namespace: process,
        mount_namespace: mount,
        device_boundary,
        runtime_inputs: ManagedRuntimeInputEvidence {
            scratch_mount: NamespaceIdentity {
                device: 15,
                inode: 16,
            },
            input_mount: NamespaceIdentity {
                device: 17,
                inode: 18,
            },
            member_count: 0,
            total_bytes: 0,
            layout_digest: Digest::sha256(b"empty-input"),
            postconditions: ManagedRuntimeInputEvidence::REQUIRED_POSTCONDITIONS,
        },
        preparation: preparation.clone(),
        target,
    };
    assert_eq!(evidence.guardian_pid(), 7);
    assert_eq!(evidence.network_namespace(), network);
    assert_eq!(evidence.user_namespace(), user);
    assert_eq!(evidence.process_namespace(), process);
    assert_eq!(evidence.mount_namespace(), mount);
    assert_eq!(evidence.device_boundary(), &device_boundary);
    assert!(evidence.device_boundary().all_visibility_canaries_passed());
    assert_eq!(evidence.preparation(), &preparation);
    assert_eq!(evidence.target().outer_pid(), 8);
    assert_eq!(evidence.target().namespace_pid(), 2);
    assert_eq!(evidence.target().process_start_token(), 9);
    assert_eq!(evidence.target().namespace_user_id(), 0);
    assert_eq!(evidence.target().executable_device(), 10);
    assert_eq!(evidence.target().executable_inode(), 11);
    assert_eq!(evidence.target().executable_bytes(), 12);
    assert_eq!(evidence.redacted_digest().as_str().len(), 64);
}

#[cfg(not(target_os = "linux"))]
#[test]
fn unsupported_prepared_and_lease_paths_remain_inert() {
    let token = CancellationToken::new();
    let prepared = PreparedIsolation {
        policy: IsolationPolicy::default(),
        preparation: IsolationPreparationEvidence {
            loopback_interface_index: 1,
            canary_protocol_version: 1,
            device_canary_protocol_version: 1,
            runtime_input_canary_protocol_version: 1,
            managed_device_visibility: ManagedDeviceVisibilityPolicy::LinuxCpuOnlyV1,
            helper_digest: Digest::sha256(b"helper"),
            helper_bytes: 6,
        },
        platform: crate::platform::Prepared,
        subject: PreparedIsolationSubjectToken(Arc::new(())),
    };
    assert!(prepared.preparation_evidence().all_canaries_passed());
    assert_eq!(prepared.policy_digest(), prepared.policy.redacted_digest());
    let specification =
        LaunchSpec::new(std::env::current_exe().expect("absolute current executable"));
    assert!(matches!(
        prepared.launch(&specification, &token),
        Err(IsolationError::UnsupportedPlatform)
    ));
    let executable =
        std::fs::File::open(std::env::current_exe().expect("absolute current executable"))
            .expect("open retained executable");
    assert!(matches!(
        prepared.launch_retained(&specification, executable, &token),
        Err(IsolationError::UnsupportedPlatform)
    ));

    let launch_spec_digest = specification.redacted_digest();
    let isolation_policy_digest = prepared.policy.redacted_digest();
    let mut lease = RetainedIsolationLease {
        platform: crate::platform::Lease,
        launch_spec_digest: launch_spec_digest.clone(),
        isolation_policy_digest: isolation_policy_digest.clone(),
    };
    assert_eq!(lease.launch_spec_digest(), &launch_spec_digest);
    assert_eq!(lease.isolation_policy_digest(), &isolation_policy_digest);
    assert_eq!(
        lease.reobserve(&token),
        Err(IsolationError::UnsupportedPlatform)
    );
    assert_eq!(
        lease.close(&token),
        Err(IsolationError::UnsupportedPlatform)
    );
}
