use std::{fs, path::Path};

use rusqlite::Connection;
use tempfile::tempdir;

use super::*;
use crate::artifact_storage::{ManagedTreeLimits, PinnedDirectory};

fn request(source_root: &Path) -> OfflineArtifactSetImportRequest {
    OfflineArtifactSetImportRequest {
        source_root: source_root.to_path_buf(),
        manifest: manifest(),
    }
}

fn storage_key(manifest: &ArtifactSetManifest) -> String {
    format!("set-v1-{}", manifest.artifact_set_id().digest().as_str())
}

fn assert_no_state(store: &ArtifactStateStore, manifest: &ArtifactSetManifest) {
    assert!(
        store
            .artifact_set_installation(&manifest.artifact_set_id())
            .expect("read artifact-set installation")
            .is_none()
    );
}

#[test]
fn destination_race_never_replaces_the_winner_or_registers_state() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = request(&source);
    let final_root = storage.join("sets").join(storage_key(&request.manifest));
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let race_root = final_root.clone();
    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactSetImportStage::Finalizing && !race_root.exists() {
                fs::create_dir(&race_root).expect("create race winner root");
                fs::write(race_root.join("winner"), b"external").expect("write race winner");
            }
        })
        .expect_err("publication must not replace a race winner");

    assert!(
        matches!(error, ArtifactSetImportError::StorageChanged),
        "unexpected destination-race result: {error:?}"
    );
    assert_eq!(
        fs::read(final_root.join("winner")).expect("read race winner"),
        b"external"
    );
    assert!(
        fs::read_dir(storage.join(".set-staging"))
            .expect("staging directory")
            .next()
            .is_none()
    );
    assert_no_state(&store, &request.manifest);
}

#[test]
fn mid_copy_cancellation_cleans_owned_state_and_preserves_source() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = request(&source);
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let cancellation = CancellationToken::new();
    let callback_token = cancellation.clone();
    let error = service
        .import(&request, &cancellation, |event| {
            if event.stage == ArtifactSetImportStage::StagingAndVerifying
                && event.completed_members == 1
            {
                callback_token.cancel();
            }
        })
        .expect_err("mid-copy cancellation must stop import");

    assert!(matches!(error, ArtifactSetImportError::Cancelled));
    assert_eq!(
        fs::read(source.join("config.json")).expect("read source config"),
        b"{}"
    );
    assert_eq!(
        fs::read(source.join("model/weights.bin")).expect("read source weights"),
        b"weights"
    );
    assert!(
        fs::read_dir(storage.join(".set-staging"))
            .expect("staging directory")
            .next()
            .is_none()
    );
    assert!(
        fs::read_dir(storage.join("sets"))
            .expect("sets directory")
            .next()
            .is_none()
    );
    assert_no_state(&store, &request.manifest);
}

#[test]
fn conflicting_final_hardlink_is_preserved_and_never_registered() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = request(&source);
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let final_root = storage.join("sets").join(storage_key(&request.manifest));
    fs::create_dir(&final_root).expect("final root");
    fs::create_dir(final_root.join("model")).expect("final nested root");
    fs::write(final_root.join("config.json"), b"{}").expect("final config");
    fs::write(final_root.join("model/empty.bin"), b"").expect("final empty member");
    let external = directory.path().join("external-weights");
    fs::write(&external, b"weights").expect("external hardlink source");
    fs::hard_link(&external, final_root.join("model/weights.bin")).expect("managed hardlink");

    assert!(matches!(
        service.import(&request, &CancellationToken::new(), |_| {}),
        Err(ArtifactSetImportError::StorageConflict)
    ));
    assert_eq!(
        fs::read(&external).expect("read external bytes"),
        b"weights"
    );
    assert_eq!(
        fs::read(final_root.join("model/weights.bin")).expect("read managed alias"),
        b"weights"
    );
    assert_no_state(&store, &request.manifest);
}

