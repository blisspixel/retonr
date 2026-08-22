use std::fs;
use std::path::PathBuf;

use rewrite_model::{
    ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath, RuntimeAbi,
    RuntimeArchitecture, RuntimeOperatingSystem,
};
use rewrite_ollama_package::{
    RUNTIME_PACKAGE_REVIEW_SCHEMA_VERSION, RuntimePackageReview, RuntimePackageReviewCheck,
    RuntimePackageReviewCheckStatus, RuntimePackageReviewDisposition, RuntimePackageReviewError,
};
use rewrite_types::Digest;
use serde_json::{Value, json};

const REVIEW_BYTES: &[u8] = include_bytes!(
    "../../../docs/reviews/runtime-packages/ollama-v0.32.15-linux-x86_64-gnu/review.json"
);

fn review_value() -> Value {
    serde_json::from_slice(REVIEW_BYTES).expect("checked-in review JSON")
}

#[test]
fn exact_candidate_review_is_bound_and_not_admitted() {
    let review = RuntimePackageReview::parse(REVIEW_BYTES).expect("exact review parses");
    assert_eq!(RUNTIME_PACKAGE_REVIEW_SCHEMA_VERSION, 1);
    assert_eq!(review.runtime_family(), "ollama");
    assert_eq!(review.reported_version(), "0.32.15");
    assert_eq!(
        review.build_revision(),
        "b7871fc0d1d82fe109536efa3e0e8e411c766c75"
    );
    assert_eq!(
        (
            review.target().operating_system(),
            review.target().architecture(),
            review.target().abi()
        ),
        (
            RuntimeOperatingSystem::Linux,
            RuntimeArchitecture::X86_64,
            RuntimeAbi::LinuxGnuLibc
        )
    );
    assert_eq!(review.source_byte_size(), 1_422_416_084);
    assert_eq!(
        review.source_digest().as_str(),
        "50539c5fe9bf85887733355098dcdb266b433cb8c73fa180713417e9ed6e42bb"
    );
    assert_eq!(review.evidence_count(), 6);
    assert_eq!(
        review.check_status(RuntimePackageReviewCheck::Transformation),
        RuntimePackageReviewCheckStatus::Passed
    );
    assert_eq!(
        review.check_status(RuntimePackageReviewCheck::License),
        RuntimePackageReviewCheckStatus::Passed
    );
    assert_eq!(
        review.disposition(),
        &RuntimePackageReviewDisposition::NotAdmitted {
            blockers: vec![
                RuntimePackageReviewCheck::SourceLineage,
                RuntimePackageReviewCheck::NativeClosure,
                RuntimePackageReviewCheck::ManagedStartup,
                RuntimePackageReviewCheck::CloudDisable,
            ]
        }
    );
}

#[test]
fn exact_source_artifact_set_and_evidence_files_match_review() {
    let review = RuntimePackageReview::parse(REVIEW_BYTES).expect("exact review parses");
    let source_set = ArtifactSetManifest::new(vec![ArtifactSetMember::new(
        ArtifactId::from_digest(
            Digest::from_sha256_hex(
                "50539c5fe9bf85887733355098dcdb266b433cb8c73fa180713417e9ed6e42bb",
            )
            .expect("source digest"),
        ),
        1_422_416_084,
        ArtifactSetRelativePath::new("upstream/ollama-linux-amd64.tar.zst").expect("source path"),
    )])
    .expect("source artifact set");
    assert_eq!(
        review.source_artifact_set_id(),
        &source_set.artifact_set_id()
    );

    let value = review_value();
    let evidence_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reviews/runtime-packages/ollama-v0.32.15-linux-x86_64-gnu");
    for item in value["evidence"].as_array().expect("evidence array") {
        let relative_path = item["relative_path"].as_str().expect("evidence path");
        let expected = item["digest"].as_str().expect("evidence digest");
        let bytes = fs::read(evidence_root.join(relative_path)).expect("read evidence file");
        assert_eq!(Digest::sha256(&bytes).as_str(), expected, "{relative_path}");
    }
}

