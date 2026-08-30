use super::*;

#[test]
fn external_component_review_fails_closed_on_limits_and_drift() {
    let package = package_with_version(PACKAGE_VERSION);
    let discovery = discovery_fixture(&package);
    let evidence = b"fixture reviewer evidence";
    assert_eq!(
        super::super::ExternalNativeComponentReview::compile(
            &discovery,
            super::super::ExternalNativeComponentReviewDisposition::Approved,
            b"",
        ),
        Err(super::super::ExternalNativeComponentReviewError::LimitExceeded)
    );
    assert_eq!(
        super::super::ExternalNativeComponentReview::compile(
            &discovery,
            super::super::ExternalNativeComponentReviewDisposition::Approved,
            &vec![0; super::super::MAXIMUM_EXTERNAL_COMPONENT_REVIEW_EVIDENCE_BYTES + 1],
        ),
        Err(super::super::ExternalNativeComponentReviewError::LimitExceeded)
    );
    let approved = super::super::ExternalNativeComponentReview::compile(
        &discovery,
        super::super::ExternalNativeComponentReviewDisposition::Approved,
        evidence,
    )
    .expect("compile approved review");
    let mut noncanonical = approved.canonical_json_bytes().to_vec();
    noncanonical.push(b'\n');
    assert_eq!(
        super::super::ExternalNativeComponentReview::verify(
            &noncanonical,
            discovery.canonical_json_bytes(),
            evidence,
            &package.runtime_package_manifest_id(),
        ),
        Err(super::super::ExternalNativeComponentReviewError::InvalidEncoding)
    );
    assert_eq!(
        super::super::ExternalNativeComponentReview::verify(
            approved.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            b"different reviewer evidence",
            &package.runtime_package_manifest_id(),
        ),
        Err(super::super::ExternalNativeComponentReviewError::InvalidBinding)
    );
    assert_eq!(
        super::super::ExternalNativeComponentReview::verify(
            b"not JSON",
            discovery.canonical_json_bytes(),
            evidence,
            &package.runtime_package_manifest_id(),
        ),
        Err(super::super::ExternalNativeComponentReviewError::InvalidEncoding)
    );
    assert_eq!(
        super::super::ExternalNativeComponentReview::verify(
            &vec![0; super::super::MAXIMUM_EXTERNAL_NATIVE_COMPONENT_REVIEW_JSON_BYTES + 1],
            discovery.canonical_json_bytes(),
            evidence,
            &package.runtime_package_manifest_id(),
        ),
        Err(super::super::ExternalNativeComponentReviewError::LimitExceeded)
    );

    let mut reordered: serde_json::Value =
        serde_json::from_slice(approved.canonical_json_bytes()).expect("parse review JSON");
    reordered["components"]
        .as_array_mut()
        .expect("component array")
        .swap(0, 1);
    let reordered = serde_json::to_vec(&reordered).expect("encode reordered review");
    assert_eq!(
        super::super::ExternalNativeComponentReview::verify(
            &reordered,
            discovery.canonical_json_bytes(),
            evidence,
            &package.runtime_package_manifest_id(),
        ),
        Err(super::super::ExternalNativeComponentReviewError::InvalidComponents)
    );

    let other_package = package_with_version("2.0.0");
    assert!(matches!(
        super::super::ExternalNativeComponentReview::verify(
            approved.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            evidence,
            &other_package.runtime_package_manifest_id(),
        ),
        Err(super::super::ExternalNativeComponentReviewError::Discovery(
            NativeLoadObserverError::InvalidObservation
        ))
    ));
}

#[test]
fn blocked_review_cannot_compile_or_verify_frozen_set() {
    let package = package_with_version(PACKAGE_VERSION);
    let discovery = discovery_fixture(&package);
    let evidence = b"fixture reviewer evidence";
    let blocked = super::super::ExternalNativeComponentReview::compile(
        &discovery,
        super::super::ExternalNativeComponentReviewDisposition::Blocked,
        evidence,
    )
    .expect("compile blocked review");
    assert_eq!(
        super::super::CompiledFrozenExternalNativeComponentSet::compile(&discovery, &blocked),
        Err(super::super::FrozenExternalNativeComponentSetError::ReviewBlocked)
    );
    assert_eq!(
        super::super::ExternalNativeComponentReview::verify(
            blocked.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            evidence,
            &package.runtime_package_manifest_id(),
        )
        .expect("verify blocked review"),
        blocked
    );

    let approved = super::super::ExternalNativeComponentReview::compile(
        &discovery,
        super::super::ExternalNativeComponentReviewDisposition::Approved,
        evidence,
    )
    .expect("compile approved review");
    let compiled =
        super::super::CompiledFrozenExternalNativeComponentSet::compile(&discovery, &approved)
            .expect("compile frozen set");
    let package_id = package.runtime_package_manifest_id();
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            compiled.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            blocked.canonical_json_bytes(),
            evidence,
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::ReviewBlocked)
    );
}