#[test]
fn source_hardlinks_are_copied_into_independent_managed_members() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    fs::create_dir(&source).expect("source root");
    fs::create_dir(source.join("nested")).expect("source nested root");
    fs::write(source.join("first.bin"), b"same").expect("source first member");
    fs::hard_link(source.join("first.bin"), source.join("nested/second.bin"))
        .expect("source hardlink");
    let manifest = ArtifactSetManifest::new(vec![
        member("first.bin", b"same"),
        member("nested/second.bin", b"same"),
    ])
    .expect("hardlink manifest");
    let request = OfflineArtifactSetImportRequest {
        source_root: source.clone(),
        manifest: manifest.clone(),
    };
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let result = service
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("import hardlinked source");
    let final_root = storage.join("sets").join(result.installed.storage_key());
    let pinned = PinnedDirectory::open_existing(&final_root).expect("pin final root");
    let snapshot = pinned
        .enumerate_tree(
            ManagedTreeLimits::new(4).expect("tree limits"),
            &CancellationToken::new(),
        )
        .expect("inspect final tree");

    assert!(snapshot.entries().iter().all(|entry| {
        entry.kind() != crate::artifact_storage::ManagedTreeEntryKind::RegularFile
            || entry.has_single_link()
    }));
    assert_eq!(
        fs::read(source.join("first.bin")).expect("read source first"),
        b"same"
    );
    assert_eq!(
        fs::read(source.join("nested/second.bin")).expect("read source second"),
        b"same"
    );
}

#[test]
fn a_live_service_excludes_a_second_owner() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("storage");
    let mut first_store =
        ArtifactStateStore::open(&directory.path().join("first.sqlite3")).expect("first store");
    let _first = OfflineArtifactSetImportService::open(&storage, &mut first_store, limits())
        .expect("first service");
    let mut second_store =
        ArtifactStateStore::open(&directory.path().join("second.sqlite3")).expect("second store");

    assert!(matches!(
        OfflineArtifactSetImportService::open(&storage, &mut second_store, limits()),
        Err(ArtifactSetImportError::StorageInUse)
    ));
}

#[test]
fn stale_staging_entries_are_counted_but_never_descended_into() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let stale = storage.join(".set-staging/stale/nested");
    fs::create_dir_all(&stale).expect("stale staging tree");
    for index in 0..40 {
        fs::write(stale.join(format!("opaque-{index}")), b"retain").expect("stale opaque bytes");
    }
    let request = request(&source);
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");

    service
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("import beside stale staging root");
    assert_eq!(
        fs::read(stale.join("opaque-39")).expect("read stale bytes"),
        b"retain"
    );
    assert_eq!(
        fs::read_dir(&stale)
            .expect("read stale descendants")
            .count(),
        40
    );
}

#[test]
fn staging_root_limit_includes_the_new_reservation() {
    for (existing, succeeds) in [(3, true), (4, false)] {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("source");
        let storage = directory.path().join("storage");
        write_source(&source);
        let request = request(&source);
        let mut store =
            ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
        let mut bounded = limits();
        bounded.maximum_staging_entries = 4;
        let mut service =
            OfflineArtifactSetImportService::open(&storage, &mut store, bounded).expect("service");
        for index in 0..existing {
            fs::create_dir(storage.join(".set-staging").join(format!("stale-{index}")))
                .expect("stale staging root");
        }

        let result = service.import(&request, &CancellationToken::new(), |_| {});
        if succeeds {
            assert!(result.is_ok(), "boundary reservation should succeed");
        } else {
            assert!(matches!(
                result,
                Err(ArtifactSetImportError::StagingEntryLimitExceeded)
            ));
            assert_no_state(&store, &request.manifest);
        }
    }
}

#[test]
fn callback_injection_is_retained_as_suspicious_and_never_registered() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = request(&source);
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let staging_parent = storage.join(".set-staging");
    let mut injected = false;
    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactSetImportStage::StagingAndVerifying && !injected {
                let stage = fs::read_dir(&staging_parent)
                    .expect("read current staging")
                    .next()
                    .expect("current staging root")
                    .expect("staging entry")
                    .path();
                fs::write(stage.join("unexpected"), b"external").expect("inject staging entry");
                injected = true;
            }
        })
        .expect_err("staging injection must fail closed");

    assert!(
        matches!(error, ArtifactSetImportError::StorageChanged),
        "unexpected staging-injection result: {error:?}"
    );
    let retained = fs::read_dir(&staging_parent)
        .expect("read retained staging")
        .next()
        .expect("retained suspicious root")
        .expect("retained staging entry")
        .path();
    assert_eq!(
        fs::read(retained.join("unexpected")).expect("read injected bytes"),
        b"external"
    );
    assert_no_state(&store, &request.manifest);
}