#[test]
fn disposition_must_exactly_match_every_incomplete_control() {
    let mut missing = review_value();
    missing["disposition"]["blockers"] =
        json!(["source_lineage", "native_closure", "managed_startup"]);
    assert_eq!(
        parse_value(&missing),
        Err(RuntimePackageReviewError::InvalidDisposition)
    );

    let mut reordered = review_value();
    reordered["disposition"]["blockers"] = json!([
        "native_closure",
        "source_lineage",
        "managed_startup",
        "cloud_disable"
    ]);
    assert_eq!(
        parse_value(&reordered),
        Err(RuntimePackageReviewError::InvalidDisposition)
    );

    let mut false_admission = review_value();
    false_admission["disposition"] = json!({
        "status": "admitted",
        "layout_digest": "0000000000000000000000000000000000000000000000000000000000000000",
        "runtime_package_manifest_id": "1111111111111111111111111111111111111111111111111111111111111111"
    });
    assert_eq!(
        parse_value(&false_admission),
        Err(RuntimePackageReviewError::InvalidDisposition)
    );
}

#[test]
fn complete_controls_are_required_once_in_canonical_order() {
    let mut missing = review_value();
    missing["checks"].as_array_mut().expect("checks").pop();
    assert_eq!(
        parse_value(&missing),
        Err(RuntimePackageReviewError::InvalidChecks)
    );

    let mut reordered = review_value();
    reordered["checks"]
        .as_array_mut()
        .expect("checks")
        .swap(0, 1);
    assert_eq!(
        parse_value(&reordered),
        Err(RuntimePackageReviewError::InvalidChecks)
    );

    let mut absent_evidence = review_value();
    absent_evidence["checks"][0]["evidence"] = json!(["absent.json"]);
    assert_eq!(
        parse_value(&absent_evidence),
        Err(RuntimePackageReviewError::InvalidChecks)
    );
}

#[test]
fn all_passed_controls_may_carry_exact_admission_identity() {
    let mut admitted = review_value();
    for check in admitted["checks"].as_array_mut().expect("checks") {
        check["status"] = json!("passed");
    }
    admitted["disposition"] = json!({
        "status": "admitted",
        "layout_digest": "0000000000000000000000000000000000000000000000000000000000000000",
        "runtime_package_manifest_id": "1111111111111111111111111111111111111111111111111111111111111111"
    });
    assert!(parse_value(&admitted).is_ok());
}

#[test]
fn malformed_ambiguous_oversized_and_unsupported_reviews_fail_closed() {
    assert_eq!(
        RuntimePackageReview::parse(b"{\"schema_version\":1,\"schema_version\":1}"),
        Err(RuntimePackageReviewError::InvalidEncoding)
    );
    assert_eq!(
        RuntimePackageReview::parse(&vec![b' '; 256 * 1024 + 1]),
        Err(RuntimePackageReviewError::TooLarge)
    );

    let mut unknown = review_value();
    unknown["unknown"] = json!(true);
    assert_eq!(
        parse_value(&unknown),
        Err(RuntimePackageReviewError::InvalidEncoding)
    );

    let mut schema = review_value();
    schema["schema_version"] = json!(2);
    assert_eq!(
        parse_value(&schema),
        Err(RuntimePackageReviewError::UnsupportedSchema)
    );

    let mut target = review_value();
    target["target"]["operating_system"] = json!("windows");
    target["target"]["abi"] = json!("windows_msvc");
    assert_eq!(
        parse_value(&target),
        Err(RuntimePackageReviewError::UnsupportedTarget)
    );
}

fn parse_value(value: &Value) -> Result<RuntimePackageReview, RuntimePackageReviewError> {
    RuntimePackageReview::parse(&serde_json::to_vec(value).expect("review serializes"))
}
