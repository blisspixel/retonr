use super::*;

#[test]
fn detects_directory_changes_before_returning_a_report() {
    let (directory, store) = initialized();
    let artifacts = artifacts(&directory);
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open inventory");

    let result = service.inventory(&CancellationToken::new(), |item| {
        if item.stage == ArtifactInventoryStage::RecheckingStorageAndState {
            fs::write(artifacts.join("late-entry"), b"late").expect("mutate during inventory");
        }
    });

    assert!(matches!(
        result,
        Err(ArtifactInventoryError::ConcurrentModification)
    ));
}

#[test]
fn detects_durable_state_changes_before_returning_a_report() {
    let (directory, store) = initialized();
    let concurrent_store = ArtifactStateStore::open(&directory.path().join("state.db"))
        .expect("open concurrent state connection");
    let added = manifest(b"concurrent state", "concurrent-state");
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open inventory");

    let result = service.inventory(&CancellationToken::new(), |item| {
        if item.stage == ArtifactInventoryStage::RecheckingStorageAndState {
            concurrent_store
                .put_manifest(&added)
                .expect("change durable state during inventory");
        }
    });

    assert!(matches!(
        result,
        Err(ArtifactInventoryError::ConcurrentModification)
    ));
}

#[test]
fn concurrent_state_growth_past_the_limit_is_concurrent_modification() {
    let (directory, store) = initialized();
    let initial = manifest(b"initial state", "initial-state");
    store
        .put_manifest(&initial)
        .expect("fill initial state limit");
    let concurrent_store = ArtifactStateStore::open(&directory.path().join("state.db"))
        .expect("open concurrent state connection");
    let added = manifest(b"added state", "added-state");
    let mut exact_limit = limits();
    exact_limit.maximum_state_entries = 1;
    let service = ArtifactInventoryService::open(storage(&directory), &store, exact_limit)
        .expect("open exact-limit inventory");

    let result = service.inventory(&CancellationToken::new(), |item| {
        if item.stage == ArtifactInventoryStage::RecheckingStorageAndState {
            concurrent_store
                .put_manifest(&added)
                .expect("grow state past limit during inventory");
        }
    });

    assert!(matches!(
        result,
        Err(ArtifactInventoryError::ConcurrentModification)
    ));
}

#[cfg(unix)]
#[test]
fn detects_same_name_replacement_before_returning_a_report() {
    let (directory, store) = initialized();
    let value = manifest(b"original", "replacement");
    write_artifact(&directory, &value.artifact_digest, b"original");
    let path = artifacts(&directory).join(value.artifact_digest.as_str());
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open inventory");

    let result = service.inventory(&CancellationToken::new(), |item| {
        if item.stage == ArtifactInventoryStage::RecheckingStorageAndState {
            fs::remove_file(&path).expect("remove inventoried entry");
            fs::write(&path, b"replaced").expect("replace with same-size bytes");
        }
    });

    assert!(matches!(
        result,
        Err(ArtifactInventoryError::ConcurrentModification)
    ));
}

#[cfg(windows)]
#[test]
fn blocks_same_name_replacement_during_inventory() {
    let (directory, store) = initialized();
    let value = manifest(b"original", "replacement");
    write_artifact(&directory, &value.artifact_digest, b"original");
    let path = artifacts(&directory).join(value.artifact_digest.as_str());
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open inventory");
    let mut replacement_blocked = false;

    service
        .inventory(&CancellationToken::new(), |item| {
            if item.stage == ArtifactInventoryStage::RecheckingStorageAndState {
                replacement_blocked = fs::remove_file(&path).is_err();
            }
        })
        .expect("pinned artifact remains verifiable");

    assert!(replacement_blocked);
    assert_eq!(fs::read(path).expect("read pinned artifact"), b"original");
}

