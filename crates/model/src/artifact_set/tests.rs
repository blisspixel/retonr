use rewrite_types::Digest;

use super::{
    ARTIFACT_SET_MANIFEST_SCHEMA_VERSION, ArtifactSetManifest, ArtifactSetManifestError,
    ArtifactSetMember, ArtifactSetPathError, ArtifactSetRelativePath,
    MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES, MAX_ARTIFACT_SET_MEMBERS,
    MAX_ARTIFACT_SET_RELATIVE_PATH_BYTES, MAX_ARTIFACT_SET_TOTAL_PATH_BYTES,
};
use crate::ArtifactId;

fn member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    let digest = Digest::sha256(bytes);
    ArtifactSetMember::new(
        ArtifactId::from_digest(digest),
        bytes.len() as u64,
        ArtifactSetRelativePath::new(path).expect("fixture path"),
    )
}

#[test]
fn freezes_canonical_json_and_artifact_set_identity() {
    let manifest = ArtifactSetManifest::new(vec![
        member("config.json", b"{}"),
        member("model.gguf", b"weights"),
    ])
    .expect("canonical fixture");
    let expected = concat!(
        "{\"members\":[{\"artifact_id\":\"44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a\",",
        "\"byte_size\":2,\"relative_path\":\"config.json\"},{\"artifact_id\":",
        "\"9a129038d9a00aed0cf6a7ea059ca50a813449061ab87848cf1a13eafdf33b2c\",",
        "\"byte_size\":7,\"relative_path\":\"model.gguf\"}],\"schema_version\":1}"
    );
    assert_eq!(manifest.canonical_json(), expected);
    assert_eq!(
        serde_json::to_string(&manifest).expect("serialize manifest"),
        expected
    );
    assert_eq!(
        manifest.artifact_set_id().digest().as_str(),
        "2625e1fbe8c354080a7e041e30ad3f078e5cb4a29e3893ebc761a62b58961696"
    );
    assert_ne!(
        manifest.artifact_set_id().digest(),
        &Digest::sha256(expected.as_bytes())
    );
    assert_eq!(manifest.total_byte_size(), 9);
}

#[test]
fn strict_round_trip_revalidates_the_complete_manifest() {
    let manifest =
        ArtifactSetManifest::new(vec![member("model.gguf", b"weights")]).expect("valid manifest");
    let encoded = serde_json::to_string(&manifest).expect("serialize manifest");
    assert_eq!(
        ArtifactSetManifest::from_json_bytes(encoded.as_bytes()).expect("validated decode"),
        manifest
    );

    let mut unknown: serde_json::Value = serde_json::from_str(&encoded).expect("parse value");
    unknown["unknown"] = serde_json::json!(true);
    assert_eq!(
        ArtifactSetManifest::from_json_bytes(
            serde_json::to_string(&unknown)
                .expect("encode unknown")
                .as_bytes()
        ),
        Err(ArtifactSetManifestError::InvalidEncoding)
    );

    let future = encoded.replace("\"schema_version\":1", "\"schema_version\":2");
    assert!(ArtifactSetManifest::from_json_bytes(future.as_bytes()).is_err());
    assert_eq!(
        manifest.schema_version(),
        ARTIFACT_SET_MANIFEST_SCHEMA_VERSION
    );
}

#[test]
fn encoded_manifest_limit_precedes_json_allocation() {
    assert_eq!(
        ArtifactSetManifest::from_json_bytes(&vec![b' '; MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES]),
        Err(ArtifactSetManifestError::InvalidEncoding)
    );
    assert_eq!(
        ArtifactSetManifest::from_json_bytes(&vec![b' '; MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES + 1]),
        Err(ArtifactSetManifestError::EncodedManifestTooLarge)
    );
}

#[test]
fn rejects_nonportable_and_reserved_paths() {
    let too_long = "a".repeat(MAX_ARTIFACT_SET_RELATIVE_PATH_BYTES + 1);
    let overlong_component = format!("dir/{}", "a".repeat(256));
    let invalid = [
        "",
        "/model",
        "model/",
        "a//b",
        ".",
        "..",
        "a/../b",
        r"C:\model",
        r"\\server\share",
        "model:stream",
        "a\\b",
        "a<b",
        "a>b",
        "a\"b",
        "a|b",
        "a?b",
        "a*b",
        " leading",
        "trailing ",
        "trailing.",
        "nonascii-\u{e9}",
        "line\nfeed",
        too_long.as_str(),
        overlong_component.as_str(),
    ];
    for value in invalid {
        assert_eq!(
            ArtifactSetRelativePath::new(value),
            Err(ArtifactSetPathError::InvalidPath),
            "unexpected result for {value:?}"
        );
    }
    for reserved in [
        "CON",
        "con.txt",
        "dir/PRN",
        "AUX.json",
        "nul",
        "COM1.bin",
        "com9",
        "LPT1",
        "lpt9.txt",
        "CONIN$",
        "conout$.log",
        "CLOCK$",
    ] {
        assert_eq!(
            ArtifactSetRelativePath::new(reserved),
            Err(ArtifactSetPathError::ReservedComponent)
        );
    }
    for valid in [".hidden", "dir name/model-1_q4.gguf", "com0", "lpt10"] {
        ArtifactSetRelativePath::new(valid).expect("portable path");
    }
}

