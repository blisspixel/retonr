use std::{fs::File, time::Duration};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    NativeMappingClass, PackageSource, PackageSourceKind, PackageTransformation, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest,
    RuntimePackageMember, RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::Digest;

use super::{
    ExpectedExternalNativeComponent, MAXIMUM_NATIVE_LOAD_HASH_BYTES,
    MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS, MAXIMUM_NATIVE_LOADED_COMPONENTS,
    MAXIMUM_NATIVE_MAPPING_METADATA_BYTES, MAXIMUM_NATIVE_MAPPING_REGIONS,
    NativeLoadObservationLimits, NativeLoadObservationRequest, NativeLoadObserverError,
    RetainedNativePackageMember, expected_key,
};

#[test]
fn limits_reject_zero_and_hard_maximum_overflow() {
    let invalid = [
        NativeLoadObservationLimits {
            maximum_mapping_regions: 0,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_mapping_regions: MAXIMUM_NATIVE_MAPPING_REGIONS + 1,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_mapping_metadata_bytes: 0,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_mapping_metadata_bytes: MAXIMUM_NATIVE_MAPPING_METADATA_BYTES + 1,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_components: 0,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_components: MAXIMUM_NATIVE_LOADED_COMPONENTS + 1,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_aggregate_hash_bytes: 0,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_aggregate_hash_bytes: MAXIMUM_NATIVE_LOAD_HASH_BYTES + 1,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_elapsed: Duration::ZERO,
            ..NativeLoadObservationLimits::default()
        },
        NativeLoadObservationLimits {
            maximum_elapsed: Duration::from_millis(MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS + 1),
            ..NativeLoadObservationLimits::default()
        },
    ];
    for limits in invalid {
        assert_eq!(
            limits.validate(),
            Err(NativeLoadObserverError::InvalidLimits)
        );
    }
    let maximums = NativeLoadObservationLimits {
        maximum_mapping_regions: MAXIMUM_NATIVE_MAPPING_REGIONS,
        maximum_mapping_metadata_bytes: MAXIMUM_NATIVE_MAPPING_METADATA_BYTES,
        maximum_components: MAXIMUM_NATIVE_LOADED_COMPONENTS,
        maximum_aggregate_hash_bytes: MAXIMUM_NATIVE_LOAD_HASH_BYTES,
        maximum_elapsed: Duration::from_millis(MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS),
    };
    assert_eq!(maximums.validate(), Ok(maximums));
}

#[test]
fn retained_member_requires_an_exact_nonempty_regular_file() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let path = ArtifactSetRelativePath::new("bin/runtime").expect("relative path");
    let artifact_id = ArtifactId::from_digest(Digest::sha256(b"entrypoint"));
    let member_path = temporary.path().join("runtime");
    std::fs::write(&member_path, b"entrypoint").expect("write member");

    let member = RetainedNativePackageMember::new(
        path.clone(),
        artifact_id.clone(),
        10,
        File::open(&member_path).expect("open member"),
    )
    .expect("retain exact member");
    assert_eq!(member.relative_path(), &path);
    assert_eq!(member.artifact_id(), &artifact_id);
    assert_eq!(member.byte_size(), 10);
    let debug = format!("{member:?}");
    assert!(debug.contains("bin/runtime"));
    assert!(!debug.contains(&temporary.path().display().to_string()));

    for (byte_size, file) in [
        (0, File::open(&member_path).expect("open zero case")),
        (9, File::open(&member_path).expect("open size case")),
    ] {
        assert!(matches!(
            RetainedNativePackageMember::new(path.clone(), artifact_id.clone(), byte_size, file,),
            Err(NativeLoadObserverError::InvalidRequest)
        ));
    }
}