#[cfg(windows)]
#[test]
fn blocks_ancestor_replacement_during_inventory() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open artifact state");
    let holder = directory.path().join("holder");
    let managed = holder.join("storage");
    {
        let _service = OfflineArtifactImportService::open(
            &managed,
            &mut store,
            ArtifactImportLimits {
                maximum_artifact_bytes: 4_096,
                maximum_storage_entries: 32,
            },
        )
        .expect("initialize nested artifact storage");
    }
    let value = manifest(b"original", "ancestor-replacement");
    register(&mut store, &value);
    fs::write(
        managed
            .join("artifacts")
            .join(value.artifact_digest.as_str()),
        b"original",
    )
    .expect("write original artifact");
    let service =
        ArtifactInventoryService::open(&managed, &store, limits()).expect("open pinned inventory");

    let error = fs::rename(&holder, directory.path().join("retained"))
        .expect_err("held storage boundaries must block ancestor replacement");
    assert!(matches!(error.raw_os_error(), Some(5 | 32)));

    let report = service
        .inventory(&CancellationToken::new(), |_| {})
        .expect("inventory remains bound to original storage");
    assert_eq!(
        status(&report, &value.artifact_id),
        &RegisteredArtifactBytes::Verified
    );

    drop(service);
}

#[test]
fn shared_inventory_lock_excludes_import() {
    let (directory, mut importing_store) = initialized();
    let inventory_store = ArtifactStateStore::open(&directory.path().join("state.db"))
        .expect("open second state connection");
    let importer = OfflineArtifactImportService::open(
        storage(&directory),
        &mut importing_store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 4_096,
            maximum_storage_entries: 32,
        },
    )
    .expect("hold exclusive import lock");

    assert!(matches!(
        ArtifactInventoryService::open(storage(&directory), &inventory_store, limits()),
        Err(ArtifactInventoryError::StorageInUse)
    ));
    drop(importer);
    ArtifactInventoryService::open(storage(&directory), &inventory_store, limits())
        .expect("inventory opens after import lock release");
}

#[test]
fn held_inventory_blocks_import_and_allows_another_inventory() {
    let (directory, mut importing_store) = initialized();
    let inventory_store = ArtifactStateStore::open(&directory.path().join("state.db"))
        .expect("open inventory state connection");
    let first = ArtifactInventoryService::open(storage(&directory), &inventory_store, limits())
        .expect("hold first shared inventory lock");
    let second = ArtifactInventoryService::open(storage(&directory), &inventory_store, limits())
        .expect("hold second shared inventory lock");
    assert!(matches!(
        OfflineArtifactImportService::open(
            storage(&directory),
            &mut importing_store,
            ArtifactImportLimits {
                maximum_artifact_bytes: 4_096,
                maximum_storage_entries: 32,
            },
        ),
        Err(crate::ArtifactImportError::StorageInUse)
    ));
    drop(second);
    drop(first);
    OfflineArtifactImportService::open(
        storage(&directory),
        &mut importing_store,
        ArtifactImportLimits {
            maximum_artifact_bytes: 4_096,
            maximum_storage_entries: 32,
        },
    )
    .expect("import opens after both shared locks are released");
}

#[cfg(unix)]
#[test]
fn pinned_directory_rejects_path_replacement_without_leaving_storage() {
    use std::os::unix::fs::symlink;

    let (directory, store) = initialized();
    let managed = artifacts(&directory);
    let retained = storage(&directory).join("artifacts-retained");
    let outside = directory.path().join("outside");
    fs::create_dir(&outside).expect("create outside directory");
    fs::write(outside.join("private"), b"outside").expect("write outside fixture");
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open pinned inventory");

    let result = service.inventory(&CancellationToken::new(), |item| {
        if item.stage == ArtifactInventoryStage::RecheckingStorageAndState {
            fs::rename(&managed, &retained).expect("retain managed directory");
            symlink(&outside, &managed).expect("replace path with symlink");
        }
    });

    assert!(matches!(
        result,
        Err(ArtifactInventoryError::ConcurrentModification)
    ));
}

#[cfg(unix)]
#[test]
fn boundary_removal_during_inventory_is_concurrent_modification() {
    let (directory, store) = initialized();
    let managed = artifacts(&directory);
    let service = ArtifactInventoryService::open(storage(&directory), &store, limits())
        .expect("open inventory");

    let result = service.inventory(&CancellationToken::new(), |item| {
        if item.stage == ArtifactInventoryStage::RecheckingStorageAndState {
            fs::remove_dir(&managed).expect("remove managed boundary");
        }
    });

    assert!(matches!(
        result,
        Err(ArtifactInventoryError::ConcurrentModification)
    ));
}
