use std::{collections::BTreeSet, path::Path};

use rewrite_model::{
    ArtifactSetId, ArtifactSetRelativePath, MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES,
    RuntimePackageManifestId,
};
use rewrite_types::Digest;
use serde_json::{Value, json};

use super::*;

fn foundation() -> RuntimeAdmissionEvidenceFoundation {
    RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
        ArtifactSetId::from_digest(Digest::sha256(b"source build evidence bundle")),
        ArtifactSetId::from_digest(Digest::sha256(b"source build inputs")),
        Digest::sha256(b"complete canonical source manifest"),
        Digest::sha256(b"controlled build plan"),
        Digest::sha256(b"verified source build report"),
        runtime_package_id(b"reconstructed runtime package"),
    ))
    .expect("fixture foundation should compile")
}

fn runtime_package_id(label: &[u8]) -> RuntimePackageManifestId {
    serde_json::from_value(json!(Digest::sha256(label)))
        .expect("fixture runtime package id should decode")
}

fn unit_sizes() -> [u64; RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT] {
    [1; RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT]
}

#[test]
fn foundation_has_exact_golden_encoding_and_round_trips() {
    let foundation = foundation();
    let expected = format!(
        concat!(
            "{{\"authority\":\"none\",",
            "\"build_plan_digest\":\"{}\",",
            "\"runtime_package_manifest_id\":\"{}\",",
            "\"schema_version\":1,",
            "\"source_build_evidence_bundle_id\":\"{}\",",
            "\"source_build_inputs_id\":\"{}\",",
            "\"source_manifest_digest\":\"{}\",",
            "\"source_report_digest\":\"{}\"}}"
        ),
        foundation.build_plan_digest(),
        foundation.runtime_package_manifest_id().digest(),
        foundation.source_build_evidence_bundle_id().digest(),
        foundation.source_build_inputs_id().digest(),
        foundation.source_manifest_digest(),
        foundation.source_report_digest(),
    );
    assert_eq!(foundation.canonical_bytes(), expected.as_bytes());

    let reparsed =
        RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(foundation.canonical_bytes())
            .expect("canonical foundation should parse");
    assert_eq!(reparsed, foundation);
    assert_ne!(
        foundation.foundation_id().digest(),
        &Digest::sha256(foundation.canonical_bytes()),
        "foundation identity must be domain separated"
    );
}

#[test]
fn foundation_parser_rejects_noncanonical_unknown_and_authoritative_forms() {
    let foundation = foundation();
    let mut spaced = foundation.canonical_bytes().to_vec();
    spaced.insert(1, b' ');
    assert!(matches!(
        RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(&spaced),
        Err(RuntimeAdmissionEvidenceContractError::InvalidEncoding)
    ));

    let mut unknown: Value = serde_json::from_slice(foundation.canonical_bytes())
        .expect("compiled foundation should decode as JSON");
    unknown["unexpected"] = json!(true);
    assert!(matches!(
        RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(
            &serde_json::to_vec(&unknown).expect("fixture mutation should encode")
        ),
        Err(RuntimeAdmissionEvidenceContractError::InvalidEncoding)
    ));

    let mut authoritative: Value = serde_json::from_slice(foundation.canonical_bytes())
        .expect("compiled foundation should decode as JSON");
    authoritative["authority"] = json!("admitted");
    assert!(
        RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(
            &serde_json::to_vec(&authoritative).expect("fixture mutation should encode")
        )
        .is_err()
    );

    let mut unsupported: Value = serde_json::from_slice(foundation.canonical_bytes())
        .expect("compiled foundation should decode as JSON");
    unsupported["schema_version"] = json!(2);
    assert!(matches!(
        RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(
            &serde_json::to_vec(&unsupported).expect("fixture mutation should encode")
        ),
        Err(RuntimeAdmissionEvidenceContractError::InvalidBinding)
    ));
}

#[test]
fn every_foundation_subject_mutation_changes_identity() {
    let original = foundation();
    for field in [
        "build_plan_digest",
        "runtime_package_manifest_id",
        "source_build_evidence_bundle_id",
        "source_build_inputs_id",
        "source_manifest_digest",
        "source_report_digest",
    ] {
        let mut mutated: Value = serde_json::from_slice(original.canonical_bytes())
            .expect("compiled foundation should decode as JSON");
        mutated[field] = json!(Digest::sha256(field.as_bytes()));
        let mutated = RuntimeAdmissionEvidenceFoundation::from_canonical_bytes(
            &serde_json::to_vec(&mutated).expect("fixture mutation should encode"),
        )
        .expect("canonical subject mutation should remain structurally valid");
        assert_ne!(mutated.foundation_id(), original.foundation_id(), "{field}");
    }
}

