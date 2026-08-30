use std::{fs::File, time::Duration};

use rewrite_model::{ArtifactId, ArtifactSetRelativePath, NativeMappingClass};
use rewrite_types::Digest;

use super::{
    ExpectedExternalNativeComponent, MAXIMUM_NATIVE_LOAD_HASH_BYTES,
    MAXIMUM_NATIVE_LOAD_OBSERVATION_MILLIS, MAXIMUM_NATIVE_LOADED_COMPONENTS,
    MAXIMUM_NATIVE_MAPPING_METADATA_BYTES, MAXIMUM_NATIVE_MAPPING_REGIONS, NativeLoadDiscovery,
    NativeLoadDiscoveryRequest, NativeLoadObservationLimits, NativeLoadObservationRequest,
    NativeLoadObserverError, RetainedNativePackageMember, expected_key,
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
    let package = package_with_version(PACKAGE_VERSION);
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
    let discovery = NativeLoadDiscoveryRequest {
        package: &package,
        expected_package_id: &package_id,
        retained_package_members: &retained,
        limits: NativeLoadObservationLimits::default(),
    };
    assert_eq!(discovery.validate(), Ok(discovery.limits));

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
        NativeLoadDiscoveryRequest {
            expected_package_id: &other_id,
            ..discovery
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
    let package = package_with_version(PACKAGE_VERSION);
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

#[test]
fn typed_review_and_verified_frozen_set_preserve_authority_boundaries() {
    let package = package_with_version(PACKAGE_VERSION);
    let discovery = discovery_fixture(&package);
    let review_evidence = b"fixture reviewer evidence";
    let review = super::ExternalNativeComponentReview::compile(
        &discovery,
        super::ExternalNativeComponentReviewDisposition::Approved,
        review_evidence,
    )
    .expect("compile review");
    assert_review_binding(&review, &discovery, review_evidence);
    assert_eq!(
        super::ExternalNativeComponentReview::verify(
            review.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            review_evidence,
            &package.runtime_package_manifest_id(),
        )
        .expect("verify review"),
        review
    );

    let compiled = super::CompiledFrozenExternalNativeComponentSet::compile(&discovery, &review)
        .expect("compile frozen set");
    assert_compiled_frozen_binding(&compiled, &discovery, &review);
    let verified = super::VerifiedFrozenExternalNativeComponentSet::verify(
        compiled.canonical_json_bytes(),
        discovery.canonical_json_bytes(),
        review.canonical_json_bytes(),
        review_evidence,
        &package.runtime_package_manifest_id(),
    )
    .expect("independently verify frozen set");
    assert_verified_frozen_binding(&verified, &compiled, &discovery);
}

fn assert_review_binding(
    review: &super::ExternalNativeComponentReview,
    discovery: &NativeLoadDiscovery,
    review_evidence: &[u8],
) {
    assert_eq!(
        review.runtime_package_manifest_id(),
        discovery.runtime_package_manifest_id()
    );
    assert_eq!(review.discovery_digest(), discovery.discovery_digest());
    assert_eq!(
        review.disposition(),
        super::ExternalNativeComponentReviewDisposition::Approved
    );
    assert_eq!(
        review.reviewer_evidence_digest(),
        &Digest::sha256(review_evidence)
    );
    assert_eq!(
        review.review_digest(),
        &Digest::sha256(review.canonical_json_bytes())
    );
    assert_eq!(review.components().len(), 2);
    assert_eq!(
        review.components()[0].artifact_id(),
        discovery.external_components()[0].artifact_id()
    );
    assert_eq!(
        review.components()[0].byte_size(),
        discovery.external_components()[0].byte_size()
    );
    assert_eq!(
        review.components()[0].mapping_class(),
        discovery.external_components()[0].mapping_class()
    );
}

fn assert_compiled_frozen_binding(
    compiled: &super::CompiledFrozenExternalNativeComponentSet,
    discovery: &NativeLoadDiscovery,
    review: &super::ExternalNativeComponentReview,
) {
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
        compiled.reviewer_evidence_digest(),
        review.reviewer_evidence_digest()
    );
    assert_eq!(
        compiled.frozen_set_id().digest(),
        &Digest::sha256(compiled.canonical_json_bytes())
    );
    assert_eq!(compiled.clone(), *compiled);
    assert!(format!("{compiled:?}").contains("CompiledFrozenExternalNativeComponentSet"));
    let encoded_id = serde_json::to_vec(compiled.frozen_set_id()).expect("encode frozen set ID");
    let decoded_id: super::FrozenExternalNativeComponentSetId =
        serde_json::from_slice(&encoded_id).expect("decode frozen set ID");
    assert_eq!(&decoded_id, compiled.frozen_set_id());
    let mut ids = std::collections::HashSet::new();
    assert!(ids.insert(decoded_id));
}

fn assert_verified_frozen_binding(
    verified: &super::VerifiedFrozenExternalNativeComponentSet,
    compiled: &super::CompiledFrozenExternalNativeComponentSet,
    discovery: &NativeLoadDiscovery,
) {
    assert_eq!(verified.frozen_set_id(), compiled.frozen_set_id());
    assert_eq!(
        verified.frozen_external_component_set_id().digest(),
        verified.frozen_set_id().digest()
    );
    assert_eq!(
        verified.runtime_package_manifest_id(),
        compiled.runtime_package_manifest_id()
    );
    assert_eq!(verified.discovery_digest(), compiled.discovery_digest());
    assert_eq!(
        verified.external_component_review_digest(),
        compiled.external_component_review_digest()
    );
    assert_eq!(
        verified.reviewer_evidence_digest(),
        compiled.reviewer_evidence_digest()
    );
    assert_eq!(
        verified.canonical_json_bytes(),
        compiled.canonical_json_bytes()
    );
    assert_eq!(
        verified.expected_components().len(),
        discovery.external_components().len()
    );
    assert_eq!(verified.clone(), *verified);
    assert!(format!("{verified:?}").contains("VerifiedFrozenExternalNativeComponentSet"));
}

#[path = "tests/frozen.rs"]
mod frozen_tests;
#[path = "tests/worker_subject.rs"]
mod worker_subject;

#[path = "tests/fixtures.rs"]
mod fixtures;
use fixtures::{
    PACKAGE_VERSION, clone_retained, discovery_fixture, package_with_version, retained_members,
};
