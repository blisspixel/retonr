use std::{fs::File, path::Path};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::Digest;

use super::super::{
    NativeLoadObservationLimits, NativeLoadObservationRequest, RetainedNativePackageMember,
};

#[test]
fn listener_subject_retains_worker_without_claiming_a_worker_mapping() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let package = package();
    let package_id = package.runtime_package_manifest_id();
    let retained = package
        .members()
        .iter()
        .filter(|member| super::super::is_retained_package_member(member))
        .map(|member| retain(member, temporary.path()))
        .collect::<Vec<_>>();
    let request = NativeLoadObservationRequest {
        package: &package,
        expected_package_id: &package_id,
        retained_package_members: &retained,
        expected_external_components: &[],
        limits: NativeLoadObservationLimits::default(),
    };

    assert_eq!(request.validate(), Ok(request.limits));
    assert_eq!(retained.len(), 2);
    assert!(retained.iter().any(|member| {
        package.members().iter().any(|declared| {
            declared.artifact_id() == member.artifact_id()
                && declared
                    .roles()
                    .contains(&RuntimePackageMemberRole::WorkerExecutable)
        })
    }));
}

fn package() -> RuntimePackageManifest {
    let worker_path = path("bin/llama-server");
    let entrypoint_path = path("bin/ollama");
    let evidence_path = path("legal/evidence");
    let worker = artifact(b"worker");
    let entrypoint = artifact(b"entrypoint");
    let evidence = artifact(b"evidence");
    let artifact_set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(worker.clone(), 6, worker_path.clone()),
        ArtifactSetMember::new(entrypoint.clone(), 10, entrypoint_path.clone()),
        ArtifactSetMember::new(evidence.clone(), 8, evidence_path.clone()),
    ])
    .expect("artifact set");
    RuntimePackageManifest::new(
        &artifact_set,
        "ollama",
        "0.32.15",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("target"),
        PackageSource::new(
            PackageSourceKind::LocalArchive,
            "local:worker-subject",
            "0.32.15",
            Digest::sha256(b"source"),
        )
        .expect("source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"transformation"),
        },
        vec![
            RuntimePackageMember::new(
                worker,
                6,
                worker_path,
                vec![RuntimePackageMemberRole::WorkerExecutable],
                RuntimePackageLoadPolicy::BackendConditional,
            ),
            RuntimePackageMember::new(
                entrypoint,
                10,
                entrypoint_path,
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                evidence,
                8,
                evidence_path,
                vec![
                    RuntimePackageMemberRole::LicenseText,
                    RuntimePackageMemberRole::ProvenanceRecord,
                ],
                RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
            ),
        ],
    )
    .expect("package")
}

fn retain(member: &RuntimePackageMember, root: &Path) -> RetainedNativePackageMember {
    let bytes: &[u8] = if member
        .roles()
        .contains(&RuntimePackageMemberRole::WorkerExecutable)
    {
        b"worker"
    } else {
        b"entrypoint"
    };
    let file_path = root.join(member.relative_path().as_str().replace('/', "_"));
    std::fs::write(&file_path, bytes).expect("write member");
    RetainedNativePackageMember::new(
        member.relative_path().clone(),
        member.artifact_id().clone(),
        member.byte_size(),
        File::open(file_path).expect("open member"),
    )
    .expect("retain member")
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("path")
}

fn artifact(bytes: &[u8]) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(bytes))
}