#[test]
fn rejects_noncanonical_order_and_portable_namespace_collisions() {
    let unordered = ArtifactSetManifest::new(vec![member("z.gguf", b"z"), member("a.gguf", b"a")]);
    assert_eq!(unordered, Err(ArtifactSetManifestError::NoncanonicalOrder));

    for paths in [
        ["a", "a"],
        ["A", "a"],
        ["Dir/a", "dir/b"],
        ["a", "a/b"],
        ["a/b", "a"],
    ] {
        let mut members = paths.map(|path| member(path, b"x")).to_vec();
        members.sort_by(|left, right| {
            left.relative_path()
                .as_str()
                .cmp(right.relative_path().as_str())
        });
        assert!(matches!(
            ArtifactSetManifest::new(members),
            Err(ArtifactSetManifestError::PathCollision
                | ArtifactSetManifestError::NoncanonicalOrder)
        ));
    }
}

#[test]
fn enforces_member_path_and_size_bounds() {
    assert_eq!(
        ArtifactSetManifest::new(Vec::new()),
        Err(ArtifactSetManifestError::EmptySet)
    );
    let too_many = (0..=MAX_ARTIFACT_SET_MEMBERS)
        .map(|index| member(&format!("{index:04}.bin"), b"x"))
        .collect();
    assert_eq!(
        ArtifactSetManifest::new(too_many),
        Err(ArtifactSetManifestError::TooManyMembers)
    );

    let maximum_members = (0..MAX_ARTIFACT_SET_MEMBERS)
        .map(|index| member(&format!("{index:04}.bin"), b"x"))
        .collect();
    ArtifactSetManifest::new(maximum_members).expect("member boundary is valid");

    let paths_over_budget = (0..514)
        .map(|index| {
            member(
                &format!("{index:04}/{}/{}", "a".repeat(250), "b".repeat(255)),
                b"x",
            )
        })
        .collect();
    const {
        assert!(514 * 511 > MAX_ARTIFACT_SET_TOTAL_PATH_BYTES);
    }
    assert_eq!(
        ArtifactSetManifest::new(paths_over_budget),
        Err(ArtifactSetManifestError::PathBudgetExceeded)
    );

    let empty = ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(b"")),
        0,
        ArtifactSetRelativePath::new("empty").expect("path"),
    );
    assert_eq!(
        ArtifactSetManifest::new(vec![empty.clone()]),
        Err(ArtifactSetManifestError::EmptyContent)
    );
    ArtifactSetManifest::new(vec![empty, member("value", b"x")])
        .expect("individual empty file is valid");

    let maximum = ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(b"maximum")),
        u64::MAX,
        ArtifactSetRelativePath::new("a").expect("path"),
    );
    let overflow = ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(b"overflow")),
        1,
        ArtifactSetRelativePath::new("b").expect("path"),
    );
    assert_eq!(
        ArtifactSetManifest::new(vec![maximum, overflow]),
        Err(ArtifactSetManifestError::TotalSizeOverflow)
    );
}

#[test]
fn every_member_identity_change_changes_the_set_identity() {
    let baseline =
        ArtifactSetManifest::new(vec![member("model.gguf", b"weights")]).expect("baseline");
    let variants = [
        ArtifactSetManifest::new(vec![member("model.gguf", b"changed")]).expect("variant"),
        ArtifactSetManifest::new(vec![member("renamed.gguf", b"weights")]).expect("variant"),
        ArtifactSetManifest::new(vec![
            member("config.json", b"{}"),
            member("model.gguf", b"weights"),
        ])
        .expect("variant"),
    ];
    for variant in variants {
        assert_ne!(variant.artifact_set_id(), baseline.artifact_set_id());
    }

    let one_member = baseline.members()[0].artifact_id();
    assert_ne!(baseline.artifact_set_id().digest(), one_member.digest());
}
