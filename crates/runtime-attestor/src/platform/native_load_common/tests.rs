use std::{io::Write as _, time::Instant};

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
    NativeLoadEvidenceClass, NativeLoadOrigin, NativeLoadedComponent, NativeMappingClass,
    PackageSource, PackageSourceKind, PackageTransformation, RuntimeAbi, RuntimeArchitecture,
    RuntimeOperatingSystem, RuntimePackageLoadPolicy, RuntimePackageManifest, RuntimePackageMember,
    RuntimePackageMemberRole, RuntimeTarget,
};
use rewrite_types::{CancellationToken, Digest};

use super::{HashBudget, finish_discovery, finish_observation, hash_file};
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
fn discovery_is_canonical_redacted_and_explicitly_non_authoritative() {
    let package = package();
    let process = Digest::sha256(b"discovery process evidence");
    let discovery = discovery_fixture(&package);
    assert_eq!(
        discovery.runtime_package_manifest_id(),
        &package.runtime_package_manifest_id()
    );
    assert_eq!(discovery.process_evidence_digest(), &process);
    assert_eq!(discovery.external_components().len(), 2);
    assert!(
        discovery.external_components()[0]
            .artifact_id()
            .digest()
            .as_str()
            < discovery.external_components()[1]
                .artifact_id()
                .digest()
                .as_str()
    );
    assert_eq!(
        discovery.external_components()[0].mapping_class(),
        NativeMappingClass::ExecutableMapped
    );
    assert!(discovery.external_components()[0].byte_size() > 0);
    let encoded = std::str::from_utf8(discovery.canonical_json_bytes()).expect("UTF-8 report");
    assert!(encoded.contains("\"authority\":\"none\""));
    assert!(encoded.contains("\"status\":\"proposed\""));
    assert!(!encoded.contains("object_evidence_digest"));
    assert!(!encoded.contains("/proc/") && !encoded.contains("path"));
    assert_eq!(
        discovery.discovery_digest(),
        &Digest::sha256(discovery.canonical_json_bytes())
    );
    assert_eq!(
        crate::NativeLoadDiscovery::from_json_bytes(
            discovery.canonical_json_bytes(),
            &package.runtime_package_manifest_id(),
        )
        .expect("reparse canonical proposal"),
        discovery
    );
    let mut noncanonical = discovery.canonical_json_bytes().to_vec();
    noncanonical.push(b'\n');
    assert_eq!(
        crate::NativeLoadDiscovery::from_json_bytes(
            &noncanonical,
            &package.runtime_package_manifest_id(),
        ),
        Err(NativeLoadObserverError::InvalidObservation)
    );
}

