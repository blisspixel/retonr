use std::fs;

use rewrite_model::{
    CandidateGenerationEvidenceBundleId, GenerationQualificationPlanId, PlannedCandidateAttemptId,
};
use rewrite_model_store::{
    CandidateGenerationEvidenceStorageRootId, CandidateGenerationEvidenceStorageV1Limits,
};
use rewrite_types::Digest;
use serde::de::DeserializeOwned;

use super::*;

fn id<T: DeserializeOwned>(label: &str) -> T {
    serde_json::from_value(serde_json::Value::String(
        Digest::sha256(label.as_bytes()).as_str().to_owned(),
    ))
    .expect("typed digest ID")
}

fn limits() -> CandidateGenerationEvidenceStorageV1Limits {
    CandidateGenerationEvidenceStorageV1Limits::new(8_193, 256, 256 * 1_024 * 1_024)
        .expect("fixed read limits")
}

#[test]
fn initialization_reopen_and_complete_move_preserve_stable_root_identity() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let data = temporary.path().join("data");
    fs::create_dir(&data).expect("app data directory");

    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("initialize root");
    let root_id = repository.root_id().clone();
    assert_eq!(root_id.as_str().len(), 64);
    assert!(
        root_id
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
    repository.revalidate().expect("initial root remains valid");
    drop(repository);

    let reopened = CandidateGenerationEvidenceRepository::open_existing(&data, &root_id)
        .expect("reopen exact root");
    assert_eq!(reopened.root_id(), &root_id);
    drop(reopened);

    let moved = temporary.path().join("moved-data");
    fs::rename(&data, &moved).expect("move complete data directory");
    let moved_repository = CandidateGenerationEvidenceRepository::open_existing(&moved, &root_id)
        .expect("reopen moved root");
    assert_eq!(moved_repository.root_id(), &root_id);
}

#[test]
fn initialization_is_no_replace_and_reopen_requires_expected_identity() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let data = temporary.path().join("data");
    fs::create_dir(&data).expect("app data directory");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("initialize root");
    let root_id = repository.root_id().clone();
    drop(repository);

    assert!(matches!(
        CandidateGenerationEvidenceRepository::initialize(&data),
        Err(CandidateGenerationEvidenceRepositoryError::AlreadyInitialized)
    ));
    let foreign =
        CandidateGenerationEvidenceStorageRootId::new(Digest::sha256(b"foreign root").as_str())
            .expect("foreign root ID");
    assert!(matches!(
        CandidateGenerationEvidenceRepository::open_existing(&data, &foreign),
        Err(CandidateGenerationEvidenceRepositoryError::RootIdentityMismatch)
    ));
    CandidateGenerationEvidenceRepository::open_existing(&data, &root_id)
        .expect("expected identity reopens");
}

#[test]
fn durable_reference_is_canonical_root_bound_and_uses_only_read_limits() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let data = temporary.path().join("data");
    fs::create_dir(&data).expect("app data directory");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("initialize root");
    let plan: GenerationQualificationPlanId = id("plan");
    let attempt: PlannedCandidateAttemptId = id("attempt");
    let bundle: CandidateGenerationEvidenceBundleId = id("bundle");

    let storage = repository
        .bundle_storage_reference(plan.clone(), attempt.clone(), bundle.clone(), limits())
        .expect("compile root-bound reference");
    assert_eq!(storage.storage_root_id(), repository.root_id());
    assert_eq!(
        storage.relative_reference().as_str(),
        format!(
            "bundles/v1/{}/{}/{}",
            plan.digest().as_str(),
            attempt.digest().as_str(),
            bundle.digest().as_str(),
        )
    );
    assert_eq!(storage.limits(), limits());
    repository
        .validate_bundle_storage(&storage)
        .expect("reference belongs to retained root");

    let other_data = temporary.path().join("other-data");
    fs::create_dir(&other_data).expect("other app data directory");
    let other = CandidateGenerationEvidenceRepository::initialize(&other_data)
        .expect("initialize other root");
    assert!(matches!(
        other.validate_bundle_storage(&storage),
        Err(CandidateGenerationEvidenceRepositoryError::RootIdentityMismatch)
    ));
}

