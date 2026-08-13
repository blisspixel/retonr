use std::{fs, fs::File};

use rewrite_model_store::ArtifactStateStore;
use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::super::MAX_RECOVERY_ENTRIES;
use super::{
    ARTIFACT_BYTES, ArtifactImportError, ArtifactImportStage, OfflineArtifactImportRequest,
    OfflineArtifactImportService, limits, manifest, storage_key,
};
use crate::artifact_storage::fingerprint_std_file;

mod platform;
mod resource_limits;

#[test]
fn final_callback_mutation_fails_before_state_registration() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let artifact_id = expected_manifest.artifact_id.clone();
    let final_path = storage.join(storage_key(&expected_manifest.artifact_digest));
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::Finalizing {
                fs::write(&final_path, vec![b'x'; ARTIFACT_BYTES.len()])
                    .expect("mutate canonical file from final callback");
            }
        })
        .expect_err("final callback mutation must fail closed");
    assert!(matches!(error, ArtifactImportError::StorageConflict));
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[test]
fn final_callback_removal_is_reported_as_storage_change() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let artifact_id = expected_manifest.artifact_id.clone();
    let final_path = storage.join(storage_key(&expected_manifest.artifact_digest));
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::Finalizing {
                fs::remove_file(&final_path).expect("remove canonical file at final callback");
            }
        })
        .expect_err("final callback removal must fail closed");
    assert!(matches!(error, ArtifactImportError::StorageChanged));
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[test]
fn final_callback_cancellation_leaves_verified_orphan_without_state() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let artifact_id = expected_manifest.artifact_id.clone();
    let final_path = storage.join(storage_key(&expected_manifest.artifact_digest));
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let cancellation = CancellationToken::new();
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &cancellation, |event| {
            if event.stage == ArtifactImportStage::Finalizing {
                cancellation.cancel();
            }
        })
        .expect_err("last pre-commit cancellation must stop state registration");
    assert!(matches!(error, ArtifactImportError::Cancelled));
    assert_eq!(
        fs::read(final_path).expect("read retained verified orphan"),
        ARTIFACT_BYTES
    );
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[test]
fn committing_callback_cancellation_leaves_no_managed_bytes_or_state() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let artifact_id = expected_manifest.artifact_id.clone();
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let cancellation = CancellationToken::new();
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &cancellation, |event| {
            if event.stage == ArtifactImportStage::CommittingFile {
                cancellation.cancel();
            }
        })
        .expect_err("pre-commit cancellation must leave no managed artifact");
    assert!(matches!(error, ArtifactImportError::Cancelled));
    assert_eq!(
        fs::read_dir(storage.join(".staging"))
            .expect("read staging directory")
            .count(),
        0
    );
    assert_eq!(
        fs::read_dir(storage.join("artifacts"))
            .expect("read artifact directory")
            .count(),
        0
    );
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[test]
fn callback_staging_replacement_is_not_deleted_during_cancellation() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let request = OfflineArtifactImportRequest {
        source,
        manifest: manifest(ARTIFACT_BYTES),
    };
    let cancellation = CancellationToken::new();
    let mut replacement_path = None;
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &cancellation, |event| {
            if event.stage == ArtifactImportStage::CommittingFile {
                let staged = fs::read_dir(storage.join(".staging"))
                    .expect("read staging directory")
                    .next()
                    .expect("one staged entry")
                    .expect("read staged entry")
                    .path();
                fs::remove_file(&staged).expect("unlink held staged entry");
                fs::write(&staged, b"callback replacement").expect("write staging replacement");
                replacement_path = Some(staged);
                cancellation.cancel();
            }
        })
        .expect_err("cancellation after replacement must fail closed");
    assert!(matches!(error, ArtifactImportError::Cancelled));
    assert_eq!(
        fs::read(replacement_path.expect("record replacement path"))
            .expect("read retained callback replacement"),
        b"callback replacement"
    );
}

#[test]
fn callback_external_hard_link_prevents_staging_commit_and_cleanup() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    let external_alias = directory.path().join("external-alias.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let request = OfflineArtifactImportRequest {
        source,
        manifest: manifest(ARTIFACT_BYTES),
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::CommittingFile {
                let staged = fs::read_dir(storage.join(".staging"))
                    .expect("read staging directory")
                    .next()
                    .expect("one staged entry")
                    .expect("read staged entry")
                    .path();
                fs::hard_link(staged, &external_alias).expect("create external staging alias");
            }
        })
        .expect_err("staging link-count drift must fail closed");
    assert!(matches!(error, ArtifactImportError::StorageChanged));
    assert_eq!(
        fs::read(&external_alias).expect("read retained external alias"),
        ARTIFACT_BYTES
    );
    assert_eq!(
        fs::read_dir(storage.join(".staging"))
            .expect("read cleaned staging directory")
            .count(),
        0
    );
}

#[test]
fn preexisting_external_hard_link_is_not_registered_as_managed_bytes() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    let external_alias = directory.path().join("external-canonical-alias.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let artifact_id = expected_manifest.artifact_id.clone();
    let final_path = storage.join(storage_key(&expected_manifest.artifact_digest));
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");
    fs::write(&final_path, ARTIFACT_BYTES).expect("write canonical fixture");
    fs::hard_link(&final_path, &external_alias).expect("create external canonical alias");

    let error = service
        .import(&request, &CancellationToken::new(), |_| {})
        .expect_err("externally aliased canonical bytes must fail closed");
    assert!(matches!(error, ArtifactImportError::StorageChanged));
    assert_eq!(
        fs::read(external_alias).expect("read alias"),
        ARTIFACT_BYTES
    );
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[test]
fn successful_import_leaves_one_canonical_link() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let final_path = storage.join(storage_key(&expected_manifest.artifact_digest));
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    service
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("import exact artifact");
    let file = File::open(final_path).expect("open final artifact");
    assert!(
        fingerprint_std_file(&file)
            .expect("fingerprint final artifact")
            .has_single_link()
    );
}

