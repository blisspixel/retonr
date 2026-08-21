use std::{net::SocketAddr, time::Duration};

#[cfg(any(target_os = "linux", windows))]
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};

use rewrite_types::{CancellationToken, Digest};

use crate::{
    AttachedProcessEvidence, AttachedProcessEvidenceClass, AttachedProcessEvidenceInput,
    AttachedProcessObserver, AttachedProcessWitnessError, AttachedProcessWitnessLimits,
    ListenerEndpoint, NativeAttachedProcessObserver, RetainedTcpConnectionEvidence,
    RetainedTcpConnectionEvidenceInput, TcpConnectionAttributionKind,
    TcpConnectionSharingLimitation, compare_evidence, observe_initial_connection,
    observe_initial_connection_with_policy, reobserve_connection_once,
};
#[cfg(any(target_os = "linux", windows))]
use crate::{AttachedProcessLease, RetainedTcpConnection};

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn evidence(label: &str) -> AttachedProcessEvidence {
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::LinuxProcPidfd,
        owner_pid: 42,
        process_instance_digest: digest(&format!("process {label}")),
        ownership_snapshot_digest: digest(&format!("listener {label}")),
        entrypoint_object_digest: digest(&format!("object {label}")),
        entrypoint_digest: digest(&format!("entrypoint {label}")),
        entrypoint_bytes: 128,
        platform_evidence_digest: digest(&format!("platform {label}")),
    })
    .expect("valid evidence")
}

fn connection_evidence(label: &str) -> RetainedTcpConnectionEvidence {
    RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
        attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
        sharing_limitation: TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable,
        process_evidence_digest: digest(&format!("process {label}")),
        platform_connection_digest: digest(&format!("connection {label}")),
    })
    .expect("valid connection evidence")
}

#[test]
fn endpoint_and_limit_validation_fail_closed() {
    assert_eq!(
        ListenerEndpoint::new(SocketAddr::from(([0, 0, 0, 0], 11_434))),
        Err(AttachedProcessWitnessError::InvalidEndpoint)
    );
    assert_eq!(
        ListenerEndpoint::new(SocketAddr::from(([127, 0, 0, 1], 0))),
        Err(AttachedProcessWitnessError::InvalidEndpoint)
    );
    assert!(ListenerEndpoint::new(SocketAddr::from(([127, 0, 0, 1], 11_434))).is_ok());

    let cancellation = CancellationToken::new();
    let limits = AttachedProcessWitnessLimits {
        maximum_entrypoint_bytes: 0,
        ..AttachedProcessWitnessLimits::default()
    };
    let result = NativeAttachedProcessObserver.attach(
        ListenerEndpoint::new(SocketAddr::from(([127, 0, 0, 1], 11_434))).expect("endpoint"),
        limits,
        &cancellation,
    );
    assert!(matches!(
        result,
        Err(AttachedProcessWitnessError::InvalidLimits)
    ));
}

#[test]
fn comparison_prioritizes_listener_process_and_entrypoint_drift() {
    let initial = evidence("initial");
    let mut changed = evidence("changed");
    assert_eq!(
        compare_evidence(&initial, &changed),
        Err(AttachedProcessWitnessError::ListenerRebound)
    );

    changed = initial.clone();
    let process_changed = AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: changed.evidence_class(),
        owner_pid: changed.owner_pid(),
        process_instance_digest: digest("other process"),
        ownership_snapshot_digest: changed.ownership_snapshot_digest().clone(),
        entrypoint_object_digest: changed.entrypoint_object_digest().clone(),
        entrypoint_digest: changed.entrypoint_digest().clone(),
        entrypoint_bytes: changed.entrypoint_bytes(),
        platform_evidence_digest: changed.platform_evidence_digest().clone(),
    })
    .expect("changed process evidence");
    assert_eq!(
        compare_evidence(&initial, &process_changed),
        Err(AttachedProcessWitnessError::ProcessInstanceChanged)
    );

    let entrypoint_changed = AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: initial.evidence_class(),
        owner_pid: initial.owner_pid(),
        process_instance_digest: initial.process_instance_digest().clone(),
        ownership_snapshot_digest: initial.ownership_snapshot_digest().clone(),
        entrypoint_object_digest: digest("other object"),
        entrypoint_digest: digest("other bytes"),
        entrypoint_bytes: initial.entrypoint_bytes(),
        platform_evidence_digest: initial.platform_evidence_digest().clone(),
    })
    .expect("changed entrypoint evidence");
    assert_eq!(
        compare_evidence(&initial, &entrypoint_changed),
        Err(AttachedProcessWitnessError::EntrypointChanged)
    );
}

