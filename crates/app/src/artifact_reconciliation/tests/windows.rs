use super::*;

#[test]
fn case_folded_target_is_never_registered_as_canonical() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    let lowercase = artifacts(&directory).join(value.artifact_digest.as_str());
    let uppercase = artifacts(&directory).join(value.artifact_digest.as_str().to_uppercase());
    fs::write(&uppercase, BYTES).expect("write uppercase fixture");
    let case_insensitive = lowercase.exists();
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    let result = service.reconcile(&request(&value), &CancellationToken::new(), |_| {});
    if case_insensitive {
        assert!(matches!(
            result,
            Err(ArtifactReconciliationError::StorageChanged)
        ));
    } else {
        assert!(matches!(
            result,
            Err(ArtifactReconciliationError::OrphanNotFound)
        ));
    }
}

#[test]
fn held_handles_block_root_and_ancestor_replacement() {
    let directory = tempdir().expect("temporary directory");
    let ancestor = directory.path().join("container");
    let managed = ancestor.join("managed");
    let moved = ancestor.join("managed-moved");
    let moved_ancestor = directory.path().join("container-moved");
    let mut store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open artifact state");
    {
        let _import = OfflineArtifactImportService::open(
            &managed,
            &mut store,
            ArtifactImportLimits {
                maximum_artifact_bytes: 4_096,
                maximum_storage_entries: 32,
            },
        )
        .expect("initialize managed storage");
    }
    let value = manifest(BYTES);
    fs::write(
        managed
            .join("artifacts")
            .join(value.artifact_digest.as_str()),
        BYTES,
    )
    .expect("write orphan fixture");
    let mut root_blocked = false;
    let mut ancestor_blocked = false;
    let mut service =
        ArtifactOrphanReconciliationService::open_existing(&managed, &mut store, limits())
            .expect("open reconciliation");
    service
        .reconcile(&request(&value), &CancellationToken::new(), |event| {
            if event.stage == ArtifactOrphanReconciliationStage::VerifyingOrphan
                && event.completed_bytes == event.total_bytes
            {
                root_blocked = replacement_is_blocked(&managed, &moved);
                ancestor_blocked = replacement_is_blocked(&ancestor, &moved_ancestor);
            }
        })
        .expect("reconcile after blocked replacement attempts");
    assert!(root_blocked);
    assert!(ancestor_blocked);
}

fn replacement_is_blocked(source: &std::path::Path, destination: &std::path::Path) -> bool {
    let error = fs::rename(source, destination).expect_err("held path must reject replacement");
    error.kind() == std::io::ErrorKind::PermissionDenied || error.raw_os_error() == Some(32)
}