#[test]
fn fixed_tree_plan_has_exact_golden_encoding_and_round_trips() {
    let plan = RuntimeAdmissionEvidenceTreePlan::compile(
        unit_sizes(),
        RuntimeAdmissionEvidenceBundleLimits::default(),
    )
    .expect("unit-sized plan should compile");
    let encoded_members = RuntimeAdmissionEvidenceMember::ALL
        .iter()
        .map(|member| {
            format!(
                "{{\"byte_size\":1,\"path\":\"{}\"}}",
                member.relative_path()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let expected = format!("{{\"members\":[{encoded_members}],\"schema_version\":2}}");
    assert_eq!(plan.canonical_bytes(), expected.as_bytes());
    assert_eq!(plan.total_member_bytes(), 12);

    let reparsed = RuntimeAdmissionEvidenceTreePlan::from_canonical_bytes(
        plan.canonical_bytes(),
        RuntimeAdmissionEvidenceBundleLimits::default(),
    )
    .expect("canonical plan should parse");
    assert_eq!(reparsed, plan);
    assert_ne!(
        plan.plan_id().digest(),
        &Digest::sha256(plan.canonical_bytes()),
        "tree-plan identity must be domain separated"
    );
}

#[test]
fn fixed_tree_plan_paths_are_portable_unique_and_bounded() {
    let mut folded = BTreeSet::new();
    for member in RuntimeAdmissionEvidenceMember::ALL {
        let path = ArtifactSetRelativePath::new(member.relative_path().to_owned())
            .expect("fixed member path should be portable");
        assert!(folded.insert(path.as_str().to_ascii_lowercase()));
    }
    let manifest =
        ArtifactSetRelativePath::new(RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH.to_owned())
            .expect("fixed manifest path should be portable");
    assert!(folded.insert(manifest.as_str().to_ascii_lowercase()));

    let plan = RuntimeAdmissionEvidenceTreePlan::compile(
        unit_sizes(),
        RuntimeAdmissionEvidenceBundleLimits::default(),
    )
    .expect("unit-sized plan should compile");
    let mut overlong: Value = serde_json::from_slice(plan.canonical_bytes())
        .expect("compiled plan should decode as JSON");
    overlong["members"][0]["path"] = json!("a".repeat(513));
    assert!(matches!(
        RuntimeAdmissionEvidenceTreePlan::from_canonical_bytes(
            &serde_json::to_vec(&overlong).expect("fixture mutation should encode"),
            RuntimeAdmissionEvidenceBundleLimits::default(),
        ),
        Err(RuntimeAdmissionEvidenceContractError::InvalidBinding)
    ));
}

#[test]
fn tree_plan_parser_rejects_reordered_missing_and_noncanonical_members() {
    let plan = RuntimeAdmissionEvidenceTreePlan::compile(
        unit_sizes(),
        RuntimeAdmissionEvidenceBundleLimits::default(),
    )
    .expect("unit-sized plan should compile");
    let mut reordered: Value = serde_json::from_slice(plan.canonical_bytes())
        .expect("compiled plan should decode as JSON");
    reordered["members"]
        .as_array_mut()
        .expect("plan members should be an array")
        .swap(0, 1);
    assert!(matches!(
        RuntimeAdmissionEvidenceTreePlan::from_canonical_bytes(
            &serde_json::to_vec(&reordered).expect("fixture mutation should encode"),
            RuntimeAdmissionEvidenceBundleLimits::default(),
        ),
        Err(RuntimeAdmissionEvidenceContractError::InvalidBinding)
    ));

    let mut missing: Value = serde_json::from_slice(plan.canonical_bytes())
        .expect("compiled plan should decode as JSON");
    missing["members"]
        .as_array_mut()
        .expect("plan members should be an array")
        .pop();
    assert!(matches!(
        RuntimeAdmissionEvidenceTreePlan::from_canonical_bytes(
            &serde_json::to_vec(&missing).expect("fixture mutation should encode"),
            RuntimeAdmissionEvidenceBundleLimits::default(),
        ),
        Err(RuntimeAdmissionEvidenceContractError::InvalidBinding)
    ));

    let mut obsolete_schema: Value = serde_json::from_slice(plan.canonical_bytes())
        .expect("compiled plan should decode as JSON");
    obsolete_schema["schema_version"] = json!(1);
    assert!(matches!(
        RuntimeAdmissionEvidenceTreePlan::from_canonical_bytes(
            &serde_json::to_vec(&obsolete_schema).expect("fixture mutation should encode"),
            RuntimeAdmissionEvidenceBundleLimits::default(),
        ),
        Err(RuntimeAdmissionEvidenceContractError::InvalidBinding)
    ));

    let mut spaced = plan.canonical_bytes().to_vec();
    spaced.insert(1, b' ');
    assert!(matches!(
        RuntimeAdmissionEvidenceTreePlan::from_canonical_bytes(
            &spaced,
            RuntimeAdmissionEvidenceBundleLimits::default(),
        ),
        Err(RuntimeAdmissionEvidenceContractError::InvalidEncoding)
    ));
}

#[test]
fn tree_plan_enforces_member_aggregate_and_tree_limits() {
    let mut zero = unit_sizes();
    zero[0] = 0;
    assert!(matches!(
        RuntimeAdmissionEvidenceTreePlan::compile(
            zero,
            RuntimeAdmissionEvidenceBundleLimits::default()
        ),
        Err(RuntimeAdmissionEvidenceContractError::LimitExceeded)
    ));

    let mut oversized = unit_sizes();
    oversized[0] = RuntimeAdmissionEvidenceMember::Foundation.maximum_bytes() + 1;
    assert!(matches!(
        RuntimeAdmissionEvidenceTreePlan::compile(
            oversized,
            RuntimeAdmissionEvidenceBundleLimits::default()
        ),
        Err(RuntimeAdmissionEvidenceContractError::LimitExceeded)
    ));

    let aggregate_limit = RuntimeAdmissionEvidenceBundleLimits {
        maximum_total_bytes: u64::try_from(MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES)
            .expect("manifest ceiling should fit u64")
            + 11,
        ..RuntimeAdmissionEvidenceBundleLimits::default()
    };
    assert!(matches!(
        RuntimeAdmissionEvidenceTreePlan::compile(unit_sizes(), aggregate_limit),
        Err(RuntimeAdmissionEvidenceContractError::LimitExceeded)
    ));

    let tree_limit = RuntimeAdmissionEvidenceBundleLimits {
        maximum_tree_entries: RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT
            + tree_plan::RUNTIME_ADMISSION_EVIDENCE_DIRECTORY_COUNT,
        ..RuntimeAdmissionEvidenceBundleLimits::default()
    };
    assert!(matches!(
        tree_limit.validate(),
        Err(RuntimeAdmissionEvidenceContractError::LimitExceeded)
    ));

    for broadened in [
        RuntimeAdmissionEvidenceBundleLimits {
            maximum_destination_entries: MAX_RUNTIME_ADMISSION_EVIDENCE_DESTINATION_ENTRIES + 1,
            ..RuntimeAdmissionEvidenceBundleLimits::default()
        },
        RuntimeAdmissionEvidenceBundleLimits {
            maximum_staging_roots: MAX_RUNTIME_ADMISSION_EVIDENCE_STAGING_ROOTS + 1,
            ..RuntimeAdmissionEvidenceBundleLimits::default()
        },
    ] {
        assert!(matches!(
            broadened.validate(),
            Err(RuntimeAdmissionEvidenceContractError::LimitExceeded)
        ));
    }
}

#[test]
fn tree_plan_member_mutations_change_identity() {
    let original = RuntimeAdmissionEvidenceTreePlan::compile(
        unit_sizes(),
        RuntimeAdmissionEvidenceBundleLimits::default(),
    )
    .expect("unit-sized plan should compile");
    for index in 0..RUNTIME_ADMISSION_EVIDENCE_MEMBER_COUNT {
        let mut mutated: Value = serde_json::from_slice(original.canonical_bytes())
            .expect("compiled plan should decode as JSON");
        mutated["members"][index]["byte_size"] = json!(2);
        let mutated = RuntimeAdmissionEvidenceTreePlan::from_canonical_bytes(
            &serde_json::to_vec(&mutated).expect("fixture mutation should encode"),
            RuntimeAdmissionEvidenceBundleLimits::default(),
        )
        .expect("bounded size mutation should parse");
        assert_ne!(mutated.plan_id(), original.plan_id(), "member {index}");
    }
}

#[test]
fn destination_source_and_overlap_contracts_fail_before_io() {
    assert!(matches!(
        RuntimeAdmissionEvidenceBundleDestination::new("CON"),
        Err(RuntimeAdmissionEvidenceContractError::UnsafeBoundary)
    ));
    assert!(matches!(
        RuntimeAdmissionEvidenceBundleDestination::new("a".repeat(256)),
        Err(RuntimeAdmissionEvidenceContractError::UnsafeBoundary)
    ));
    let destination = RuntimeAdmissionEvidenceBundleDestination::new("admission-evidence")
        .expect("portable absent destination should form");
    let source = RuntimeAdmissionEvidenceBundleSource::new("admission-evidence")
        .expect("source selection should form");
    assert!(destination.path().is_absolute());
    assert!(source.path().is_absolute());
    assert!(contract::paths_overlap(
        Path::new("C:/Evidence/Root"),
        Path::new("c:/evidence/root/child")
    ));
    assert!(!contract::paths_overlap(
        Path::new("C:/Evidence/Root"),
        Path::new("C:/Evidence/Rooted")
    ));
}