#[test]
fn evidence_serialization_is_redacted_and_inert() {
    let encoded = serde_json::to_string(&evidence("safe")).expect("serialize evidence");
    assert!(encoded.contains("linux_proc_pidfd"));
    assert!(encoded.contains("\"owner_pid\":42"));
    assert!(!encoded.contains("path"));
    assert!(!encoded.contains("argument"));
    assert!(!encoded.contains("environment"));
    assert!(!encoded.contains("qualified"));
}

#[test]
fn initial_connection_publication_retry_is_bounded_and_selective() {
    let cancellation = CancellationToken::new();
    let limits = AttachedProcessWitnessLimits::default();
    let expected = connection_evidence("expected");
    let mut attempts = 0_usize;
    let observed =
        observe_initial_connection(&cancellation, std::time::Instant::now(), limits, || {
            attempts += 1;
            if attempts == 1 {
                Err(AttachedProcessWitnessError::ConnectionNotFound)
            } else {
                Ok(expected.clone())
            }
        })
        .expect("retry initial publication");
    assert_eq!(observed, expected);
    assert_eq!(attempts, 2);

    let mut ceiling_attempts = 0_usize;
    assert_eq!(
        observe_initial_connection_with_policy(
            &cancellation,
            std::time::Instant::now(),
            limits,
            3,
            Duration::from_secs(1),
            Duration::ZERO,
            || {
                ceiling_attempts += 1;
                Err(AttachedProcessWitnessError::ConnectionNotEstablished)
            },
        ),
        Err(AttachedProcessWitnessError::ConnectionNotEstablished)
    );
    assert_eq!(ceiling_attempts, 3);

    let mut timed_attempts = 0_usize;
    assert_eq!(
        observe_initial_connection_with_policy(
            &cancellation,
            std::time::Instant::now(),
            limits,
            8,
            Duration::ZERO,
            Duration::ZERO,
            || {
                timed_attempts += 1;
                Err(AttachedProcessWitnessError::ConnectionNotFound)
            },
        ),
        Err(AttachedProcessWitnessError::ConnectionNotFound)
    );
    assert_eq!(timed_attempts, 1);

    let mut fail_fast_attempts = 0_usize;
    assert_eq!(
        observe_initial_connection(&cancellation, std::time::Instant::now(), limits, || {
            fail_fast_attempts += 1;
            Err(AttachedProcessWitnessError::ConnectionAmbiguous)
        }),
        Err(AttachedProcessWitnessError::ConnectionAmbiguous)
    );
    assert_eq!(fail_fast_attempts, 1);
}

