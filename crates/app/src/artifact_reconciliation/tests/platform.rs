use std::os::unix::fs::symlink;

use super::*;

#[test]
fn rejects_symlink_target_without_touching_external_bytes() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    let external = directory.path().join("external");
    fs::write(&external, BYTES).expect("write external fixture");
    symlink(
        &external,
        artifacts(&directory).join(value.artifact_digest.as_str()),
    )
    .expect("create symlink target");
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    assert!(matches!(
        service.reconcile(&request(&value), &CancellationToken::new(), |_| {}),
        Err(ArtifactReconciliationError::StorageChanged)
    ));
    assert_eq!(fs::read(external).expect("read external fixture"), BYTES);
}

#[test]
fn last_byte_root_redirect_cannot_escape_pinned_storage() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    let managed = storage(&directory);
    let moved = directory.path().join("moved-storage");
    let outside = directory.path().join("outside");
    fs::create_dir(&outside).expect("create outside directory");
    let sentinel = outside.join("sentinel");
    fs::write(&sentinel, b"outside").expect("write outside sentinel");
    let mut service =
        ArtifactOrphanReconciliationService::open_existing(&managed, &mut store, limits())
            .expect("open reconciliation");
    let result = service.reconcile(&request(&value), &CancellationToken::new(), |event| {
        if event.stage == ArtifactOrphanReconciliationStage::VerifyingOrphan
            && event.completed_bytes == event.total_bytes
        {
            fs::rename(&managed, &moved).expect("move managed root");
            symlink(&outside, &managed).expect("redirect managed root");
        }
    });
    assert!(matches!(
        result,
        Err(ArtifactReconciliationError::StorageChanged)
    ));
    assert_eq!(
        fs::read(sentinel).expect("read outside sentinel"),
        b"outside"
    );
}
