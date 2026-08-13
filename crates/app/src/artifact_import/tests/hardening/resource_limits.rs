use std::fs;

use rewrite_model_store::ArtifactStateStore;
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::super::super::MAX_RECOVERY_ENTRIES;
use super::super::{
    ARTIFACT_BYTES, ArtifactImportError, ArtifactImportLimits, ArtifactImportStage,
    OfflineArtifactImportRequest, OfflineArtifactImportService, manifest,
};

#[test]
fn storage_entry_ceiling_accepts_the_boundary_and_blocks_growth() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let first = OfflineArtifactImportRequest {
        source: source.clone(),
        manifest: manifest(ARTIFACT_BYTES),
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 1_024,
            maximum_storage_entries: 2,
        },
    )
    .expect("open bounded import service");
    let unrelated_digest = Digest::sha256(b"unrelated managed bytes");
    fs::write(
        storage.join("artifacts").join(unrelated_digest.as_str()),
        b"unrelated managed bytes",
    )
    .expect("write existing managed entry");

    service
        .import(&first, &CancellationToken::new(), |_| {})
        .expect("import fills the exact entry ceiling");

    let second_bytes = b"second exact artifact";
    fs::write(&source, second_bytes).expect("replace source fixture");
    let second = OfflineArtifactImportRequest {
        source,
        manifest: manifest(second_bytes),
    };
    let error = service
        .import(&second, &CancellationToken::new(), |_| {})
        .expect_err("storage growth past the caller ceiling must fail");
    assert!(matches!(
        error,
        ArtifactImportError::StorageEntryLimitExceeded
    ));
    assert_eq!(
        fs::read_dir(storage.join("artifacts"))
            .expect("read bounded artifact directory")
            .count(),
        2
    );
}

#[test]
fn cancellation_from_initial_progress_interrupts_exact_name_scan() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let request = OfflineArtifactImportRequest {
        source,
        manifest: manifest(ARTIFACT_BYTES),
    };
    let cancellation = CancellationToken::new();
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 1_024,
            maximum_storage_entries: 16,
        },
    )
    .expect("open import service");
    fs::write(storage.join("artifacts/unrelated"), b"unrelated").expect("write scan fixture");

    let error = service
        .import(&request, &cancellation, |event| {
            if event.stage == ArtifactImportStage::InspectingSource {
                cancellation.cancel();
            }
        })
        .expect_err("caller cancellation must interrupt the entry scan");
    assert!(matches!(error, ArtifactImportError::Cancelled));
}

#[test]
fn full_staging_directory_fails_before_reserving_an_import_file() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let request = OfflineArtifactImportRequest {
        source,
        manifest: manifest(ARTIFACT_BYTES),
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 1_024,
            maximum_storage_entries: 16,
        },
    )
    .expect("open import service");
    let staging = storage.join(".staging");
    for index in 0..MAX_RECOVERY_ENTRIES {
        fs::write(staging.join(format!("unrelated-{index:04}")), b"retained")
            .expect("write staging capacity fixture");
    }

    let error = service
        .import(&request, &CancellationToken::new(), |_| {})
        .expect_err("full staging storage must fail before reservation");
    assert!(matches!(
        error,
        ArtifactImportError::StagingEntryLimitExceeded
    ));
    assert_eq!(
        fs::read_dir(&staging)
            .expect("read unchanged staging directory")
            .count(),
        MAX_RECOVERY_ENTRIES
    );
    drop(service);
    OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 1_024,
            maximum_storage_entries: 16,
        },
    )
    .expect("full but valid staging directory remains reopenable");
}

#[test]
fn callback_filled_staging_still_cleans_the_owned_import_file() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let request = OfflineArtifactImportRequest {
        source,
        manifest: manifest(ARTIFACT_BYTES),
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 1_024,
            maximum_storage_entries: 16,
        },
    )
    .expect("open import service");
    let staging = storage.join(".staging");

    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::CommittingFile {
                for index in 0..MAX_RECOVERY_ENTRIES {
                    fs::write(staging.join(format!("callback-{index:04}")), b"retained")
                        .expect("fill staging during callback");
                }
            }
        })
        .expect_err("staging growth during callback must fail closed");
    assert!(matches!(
        error,
        ArtifactImportError::StagingEntryLimitExceeded
    ));
    assert!(
        fs::read_dir(&staging)
            .expect("read staging after cleanup")
            .all(|entry| !entry
                .expect("read staging entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".import-"))
    );
    drop(service);
    OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 1_024,
            maximum_storage_entries: 16,
        },
    )
    .expect("callback-filled staging remains reopenable");
}

#[test]
fn callback_filled_artifact_capacity_blocks_before_final_link() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("managed");
    let source = directory.path().join("source.gguf");
    fs::write(&source, ARTIFACT_BYTES).expect("write source fixture");
    let request = OfflineArtifactImportRequest {
        source,
        manifest: manifest(ARTIFACT_BYTES),
    };
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    let mut service = OfflineArtifactImportService::open(
        &storage,
        &mut store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 1_024,
            maximum_storage_entries: 2,
        },
    )
    .expect("open import service");
    let artifacts = storage.join("artifacts");
    fs::write(artifacts.join("initial"), b"retained").expect("write initial entry");

    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactImportStage::CommittingFile {
                fs::write(artifacts.join("callback"), b"retained")
                    .expect("fill artifact capacity during callback");
            }
        })
        .expect_err("callback-filled artifact storage must fail before final link");
    assert!(matches!(
        error,
        ArtifactImportError::StorageEntryLimitExceeded
    ));
    assert_eq!(
        fs::read_dir(&artifacts)
            .expect("read bounded artifact directory")
            .count(),
        2
    );
    assert_eq!(
        fs::read_dir(storage.join(".staging"))
            .expect("read cleaned staging directory")
            .count(),
        0
    );
}
