use rewrite_types::Digest;

use crate::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, PackageSource,
    PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture, RuntimeBuildMode,
    RuntimeOperatingSystem, RuntimeTarget,
};

use super::{
    MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES, RuntimePackageLoadPolicy, RuntimePackageManifest,
    RuntimePackageManifestError, RuntimePackageMember, RuntimePackageMemberRole,
};

const TRANSFORMED_RUNTIME_GOLDEN_ID: &str =
    "fb518ac939b1ca4c853376ccf2be586c9dadecfb17b19e835779450328b266f6";

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("valid path")
}

fn artifact(value: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(value.as_bytes()))
}

fn base_manifest() -> ArtifactSetManifest {
    ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(artifact("entrypoint"), 10, path("bin/runtime")),
        ArtifactSetMember::new(artifact("build"), 11, path("config/build.json")),
        ArtifactSetMember::new(artifact("license"), 12, path("legal/license.txt")),
        ArtifactSetMember::new(artifact("provenance"), 13, path("legal/provenance.txt")),
        ArtifactSetMember::new(artifact("dependency"), 14, path("lib/backend.so")),
    ])
    .expect("valid artifact set")
}

fn source() -> PackageSource {
    PackageSource::new(
        PackageSourceKind::UpstreamRelease,
        "https://example.invalid/runtime",
        "v1.2.3",
        Digest::sha256(b"source provenance"),
    )
    .expect("valid source")
}

fn members() -> Vec<RuntimePackageMember> {
    vec![
        RuntimePackageMember::new(
            artifact("entrypoint"),
            10,
            path("bin/runtime"),
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        RuntimePackageMember::new(
            artifact("build"),
            11,
            path("config/build.json"),
            vec![RuntimePackageMemberRole::BuildConfiguration],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("license"),
            12,
            path("legal/license.txt"),
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("provenance"),
            13,
            path("legal/provenance.txt"),
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        RuntimePackageMember::new(
            artifact("dependency"),
            14,
            path("lib/backend.so"),
            vec![RuntimePackageMemberRole::NativeDependency],
            RuntimePackageLoadPolicy::BackendConditional,
        ),
    ]
}

fn target() -> RuntimeTarget {
    RuntimeTarget::new(
        RuntimeOperatingSystem::Linux,
        RuntimeArchitecture::X86_64,
        RuntimeAbi::LinuxGnuLibc,
    )
    .expect("valid target")
}

fn manifest() -> RuntimePackageManifest {
    RuntimePackageManifest::new(
        &base_manifest(),
        "fixture-runtime",
        "1.2.3",
        Some("revision-1".to_owned()),
        target(),
        source(),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"untransformed"),
        },
        members(),
    )
    .expect("valid package")
}

#[test]
fn runtime_package_has_stable_identity_and_derived_build_fields() {
    let package = manifest();
    assert!(package.schema_version() == 1 && package.runtime_family() == "fixture-runtime");
    assert_eq!(package.reported_version(), "1.2.3");
    assert_eq!(package.build_revision(), Some("revision-1"));
    assert_eq!(package.target(), target());
    assert_eq!(package.source(), &source());
    assert_eq!(package.members().len(), 5);
    assert_eq!(package.entrypoint().relative_path().as_str(), "bin/runtime");
    assert_eq!(
        package.runtime_package_manifest_id().digest().as_str(),
        "b730dfc87756143b6879224c8fe187e70eacf3cd7140e770a64b4c092eb2c9c4"
    );

    let build = crate::RuntimeBuildIdentity::new_from_package_manifest(
        RuntimeBuildMode::ManagedProcess,
        &package,
    )
    .expect("typed build");
    assert_eq!(
        build.entrypoint_digest(),
        package.entrypoint().artifact_id().digest()
    );
    assert_eq!(
        build.package_manifest_digest(),
        package.runtime_package_manifest_id().digest()
    );
    assert_eq!(
        build.packaged_dependencies_digest(),
        &package.packaged_dependencies_digest()
    );
    assert_eq!(
        build.build_configuration_digest(),
        &package.build_configuration_digest()
    );
}

#[test]
fn runtime_package_round_trips_and_rejects_encoding_boundaries() {
    let base = base_manifest();
    let package = manifest();
    let encoded = serde_json::to_vec(&package).expect("serialize");
    assert_eq!(
        RuntimePackageManifest::from_json_bytes(&encoded, &base).expect("decode"),
        package
    );
    let mut value = serde_json::to_value(&package).expect("value");
    value["schema_version"] = serde_json::json!(2);
    assert_eq!(
        RuntimePackageManifest::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &base
        ),
        Err(RuntimePackageManifestError::UnsupportedSchema(2))
    );
    value["schema_version"] = serde_json::json!(1);
    value["extra"] = serde_json::json!(true);
    assert_eq!(
        RuntimePackageManifest::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &base
        ),
        Err(RuntimePackageManifestError::InvalidEncoding)
    );
    assert_eq!(
        RuntimePackageManifest::from_json_bytes(
            &vec![b' '; MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES + 1],
            &base
        ),
        Err(RuntimePackageManifestError::EncodedManifestTooLarge)
    );
}

