use rewrite_model::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    PackageSource, PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_ollama::{
    OllamaCloudDisableVersionStatus, OllamaManagedGenerationAdmissionStatus, OllamaVersion,
};
use rewrite_runtime_attestor::FrozenExternalNativeComponentSetId;
use rewrite_types::Digest;

use super::super::*;
use crate::{RuntimeAdmissionEvidenceFoundation, RuntimeAdmissionEvidenceFoundationInput};

pub(super) fn review_bytes(
    admitted: &VerifiedAdmittedRuntime,
    package: &RuntimePackageManifest,
) -> Vec<u8> {
    let worker = package
        .members()
        .iter()
        .find(|member| {
            member
                .roles()
                .contains(&RuntimePackageMemberRole::WorkerExecutable)
        })
        .expect("fixture worker");
    review_bytes_for_worker(admitted, package, worker.artifact_id())
}

pub(super) fn review_bytes_for_worker(
    admitted: &VerifiedAdmittedRuntime,
    package: &RuntimePackageManifest,
    worker: &ArtifactId,
) -> Vec<u8> {
    encode(&ManagedGenerationPathReviewWire {
        admitted_runtime_id: admitted.admitted_runtime_id().digest().clone(),
        frozen_external_component_set_id: admitted.frozen_external_component_set_id().clone(),
        package_source_id: admitted.package_source_id().clone(),
        procedure_id: MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_ID.to_owned(),
        procedure_version: MANAGED_GENERATION_PATH_REVIEW_PROCEDURE_VERSION,
        reviewed_source_build_inputs_id: admitted.source_build_inputs_id().clone(),
        runtime_package_manifest_id: package.runtime_package_manifest_id(),
        runtime_version: package.reported_version().to_owned(),
        schema_version: MANAGED_GENERATION_PATH_REVIEW_SCHEMA_VERSION,
        status: GenerationPathReviewStatus::Reviewed,
        worker_artifact_id: worker.clone(),
    })
    .expect("review encoding")
}

pub(super) fn admitted(
    package: &RuntimePackageManifest,
    identity: &[u8],
) -> VerifiedAdmittedRuntime {
    let source_build_inputs_id = artifact_set_id(b"reviewed source build inputs");
    let foundation =
        RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
            artifact_set_id(b"evidence"),
            source_build_inputs_id.clone(),
            Digest::sha256(b"source manifest"),
            Digest::sha256(b"build plan"),
            Digest::sha256(b"source report"),
            package.runtime_package_manifest_id(),
        ))
        .expect("foundation");
    VerifiedAdmittedRuntime {
        admitted_runtime_id: VerifiedAdmittedRuntimeId(Digest::sha256(identity)),
        foundation_id: foundation.foundation_id().clone(),
        frozen_external_component_set_id: frozen_set_id(b"frozen external components"),
        cloud_disable_version_status: OllamaCloudDisableVersionStatus::Reviewed,
        runtime_package_manifest_id: package.runtime_package_manifest_id(),
        source_build_inputs_id,
        package_source_id: package.source().package_source_id(),
        startup_launch_spec_digest: Digest::sha256(b"launch"),
    }
}

pub(super) fn package_with_workers(worker_count: usize) -> RuntimePackageManifest {
    let mut specifications = vec![
        (
            "bin/ollama",
            b"entrypoint".as_slice(),
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        (
            "legal/license.txt",
            b"license".as_slice(),
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        (
            "provenance/source.json",
            b"provenance".as_slice(),
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
    ];
    for index in 0..worker_count {
        specifications.push((
            if index == 0 {
                "lib/ollama/worker"
            } else {
                "lib/ollama/worker-2"
            },
            if index == 0 {
                b"worker".as_slice()
            } else {
                b"worker-2".as_slice()
            },
            vec![RuntimePackageMemberRole::WorkerExecutable],
            RuntimePackageLoadPolicy::BackendConditional,
        ));
    }
    specifications.sort_by_key(|(path, ..)| *path);
    let set_members = specifications
        .iter()
        .map(|(path, bytes, ..)| {
            ArtifactSetMember::new(
                ArtifactId::from_digest(Digest::sha256(bytes)),
                u64::try_from(bytes.len()).expect("fixture length"),
                relative_path(path),
            )
        })
        .collect::<Vec<_>>();
    let artifact_set = ArtifactSetManifest::new(set_members).expect("artifact set");
    let package_members = specifications
        .iter()
        .zip(artifact_set.members())
        .map(|((_, _, roles, policy), member)| {
            RuntimePackageMember::new(
                member.artifact_id().clone(),
                member.byte_size(),
                member.relative_path().clone(),
                roles.clone(),
                *policy,
            )
        })
        .collect();
    RuntimePackageManifest::new(
        &artifact_set,
        "ollama",
        "0.32.15",
        Some("reviewed-revision".to_owned()),
        RuntimeTarget::new(
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc,
        )
        .expect("target"),
        PackageSource::new(
            PackageSourceKind::RepositoryRevision,
            "https://example.invalid/ollama",
            "reviewed-revision",
            Digest::sha256(b"source provenance"),
        )
        .expect("source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"unchanged"),
        },
        package_members,
    )
    .expect("runtime package")
}

pub(super) fn artifact_set_id(bytes: &[u8]) -> ArtifactSetId {
    ArtifactSetManifest::new(vec![ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture length"),
        relative_path("input.bin"),
    )])
    .expect("single-member set")
    .artifact_set_id()
}

fn relative_path(path: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(path.to_owned()).expect("fixture path")
}

pub(super) fn frozen_set_id(bytes: &[u8]) -> FrozenExternalNativeComponentSetId {
    serde_json::from_value(serde_json::json!(Digest::sha256(bytes))).expect("frozen set identity")
}

pub(super) fn verify_reviewed_path(
    review: &[u8],
    admitted: &VerifiedAdmittedRuntime,
    package: &RuntimePackageManifest,
) -> Result<VerifiedManagedGenerationPath, RuntimeAdmissionAuthorityError> {
    VerifiedManagedGenerationPath::verify_with_admission_status(
        review,
        admitted,
        package,
        version(),
        OllamaManagedGenerationAdmissionStatus::Reviewed,
    )
}

pub(super) fn version() -> OllamaVersion {
    "0.32.15".parse().expect("fixture version")
}
