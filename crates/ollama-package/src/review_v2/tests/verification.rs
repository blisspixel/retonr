use std::io::Cursor;

use rewrite_model::ArtifactSetId;
use rewrite_types::Digest;
use serde_json::{Value, json};

use super::*;
use crate::{MemberOpenError, RuntimeReconstructionError, RuntimeSourceBuildInputOpenError};

fn unavailable_source(
    _path: &rewrite_model::ArtifactSetRelativePath,
) -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildInputOpenError> {
    Err(RuntimeSourceBuildInputOpenError)
}

#[test]
fn admitted_review_verifies_every_relationship_and_derives_runtime_identity() {
    let fixture = fixture();
    let verified = verify(&fixture).expect("admitted fixture verifies");
    assert_eq!(verified.review().runtime_family(), "ollama");
    assert_eq!(verified.review().reported_version(), "0.32.15-retonr.1");
    assert_eq!(verified.review().evidence_count(), 4);
    assert_eq!(
        verified.review().source_build_inputs_id(),
        &fixture.source_build_inputs_id
    );
    assert_eq!(
        verified
            .source_build_inputs()
            .artifact_set()
            .artifact_set_id(),
        fixture.source_build_inputs_id
    );
    assert_eq!(
        verified
            .reconstructed_runtime()
            .expect("admitted runtime")
            .runtime_package()
            .runtime_package_manifest_id(),
        fixture.runtime_package_manifest_id
    );
    for check in [
        RuntimePackageReviewCheck::SourceLineage,
        RuntimePackageReviewCheck::Transformation,
        RuntimePackageReviewCheck::License,
        RuntimePackageReviewCheck::NativeClosure,
        RuntimePackageReviewCheck::ManagedStartup,
        RuntimePackageReviewCheck::CloudDisable,
    ] {
        assert_eq!(
            verified.review().check_status(check),
            RuntimePackageReviewCheckStatus::Passed
        );
    }
}

#[test]
fn blocked_review_verifies_metadata_without_opening_runtime_members() {
    let mut fixture = fixture();
    let mut review = review_value(&fixture);
    review["checks"][4]["status"] = json!("not_run");
    review["checks"][5]["status"] = json!("blocked");
    review["disposition"] = json!({
        "blockers": ["managed_startup", "cloud_disable"],
        "status": "not_admitted"
    });
    fixture.review = canonical(&review);
    let verified = verify_runtime_package_review_v2(
        &fixture.review,
        &RuntimePackageReviewV2Limits::default(),
        |path| {
            fixture
                .evidence
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimePackageReviewEvidenceOpenError)
        },
        |path| {
            fixture
                .source_inputs
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(RuntimeSourceBuildInputOpenError)
        },
        |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
            panic!("non-admitted review must not open runtime members")
        },
        || false,
    )
    .expect("blocked review verifies as non-admitted");
    assert!(verified.reconstructed_runtime().is_none());
    assert!(matches!(
        verified.review().disposition(),
        RuntimePackageReviewDispositionV2::NotAdmitted { blockers }
            if blockers == &vec![
                RuntimePackageReviewCheck::ManagedStartup,
                RuntimePackageReviewCheck::CloudDisable
            ]
    ));
}

#[test]
fn evidence_open_read_size_and_digest_failures_remain_distinct() {
    let fixture = fixture();
    assert_eq!(
        verify_runtime_package_review_v2(
            &fixture.review,
            &RuntimePackageReviewV2Limits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                Err(RuntimePackageReviewEvidenceOpenError)
            },
            unavailable_source,
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
            || false
        ),
        Err(RuntimePackageReviewV2Error::EvidenceUnavailable)
    );
    assert_eq!(
        verify_runtime_package_review_v2(
            &fixture.review,
            &RuntimePackageReviewV2Limits::default(),
            |_path| Ok(FailingRead),
            unavailable_source,
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
            || false
        ),
        Err(RuntimePackageReviewV2Error::EvidenceRead)
    );
    for append in [false, true] {
        let result = verify_runtime_package_review_v2(
            &fixture.review,
            &RuntimePackageReviewV2Limits::default(),
            |path| {
                let mut bytes = fixture
                    .evidence
                    .get(path.as_str())
                    .cloned()
                    .ok_or(RuntimePackageReviewEvidenceOpenError)?;
                if path.as_str() == "evidence/build-tools.json" {
                    if append {
                        bytes.push(0);
                    } else {
                        bytes.pop();
                    }
                }
                Ok(Cursor::new(bytes))
            },
            unavailable_source,
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
            || false,
        );
        assert_eq!(
            result,
            Err(RuntimePackageReviewV2Error::EvidenceSizeMismatch)
        );
    }
    let result = verify_runtime_package_review_v2(
        &fixture.review,
        &RuntimePackageReviewV2Limits::default(),
        |path| {
            let mut bytes = fixture
                .evidence
                .get(path.as_str())
                .cloned()
                .ok_or(RuntimePackageReviewEvidenceOpenError)?;
            if path.as_str() == "evidence/build-tools.json" {
                *bytes.last_mut().expect("nonempty fixture") ^= 1;
            }
            Ok(Cursor::new(bytes))
        },
        unavailable_source,
        |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
        || false,
    );
    assert_eq!(
        result,
        Err(RuntimePackageReviewV2Error::EvidenceDigestMismatch)
    );
}