#[test]
fn runtime_package_rejects_incomplete_or_inconsistent_semantics() {
    let base = base_manifest();
    let construct = |members, transformation| {
        RuntimePackageManifest::new(
            &base,
            "fixture-runtime",
            "1.2.3",
            None,
            target(),
            source(),
            transformation,
            members,
        )
    };
    let untransformed = || PackageTransformation::Untransformed {
        evidence_digest: Digest::sha256(b"same"),
    };

    let mut invalid = members();
    invalid.pop();
    assert_eq!(
        construct(invalid, untransformed()),
        Err(RuntimePackageManifestError::MemberCoverageMismatch)
    );
    let mut invalid = members();
    invalid[0] = RuntimePackageMember::new(
        artifact("entrypoint"),
        10,
        path("bin/runtime"),
        vec![RuntimePackageMemberRole::Entrypoint],
        RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
    );
    assert_eq!(
        construct(invalid, untransformed()),
        Err(RuntimePackageManifestError::InvalidLoadPolicy)
    );
    let mut invalid = members();
    invalid[2] = RuntimePackageMember::new(
        artifact("license"),
        12,
        path("legal/license.txt"),
        vec![RuntimePackageMemberRole::RuntimeResource],
        RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
    );
    assert_eq!(
        construct(invalid, untransformed()),
        Err(RuntimePackageManifestError::MissingEvidence)
    );
    let mut invalid = members();
    invalid[1] = RuntimePackageMember::new(
        artifact("build"),
        11,
        path("config/build.json"),
        vec![
            RuntimePackageMemberRole::BuildConfiguration,
            RuntimePackageMemberRole::BuildConfiguration,
        ],
        RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
    );
    assert_eq!(
        construct(invalid, untransformed()),
        Err(RuntimePackageManifestError::InvalidMemberRoles)
    );
    assert_eq!(
        construct(
            members(),
            PackageTransformation::Transformed {
                source_artifact_set_id: base.artifact_set_id(),
                tool_evidence_digest: Digest::sha256(b"tool"),
                parameters_digest: Digest::sha256(b"parameters"),
                log_digest: Digest::sha256(b"log"),
            }
        ),
        Err(RuntimePackageManifestError::MissingTransformationEvidence)
    );
}

#[test]
fn runtime_package_identity_changes_with_every_semantic_class() {
    let baseline = manifest();
    let base = base_manifest();
    let changed = RuntimePackageManifest::new(
        &base,
        "fixture-runtime",
        "1.2.4",
        baseline.build_revision().map(str::to_owned),
        target(),
        source(),
        baseline.transformation().clone(),
        members(),
    )
    .expect("changed package");
    assert_ne!(
        changed.runtime_package_manifest_id(),
        baseline.runtime_package_manifest_id()
    );
    let mut changed_members = members();
    changed_members[4] = RuntimePackageMember::new(
        artifact("dependency"),
        14,
        path("lib/backend.so"),
        vec![RuntimePackageMemberRole::NativeDependency],
        RuntimePackageLoadPolicy::RequiredAtReady,
    );
    let changed = RuntimePackageManifest::new(
        &base,
        baseline.runtime_family(),
        baseline.reported_version(),
        baseline.build_revision().map(str::to_owned),
        target(),
        source(),
        baseline.transformation().clone(),
        changed_members,
    )
    .expect("changed policy");
    assert_ne!(
        changed.runtime_package_manifest_id(),
        baseline.runtime_package_manifest_id()
    );
}

