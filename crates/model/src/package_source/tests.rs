use rewrite_types::Digest;

use super::{
    MAX_PACKAGE_SOURCE_JSON_BYTES, PACKAGE_SOURCE_SCHEMA_VERSION, PackageSource,
    PackageSourceError, PackageSourceKind,
};

fn source() -> PackageSource {
    PackageSource::new(
        PackageSourceKind::UpstreamRelease,
        "https://example.invalid/runtime",
        "v1.2.3",
        Digest::sha256(b"provenance"),
    )
    .expect("valid source")
}

#[test]
fn source_identity_is_stable_and_field_sensitive() {
    let source = source();
    assert_eq!(source.schema_version(), PACKAGE_SOURCE_SCHEMA_VERSION);
    assert_eq!(source.kind(), PackageSourceKind::UpstreamRelease);
    assert_eq!(source.locator(), "https://example.invalid/runtime");
    assert_eq!(source.revision(), "v1.2.3");
    assert_eq!(source.provenance_digest(), &Digest::sha256(b"provenance"));
    assert_eq!(
        source.package_source_id().digest().as_str(),
        "66a3b2b9b924d451e08eb27a062eb7526e4864f79ce2c9337fb4a3dee7eb528f"
    );

    for changed in [
        PackageSource::new(
            PackageSourceKind::RepositoryRevision,
            source.locator(),
            source.revision(),
            source.provenance_digest().clone(),
        ),
        PackageSource::new(
            source.kind(),
            "https://example.invalid/other",
            source.revision(),
            source.provenance_digest().clone(),
        ),
        PackageSource::new(
            source.kind(),
            source.locator(),
            "v1.2.4",
            source.provenance_digest().clone(),
        ),
        PackageSource::new(
            source.kind(),
            source.locator(),
            source.revision(),
            Digest::sha256(b"other"),
        ),
    ] {
        assert_ne!(
            changed.expect("valid change").package_source_id(),
            source.package_source_id()
        );
    }
}

#[test]
fn source_json_round_trips_and_rejects_unsafe_or_unbounded_input() {
    let source = source();
    let encoded = serde_json::to_vec(&source).expect("serialize");
    assert_eq!(
        PackageSource::from_json_bytes(&encoded).expect("decode"),
        source
    );
    for invalid in [
        PackageSource::new(
            source.kind(),
            "",
            source.revision(),
            source.provenance_digest().clone(),
        ),
        PackageSource::new(
            source.kind(),
            "https://user@example.invalid/runtime",
            source.revision(),
            source.provenance_digest().clone(),
        ),
        PackageSource::new(
            source.kind(),
            source.locator(),
            "revision with space",
            source.provenance_digest().clone(),
        ),
    ] {
        assert_eq!(invalid, Err(PackageSourceError::InvalidMetadata));
    }
    assert_eq!(
        PackageSource::from_json_bytes(br#"{"schema_version":2,"kind":"local_archive","locator":"local","revision":"one","provenance_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#),
        Err(PackageSourceError::UnsupportedSchema(2))
    );
    assert_eq!(
        PackageSource::from_json_bytes(br#"{"schema_version":1,"kind":"local_archive","locator":"local","revision":"one","provenance_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","extra":true}"#),
        Err(PackageSourceError::InvalidEncoding)
    );
    assert_eq!(
        PackageSource::from_json_bytes(&vec![b' '; MAX_PACKAGE_SOURCE_JSON_BYTES + 1]),
        Err(PackageSourceError::EncodedSourceTooLarge)
    );
}

#[test]
fn source_and_transformation_tags_are_append_only() {
    let kinds = [
        PackageSourceKind::UpstreamRelease,
        PackageSourceKind::RepositoryRevision,
        PackageSourceKind::LocalArchive,
    ];
    assert_eq!(kinds.map(super::source_kind_byte), [0, 1, 2]);
    for (kind, name) in
        kinds
            .into_iter()
            .zip(["upstream_release", "repository_revision", "local_archive"])
    {
        let encoded = format!("\"{name}\"").into_bytes();
        assert_eq!(serde_json::to_vec(&kind).expect("serialize"), encoded);
        assert_eq!(
            serde_json::from_slice::<PackageSourceKind>(&encoded).expect("deserialize"),
            kind
        );
    }
    let untransformed = crate::PackageTransformation::Untransformed {
        evidence_digest: Digest::sha256(b"same"),
    };
    let transformed = crate::PackageTransformation::Transformed {
        source_artifact_set_id: crate::ArtifactSetManifest::new(vec![
            crate::ArtifactSetMember::new(
                crate::ArtifactId::from_digest(Digest::sha256(b"source")),
                1,
                crate::ArtifactSetRelativePath::new("source").expect("path"),
            ),
        ])
        .expect("set")
        .artifact_set_id(),
        tool_evidence_digest: Digest::sha256(b"tool"),
        parameters_digest: Digest::sha256(b"parameters"),
        log_digest: Digest::sha256(b"log"),
    };
    for (value, tag, name) in [
        (&untransformed, 0, "untransformed"),
        (&transformed, 1, "transformed"),
    ] {
        let mut canonical = Vec::new();
        value.append_canonical(&mut canonical);
        assert_eq!(canonical[0], tag);
        assert_eq!(
            serde_json::to_value(value).expect("serialize")["kind"],
            name
        );
    }
}