#[test]
fn request_validation_binds_every_retained_member_and_external_component() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let package = package();
    let package_id = package.runtime_package_manifest_id();
    let retained = retained_members(&package, temporary.path());
    let mut external = [
        ExpectedExternalNativeComponent::new(
            ArtifactId::from_digest(Digest::sha256(b"platform one")),
            12,
            NativeMappingClass::ExecutableMapped,
        ),
        ExpectedExternalNativeComponent::new(
            ArtifactId::from_digest(Digest::sha256(b"platform two")),
            12,
            NativeMappingClass::ExecutableMapped,
        ),
    ];
    external.sort_by_key(expected_key);
    let request = NativeLoadObservationRequest {
        package: &package,
        expected_package_id: &package_id,
        retained_package_members: &retained,
        expected_external_components: &external,
        limits: NativeLoadObservationLimits::default(),
    };
    assert_eq!(request.validate(), Ok(request.limits));

    let other_id = package_with_version("2.0.0").runtime_package_manifest_id();
    assert_eq!(
        NativeLoadObservationRequest {
            expected_package_id: &other_id,
            ..request
        }
        .validate(),
        Err(NativeLoadObserverError::InvalidRequest)
    );
    assert_eq!(
        NativeLoadObservationRequest {
            retained_package_members: &retained[..1],
            ..request
        }
        .validate(),
        Err(NativeLoadObserverError::InvalidRequest)
    );
    let reversed = [clone_retained(&retained[1]), clone_retained(&retained[0])];
    assert_eq!(
        NativeLoadObservationRequest {
            retained_package_members: &reversed,
            ..request
        }
        .validate(),
        Err(NativeLoadObserverError::InvalidRequest)
    );
    assert_eq!(
        NativeLoadObservationRequest {
            limits: NativeLoadObservationLimits {
                maximum_components: 1,
                ..request.limits
            },
            ..request
        }
        .validate(),
        Err(NativeLoadObserverError::InvalidRequest)
    );
}

#[test]
fn request_rejects_noncanonical_external_component_policies() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let package = package();
    let package_id = package.runtime_package_manifest_id();
    let retained = retained_members(&package, temporary.path());
    let first = ExpectedExternalNativeComponent::new(
        ArtifactId::from_digest(Digest::sha256(b"first")),
        5,
        NativeMappingClass::ExecutableMapped,
    );
    let second = ExpectedExternalNativeComponent::new(
        ArtifactId::from_digest(Digest::sha256(b"second")),
        6,
        NativeMappingClass::ExecutableMapped,
    );
    let mut canonical = [first.clone(), second.clone()];
    canonical.sort_by_key(expected_key);
    let invalid = [
        vec![ExpectedExternalNativeComponent::new(
            first.artifact_id().clone(),
            0,
            NativeMappingClass::ExecutableMapped,
        )],
        vec![ExpectedExternalNativeComponent::new(
            first.artifact_id().clone(),
            5,
            NativeMappingClass::ExecutableImage,
        )],
        vec![ExpectedExternalNativeComponent::new(
            first.artifact_id().clone(),
            5,
            NativeMappingClass::DataMapped,
        )],
        vec![canonical[1].clone(), canonical[0].clone()],
        vec![first.clone(), first],
    ];
    for expected_external_components in &invalid {
        assert_eq!(
            NativeLoadObservationRequest {
                package: &package,
                expected_package_id: &package_id,
                retained_package_members: &retained,
                expected_external_components,
                limits: NativeLoadObservationLimits::default(),
            }
            .validate(),
            Err(NativeLoadObserverError::InvalidRequest)
        );
    }
}

fn clone_retained(member: &RetainedNativePackageMember) -> RetainedNativePackageMember {
    RetainedNativePackageMember::new(
        member.relative_path().clone(),
        member.artifact_id().clone(),
        member.byte_size(),
        member.file().try_clone().expect("clone retained member"),
    )
    .expect("clone exact retained member")
}

fn retained_members(
    package: &RuntimePackageManifest,
    root: &std::path::Path,
) -> Vec<RetainedNativePackageMember> {
    package
        .members()
        .iter()
        .filter(|member| super::is_retained_package_member(member))
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

fn package() -> RuntimePackageManifest {
    package_with_version("1.0.0")
}

fn package_with_version(version: &str) -> RuntimePackageManifest {
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