#[test]
fn missing_root_and_ascii_case_alias_are_rejected() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let empty = temporary.path().join("empty");
    fs::create_dir(&empty).expect("empty app data directory");
    let expected =
        CandidateGenerationEvidenceStorageRootId::new(Digest::sha256(b"missing root").as_str())
            .expect("expected root ID");
    assert!(matches!(
        CandidateGenerationEvidenceRepository::open_existing(&empty, &expected),
        Err(CandidateGenerationEvidenceRepositoryError::NotInitialized)
    ));

    let aliased = temporary.path().join("aliased");
    fs::create_dir(&aliased).expect("aliased app data directory");
    fs::create_dir(aliased.join("Generation-Evidence")).expect("case alias");
    assert!(matches!(
        CandidateGenerationEvidenceRepository::initialize(&aliased),
        Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary)
    ));
}

#[test]
fn malformed_noncanonical_and_mutated_markers_are_rejected() {
    for replacement in [
        br#"{"schema_version":1,"root_nonce":"00"}"#.as_slice(),
        br#"{"root_nonce":"0000000000000000000000000000000000000000000000000000000000000000","schema_version":1}"#
            .as_slice(),
        br#"{"schema_version":2,"root_nonce":"0000000000000000000000000000000000000000000000000000000000000000"}"#
            .as_slice(),
        br#"{"schema_version":1,"root_nonce":"0000000000000000000000000000000000000000000000000000000000000000","extra":true}"#
            .as_slice(),
    ] {
        let temporary = tempfile::tempdir().expect("temporary root");
        let data = temporary.path().join("data");
        fs::create_dir(&data).expect("app data directory");
        let repository = CandidateGenerationEvidenceRepository::initialize(&data)
            .expect("initialize root");
        let root_id = repository.root_id().clone();
        drop(repository);
        fs::write(
            data.join(STORAGE_DIRECTORY).join(ROOT_MARKER_FILE),
            replacement,
        )
        .expect("replace marker bytes");
        assert!(matches!(
            CandidateGenerationEvidenceRepository::open_existing(&data, &root_id),
            Err(CandidateGenerationEvidenceRepositoryError::InvalidRootMarker)
        ));
    }

    let temporary = tempfile::tempdir().expect("temporary root");
    let data = temporary.path().join("data");
    fs::create_dir(&data).expect("app data directory");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("initialize root");
    let mutation = fs::write(
        data.join(STORAGE_DIRECTORY).join(ROOT_MARKER_FILE),
        br#"{"schema_version":1,"root_nonce":"0000000000000000000000000000000000000000000000000000000000000000"}"#,
    );
    if mutation.is_ok() {
        assert!(repository.revalidate().is_err());
    } else {
        repository
            .revalidate()
            .expect("sealed platform blocks retained marker mutation");
    }
}

#[test]
fn unexpected_entries_and_hard_linked_marker_are_rejected() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let data = temporary.path().join("data");
    fs::create_dir(&data).expect("app data directory");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&data).expect("initialize root");
    let root_id = repository.root_id().clone();
    drop(repository);
    let root = data.join(STORAGE_DIRECTORY);
    fs::write(root.join("unexpected"), b"unexpected").expect("unexpected entry");
    assert!(matches!(
        CandidateGenerationEvidenceRepository::open_existing(&data, &root_id),
        Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary)
    ));
    fs::remove_file(root.join("unexpected")).expect("remove unexpected entry");
    fs::hard_link(root.join(ROOT_MARKER_FILE), root.join("marker-alias"))
        .expect("hard link marker");
    assert!(matches!(
        CandidateGenerationEvidenceRepository::open_existing(&data, &root_id),
        Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary)
    ));
}

#[cfg(unix)]
#[test]
fn link_and_fixed_component_case_substitutions_are_rejected() {
    use std::os::unix::fs::symlink;

    let temporary = tempfile::tempdir().expect("temporary root");
    let target = temporary.path().join("target");
    let data = temporary.path().join("data");
    fs::create_dir(&target).expect("link target");
    fs::create_dir(&data).expect("app data directory");
    symlink(&target, data.join(STORAGE_DIRECTORY)).expect("storage link");
    assert!(matches!(
        CandidateGenerationEvidenceRepository::initialize(&data),
        Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary)
    ));

    let canonical = temporary.path().join("canonical");
    fs::create_dir(&canonical).expect("canonical app data directory");
    let repository =
        CandidateGenerationEvidenceRepository::initialize(&canonical).expect("initialize root");
    let root_id = repository.root_id().clone();
    drop(repository);
    fs::create_dir(canonical.join(STORAGE_DIRECTORY).join("Bundles"))
        .expect("fixed component case alias");
    assert!(matches!(
        CandidateGenerationEvidenceRepository::open_existing(&canonical, &root_id),
        Err(CandidateGenerationEvidenceRepositoryError::UnsafeBoundary)
    ));
}
