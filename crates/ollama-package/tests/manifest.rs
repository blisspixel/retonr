mod support;

use rewrite_ollama_package::{
    CONFIG_MEDIA_TYPE, MODEL_MEDIA_TYPE, ReconstructionError, ReconstructionLimits,
    parse_manifest_v2,
};

use support::{MetadataValue, package_fixture};

#[test]
fn exact_manifest_shape_produces_a_bounded_plan() {
    let fixture = package_fixture();
    let plan = parse_manifest_v2(&fixture.manifest, &ReconstructionLimits::default())
        .expect("fixture manifest parses");
    assert_eq!(plan.raw_manifest_size(), fixture.manifest.len() as u64);
    assert_eq!(
        plan.raw_manifest_digest(),
        &rewrite_types::Digest::sha256(&fixture.manifest)
    );
    assert_eq!(plan.config().media_type(), CONFIG_MEDIA_TYPE);
    assert_eq!(plan.model().media_type(), MODEL_MEDIA_TYPE);
    assert!(plan.config().size() > 0);
    assert!(plan.model().size() > plan.template().size());
    assert_eq!(fixture.blobs.len(), 5);
    assert!(matches!(
        MetadataValue::Raw {
            kind: 0,
            bytes: Vec::new()
        },
        MetadataValue::Raw { .. }
    ));
}

#[test]
fn manifest_rejects_size_encoding_schema_and_unknown_fields() {
    let fixture = package_fixture();
    let limits = ReconstructionLimits {
        manifest_bytes: fixture.manifest.len() - 1,
        ..ReconstructionLimits::default()
    };
    assert_eq!(
        parse_manifest_v2(&fixture.manifest, &limits),
        Err(ReconstructionError::ManifestTooLarge)
    );
    let excessive = ReconstructionLimits {
        manifest_bytes: ReconstructionLimits::default().manifest_bytes + 1,
        ..ReconstructionLimits::default()
    };
    assert_eq!(
        parse_manifest_v2(&fixture.manifest, &excessive),
        Err(ReconstructionError::LimitExceeded)
    );
    assert_eq!(
        parse_manifest_v2(
            br#"{"schemaVersion":2,"schemaVersion":2}"#,
            &ReconstructionLimits::default()
        ),
        Err(ReconstructionError::InvalidManifest)
    );
    for mutation in [
        ("schemaVersion", serde_json::json!(3)),
        ("mediaType", serde_json::json!("wrong")),
        ("extra", serde_json::json!(true)),
    ] {
        let mut value: serde_json::Value =
            serde_json::from_slice(&fixture.manifest).expect("fixture JSON");
        value[mutation.0] = mutation.1;
        let result = parse_manifest_v2(
            &serde_json::to_vec(&value).expect("mutation serializes"),
            &ReconstructionLimits::default(),
        );
        let expected = if mutation.0 == "extra" {
            ReconstructionError::InvalidManifest
        } else {
            ReconstructionError::UnsupportedManifest
        };
        assert_eq!(result, Err(expected));
    }
}

#[test]
fn manifest_rejects_cardinality_order_digest_and_size_drift() {
    let fixture = package_fixture();
    let base: serde_json::Value = serde_json::from_slice(&fixture.manifest).expect("fixture JSON");
    let mutate = |value: serde_json::Value| {
        parse_manifest_v2(
            &serde_json::to_vec(&value).expect("mutation serializes"),
            &ReconstructionLimits::default(),
        )
    };

    let mut short = base.clone();
    short["layers"].as_array_mut().expect("layers").pop();
    assert_eq!(mutate(short), Err(ReconstructionError::UnsupportedManifest));

    let mut reordered = base.clone();
    reordered["layers"]
        .as_array_mut()
        .expect("layers")
        .swap(0, 1);
    assert_eq!(
        mutate(reordered),
        Err(ReconstructionError::InvalidDescriptor)
    );

    for digest in ["sha512:abc", "sha256:ABCDEF", "sha256:abc"] {
        let mut changed = base.clone();
        changed["layers"][0]["digest"] = serde_json::json!(digest);
        assert_eq!(mutate(changed), Err(ReconstructionError::InvalidDescriptor));
    }
    for size in [0_u64, u64::MAX] {
        let mut changed = base.clone();
        changed["layers"][0]["size"] = serde_json::json!(size);
        assert_eq!(mutate(changed), Err(ReconstructionError::InvalidDescriptor));
    }
}