#[test]
fn connection_reobservation_checks_once_and_rejects_drift() {
    let initial = connection_evidence("initial");
    let mut attempts = 0_usize;
    assert_eq!(
        reobserve_connection_once(&initial, || {
            attempts += 1;
            Err(AttachedProcessWitnessError::ConnectionNotFound)
        }),
        Err(AttachedProcessWitnessError::ConnectionClosed)
    );
    assert_eq!(attempts, 1);
    assert_eq!(
        reobserve_connection_once(&initial, || Ok(connection_evidence("changed"))),
        Err(AttachedProcessWitnessError::ConnectionChanged)
    );
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn native_observer_retains_and_rechecks_the_current_listener_process() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let endpoint = ListenerEndpoint::new(address).expect("loopback endpoint");
    let cancellation = CancellationToken::new();
    let attached = NativeAttachedProcessObserver.attach(
        endpoint,
        AttachedProcessWitnessLimits::default(),
        &cancellation,
    );
    #[cfg(target_os = "linux")]
    let mut lease = match attached {
        Ok(lease) => lease,
        Err(
            AttachedProcessWitnessError::ProcessAccessDenied
            | AttachedProcessWitnessError::ListenerSnapshotIncomplete,
        ) => return,
        Err(error) => panic!("unexpected Linux witness result: {error}"),
    };
    #[cfg(windows)]
    let mut lease = attached.expect("attach current listener");
    assert_eq!(lease.initial_evidence().owner_pid(), std::process::id());
    let initial_digest = lease.initial_evidence().entrypoint_digest().clone();
    assert_eq!(
        &initial_digest,
        lease
            .reobserve(&cancellation)
            .expect("reobserve listener")
            .entrypoint_digest()
    );
    drop(listener);
    assert!(matches!(
        lease.reobserve(&cancellation),
        Err(AttachedProcessWitnessError::ListenerNotFound
            | AttachedProcessWitnessError::ListenerRebound)
    ));
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn native_observer_attributes_one_retained_established_connection() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    let client = TcpStream::connect(address).expect("connect client");
    let (server, _) = listener.accept().expect("accept connection");
    let connection = RetainedTcpConnection::new(
        client.local_addr().expect("client address"),
        client.peer_addr().expect("server address"),
    )
    .expect("retained connection endpoints");
    let cancellation = CancellationToken::new();
    let attached = NativeAttachedProcessObserver.attach(
        ListenerEndpoint::new(address).expect("loopback endpoint"),
        AttachedProcessWitnessLimits::default(),
        &cancellation,
    );
    #[cfg(target_os = "linux")]
    let mut lease = match attached {
        Ok(lease) => lease,
        Err(
            AttachedProcessWitnessError::ProcessAccessDenied
            | AttachedProcessWitnessError::ListenerSnapshotIncomplete,
        ) => return,
        Err(error) => panic!("unexpected Linux witness result: {error}"),
    };
    #[cfg(windows)]
    let mut lease = attached.expect("attach current listener");
    let initial = lease
        .observe_connection(connection, &cancellation)
        .expect("observe exact connection");
    #[cfg(target_os = "linux")]
    {
        assert_eq!(
            initial.attribution_kind(),
            TcpConnectionAttributionKind::LinuxSocketInodeVisibleSameUidHolder
        );
        assert_eq!(
            initial.sharing_limitation(),
            TcpConnectionSharingLimitation::LinuxOnlyVisibleSameUidDescriptorHoldersChecked
        );
    }
    #[cfg(windows)]
    {
        assert_eq!(
            initial.attribution_kind(),
            TcpConnectionAttributionKind::WindowsContextBindingPid
        );
        assert_eq!(
            initial.sharing_limitation(),
            TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable
        );
    }
    assert_eq!(
        lease
            .reobserve_connection(connection, &initial, &cancellation)
            .expect("reobserve exact connection"),
        initial
    );
    drop(server);
    drop(client);
}

#[cfg(any(target_os = "linux", windows))]
#[test]
fn native_observer_honors_pre_cancelled_requests() {
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let endpoint = ListenerEndpoint::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 11_434))
        .expect("endpoint");
    assert!(matches!(
        NativeAttachedProcessObserver.attach(
            endpoint,
            AttachedProcessWitnessLimits::default(),
            &cancellation,
        ),
        Err(AttachedProcessWitnessError::Cancelled)
    ));
}

#[cfg(target_os = "macos")]
#[test]
fn macos_fails_closed_without_private_or_entitled_apis() {
    let endpoint =
        ListenerEndpoint::new(SocketAddr::from(([127, 0, 0, 1], 11_434))).expect("endpoint");
    assert!(matches!(
        NativeAttachedProcessObserver.attach(
            endpoint,
            AttachedProcessWitnessLimits::default(),
            &CancellationToken::new(),
        ),
        Err(AttachedProcessWitnessError::Unsupported)
    ));
}
