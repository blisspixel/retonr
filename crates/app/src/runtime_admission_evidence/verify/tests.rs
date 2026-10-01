use std::{fs, path::Path};

use rewrite_model::{ArtifactId, ArtifactSetId, ArtifactSetMember};
use tempfile::TempDir;

use super::*;
use crate::RuntimeAdmissionEvidenceFoundationInput;

fn fixture() -> TempDir {
    let root = tempfile::tempdir().expect("fixture operation succeeds");
    let digest = Digest::sha256(b"fixture immutable subject");
    let foundation =
        RuntimeAdmissionEvidenceFoundation::compile(RuntimeAdmissionEvidenceFoundationInput::new(
            ArtifactSetId::from_digest(digest.clone()),
            ArtifactSetId::from_digest(digest.clone()),
            digest.clone(),
            digest.clone(),
            digest.clone(),
            serde_json::from_value(serde_json::json!(digest)).expect("fixture operation succeeds"),
        ))
        .expect("fixture operation succeeds");
    for member in RuntimeAdmissionEvidenceMember::ALL {
        let path = root.path().join(member.relative_path());
        fs::create_dir_all(path.parent().expect("fixture operation succeeds"))
            .expect("fixture operation succeeds");
        let bytes = if member == RuntimeAdmissionEvidenceMember::Foundation {
            foundation.canonical_bytes()
        } else {
            b"opaque unadjudicated evidence"
        };
        fs::write(path, bytes).expect("fixture operation succeeds");
    }
    refresh_manifest(root.path());
    root
}

fn refresh_manifest(root: &Path) {
    let mut members = RuntimeAdmissionEvidenceMember::ALL
        .into_iter()
        .map(|member| {
            let bytes =
                fs::read(root.join(member.relative_path())).expect("fixture operation succeeds");
            ArtifactSetMember::new(
                ArtifactId::from_digest(Digest::sha256(&bytes)),
                bytes.len() as u64,
                ArtifactSetRelativePath::new(member.relative_path())
                    .expect("fixture operation succeeds"),
            )
        })
        .collect::<Vec<_>>();
    members.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    let manifest = ArtifactSetManifest::new(members).expect("fixture operation succeeds");
    fs::write(
        root.join(RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH),
        manifest.canonical_json(),
    )
    .expect("fixture operation succeeds");
}

fn acquire(
    root: &Path,
) -> Result<RuntimeAdmissionEvidenceBundleLease, RuntimeAdmissionEvidenceBundleError> {
    RuntimeAdmissionEvidenceBundleVerifier::acquire(
        &RuntimeAdmissionEvidenceBundleSource::new(root).expect("fixture operation succeeds"),
        RuntimeAdmissionEvidenceBundleLimits::default(),
        &CancellationToken::new(),
    )
}

#[test]
fn exact_opaque_tree_is_inert_revalidated_and_debug_redacted() {
    let root = fixture();
    let lease = acquire(root.path()).expect("fixture operation succeeds");
    assert_eq!(lease.manifest().members().len(), 12);
    assert_eq!(lease.tree_plan().members().len(), 12);
    assert_eq!(
        lease
            .member_bytes(
                RuntimeAdmissionEvidenceMember::LicenseControl,
                &CancellationToken::new()
            )
            .expect("fixture operation succeeds"),
        b"opaque unadjudicated evidence"
    );
    assert_eq!(
        lease.foundation().canonical_bytes(),
        lease
            .member_bytes(
                RuntimeAdmissionEvidenceMember::Foundation,
                &CancellationToken::new()
            )
            .expect("fixture operation succeeds")
    );
    let debug = format!("{lease:?}");
    assert!(!debug.contains(&root.path().display().to_string()));
    assert!(!debug.contains("opaque unadjudicated"));
    lease
        .revalidate(&CancellationToken::new())
        .expect("fixture operation succeeds");
}

#[test]
fn same_size_content_drift_rejects_acquisition_and_retained_read() {
    let root = fixture();
    let lease = acquire(root.path()).expect("fixture operation succeeds");
    fs::write(
        root.path()
            .join(RuntimeAdmissionEvidenceMember::LicenseControl.relative_path()),
        b"changed unadjudicated bytes!!",
    )
    .expect("fixture operation succeeds");
    assert!(acquire(root.path()).is_err());
    assert!(
        lease
            .member_bytes(
                RuntimeAdmissionEvidenceMember::Foundation,
                &CancellationToken::new()
            )
            .is_err()
    );
}

#[test]
fn rehashed_manifest_cannot_change_retained_snapshot() {
    let root = fixture();
    let lease = acquire(root.path()).expect("fixture operation succeeds");
    fs::write(
        root.path()
            .join(RuntimeAdmissionEvidenceMember::LicenseControl.relative_path()),
        b"substitute",
    )
    .expect("fixture operation succeeds");
    refresh_manifest(root.path());
    assert!(acquire(root.path()).is_ok());
    assert!(lease.revalidate(&CancellationToken::new()).is_err());
}

