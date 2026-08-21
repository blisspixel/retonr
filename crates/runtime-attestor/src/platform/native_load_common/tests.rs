use std::{io::Write as _, time::Instant};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    NativeLoadEvidenceClass, NativeLoadOrigin, NativeLoadedComponent, NativeMappingClass,
    PackageSource, PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::{CancellationToken, Digest};

use super::{HashBudget, finish_observation, hash_file};
use crate::{
    ExpectedExternalNativeComponent, NativeLoadObservationLimits, NativeLoadObserverError,
};

#[test]
fn file_hash_honors_budget_cancellation_and_exact_length() {
    let mut file = tempfile::tempfile().expect("temporary file");
    file.write_all(b"bounded").expect("write fixture");
    let limits = NativeLoadObservationLimits::default();
    let cancellation = CancellationToken::new();
    let mut budget = HashBudget::new(6);
    assert_eq!(
        hash_file(
            &mut file,
            7,
            &mut budget,
            limits,
            &cancellation,
            Instant::now(),
        ),
        Err(NativeLoadObserverError::ResourceLimit)
    );
    let mut budget = HashBudget::new(8);
    assert_eq!(
        hash_file(
            &mut file,
            8,
            &mut budget,
            limits,
            &cancellation,
            Instant::now(),
        ),
        Err(NativeLoadObserverError::ObservationChanged)
    );
    let mut budget = HashBudget::new(7);
    cancellation.cancel();
    assert_eq!(
        hash_file(
            &mut file,
            7,
            &mut budget,
            limits,
            &cancellation,
            Instant::now(),
        ),
        Err(NativeLoadObserverError::Cancelled)
    );
}

#[test]
fn finalization_enforces_exact_external_set_and_redacts_native_facts() {
    let package = package();
    let external = artifact("external");
    let expected = [ExpectedExternalNativeComponent::new(
        external.clone(),
        12,
        NativeMappingClass::ExecutableMapped,
    )];
    let observation = finish_observation(
        &package,
        &expected,
        evidence_class(),
        "test-native-load",
        &Digest::sha256(b"process evidence"),
        vec![
            NativeLoadedComponent::new(
                external,
                12,
                NativeLoadOrigin::ExternalPlatformComponent,
                NativeMappingClass::ExecutableMapped,
                Digest::sha256(b"external object evidence"),
            ),
            NativeLoadedComponent::new(
                artifact("entrypoint"),
                10,
                NativeLoadOrigin::PackagedMember {
                    relative_path: path("bin/runtime"),
                },
                NativeMappingClass::ExecutableImage,
                Digest::sha256(b"entrypoint object evidence"),
            ),
        ],
    )
    .expect("finalize exact observation");
    let encoded = serde_json::to_string(&observation).expect("serialize observation");
    assert!(!encoded.contains("C:\\") && !encoded.contains("/proc/") && !encoded.contains("pid"));
    assert_eq!(observation.components().len(), 2);

    let wrong = [ExpectedExternalNativeComponent::new(
        artifact("other"),
        12,
        NativeMappingClass::ExecutableMapped,
    )];
    assert_eq!(
        finish_observation(
            &package,
            &wrong,
            evidence_class(),
            "test-native-load",
            &Digest::sha256(b"process evidence"),
            observation.components().to_vec(),
        ),
        Err(NativeLoadObserverError::ComponentPolicyMismatch)
    );
}

#[test]
fn copied_must_not_load_content_is_rejected_as_external_code() {
    let package = package();
    let forbidden = artifact("evidence");
    let expected = [ExpectedExternalNativeComponent::new(
        forbidden.clone(),
        11,
        NativeMappingClass::ExecutableMapped,
    )];
    assert_eq!(
        finish_observation(
            &package,
            &expected,
            evidence_class(),
            "test-native-load",
            &Digest::sha256(b"process evidence"),
            vec![
                NativeLoadedComponent::new(
                    artifact("entrypoint"),
                    10,
                    NativeLoadOrigin::PackagedMember {
                        relative_path: path("bin/runtime"),
                    },
                    NativeMappingClass::ExecutableImage,
                    Digest::sha256(b"entrypoint object evidence"),
                ),
                NativeLoadedComponent::new(
                    forbidden,
                    11,
                    NativeLoadOrigin::ExternalPlatformComponent,
                    NativeMappingClass::ExecutableMapped,
                    Digest::sha256(b"copied forbidden object"),
                ),
            ],
        ),
        Err(NativeLoadObserverError::ComponentPolicyMismatch)
    );
}

fn package() -> RuntimePackageManifest {
    let artifact_set = ArtifactSetManifest::new(vec![
        ArtifactSetMember::new(artifact("entrypoint"), 10, path("bin/runtime")),
        ArtifactSetMember::new(artifact("evidence"), 11, path("legal/evidence")),
    ])
    .expect("artifact set");
    RuntimePackageManifest::new(
        &artifact_set,
        "test-runtime",
        "1.0.0",
        None,
        target(),
        PackageSource::new(
            PackageSourceKind::LocalArchive,
            "local:test",
            "1",
            Digest::sha256(b"source"),
        )
        .expect("source"),
        PackageTransformation::Untransformed {
            evidence_digest: Digest::sha256(b"comparison"),
        },
        vec![
            RuntimePackageMember::new(
                artifact("entrypoint"),
                10,
                path("bin/runtime"),
                vec![RuntimePackageMemberRole::Entrypoint],
                RuntimePackageLoadPolicy::RequiredAtReady,
            ),
            RuntimePackageMember::new(
                artifact("evidence"),
                11,
                path("legal/evidence"),
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

fn target() -> RuntimeTarget {
    RuntimeTarget::new(
        if cfg!(windows) {
            RuntimeOperatingSystem::Windows
        } else {
            RuntimeOperatingSystem::Linux
        },
        if cfg!(target_arch = "aarch64") {
            RuntimeArchitecture::Aarch64
        } else {
            RuntimeArchitecture::X86_64
        },
        if cfg!(windows) {
            RuntimeAbi::WindowsMsvc
        } else {
            RuntimeAbi::LinuxGnuLibc
        },
    )
    .expect("native target")
}

fn evidence_class() -> NativeLoadEvidenceClass {
    NativeLoadEvidenceClass::LinuxProcMapFiles
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("portable path")
}

fn artifact(value: &str) -> ArtifactId {
    ArtifactId::from_digest(Digest::sha256(value.as_bytes()))
}