#[test]
fn transformed_runtime_package_round_trips_every_role_and_accessor() {
    let members = vec![
        runtime_member(
            "entrypoint-2",
            20,
            "bin/runtime",
            vec![RuntimePackageMemberRole::Entrypoint],
            RuntimePackageLoadPolicy::RequiredAtReady,
        ),
        runtime_member(
            "helper",
            21,
            "bin/tool",
            vec![RuntimePackageMemberRole::HelperExecutable],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        runtime_member(
            "build-2",
            22,
            "config/build.json",
            vec![RuntimePackageMemberRole::BuildConfiguration],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        runtime_member(
            "default",
            23,
            "config/default.json",
            vec![RuntimePackageMemberRole::DefaultConfiguration],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        runtime_member(
            "license-2",
            24,
            "legal/license.txt",
            vec![RuntimePackageMemberRole::LicenseText],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        runtime_member(
            "provenance-2",
            25,
            "legal/provenance.txt",
            vec![RuntimePackageMemberRole::ProvenanceRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        runtime_member(
            "transformation",
            26,
            "legal/transformation.json",
            vec![RuntimePackageMemberRole::TransformationRecord],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
        runtime_member(
            "dependency-2",
            27,
            "lib/backend.so",
            vec![RuntimePackageMemberRole::NativeDependency],
            RuntimePackageLoadPolicy::BackendConditional,
        ),
        runtime_member(
            "resource",
            28,
            "resources/default.bin",
            vec![RuntimePackageMemberRole::RuntimeResource],
            RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
        ),
    ];
    let artifact_set = artifact_set_for(&members);
    let package = RuntimePackageManifest::new(
        &artifact_set,
        "fixture-runtime",
        "2.0.0",
        None,
        target(),
        source(),
        PackageTransformation::Transformed {
            source_artifact_set_id: base_manifest().artifact_set_id(),
            tool_evidence_digest: Digest::sha256(b"tool"),
            parameters_digest: Digest::sha256(b"parameters"),
            log_digest: Digest::sha256(b"log"),
        },
        members,
    )
    .expect("valid transformed package");
    assert_eq!(package.artifact_set_id(), &artifact_set.artifact_set_id());
    let helper = &package.members()[1];
    assert_eq!(helper.roles().len(), 1);
    let helper_policy = helper.load_policy();
    assert_eq!(helper_policy, RuntimePackageLoadPolicy::MustNotBeCodeLoaded);
    let encoded = serde_json::to_vec(&package).expect("serialize");
    assert_eq!(
        RuntimePackageManifest::from_json_bytes(&encoded, &artifact_set).expect("decode"),
        package
    );
    let package_id = package.runtime_package_manifest_id();
    assert_eq!(package_id.digest().as_str(), TRANSFORMED_RUNTIME_GOLDEN_ID);
}

#[test]
fn runtime_package_rejects_metadata_reference_and_nested_decode_drift() {
    let base = base_manifest();
    assert_eq!(
        RuntimePackageManifest::new(
            &base,
            "Bad_Family",
            "version with space",
            Some(String::new()),
            target(),
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"same"),
            },
            members(),
        ),
        Err(RuntimePackageManifestError::InvalidMetadata)
    );
    let mut no_entrypoint = members();
    no_entrypoint[0] = runtime_member(
        "entrypoint",
        10,
        "bin/runtime",
        vec![RuntimePackageMemberRole::RuntimeResource],
        RuntimePackageLoadPolicy::MustNotBeCodeLoaded,
    );
    assert_eq!(
        RuntimePackageManifest::new(
            &base,
            "fixture-runtime",
            "1.0.0",
            None,
            target(),
            source(),
            PackageTransformation::Untransformed {
                evidence_digest: Digest::sha256(b"same"),
            },
            no_entrypoint,
        ),
        Err(RuntimePackageManifestError::InvalidEntrypoint)
    );
    let package = manifest();
    let value = serde_json::to_value(&package).expect("value");
    let other = ArtifactSetManifest::new(vec![ArtifactSetMember::new(
        artifact("other"),
        1,
        path("other"),
    )])
    .expect("other set");
    assert_eq!(
        RuntimePackageManifest::from_json_bytes(
            &serde_json::to_vec(&value).expect("encode"),
            &other
        ),
        Err(RuntimePackageManifestError::ArtifactSetMismatch)
    );
    assert_eq!(
        package.validate_against(&other),
        Err(RuntimePackageManifestError::ArtifactSetMismatch)
    );
    let mut invalid = value.clone();
    invalid["members"][0]["relative_path"] = serde_json::json!("../runtime");
    assert_eq!(
        decode_value(&invalid, &base),
        Err(RuntimePackageManifestError::InvalidMemberPath)
    );
    let mut invalid = value;
    invalid["source"]["locator"] = serde_json::json!("https://user@example.invalid/runtime");
    assert_eq!(
        decode_value(&invalid, &base),
        Err(RuntimePackageManifestError::InvalidSource)
    );
}

fn runtime_member(
    name: &str,
    byte_size: u64,
    relative_path: &str,
    roles: Vec<RuntimePackageMemberRole>,
    load_policy: RuntimePackageLoadPolicy,
) -> RuntimePackageMember {
    RuntimePackageMember::new(
        artifact(name),
        byte_size,
        path(relative_path),
        roles,
        load_policy,
    )
}

fn artifact_set_for(members: &[RuntimePackageMember]) -> ArtifactSetManifest {
    ArtifactSetManifest::new(
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
    .expect("valid artifact set")
}

fn decode_value(
    value: &serde_json::Value,
    artifact_set: &ArtifactSetManifest,
) -> Result<RuntimePackageManifest, RuntimePackageManifestError> {
    RuntimePackageManifest::from_json_bytes(
        &serde_json::to_vec(value).expect("encode"),
        artifact_set,
    )
}