#[test]
fn case_folded_destination_is_never_accepted_as_the_canonical_name() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let lowercase = storage.join(storage_key(&expected_manifest.artifact_digest));
    let uppercase =
        lowercase.with_file_name(expected_manifest.artifact_digest.as_str().to_uppercase());
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");
    fs::write(&uppercase, ARTIFACT_BYTES).expect("write case-folded fixture");
    let case_insensitive = lowercase.exists();

    let result = service.import(&request, &CancellationToken::new(), |_| {});
    if case_insensitive {
        assert!(matches!(result, Err(ArtifactImportError::StorageChanged)));
    } else {
        result.expect("case-sensitive storage permits the distinct canonical name");
    }
}

#[test]
fn no_clobber_commit_defers_an_exact_race_winner_to_a_retry() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let final_path = storage.join(storage_key(&expected_manifest.artifact_digest));
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::CommittingFile && !final_path.exists() {
                fs::write(&final_path, ARTIFACT_BYTES).expect("commit exact race winner");
            }
        })
        .expect_err("newly observed race winner requires a fresh import");
    assert!(matches!(error, ArtifactImportError::StorageChanged));
    drop(service);
    assert_eq!(
        store
            .manifest(&request.manifest.artifact_id)
            .expect("check absent state"),
        None
    );
    assert_eq!(
        fs::read(&final_path).expect("read exact final bytes"),
        ARTIFACT_BYTES
    );
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("reopen import service");
    service
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("fresh import accepts the stable exact destination");
}

#[test]
fn no_clobber_commit_preserves_a_conflicting_race_winner() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let expected_manifest = manifest(ARTIFACT_BYTES);
    let artifact_id = expected_manifest.artifact_id.clone();
    let final_path = storage.join(storage_key(&expected_manifest.artifact_digest));
    let conflicting = vec![b'x'; ARTIFACT_BYTES.len()];
    let request = OfflineArtifactImportRequest {
        source,
        manifest: expected_manifest,
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");

    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::CommittingFile && !final_path.exists() {
                fs::write(&final_path, &conflicting).expect("commit conflicting race winner");
            }
        })
        .expect_err("conflicting race winner must fail closed");
    assert!(matches!(error, ArtifactImportError::StorageChanged));
    assert_eq!(
        fs::read(final_path).expect("read preserved conflicting bytes"),
        conflicting
    );
    drop(service);
    assert_eq!(
        store.manifest(&artifact_id).expect("check absent state"),
        None
    );
}

#[test]
fn staging_name_uses_128_bits_of_lowercase_hex() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source.gguf");
    let storage = directory.path().join("managed");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let request = OfflineArtifactImportRequest {
        source,
        manifest: manifest(ARTIFACT_BYTES),
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(&storage, &mut store, limits())
        .expect("open import service");
    let mut observed_name = None;

    service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::StagingAndVerifying && observed_name.is_none() {
                observed_name = fs::read_dir(storage.join(".staging"))
                    .expect("read staging directory")
                    .next()
                    .map(|entry| entry.expect("read staging entry").file_name());
            }
        })
        .expect("import exact artifact");
    let name = observed_name
        .expect("observe staging name")
        .into_string()
        .expect("staging name is valid Unicode");
    let suffix = name
        .strip_prefix(".import-")
        .expect("staging name has reserved prefix");
    assert_eq!(suffix.len(), 32);
    assert!(
        suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
}

#[test]
fn recovery_rejects_reserved_non_regular_entry() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let staging = storage.join(".staging");
    let regular = staging.join(".import-regular");
    let reserved = staging.join(".import-not-a-file");
    fs::create_dir_all(&reserved).expect("create reserved directory fixture");
    fs::write(&regular, b"must remain").expect("create reserved regular fixture");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");

    let Err(error) = OfflineArtifactImportService::open(&storage, &mut store, limits()) else {
        panic!("reserved non-regular entry must fail recovery");
    };
    assert!(matches!(error, ArtifactImportError::UnsafeStorageLayout));
    assert!(reserved.is_dir());
    assert_eq!(
        fs::read(regular).expect("read retained regular staging fixture"),
        b"must remain"
    );
}

#[test]
fn recovery_entry_count_is_bounded_before_cleanup() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let staging = storage.join(".staging");
    fs::create_dir_all(&staging).expect("create staging directory");
    for index in 0..=MAX_RECOVERY_ENTRIES {
        fs::write(staging.join(format!("entry-{index:04}")), b"retained")
            .expect("write bounded recovery fixture");
    }
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");

    let Err(error) = OfflineArtifactImportService::open(&storage, &mut store, limits()) else {
        panic!("oversized staging inventory must fail recovery");
    };
    assert!(matches!(
        error,
        ArtifactImportError::StagingEntryLimitExceeded
    ));
    assert_eq!(
        fs::read_dir(staging)
            .expect("read unchanged staging directory")
            .count(),
        MAX_RECOVERY_ENTRIES + 1
    );
}
