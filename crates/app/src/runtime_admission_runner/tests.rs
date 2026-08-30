use std::{
    fs::File,
    io::{Read as _, Write as _},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::Duration,
};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    NativeLoadEvidenceClass, NativeLoadObservationInput, NativeLoadOrigin,
    NativeLoadVisibilityScope, NativeLoadedComponent, NativeMappingClass, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_ollama::{OllamaEndpoint, OllamaLimits, OllamaSingleConnectionRuntimeProbe};
use rewrite_runtime_attestor::{
    AttachedProcessEvidenceInput, NativeLoadObservationLimits, RetainedTcpConnection,
    RetainedTcpConnectionEvidenceInput, TcpConnectionAttributionKind,
    TcpConnectionSharingLimitation,
};

use super::*;

#[path = "tests/binding.rs"]
mod binding;

#[tokio::test(flavor = "current_thread")]
async fn exact_probe_observes_before_then_after_one_version_request() {
    let (endpoint, stream, server) = one_request_version_server("0.16.2");
    let initial = process_evidence(&runtime_package("0.16.2"), b"probe process");
    let mut process = FakeProcessLease::new(initial, None, None);
    let probe = OllamaSingleConnectionRuntimeProbe::new(
        endpoint,
        "0.16.2".parse().expect("exact version"),
        OllamaLimits::default(),
    )
    .expect("valid probe");

    let (runtime, initial_connection, final_connection) =
        run_exact_probe(&probe, stream, &mut process, &CancellationToken::new())
            .await
            .expect("one exact probe");
    let request = server.join().expect("server thread");

    assert_eq!(runtime.runtime_version().to_string(), "0.16.2");
    assert_eq!(process.events, ["connection_before", "connection_after"]);
    assert_eq!(
        initial_connection.attribution_kind(),
        TcpConnectionAttributionKind::WindowsContextBindingPid
    );
    assert_eq!(
        final_connection.attribution_kind(),
        TcpConnectionAttributionKind::WindowsContextBindingPid
    );
    assert_eq!(request.matches("GET /api/version HTTP/1.1").count(), 1);
    assert_eq!(request.matches("GET ").count(), 1);
}

#[test]
fn frozen_set_mismatch_precedes_managed_process_validation() {
    let package = runtime_package("0.16.2");
    let package_id = package.runtime_package_manifest_id();
    let other_id = runtime_package("0.16.3").runtime_package_manifest_id();
    let attached_not_managed = process_evidence(&package, b"attached process");

    let error =
        validate_final_identity_bindings(&package, &package_id, &other_id, &attached_not_managed)
            .expect_err("cross-package frozen set");

    assert!(matches!(
        error,
        RuntimeAdmissionRunnerError::InvalidFrozenSetBinding
    ));
}

#[test]
fn stale_and_cross_package_native_observations_fail_binding() {
    let package = runtime_package("0.16.2");
    let other = runtime_package("0.16.3");
    let initial = process_evidence(&package, b"current process");
    let stale = native_observation(&package, Digest::sha256(b"stale process"));
    let cross_package = native_observation(&other, initial.evidence_digest().clone());

    assert!(matches!(
        validate_native_binding(&stale, &package.runtime_package_manifest_id(), &initial),
        Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding)
    ));
    assert!(matches!(
        validate_native_binding(
            &cross_package,
            &package.runtime_package_manifest_id(),
            &initial
        ),
        Err(RuntimeAdmissionRunnerError::InvalidEvidenceBinding)
    ));
}

#[test]
fn discovery_cancellation_happens_before_observer_work() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let package = runtime_package("0.16.2");
    let retained = retained_members(&package, temporary.path());
    let mut process = FakeProcessLease::new(
        process_evidence(&package, b"cancelled process"),
        Some(NativeLoadObserverError::Unsupported),
        None,
    );
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let error = discover_with_retained_members(
        &package,
        &package.runtime_package_manifest_id(),
        &retained,
        &mut process,
        NativeLoadObservationLimits::default(),
        &cancellation,
    )
    .expect_err("cancelled discovery");

    assert!(matches!(error, RuntimeAdmissionRunnerError::Cancelled));
    assert!(process.events.is_empty());
}