#[test]
fn frozen_set_rejects_limits_encoding_and_review_drift() {
    let package = package_with_version(PACKAGE_VERSION);
    let discovery = discovery_fixture(&package);
    let evidence = b"fixture reviewer evidence";
    let approved = super::super::ExternalNativeComponentReview::compile(
        &discovery,
        super::super::ExternalNativeComponentReviewDisposition::Approved,
        evidence,
    )
    .expect("compile approved review");
    let compiled =
        super::super::CompiledFrozenExternalNativeComponentSet::compile(&discovery, &approved)
            .expect("compile frozen set");
    let package_id = package.runtime_package_manifest_id();
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            b"",
            discovery.canonical_json_bytes(),
            approved.canonical_json_bytes(),
            evidence,
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::LimitExceeded)
    );
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            &vec![0; super::super::MAXIMUM_FROZEN_EXTERNAL_NATIVE_COMPONENT_SET_JSON_BYTES + 1],
            discovery.canonical_json_bytes(),
            approved.canonical_json_bytes(),
            evidence,
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::LimitExceeded)
    );
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            b"not JSON",
            discovery.canonical_json_bytes(),
            approved.canonical_json_bytes(),
            evidence,
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::InvalidEncoding)
    );
    let mut noncanonical = compiled.canonical_json_bytes().to_vec();
    noncanonical.push(b'\n');
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            &noncanonical,
            discovery.canonical_json_bytes(),
            approved.canonical_json_bytes(),
            evidence,
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::InvalidEncoding)
    );
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            compiled.canonical_json_bytes(),
            discovery.canonical_json_bytes(),
            approved.canonical_json_bytes(),
            b"different reviewer evidence",
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::Review(
            super::super::ExternalNativeComponentReviewError::InvalidBinding
        ))
    );
}

#[test]
fn frozen_set_rejects_binding_and_component_drift() {
    let package = package_with_version(PACKAGE_VERSION);
    let discovery = discovery_fixture(&package);
    let evidence = b"fixture reviewer evidence";
    let approved = super::super::ExternalNativeComponentReview::compile(
        &discovery,
        super::super::ExternalNativeComponentReviewDisposition::Approved,
        evidence,
    )
    .expect("compile approved review");
    let compiled =
        super::super::CompiledFrozenExternalNativeComponentSet::compile(&discovery, &approved)
            .expect("compile frozen set");
    let package_id = package.runtime_package_manifest_id();
    let mut changed_binding: serde_json::Value =
        serde_json::from_slice(compiled.canonical_json_bytes()).expect("parse frozen JSON");
    changed_binding["discovery_digest"] =
        serde_json::to_value(Digest::sha256(b"different discovery")).expect("encode digest");
    let changed_binding = serde_json::to_vec(&changed_binding).expect("encode changed binding");
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            &changed_binding,
            discovery.canonical_json_bytes(),
            approved.canonical_json_bytes(),
            evidence,
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::InvalidBinding)
    );

    let other_discovery = discovery_fixture(&package_with_version("2.0.0"));
    assert_eq!(
        super::super::CompiledFrozenExternalNativeComponentSet::compile(
            &other_discovery,
            &approved
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::InvalidBinding)
    );

    let mut changed_components: serde_json::Value =
        serde_json::from_slice(compiled.canonical_json_bytes()).expect("parse frozen JSON");
    changed_components["components"][0]["byte_size"] = serde_json::json!(999);
    let changed_components =
        serde_json::to_vec(&changed_components).expect("encode changed frozen set");
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            &changed_components,
            discovery.canonical_json_bytes(),
            approved.canonical_json_bytes(),
            evidence,
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::InvalidComponents)
    );

    let mut invalid_components: serde_json::Value =
        serde_json::from_slice(compiled.canonical_json_bytes()).expect("parse frozen JSON");
    invalid_components["components"][0]["mapping_class"] =
        serde_json::json!(NativeMappingClass::ExecutableImage);
    let invalid_components =
        serde_json::to_vec(&invalid_components).expect("encode invalid frozen set");
    assert_eq!(
        super::super::VerifiedFrozenExternalNativeComponentSet::verify(
            &invalid_components,
            discovery.canonical_json_bytes(),
            approved.canonical_json_bytes(),
            evidence,
            &package_id,
        ),
        Err(super::super::FrozenExternalNativeComponentSetError::InvalidComponents)
    );
}
