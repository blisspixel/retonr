use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::Digest;

use super::*;

fn artifact(value: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(value.as_bytes()))
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid path")
}

fn package(worker_count: usize) -> RuntimePackageManifest {
    let mut members = vec![
        RuntimePackageMember::new(
            artifact("entrypoint"),
            10,
            path("bin/ollama"),
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        RuntimePackageMember::new(
            artifact("license"),
            7,
            path("legal/license"),
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("provenance"),
            10,
            path("legal/provenance"),
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
    ];
    for ordinal in 0..worker_count {
        members.push(RuntimePackageMember::new(
            artifact(&format!("worker-{ordinal}")),
            11,
            path(&format!("lib/ollama/worker-{ordinal}")),
            vec![RuntimePackageMemberRole::WorkerExecutable],
            RuntimePackageLoadPolicy::BackendConditional,
        ));
    }
    let set = ArtifactSetManifest::new(
        members
            .iter()
            .map(|member| {
                ArtifactSetMember::new(
                    member.artifact_id().clone(),
                    member.byte_size(),
                    member.relative_path().clone(),
                )
            })
            .collect(),
    )
    .expect("valid artifact set");
    RuntimePackageManifest::new(
        &set,
        "ollama",
        "0.16.2",
        None,
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("valid target"),
        PackageSource::new(
            PackageSourceKind::UpstreamRelease,
            "https://example.invalid/ollama",
            "revision",
            Digest::sha256(b"source"),
        )
        .expect("valid source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"same"),
        },
        members,
    )
    .expect("valid package")
}

#[test]
fn production_generation_policy_is_empty_and_fail_closed() {
    let package = package(1);
    let version = "0.16.2".parse().expect("valid version");

    assert_eq!(
        OllamaManagedGenerationAdmissionPolicy::reviewed_runtime_count(),
        0
    );
    assert_eq!(
        OllamaManagedGenerationAdmissionPolicy::assess(version, &package),
        OllamaManagedGenerationAdmissionStatus::Unreviewed
    );
}

#[test]
fn generation_review_binds_version_package_and_worker() {
    let runtime = package(1);
    let worker = exact_worker_artifact_id(&runtime).expect("one exact worker");
    let version = "0.16.2".parse().expect("valid version");
    let reviewed = [ReviewedManagedGenerationRuntime {
        version,
        runtime_package_manifest_id: runtime.runtime_package_manifest_id(),
        worker_artifact_id: worker.clone(),
    }];

    assert_eq!(
        assess_runtime(
            version,
            &runtime.runtime_package_manifest_id(),
            Some(worker),
            &reviewed,
        ),
        OllamaManagedGenerationAdmissionStatus::Reviewed
    );
    assert_eq!(
        assess_runtime(
            "0.16.3".parse().expect("valid version"),
            &runtime.runtime_package_manifest_id(),
            Some(worker),
            &reviewed,
        ),
        OllamaManagedGenerationAdmissionStatus::Unreviewed
    );
    let substituted_package = package(0);
    assert_eq!(
        assess_runtime(
            version,
            &substituted_package.runtime_package_manifest_id(),
            Some(worker),
            &reviewed,
        ),
        OllamaManagedGenerationAdmissionStatus::Unreviewed
    );
    let substituted_worker = artifact("substituted-worker");
    assert_eq!(
        assess_runtime(
            version,
            &runtime.runtime_package_manifest_id(),
            Some(&substituted_worker),
            &reviewed,
        ),
        OllamaManagedGenerationAdmissionStatus::Unreviewed
    );
}

#[test]
fn generation_policy_requires_exactly_one_worker() {
    assert!(exact_worker_artifact_id(&package(0)).is_none());
    assert!(exact_worker_artifact_id(&package(2)).is_none());
}
