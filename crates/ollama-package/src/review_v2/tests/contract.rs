use std::io::Cursor;

use rewrite_types::Digest;
use serde_json::json;

use super::*;
use crate::{MemberOpenError, RuntimeSourceBuildInputOpenError};

fn unavailable_source(
    _path: &rewrite_model::ArtifactSetRelativePath,
) -> Result<Cursor<Vec<u8>>, RuntimeSourceBuildInputOpenError> {
    Err(RuntimeSourceBuildInputOpenError)
}

#[test]
fn canonical_unique_bounded_review_encoding_is_required() {
    let fixture = fixture();
    let pretty = serde_json::to_vec_pretty(&review_value(&fixture)).expect("pretty review");
    assert_eq!(
        verify_runtime_package_review_v2(
            &pretty,
            &RuntimePackageReviewV2Limits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                Err(RuntimePackageReviewEvidenceOpenError)
            },
            unavailable_source,
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
            || false
        ),
        Err(RuntimePackageReviewV2Error::NoncanonicalEncoding)
    );
    assert_eq!(
        verify_runtime_package_review_v2(
            br#"{"schema_version":2,"schema_version":2}"#,
            &RuntimePackageReviewV2Limits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                Err(RuntimePackageReviewEvidenceOpenError)
            },
            unavailable_source,
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
            || false
        ),
        Err(RuntimePackageReviewV2Error::InvalidEncoding)
    );
    let oversized = vec![b' '; DEFAULT_REVIEW_BYTES + 1];
    assert_eq!(
        verify_runtime_package_review_v2(
            &oversized,
            &RuntimePackageReviewV2Limits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                Err(RuntimePackageReviewEvidenceOpenError)
            },
            unavailable_source,
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
            || false
        ),
        Err(RuntimePackageReviewV2Error::ReviewTooLarge)
    );
}

#[test]
fn schema_target_identity_and_unknown_fields_fail_closed() {
    let fixture = fixture();
    for (pointer, replacement, expected) in [
        (
            "/schema_version",
            json!(1),
            RuntimePackageReviewV2Error::UnsupportedSchema,
        ),
        (
            "/runtime_family",
            json!("llama-cpp"),
            RuntimePackageReviewV2Error::InvalidIdentity,
        ),
        (
            "/reported_version",
            json!("contains space"),
            RuntimePackageReviewV2Error::InvalidIdentity,
        ),
        (
            "/target/architecture",
            json!("aarch64"),
            RuntimePackageReviewV2Error::UnsupportedTarget,
        ),
    ] {
        let mut review = review_value(&fixture);
        *review.pointer_mut(pointer).expect("fixture pointer") = replacement;
        let bytes = canonical(&review);
        assert_eq!(
            verify_runtime_package_review_v2(
                &bytes,
                &RuntimePackageReviewV2Limits::default(),
                |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                    Err(RuntimePackageReviewEvidenceOpenError)
                },
                unavailable_source,
                |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
                || false
            ),
            Err(expected)
        );
    }
    let mut unknown = review_value(&fixture);
    unknown["unknown"] = json!(true);
    assert_eq!(
        verify_runtime_package_review_v2(
            &canonical(&unknown),
            &RuntimePackageReviewV2Limits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                Err(RuntimePackageReviewEvidenceOpenError)
            },
            unavailable_source,
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> { Err(MemberOpenError) },
            || false
        ),
        Err(RuntimePackageReviewV2Error::InvalidEncoding)
    );
}

#[test]
fn evidence_and_check_order_class_and_reference_contracts_fail_closed() {
    let fixture = fixture();
    let mut reordered_evidence = review_value(&fixture);
    reordered_evidence["evidence"]
        .as_array_mut()
        .expect("evidence")
        .swap(0, 1);
    assert_parse_error(
        &canonical(&reordered_evidence),
        RuntimePackageReviewV2Error::InvalidEvidence,
    );

    let mut reordered_checks = review_value(&fixture);
    reordered_checks["checks"]
        .as_array_mut()
        .expect("checks")
        .swap(0, 1);
    assert_parse_error(
        &canonical(&reordered_checks),
        RuntimePackageReviewV2Error::InvalidChecks,
    );

    let mut weak_lineage = review_value(&fixture);
    weak_lineage["checks"][0]["evidence"] = json!([SOURCE_PATH]);
    assert_parse_error(
        &canonical(&weak_lineage),
        RuntimePackageReviewV2Error::InvalidChecks,
    );

    let mut dangling = review_value(&fixture);
    dangling["evidence"]
        .as_array_mut()
        .expect("evidence")
        .push(json!({
            "byte_size": 2,
            "class": "build_output",
            "digest": Digest::sha256(b"{}"),
            "relative_path": "unused.json"
        }));
    assert_parse_error(
        &canonical(&dangling),
        RuntimePackageReviewV2Error::InvalidEvidence,
    );
}

#[test]
fn disposition_must_match_checks_layout_and_package_identity() {
    let baseline = fixture();
    let mut blockers = review_value(&baseline);
    blockers["disposition"] = json!({
        "blockers": ["cloud_disable"],
        "status": "not_admitted"
    });
    assert_parse_error(
        &canonical(&blockers),
        RuntimePackageReviewV2Error::InvalidDisposition,
    );

    let mut wrong_layout_digest = review_value(&baseline);
    wrong_layout_digest["disposition"]["layout_digest"] = json!(Digest::sha256(b"wrong layout"));
    assert_parse_error(
        &canonical(&wrong_layout_digest),
        RuntimePackageReviewV2Error::InvalidDisposition,
    );

    let mut wrong_package_id = review_value(&baseline);
    wrong_package_id["disposition"]["runtime_package_manifest_id"] =
        json!(Digest::sha256(b"wrong package"));
    let mut changed = fixture();
    changed.review = canonical(&wrong_package_id);
    assert_eq!(
        verify(&changed),
        Err(RuntimePackageReviewV2Error::InvalidRuntimeBinding)
    );
}

#[test]
fn explicit_limits_and_cancellation_apply_before_expensive_work() {
    let fixture = fixture();
    for limits in [
        RuntimePackageReviewV2Limits {
            review_bytes: 0,
            ..RuntimePackageReviewV2Limits::default()
        },
        RuntimePackageReviewV2Limits {
            maximum_evidence_records: 3,
            ..RuntimePackageReviewV2Limits::default()
        },
        RuntimePackageReviewV2Limits {
            maximum_evidence_bytes: 1,
            ..RuntimePackageReviewV2Limits::default()
        },
        RuntimePackageReviewV2Limits {
            maximum_total_evidence_bytes: 1,
            ..RuntimePackageReviewV2Limits::default()
        },
    ] {
        assert_eq!(
            verify_runtime_package_review_v2(
                &fixture.review,
                &limits,
                |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                    panic!("invalid limits must fail before opening evidence")
                },
                unavailable_source,
                |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
                    panic!("invalid limits must fail before opening members")
                },
                || false
            ),
            Err(RuntimePackageReviewV2Error::LimitExceeded)
        );
    }
    assert_eq!(
        verify_runtime_package_review_v2(
            &fixture.review,
            &RuntimePackageReviewV2Limits::default(),
            |_path| -> Result<Cursor<Vec<u8>>, RuntimePackageReviewEvidenceOpenError> {
                panic!("cancellation must win before opening evidence")
            },
            unavailable_source,
            |_path| -> Result<Cursor<Vec<u8>>, MemberOpenError> {
                panic!("cancellation must win before opening members")
            },
            || true
        ),
        Err(RuntimePackageReviewV2Error::Cancelled)
    );
}