#[test]
fn discovery_preserves_unsupported_and_access_visibility_failures() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let package = runtime_package("0.16.2");
    let package_id = package.runtime_package_manifest_id();
    let retained = retained_members(&package, temporary.path());
    for expected in [
        NativeLoadObserverError::Unsupported,
        NativeLoadObserverError::ProcessVisibilityInsufficient,
    ] {
        let mut process = FakeProcessLease::new(
            process_evidence(&package, b"failed discovery process"),
            Some(expected),
            None,
        );
        let error = discover_with_retained_members(
            &package,
            &package_id,
            &retained,
            &mut process,
            NativeLoadObservationLimits::default(),
            &CancellationToken::new(),
        )
        .expect_err("discovery must fail closed");
        assert!(matches!(
            error,
            RuntimeAdmissionRunnerError::NativeLoad(observed) if observed == expected
        ));
        assert_eq!(process.events, ["discovery"]);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn exact_probe_preserves_access_denied_before_request() {
    let (endpoint, stream, server) = one_request_version_server("0.16.2");
    let initial = process_evidence(&runtime_package("0.16.2"), b"denied process");
    let mut process = FakeProcessLease::new(
        initial,
        None,
        Some(AttachedProcessWitnessError::ProcessAccessDenied),
    );
    let probe = OllamaSingleConnectionRuntimeProbe::new(
        endpoint,
        "0.16.2".parse().expect("exact version"),
        OllamaLimits::default(),
    )
    .expect("valid probe");

    let error = run_exact_probe(&probe, stream, &mut process, &CancellationToken::new())
        .await
        .expect_err("process access must fail closed");
    let request = server.join().expect("server thread");

    assert!(matches!(
        error,
        RuntimeAdmissionRunnerError::Witness(AttachedProcessWitnessError::ProcessAccessDenied)
    ));
    assert_eq!(process.events, ["connection_before"]);
    assert!(request.is_empty());
}

struct FakeProcessLease {
    initial: AttachedProcessEvidence,
    discovery_error: Option<NativeLoadObserverError>,
    connection_error: Option<AttachedProcessWitnessError>,
    events: Vec<&'static str>,
}

impl FakeProcessLease {
    fn new(
        initial: AttachedProcessEvidence,
        discovery_error: Option<NativeLoadObserverError>,
        connection_error: Option<AttachedProcessWitnessError>,
    ) -> Self {
        Self {
            initial,
            discovery_error,
            connection_error,
            events: Vec::new(),
        }
    }
}

impl AttachedProcessLease for FakeProcessLease {
    fn initial_evidence(&self) -> &AttachedProcessEvidence {
        &self.initial
    }

    fn reobserve(
        &mut self,
        _cancellation: &CancellationToken,
    ) -> Result<AttachedProcessEvidence, AttachedProcessWitnessError> {
        self.events.push("process_reobserve");
        Ok(self.initial.clone())
    }

    fn observe_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.events.push("connection_before");
        if let Some(error) = self.connection_error {
            return Err(error);
        }
        connection_evidence(b"before")
    }

    fn reobserve_connection(
        &mut self,
        _connection: RetainedTcpConnection,
        _initial: &RetainedTcpConnectionEvidence,
        _cancellation: &CancellationToken,
    ) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
        self.events.push("connection_after");
        connection_evidence(b"after")
    }

    fn discover_external_native_components(
        &mut self,
        _request: &NativeLoadDiscoveryRequest<'_>,
        _cancellation: &CancellationToken,
    ) -> Result<NativeLoadDiscovery, NativeLoadObserverError> {
        self.events.push("discovery");
        Err(self
            .discovery_error
            .expect("discovery test configures a failure"))
    }
}

fn connection_evidence(
    label: &[u8],
) -> Result<RetainedTcpConnectionEvidence, AttachedProcessWitnessError> {
    RetainedTcpConnectionEvidence::new(&RetainedTcpConnectionEvidenceInput {
        attribution_kind: TcpConnectionAttributionKind::WindowsContextBindingPid,
        sharing_limitation: TcpConnectionSharingLimitation::WindowsDuplicatedHandlesNotObservable,
        process_evidence_digest: Digest::sha256(b"process"),
        platform_connection_digest: Digest::sha256(label),
    })
}

fn one_request_version_server(
    version: &str,
) -> (OllamaEndpoint, TcpStream, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback server");
    let address = listener.local_addr().expect("server address");
    let endpoint =
        OllamaEndpoint::parse(&format!("http://{address}")).expect("loopback Ollama endpoint");
    let (ready_tx, ready_rx) = mpsc::sync_channel(0);
    let version = version.to_owned();
    let server = thread::spawn(move || {
        ready_tx.send(()).expect("publish server readiness");
        let (mut stream, _) = listener.accept().expect("accept probe stream");
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("set read timeout");
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while !request.ends_with(b"\r\n\r\n") {
            match stream.read(&mut byte) {
                Ok(0) | Err(_) => break,
                Ok(_) => request.push(byte[0]),
            }
        }
        if request.is_empty() {
            return String::new();
        }
        let body = format!("{{\"version\":\"{version}\"}}");
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: keep-alive\r\n\r\n{body}",
            body.len()
        )
        .expect("write response");
        stream.flush().expect("flush response");
        loop {
            match stream.read(&mut byte) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        String::from_utf8(request).expect("ASCII request")
    });
    ready_rx.recv().expect("server ready");
    let stream = TcpStream::connect(address).expect("connect retained stream");
    (endpoint, stream, server)
}

