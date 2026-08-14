use std::path::PathBuf;

use rewrite_model::{ArtifactId, ArtifactSetManifest, ArtifactSetMember, ArtifactSetRelativePath};
use rewrite_types::Digest;

use super::{
    ArtifactSetImportError, ArtifactSetImportLimits, OfflineArtifactSetImportRequest,
    manifest::validate_manifest_and_limits,
};

fn member(path: &str, bytes: &[u8]) -> ArtifactSetMember {
    ArtifactSetMember::new(
        ArtifactId::from_digest(Digest::sha256(bytes)),
        u64::try_from(bytes.len()).expect("fixture size fits u64"),
        ArtifactSetRelativePath::new(path).expect("portable fixture path"),
    )
}

fn manifest(mut members: Vec<ArtifactSetMember>) -> ArtifactSetManifest {
    members.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    ArtifactSetManifest::new(members).expect("valid fixture manifest")
}

const fn generous_limits() -> ArtifactSetImportLimits {
    ArtifactSetImportLimits {
        maximum_members: 16,
        maximum_member_bytes: 64,
        maximum_total_bytes: 256,
        maximum_tree_entries: 32,
        maximum_storage_entries: 8,
        maximum_staging_entries: 8,
    }
}

fn boundary_manifest() -> ArtifactSetManifest {
    manifest(vec![
        member("dir/a.bin", b"ab"),
        member("dir/b.bin", b"xyz"),
        member("empty.bin", b""),
    ])
}

#[test]
fn freezes_versioned_storage_key_derivation() {
    let manifest = manifest(vec![
        member("config.json", b"{}"),
        member("model.gguf", b"weights"),
    ]);
    let plan = validate_manifest_and_limits(&manifest, generous_limits())
        .expect("validate canonical fixture");

    assert_eq!(
        plan.artifact_set_id.digest().as_str(),
        "2625e1fbe8c354080a7e041e30ad3f078e5cb4a29e3893ebc761a62b58961696"
    );
    assert_eq!(
        plan.storage_key,
        "set-v1-2625e1fbe8c354080a7e041e30ad3f078e5cb4a29e3893ebc761a62b58961696"
    );
}

#[test]
fn counts_unique_implied_directories_and_maximum_depth() {
    let manifest = manifest(vec![
        member("a/b/c/one.bin", b"one"),
        member("a/b/d/two.bin", b"two"),
        member("top.bin", b"top"),
    ]);
    let plan = validate_manifest_and_limits(&manifest, generous_limits())
        .expect("validate nested fixture");

    assert_eq!(
        plan.directories
            .iter()
            .map(ArtifactSetRelativePath::as_str)
            .collect::<Vec<_>>(),
        vec!["a", "a/b", "a/b/c", "a/b/d",]
    );
    assert_eq!(plan.tree_entries, 7);
    assert_eq!(plan.maximum_depth, 4);
}

#[test]
fn admits_zero_byte_member_inside_a_nonempty_set() {
    let manifest = manifest(vec![member("empty.bin", b""), member("value.bin", b"x")]);
    let limits = ArtifactSetImportLimits {
        maximum_members: 2,
        maximum_member_bytes: 1,
        maximum_total_bytes: 1,
        maximum_tree_entries: 2,
        maximum_storage_entries: 1,
        maximum_staging_entries: 1,
    };
    let plan = validate_manifest_and_limits(&manifest, limits)
        .expect("zero-byte member is valid when the set is nonempty");

    assert_eq!(manifest.total_byte_size(), 1);
    assert_eq!(plan.tree_entries, 2);
    assert_eq!(plan.maximum_depth, 1);
}

#[test]
fn accepts_every_limit_at_its_exact_required_boundary() {
    let manifest = boundary_manifest();
    let limits = ArtifactSetImportLimits {
        maximum_members: 3,
        maximum_member_bytes: 3,
        maximum_total_bytes: 5,
        maximum_tree_entries: 4,
        maximum_storage_entries: 1,
        maximum_staging_entries: 1,
    };

    let plan = validate_manifest_and_limits(&manifest, limits)
        .expect("all exact required boundaries are inclusive");
    assert_eq!(plan.tree_entries, 4);
}

