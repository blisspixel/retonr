use serde_json::{Value, json};

use rewrite_types::Digest;

use super::{
    INSTALLED_ARTIFACT_SET_SCHEMA_VERSION, InstalledArtifactSet, InstalledArtifactSetError,
    MAX_INSTALLED_ARTIFACT_SET_JSON_BYTES,
};
use crate::{
    ArtifactId, ArtifactSetId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath,
};

fn member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture length fits u64"),
        ArtifactSetRelativePath::new(path).expect("valid fixture path"),
    )
}

fn manifest() -> ArtifactSetManifest {
    ArtifactSetManifest::new(vec![
        member("config/empty.json", b""),
        member("model/weights.gguf", b"weights"),
    ])
    .expect("valid manifest")
}

fn fixture() -> (ArtifactSetManifest, InstalledArtifactSet) {
    let manifest = manifest();
    let installed =
        InstalledArtifactSet::new(&manifest, "set_01.root").expect("valid installed artifact set");
    (manifest, installed)
}

fn encoded_value(installed: &InstalledArtifactSet) -> Value {
    serde_json::to_value(installed).expect("serialize installed set")
}

fn decode_value(
    value: &Value,
    manifest: &ArtifactSetManifest,
) -> Result<InstalledArtifactSet, InstalledArtifactSetError> {
    InstalledArtifactSet::from_json_bytes(
        &serde_json::to_vec(value).expect("encode fixture JSON"),
        manifest,
    )
}

#[test]
fn constructs_exposes_and_round_trips_exact_manifest_join() {
    let (manifest, installed) = fixture();
    assert_eq!(
        installed.schema_version(),
        INSTALLED_ARTIFACT_SET_SCHEMA_VERSION
    );
    assert_eq!(installed.artifact_set_id(), &manifest.artifact_set_id());
    assert_eq!(installed.storage_key(), "set_01.root");
    installed.validate_against(&manifest).expect("revalidate");

    let canonical = installed.canonical_json();
    assert_eq!(
        canonical,
        serde_json::to_string(&installed).expect("encode")
    );
    assert_eq!(
        canonical,
        format!(
            "{{\"artifact_set_id\":\"{}\",\"schema_version\":1,\"storage_key\":\"set_01.root\"}}",
            manifest.artifact_set_id().digest().as_str()
        )
    );
    assert_eq!(
        InstalledArtifactSet::from_json_bytes(canonical.as_bytes(), &manifest)
            .expect("decode canonical state"),
        installed
    );
}

#[test]
fn manifest_identity_commits_to_member_order_path_identity_and_size() {
    let (manifest, installed) = fixture();
    let variants = [
        ArtifactSetManifest::new(vec![
            member("config/empty.json", b""),
            member("model/other.gguf", b"weights"),
        ])
        .expect("path variant"),
        ArtifactSetManifest::new(vec![
            member("config/empty.json", b""),
            member("model/weights.gguf", b"other"),
        ])
        .expect("identity variant"),
        ArtifactSetManifest::new(vec![
            member("config/empty.json", b"x"),
            member("model/weights.gguf", b"weights"),
        ])
        .expect("size variant"),
    ];
    assert_ne!(manifest.artifact_set_id(), variants[0].artifact_set_id());
    for variant in variants {
        assert_eq!(
            installed.validate_against(&variant),
            Err(InstalledArtifactSetError::ArtifactSetMismatch)
        );
    }
}

#[test]
fn accepts_zero_byte_members_without_duplicating_member_state() {
    let manifest = manifest();
    assert_eq!(manifest.members()[0].byte_size(), 0);
    let installed = InstalledArtifactSet::new(&manifest, "zero-member-set")
        .expect("install set containing an empty member");
    let decoded =
        InstalledArtifactSet::from_json_bytes(installed.canonical_json().as_bytes(), &manifest)
            .expect("round trip set containing an empty member");
    assert_eq!(decoded, installed);
}

#[test]
fn validates_single_component_portable_storage_keys_and_bounds() {
    let manifest = manifest();
    for valid in ["a", "a-z_09.key", &"x".repeat(128)] {
        assert!(
            InstalledArtifactSet::new(&manifest, valid).is_ok(),
            "{valid}"
        );
    }
    for invalid in [
        String::new(),
        ".".repeat(129),
        "sets/root".to_owned(),
        "sets\\root".to_owned(),
        ".".to_owned(),
        "..".to_owned(),
        "white space".to_owned(),
        "Uppercase".to_owned(),
        "nonascii-\u{e9}".to_owned(),
        "..\\root".to_owned(),
        "model.".to_owned(),
        "con".to_owned(),
        "nul.txt".to_owned(),
        "lpt1".to_owned(),
    ] {
        assert_eq!(
            InstalledArtifactSet::new(&manifest, invalid),
            Err(InstalledArtifactSetError::InvalidStorageKey)
        );
    }
}

#[test]
fn rejects_manifest_schema_and_storage_mismatches_on_decode() {
    let (manifest, installed) = fixture();
    let mut value = encoded_value(&installed);
    value["artifact_set_id"] = json!(ArtifactSetId::from_digest(Digest::sha256(b"other")));
    assert_eq!(
        decode_value(&value, &manifest),
        Err(InstalledArtifactSetError::ArtifactSetMismatch)
    );

    let mut value = encoded_value(&installed);
    value["schema_version"] = json!(2);
    assert_eq!(
        decode_value(&value, &manifest),
        Err(InstalledArtifactSetError::UnsupportedSchema(2))
    );

    let mut value = encoded_value(&installed);
    value["storage_key"] = json!("sets/root");
    assert_eq!(
        decode_value(&value, &manifest),
        Err(InstalledArtifactSetError::InvalidStorageKey)
    );
}

#[test]
fn rejects_unknown_malformed_and_byte_limit_violations() {
    let (manifest, installed) = fixture();
    let mut value = encoded_value(&installed);
    value["unexpected"] = json!(true);
    assert_eq!(
        decode_value(&value, &manifest),
        Err(InstalledArtifactSetError::InvalidEncoding)
    );
    assert_eq!(
        InstalledArtifactSet::from_json_bytes(b"not json", &manifest),
        Err(InstalledArtifactSetError::InvalidEncoding)
    );

    let exact = vec![b' '; MAX_INSTALLED_ARTIFACT_SET_JSON_BYTES];
    assert_eq!(
        InstalledArtifactSet::from_json_bytes(&exact, &manifest),
        Err(InstalledArtifactSetError::InvalidEncoding)
    );
    let oversized = vec![b' '; MAX_INSTALLED_ARTIFACT_SET_JSON_BYTES + 1];
    assert_eq!(
        InstalledArtifactSet::from_json_bytes(&oversized, &manifest),
        Err(InstalledArtifactSetError::EncodedInstallationTooLarge)
    );
}

#[test]
fn serialized_contract_contains_no_authority_fields() {
    let (_, installed) = fixture();
    let canonical = installed.canonical_json();
    for forbidden in ["active", "qualified", "runtime", "lease", "role"] {
        assert!(!canonical.contains(forbidden));
    }
}