#[test]
fn source_manifest_and_runtime_member_bytes_are_independently_verified() {
    let mut noncanonical_source = fixture();
    let source: Value = serde_json::from_slice(
        noncanonical_source
            .evidence
            .get(SOURCE_PATH)
            .expect("source manifest"),
    )
    .expect("source manifest JSON");
    replace_evidence(
        &mut noncanonical_source,
        SOURCE_PATH,
        &serde_json::to_vec_pretty(&source).expect("pretty source manifest"),
    );
    assert_eq!(
        verify(&noncanonical_source),
        Err(RuntimePackageReviewV2Error::SourceBuildInputs(
            crate::RuntimeSourceBuildInputError::NoncanonicalEncoding
        ))
    );

    let mut wrong_source_identity = fixture();
    let mut review = review_value(&wrong_source_identity);
    review["source_build_inputs"]["artifact_set_id"] = json!(ArtifactSetId::from_digest(
        Digest::sha256(b"wrong input set")
    ));
    wrong_source_identity.review = canonical(&review);
    assert_eq!(
        verify(&wrong_source_identity),
        Err(RuntimePackageReviewV2Error::InvalidSourceBuildInputs)
    );

    let mut drifted_source_input = fixture();
    drifted_source_input
        .source_inputs
        .get_mut("helper/isolation")
        .expect("source input")
        .push(0);
    assert_eq!(
        verify(&drifted_source_input),
        Err(RuntimePackageReviewV2Error::SourceBuildInputs(
            crate::RuntimeSourceBuildInputError::ComponentSizeMismatch
        ))
    );

    let mut drifted_member = fixture();
    drifted_member
        .members
        .get_mut("bin/ollama")
        .expect("entrypoint")
        .push(0);
    assert_eq!(
        verify(&drifted_member),
        Err(RuntimePackageReviewV2Error::RuntimeReconstruction(
            RuntimeReconstructionError::MemberSizeMismatch
        ))
    );
}

#[test]
fn final_layout_transformation_must_bind_the_verified_source_input_set() {
    let mut fixture = fixture();
    let mut layout: Value =
        serde_json::from_slice(fixture.evidence.get(LAYOUT_PATH).expect("layout evidence"))
            .expect("layout JSON");
    layout["transformation"]["source_artifact_set_id"] = json!(ArtifactSetId::from_digest(
        Digest::sha256(b"other input set")
    ));
    let layout = canonical(&layout);
    let reconstructed = reconstruct_runtime_package_with_limits(
        &layout,
        &RuntimeLayoutLimits::default(),
        |path| {
            fixture
                .members
                .get(path.as_str())
                .cloned()
                .map(Cursor::new)
                .ok_or(MemberOpenError)
        },
        || false,
    )
    .expect("mismatched-source fixture still reconstructs structurally");
    let package_id = reconstructed
        .runtime_package()
        .runtime_package_manifest_id();
    replace_evidence(&mut fixture, LAYOUT_PATH, &layout);
    let mut review = review_value(&fixture);
    review["disposition"]["runtime_package_manifest_id"] = json!(package_id);
    fixture.review = canonical(&review);
    assert_eq!(
        verify(&fixture),
        Err(RuntimePackageReviewV2Error::InvalidRuntimeBinding)
    );
}