#[test]
fn missing_extra_and_wrong_case_members_fail() {
    for mutation in 0..4 {
        let root = fixture();
        match mutation {
            0 => fs::remove_file(root.path().join("controls/license-v2.json"))
                .expect("fixture operation succeeds"),
            1 => fs::write(root.path().join("extra.json"), b"extra")
                .expect("fixture operation succeeds"),
            2 => fs::create_dir(root.path().join("extra")).expect("fixture operation succeeds"),
            _ => {
                fs::remove_dir_all(root.path().join("policy")).expect("fixture operation succeeds");
                fs::write(root.path().join("policy"), b"wrong kind")
                    .expect("fixture operation succeeds");
            }
        }
        assert!(acquire(root.path()).is_err(), "mutation {mutation}");
    }
}

#[test]
fn malformed_noncanonical_and_incomplete_manifests_fail() {
    for mutation in 0..6 {
        let root = fixture();
        let path = root
            .path()
            .join(RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH);
        let original = fs::read(&path).expect("fixture operation succeeds");
        let bytes = match mutation {
            0 => b"{}".to_vec(),
            1 => [original.as_slice(), b"\n"].concat(),
            2 => {
                let manifest = ArtifactSetManifest::from_json_bytes(&original)
                    .expect("fixture operation succeeds");
                ArtifactSetManifest::new(manifest.members()[1..].to_vec())
                    .expect("fixture operation succeeds")
                    .canonical_json()
                    .into_bytes()
            }
            3 => b"{\"schema_version\":1,\"schema_version\":1,\"members\":[]}".to_vec(),
            4 => {
                let mut value: serde_json::Value =
                    serde_json::from_slice(&original).expect("fixture operation succeeds");
                value["members"][0]["relative_path"] =
                    serde_json::json!("controls/LICENSE-v2.json");
                serde_json::to_vec(&value).expect("fixture operation succeeds")
            }
            _ => vec![b' '; MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES + 1],
        };
        fs::write(path, bytes).expect("fixture operation succeeds");
        assert!(acquire(root.path()).is_err());
    }
}

#[test]
fn manifest_declared_size_mismatch_fails() {
    let root = fixture();
    let path = root
        .path()
        .join(RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH);
    let original = fs::read(&path).expect("fixture operation succeeds");
    let manifest =
        ArtifactSetManifest::from_json_bytes(&original).expect("fixture operation succeeds");
    let mut members = manifest.members().to_vec();
    let first = &members[0];
    members[0] = ArtifactSetMember::new(
        first.artifact_id().clone(),
        first.byte_size() + 1,
        first.relative_path().clone(),
    );
    fs::write(
        path,
        ArtifactSetManifest::new(members)
            .expect("fixture operation succeeds")
            .canonical_json(),
    )
    .expect("fixture operation succeeds");
    assert!(matches!(
        acquire(root.path()),
        Err(RuntimeAdmissionEvidenceBundleError::InvalidTree)
    ));
}

#[test]
fn storage_errors_preserve_bounded_failure_classes() {
    for error in [
        ArtifactInventoryError::InvalidLimits,
        ArtifactInventoryError::StorageEntryLimitExceeded,
        ArtifactInventoryError::StateEntryLimitExceeded,
        ArtifactInventoryError::TotalVerificationLimitExceeded,
    ] {
        assert!(matches!(
            map_storage(error),
            RuntimeAdmissionEvidenceBundleError::LimitExceeded
        ));
    }
    assert!(matches!(
        map_storage(ArtifactInventoryError::Cancelled),
        RuntimeAdmissionEvidenceBundleError::Cancelled
    ));
    assert!(matches!(
        map_storage(ArtifactInventoryError::ConcurrentModification),
        RuntimeAdmissionEvidenceBundleError::Changed
    ));
    assert!(matches!(
        map_storage(ArtifactInventoryError::UnsafeStorageLayout),
        RuntimeAdmissionEvidenceBundleError::UnsafeBoundary
    ));
    assert!(matches!(
        map_storage(ArtifactInventoryError::StorageIo(std::io::Error::other(
            "fixture"
        ))),
        RuntimeAdmissionEvidenceBundleError::StorageIo(_)
    ));
}

#[test]
fn opaque_foundation_is_not_accepted_even_with_valid_digest() {
    let root = fixture();
    fs::write(
        root.path()
            .join(RuntimeAdmissionEvidenceMember::Foundation.relative_path()),
        b"opaque foundation",
    )
    .expect("fixture operation succeeds");
    refresh_manifest(root.path());
    assert!(matches!(
        acquire(root.path()),
        Err(RuntimeAdmissionEvidenceBundleError::Contract(_))
    ));
}

#[test]
fn member_limits_precede_member_reads() {
    let root = fixture();
    fs::write(
        root.path()
            .join(RuntimeAdmissionEvidenceMember::CloudDisablePolicyMaterial.relative_path()),
        vec![0; 65_537],
    )
    .expect("fixture operation succeeds");
    refresh_manifest(root.path());
    assert!(matches!(
        acquire(root.path()),
        Err(RuntimeAdmissionEvidenceBundleError::LimitExceeded)
    ));
}