#[test]
fn rejects_each_manifest_limit_one_below_the_required_boundary() {
    let manifest = boundary_manifest();
    let exact = ArtifactSetImportLimits {
        maximum_members: 3,
        maximum_member_bytes: 3,
        maximum_total_bytes: 5,
        maximum_tree_entries: 4,
        maximum_storage_entries: 1,
        maximum_staging_entries: 1,
    };

    assert!(matches!(
        validate_manifest_and_limits(
            &manifest,
            ArtifactSetImportLimits {
                maximum_members: 2,
                ..exact
            }
        ),
        Err(ArtifactSetImportError::TooManyMembers {
            actual: 3,
            maximum: 2
        })
    ));
    assert!(matches!(
        validate_manifest_and_limits(
            &manifest,
            ArtifactSetImportLimits {
                maximum_member_bytes: 2,
                ..exact
            }
        ),
        Err(ArtifactSetImportError::MemberTooLarge {
            actual: 3,
            maximum: 2
        })
    ));
    assert!(matches!(
        validate_manifest_and_limits(
            &manifest,
            ArtifactSetImportLimits {
                maximum_total_bytes: 4,
                ..exact
            }
        ),
        Err(ArtifactSetImportError::ArtifactSetTooLarge {
            actual: 5,
            maximum: 4
        })
    ));
    assert!(matches!(
        validate_manifest_and_limits(
            &manifest,
            ArtifactSetImportLimits {
                maximum_tree_entries: 3,
                ..exact
            }
        ),
        Err(ArtifactSetImportError::TreeEntryLimitExceeded)
    ));
}

#[test]
fn rejects_zero_for_every_public_limit_before_source_access() {
    let request = OfflineArtifactSetImportRequest {
        source_root: PathBuf::from("source-path-must-not-be-opened"),
        manifest: boundary_manifest(),
    };
    let valid = generous_limits();
    let invalid = [
        ArtifactSetImportLimits {
            maximum_members: 0,
            ..valid
        },
        ArtifactSetImportLimits {
            maximum_member_bytes: 0,
            ..valid
        },
        ArtifactSetImportLimits {
            maximum_total_bytes: 0,
            ..valid
        },
        ArtifactSetImportLimits {
            maximum_tree_entries: 0,
            ..valid
        },
        ArtifactSetImportLimits {
            maximum_storage_entries: 0,
            ..valid
        },
        ArtifactSetImportLimits {
            maximum_staging_entries: 0,
            ..valid
        },
    ];

    for limits in invalid {
        assert!(matches!(
            validate_manifest_and_limits(&request.manifest, limits),
            Err(ArtifactSetImportError::InvalidLimits)
        ));
    }
}

#[test]
fn rejects_count_limits_that_cannot_reserve_one_more_entry() {
    let manifest = boundary_manifest();
    let valid = generous_limits();
    for limits in [
        ArtifactSetImportLimits {
            maximum_members: usize::MAX,
            ..valid
        },
        ArtifactSetImportLimits {
            maximum_tree_entries: usize::MAX,
            ..valid
        },
        ArtifactSetImportLimits {
            maximum_storage_entries: usize::MAX,
            ..valid
        },
        ArtifactSetImportLimits {
            maximum_staging_entries: usize::MAX,
            ..valid
        },
    ] {
        assert!(matches!(
            validate_manifest_and_limits(&manifest, limits),
            Err(ArtifactSetImportError::InvalidLimits)
        ));
    }
}

#[test]
fn accepts_maximum_byte_ceilings_without_overflow() {
    let manifest = boundary_manifest();
    let limits = ArtifactSetImportLimits {
        maximum_member_bytes: u64::MAX,
        maximum_total_bytes: u64::MAX,
        ..generous_limits()
    };

    validate_manifest_and_limits(&manifest, limits)
        .expect("maximum byte ceilings remain valid comparisons");
}