#[test]
fn frozen_set_binds_separate_discovery_and_review_evidence() {
    let package = package();
    let discovery = discovery_fixture(&package);
    let review_evidence = br#"{"decision":"accepted","reviewer_policy":"fixture-v1"}"#;
    let review = crate::ExternalNativeComponentReview::compile(
        &discovery,
        crate::ExternalNativeComponentReviewDisposition::Approved,
        review_evidence,
    )
    .expect("compile typed external-component review");
    assert_eq!(
        review.runtime_package_manifest_id(),
        discovery.runtime_package_manifest_id()
    );
    assert_eq!(review.discovery_digest(), discovery.discovery_digest());
    assert_eq!(
        review.disposition(),
        crate::ExternalNativeComponentReviewDisposition::Approved
    );
    assert_eq!(
        review.reviewer_evidence_digest(),
        &Digest::sha256(review_evidence)
    );
    assert_eq!(
        review.review_digest(),
        &Digest::sha256(review.canonical_json_bytes())
    );
    assert_eq!(
        review.components().len(),
        discovery.external_components().len()
    );
    assert_eq!(
        crate::ExternalNativeComponentReview::verify(
            review.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            review_evidence,
            &package.runtime_package_manifest_id(),
        )
        .expect("independently verify typed review"),
        review
    );
    let compiled = crate::CompiledFrozenExternalNativeComponentSet::compile(&discovery, &review)
        .expect("compile separately reviewed frozen set");
    assert_eq!(
        compiled.runtime_package_manifest_id(),
        discovery.runtime_package_manifest_id()
    );
    assert_eq!(compiled.discovery_digest(), discovery.discovery_digest());
    assert_eq!(
        compiled.external_component_review_digest(),
        review.review_digest()
    );
    assert_eq!(
        compiled.frozen_set_id().digest(),
        &Digest::sha256(compiled.canonical_json_bytes())
    );
    assert_eq!(
        compiled.reviewer_evidence_digest(),
        &Digest::sha256(review_evidence)
    );
    let verified = crate::VerifiedFrozenExternalNativeComponentSet::verify(
        compiled.canonical_json_bytes(),
        discovery.canonical_json_bytes(),
        review.canonical_json_bytes(),
        review_evidence,
        &package.runtime_package_manifest_id(),
    )
    .expect("independently verify frozen set");
    assert_eq!(verified.frozen_set_id(), compiled.frozen_set_id());
    assert_eq!(
        verified.canonical_json_bytes(),
        compiled.canonical_json_bytes()
    );
    assert_eq!(
        verified.expected_components().len(),
        discovery.external_components().len()
    );
    let changed_review = crate::ExternalNativeComponentReview::compile(
        &discovery,
        crate::ExternalNativeComponentReviewDisposition::Approved,
        b"different accepted review evidence",
    )
    .expect("compile changed typed review");
    let changed =
        crate::CompiledFrozenExternalNativeComponentSet::compile(&discovery, &changed_review)
            .expect("compile changed reviewed frozen set");
    assert_ne!(changed.frozen_set_id(), compiled.frozen_set_id());
    assert_eq!(
        crate::VerifiedFrozenExternalNativeComponentSet::verify(
            compiled.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            review.canonical_json_bytes(),
            b"changed review evidence",
            &package.runtime_package_manifest_id(),
        ),
        Err(crate::FrozenExternalNativeComponentSetError::Review(
            crate::ExternalNativeComponentReviewError::InvalidBinding
        ))
    );
}

#[test]
fn blocked_external_component_review_is_canonical_but_cannot_freeze() {
    let package = package();
    let discovery = discovery_fixture(&package);
    let review_evidence = b"fixture blocked-review evidence";
    let review = crate::ExternalNativeComponentReview::compile(
        &discovery,
        crate::ExternalNativeComponentReviewDisposition::Blocked,
        review_evidence,
    )
    .expect("compile blocked review");
    assert_eq!(
        review.disposition(),
        crate::ExternalNativeComponentReviewDisposition::Blocked
    );
    assert_eq!(
        crate::ExternalNativeComponentReview::verify(
            review.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            review_evidence,
            &package.runtime_package_manifest_id(),
        )
        .expect("independently verify blocked review"),
        review
    );
    assert_eq!(
        crate::CompiledFrozenExternalNativeComponentSet::compile(&discovery, &review),
        Err(crate::FrozenExternalNativeComponentSetError::ReviewBlocked)
    );
}

fn discovery_fixture(package: &RuntimePackageManifest) -> crate::NativeLoadDiscovery {
    finish_discovery(
        package,
        evidence_class(),
        "test-native-load",
        &Digest::sha256(b"discovery process evidence"),
        vec![
            NativeLoadedComponent::new(
                artifact("platform two"),
                22,
                NativeLoadOrigin::ExternalPlatformComponent,
                NativeMappingClass::ExecutableMapped,
                Digest::sha256(b"second external object"),
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
            NativeLoadedComponent::new(
                artifact("platform one"),
                11,
                NativeLoadOrigin::ExternalPlatformComponent,
                NativeMappingClass::ExecutableMapped,
                Digest::sha256(b"first external object"),
            ),
        ],
    )
    .expect("compile discovery proposal")
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
