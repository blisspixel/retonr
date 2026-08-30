use std::fs::File;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    NativeLoadEvidenceClass, NativeMappingClass, PackageSource, PackageSourceKind,
    PackageTransformation, RuntimeAbi, RuntimeArchitecture, RuntimeOperatingSystem,
    RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::Digest;

use super::{NativeLoadDiscovery, RetainedNativePackageMember};

pub(super) const PACKAGE_VERSION: &str = "1.0.0";

pub(super) fn discovery_fixture(package: &RuntimePackageManifest) -> NativeLoadDiscovery {
    let mut external = vec![
        (ArtifactId::from_digest(Digest::sha256(b"platform one")), 11),
        (ArtifactId::from_digest(Digest::sha256(b"platform two")), 22),
    ];
    external.sort_by(|left, right| left.0.digest().as_str().cmp(right.0.digest().as_str()));
    let components = external
        .into_iter()
        .map(|(artifact_id, byte_size)| {
            serde_json::json!({
                "artifact_id": artifact_id,
                "byte_size": byte_size,
                "mapping_class": NativeMappingClass::ExecutableMapped
            })
        })
        .collect::<Vec<_>>();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "authority": "none",
        "evidence_class": NativeLoadEvidenceClass::LinuxProcMapFiles,
        "external_components": components,
        "observation_contract_id": "test-native-load",
        "observation_contract_schema_version": 1,
        "process_evidence_digest": Digest::sha256(b"process evidence"),
        "runtime_package_manifest_id": package.runtime_package_manifest_id(),
        "schema_version": super::super::NATIVE_LOAD_DISCOVERY_SCHEMA_VERSION,
        "status": "proposed"
    }))
    .expect("encode discovery fixture");
    NativeLoadDiscovery::from_json_bytes(&bytes, &package.runtime_package_manifest_id())
        .expect("parse canonical discovery fixture")
}

pub(super) fn clone_retained(member: &RetainedNativePackageMember) -> RetainedNativePackageMember {
    RetainedNativePackageMember::new(
        member.relative_path().clone(),
        member.artifact_id().clone(),
        member.byte_size(),
        member.file().try_clone().expect("clone retained member"),
    )
    .expect("clone exact retained member")
}

pub(super) fn retained_members(
    package: &RuntimePackageManifest,
    root: &std::path::Path,
) -> Vec<RetainedNativePackageMember> {
    package
        .members()
        .iter()
        .filter(|member| super::super::is_retained_package_member(member))
        .map(|member| {
            let bytes: &[u8] = if member.relative_path().as_str().contains("runtime") {
                b"entrypoint"
            } else {
                b"native"
            };
            let file_path = root.join(member.relative_path().as_str().replace('/', "_"));
            std::fs::write(&file_path, bytes).expect("write retained member");
            RetainedNativePackageMember::new(
                member.relative_path().clone(),
                member.artifact_id().clone(),
                member.byte_size(),
                File::open(file_path).expect("open retained member"),
            )
            .expect("retain package member")
        })
        .collect()
}

pub(super) fn package_with_version(version: &str) -> RuntimePackageManifest {
    let entrypoint_path = ArtifactSetRelativePath::new("bin/runtime").expect("entrypoint path");
    let native_path = ArtifactSetRelativePath::new("lib/native.so").expect("native path");
    let evidence_path = ArtifactSetRelativePath::new("legal/evidence.txt").expect("evidence path");
    let entrypoint = ArtifactId::from_digest(Digest::sha256(b"entrypoint"));
    let native = ArtifactId::from_digest(Digest::sha256(b"native"));
    let evidence = ArtifactId::from_digest(Digest::sha256(b"package evidence"));
    let artifact_set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(entrypoint.clone(), 10, entrypoint_path.clone()),
        ArtifactSetMember::new(evidence.clone(), 16, evidence_path.clone()),
        ArtifactSetMember::new(native.clone(), 6, native_path.clone()),
    ])
    .expect("artifact set");
    RuntimePackageManifest::new(
        &artifact_set,
        "native-load-test",
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
            "local:native-load-test",
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
            RuntimePackageMember::new(
                native,
                6,
                native_path,
                vec![RuntimePackageMemberRole::NativeDependency],
                RuntimePackageLoadPolicy::BackendConditional,
            ),
        ],
    )
    .expect("runtime package")
}
