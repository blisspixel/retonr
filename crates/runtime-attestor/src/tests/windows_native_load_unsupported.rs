use std::{fs::File, net::Ipv4Addr};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::{CancellationToken, Digest};

use crate::{
    AttachedProcessLease, AttachedProcessObserver, ListenerEndpoint, NativeAttachedProcessObserver,
    NativeLoadObservationLimits, NativeLoadObservationRequest, NativeLoadObserverError,
    RetainedNativePackageMember,
};

#[test]
fn windows_native_load_observation_fails_closed_without_object_binding() {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind listener");
    let endpoint = ListenerEndpoint::new(listener.local_addr().expect("listener address"))
        .expect("loopback endpoint");
    let cancellation = CancellationToken::new();
    let mut lease = NativeAttachedProcessObserver
        .attach(
            endpoint,
            crate::AttachedProcessWitnessLimits::default(),
            &cancellation,
        )
        .expect("attach current listener");
    let package = package();
    let package_id = package.runtime_package_manifest_id();
    let directory = tempfile::tempdir().expect("temporary package member");
    let entrypoint = directory.path().join("runtime.exe");
    std::fs::write(&entrypoint, b"entrypoint").expect("write package member");
    let declared = package.entrypoint();
    let retained = [RetainedNativePackageMember::new(
        declared.relative_path().clone(),
        declared.artifact_id().clone(),
        declared.byte_size(),
        File::open(entrypoint).expect("open package member"),
    )
    .expect("retain package member")];
    let request = NativeLoadObservationRequest {
        package: &package,
        expected_package_id: &package_id,
        retained_package_members: &retained,
        expected_external_components: &[],
        limits: NativeLoadObservationLimits::default(),
    };
    assert_eq!(
        lease.observe_native_load(&request, &cancellation),
        Err(NativeLoadObserverError::Unsupported)
    );
}

fn package() -> RuntimePackageManifest {
    let entrypoint_path = ArtifactSetRelativePath::new("bin/runtime.exe").expect("entrypoint path");
    let evidence_path =
        ArtifactSetRelativePath::new("evidence/package.txt").expect("evidence path");
    let entrypoint = ArtifactId::from_digest(Digest::sha256(b"entrypoint"));
    let evidence = ArtifactId::from_digest(Digest::sha256(b"package evidence"));
    let artifact_set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(entrypoint.clone(), 10, entrypoint_path.clone()),
        ArtifactSetMember::new(evidence.clone(), 16, evidence_path.clone()),
    ])
    .expect("artifact set");
    RuntimePackageManifest::new(
        &artifact_set,
        "windows-native-load-test",
        "1.0.0",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Windows,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::WindowsMsvc,
        )
        .expect("Windows target"),
        PackageSource::new(
            PackageSourceKind::LocalArchive,
            "local:windows-native-load-test",
            "1",
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