#[test]
fn state_failure_after_publication_leaves_a_retryable_exact_orphan() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    let state_path = directory.path().join("state.sqlite3");
    write_source(&source);
    let request = request(&source);
    let mut store = ArtifactStateStore::open(&state_path).expect("state store");
    Connection::open(&state_path)
        .expect("open trigger connection")
        .execute_batch(
            "CREATE TRIGGER fail_set_install
             BEFORE INSERT ON installed_artifact_sets
             BEGIN SELECT RAISE(ABORT, 'test state failure'); END;",
        )
        .expect("install failure trigger");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    assert!(matches!(
        service.import(&request, &CancellationToken::new(), |_| {}),
        Err(ArtifactSetImportError::State(_))
    ));
    drop(service);
    assert!(
        storage
            .join("sets")
            .join(storage_key(&request.manifest))
            .is_dir()
    );
    assert_no_state(&store, &request.manifest);
    Connection::open(&state_path)
        .expect("open trigger cleanup connection")
        .execute_batch("DROP TRIGGER fail_set_install;")
        .expect("remove failure trigger");

    let retry = OfflineArtifactSetImportService::open(&storage, &mut store, limits())
        .expect("retry service")
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("register exact published orphan");
    assert_eq!(
        retry.disposition,
        ArtifactSetImportDisposition::RegisteredExisting
    );
}

#[cfg(unix)]
#[test]
fn managed_storage_root_replacement_blocks_state_registration() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    let displaced = directory.path().join("displaced-storage");
    write_source(&source);
    let request = request(&source);
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");
    let mut service =
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()).expect("service");
    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactSetImportStage::Finalizing && !displaced.exists() {
                fs::rename(&storage, &displaced).expect("displace held storage root");
                fs::create_dir(&storage).expect("replace storage root path");
            }
        })
        .expect_err("replaced managed root must not register state");

    assert!(matches!(error, ArtifactSetImportError::StorageChanged));
    assert_no_state(&store, &request.manifest);
    assert!(storage.is_dir());
    assert!(
        displaced
            .join("sets")
            .join(storage_key(&request.manifest))
            .is_dir()
    );
}

#[cfg(unix)]
#[test]
fn final_root_replacement_blocks_state_registration() {
    let directory = tempdir().expect("temporary directory");
    let source = directory.path().join("source");
    let storage = directory.path().join("storage");
    write_source(&source);
    let request = request(&source);
    let final_root = storage.join("sets").join(storage_key(&request.manifest));
    let displaced = directory.path().join("displaced-final");
    let mut first_store =
        ArtifactStateStore::open(&directory.path().join("first.sqlite3")).expect("first store");
    OfflineArtifactSetImportService::open(&storage, &mut first_store, limits())
        .expect("first service")
        .import(&request, &CancellationToken::new(), |_| {})
        .expect("publish exact final tree");

    let mut second_store =
        ArtifactStateStore::open(&directory.path().join("second.sqlite3")).expect("second store");
    let mut service = OfflineArtifactSetImportService::open(&storage, &mut second_store, limits())
        .expect("second service");
    let race_root = final_root.clone();
    let error = service
        .import(&request, &CancellationToken::new(), |event| {
            if event.stage == ArtifactSetImportStage::Finalizing && !displaced.exists() {
                fs::rename(&race_root, &displaced).expect("displace verified final root");
                fs::create_dir(&race_root).expect("replace final root name");
                fs::write(race_root.join("winner"), b"external").expect("write replacement root");
            }
        })
        .expect_err("replaced final root must not register state");

    assert!(matches!(error, ArtifactSetImportError::StorageChanged));
    assert_no_state(&second_store, &request.manifest);
    assert_eq!(
        fs::read(final_root.join("winner")).expect("read replacement bytes"),
        b"external"
    );
    assert_eq!(
        fs::read(displaced.join("model/weights.bin")).expect("read displaced verified tree"),
        b"weights"
    );
}

#[cfg(windows)]
#[test]
fn wrong_case_managed_layout_names_are_never_adopted() {
    let directory = tempdir().expect("temporary directory");
    let storage = directory.path().join("storage");
    fs::create_dir(&storage).expect("storage root");
    fs::create_dir(storage.join("SETS")).expect("wrong-case sets root");
    fs::create_dir(storage.join(".SET-STAGING")).expect("wrong-case staging root");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.sqlite3")).expect("state store");

    assert!(matches!(
        OfflineArtifactSetImportService::open(&storage, &mut store, limits()),
        Err(ArtifactSetImportError::StorageChanged)
    ));
}