#[test]
fn invalid_limits_and_cancellation_precede_source_access() {
    let source = RuntimeAdmissionEvidenceBundleSource::new("absent-admission-evidence-fixture")
        .expect("fixture operation succeeds");
    let limits = RuntimeAdmissionEvidenceBundleLimits {
        maximum_tree_entries: 16,
        ..RuntimeAdmissionEvidenceBundleLimits::default()
    };
    assert!(matches!(
        RuntimeAdmissionEvidenceBundleVerifier::acquire(&source, limits, &CancellationToken::new()),
        Err(RuntimeAdmissionEvidenceBundleError::Contract(_))
    ));
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        RuntimeAdmissionEvidenceBundleVerifier::acquire(
            &source,
            RuntimeAdmissionEvidenceBundleLimits::default(),
            &cancellation
        ),
        Err(RuntimeAdmissionEvidenceBundleError::Cancelled)
    ));
    let root = fixture();
    let lease = acquire(root.path()).expect("fixture operation succeeds");
    assert!(matches!(
        lease.revalidate(&cancellation),
        Err(RuntimeAdmissionEvidenceBundleError::Cancelled)
    ));
}

#[test]
fn caller_total_and_tree_ceilings_are_enforced() {
    let root = fixture();
    let source =
        RuntimeAdmissionEvidenceBundleSource::new(root.path()).expect("fixture operation succeeds");
    fs::write(
        root.path().join("native/reviewer-evidence-v1.json"),
        vec![0; 1_048_576],
    )
    .expect("fixture operation succeeds");
    refresh_manifest(root.path());
    let limits = RuntimeAdmissionEvidenceBundleLimits {
        maximum_total_bytes: 1_048_576,
        ..RuntimeAdmissionEvidenceBundleLimits::default()
    };
    assert!(matches!(
        RuntimeAdmissionEvidenceBundleVerifier::acquire(&source, limits, &CancellationToken::new()),
        Err(RuntimeAdmissionEvidenceBundleError::LimitExceeded)
    ));
    fs::create_dir(root.path().join("extra")).expect("fixture operation succeeds");
    let limits = RuntimeAdmissionEvidenceBundleLimits {
        maximum_tree_entries: 17,
        ..RuntimeAdmissionEvidenceBundleLimits::default()
    };
    assert!(matches!(
        RuntimeAdmissionEvidenceBundleVerifier::acquire(&source, limits, &CancellationToken::new()),
        Err(RuntimeAdmissionEvidenceBundleError::LimitExceeded)
    ));
}

#[test]
fn hardlinked_manifest_and_member_fail_closed() {
    for path in [
        RUNTIME_ADMISSION_EVIDENCE_BUNDLE_MANIFEST_PATH,
        "controls/license-v2.json",
    ] {
        let root = fixture();
        let outside = tempfile::tempdir().expect("fixture operation succeeds");
        fs::hard_link(root.path().join(path), outside.path().join("alias"))
            .expect("fixture operation succeeds");
        assert!(acquire(root.path()).is_err());
    }
}

#[test]
fn absent_root_and_regular_file_root_fail_closed() {
    let root = tempfile::tempdir().expect("fixture operation succeeds");
    assert!(matches!(
        acquire(&root.path().join("absent")),
        Err(RuntimeAdmissionEvidenceBundleError::StorageIo(_))
    ));
    fs::write(root.path().join("file"), b"file").expect("fixture operation succeeds");
    assert!(matches!(
        acquire(&root.path().join("file")),
        Err(RuntimeAdmissionEvidenceBundleError::UnsafeBoundary)
    ));
}

#[cfg(unix)]
#[test]
fn symlinked_root_member_and_directory_fail_closed() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    let outside = tempfile::tempdir().expect("fixture operation succeeds");
    symlink(root.path(), outside.path().join("root")).expect("fixture operation succeeds");
    assert!(acquire(&outside.path().join("root")).is_err());
    fs::remove_file(root.path().join("controls/license-v2.json"))
        .expect("fixture operation succeeds");
    fs::write(
        outside.path().join("member"),
        b"opaque unadjudicated evidence",
    )
    .expect("fixture operation succeeds");
    symlink(
        outside.path().join("member"),
        root.path().join("controls/license-v2.json"),
    )
    .expect("fixture operation succeeds");
    assert!(acquire(root.path()).is_err());
    let root = fixture();
    fs::rename(
        root.path().join("controls"),
        outside.path().join("controls"),
    )
    .expect("fixture operation succeeds");
    symlink(
        outside.path().join("controls"),
        root.path().join("controls"),
    )
    .expect("fixture operation succeeds");
    assert!(acquire(root.path()).is_err());
}

#[cfg(unix)]
#[test]
fn retained_root_substitution_is_detected() {
    let parent = tempfile::tempdir().expect("fixture operation succeeds");
    let root = fixture();
    let lease = acquire(root.path()).expect("fixture operation succeeds");
    fs::rename(root.path(), parent.path().join("moved")).expect("fixture operation succeeds");
    fs::create_dir(root.path()).expect("fixture operation succeeds");
    assert!(lease.revalidate(&CancellationToken::new()).is_err());
}