fn process_evidence(package: &RuntimePackageManifest, label: &[u8]) -> AttachedProcessEvidence {
    AttachedProcessEvidence::new(AttachedProcessEvidenceInput {
        evidence_class: AttachedProcessEvidenceClass::LinuxProcPidfd,
        owner_pid: 41,
        process_instance_digest: Digest::sha256(&[label, b":instance"].concat()),
        ownership_snapshot_digest: Digest::sha256(&[label, b":ownership"].concat()),
        entrypoint_object_digest: Digest::sha256(&[label, b":object"].concat()),
        entrypoint_digest: package.entrypoint().artifact_id().digest().clone(),
        entrypoint_bytes: package.entrypoint().byte_size(),
        platform_evidence_digest: Digest::sha256(&[label, b":platform"].concat()),
    })
    .expect("valid attached process evidence")
}

pub(super) fn native_observation(
    package: &RuntimePackageManifest,
    process_evidence_digest: Digest,
) -> NativeLoadObservation {
    let entrypoint = package.entrypoint();
    NativeLoadObservation::new(
        package,
        NativeLoadObservationInput {
            evidence_class: NativeLoadEvidenceClass::LinuxProcMapFiles,
            visibility_scope: NativeLoadVisibilityScope::FileBackedExecutableMappings,
            process_evidence_digest,
            observation_contract_id: "admission-test".to_owned(),
            observation_contract_schema_version: 1,
            components: vec![NativeLoadedComponent::new(
                entrypoint.artifact_id().clone(),
                entrypoint.byte_size(),
                NativeLoadOrigin::PackagedMember {
                    relative_path: entrypoint.relative_path().clone(),
                },
                NativeMappingClass::ExecutableImage,
                Digest::sha256(b"entrypoint object evidence"),
            )],
        },
    )
    .expect("valid native-load observation")
}

fn retained_members(
    package: &RuntimePackageManifest,
    directory: &std::path::Path,
) -> Vec<RetainedNativePackageMember> {
    package
        .members()
        .iter()
        .filter(|member| {
            member.roles().iter().any(|role| {
                matches!(
                    role,
                    RuntimePackageMemberRole::Entrypoint
                        | RuntimePackageMemberRole::NativeDependency
                        | RuntimePackageMemberRole::HelperExecutable
                )
            })
        })
        .map(|member| {
            let path = directory.join("runtime");
            std::fs::write(&path, b"entrypoint").expect("write retained entrypoint");
            RetainedNativePackageMember::new(
                member.relative_path().clone(),
                member.artifact_id().clone(),
                member.byte_size(),
                File::open(path).expect("open retained entrypoint"),
            )
            .expect("retain exact package member")
        })
        .collect()
}

pub(super) fn runtime_package(version: &str) -> RuntimePackageManifest {
    let entrypoint_path = ArtifactSetRelativePath::new("bin/runtime").expect("entrypoint path");
    let evidence_path = ArtifactSetRelativePath::new("legal/evidence.txt").expect("evidence path");
    let entrypoint = ArtifactId::from_digest(Digest::sha256(b"entrypoint"));
    let evidence = ArtifactId::from_digest(Digest::sha256(b"package evidence"));
    let artifact_set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(entrypoint.clone(), 10, entrypoint_path.clone()),
        ArtifactSetMember::new(evidence.clone(), 16, evidence_path.clone()),
    ])
    .expect("artifact set");
    RuntimePackageManifest::new(
        &artifact_set,
        ADMITTED_RUNTIME_FAMILY,
        version,
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("Linux target"),
        PackageSource::new(
            PackageSourceKind::LocalArchive,
            "local:runtime-admission-test",
            version,
            Digest::sha256(b"source evidence"),
        )
        .expect("package source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"comparison evidence"),
        },
        vec![
            RuntimePackageMember::new(
                entrypoint,
                10,
                entrypoint_path,
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                evidence,
                16,
                evidence_path,
                vec![
                    RuntimePackageMemberRole::LicenseText,
                    RuntimePackageMemberRole::ProvenanceRecord,
                ],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
        ],
    )
    .expect("runtime package")
}
