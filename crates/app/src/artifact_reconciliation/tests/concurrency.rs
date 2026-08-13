use super::*;
use crate::{ArtifactInventoryLimits, ArtifactInventoryService};

#[test]
fn inventory_shared_lock_excludes_reconciliation() {
    let (directory, mut store) = initialize();
    let inventory_store = ArtifactStateStore::open(&directory.path().join("state.sqlite3"))
        .expect("open second state connection");
    let inventory = ArtifactInventoryService::open(
        storage(&directory),
        &inventory_store,
        ArtifactInventoryLimits {
            maximum_state_entries: 8,
            maximum_storage_entries: 8,
            maximum_artifact_bytes: 4_096,
            maximum_total_verification_bytes: 8_192,
        },
    )
    .expect("hold shared inventory lock");
    assert!(matches!(
        ArtifactOrphanReconciliationService::open_existing(
            storage(&directory),
            &mut store,
            limits()
        ),
        Err(ArtifactReconciliationError::StorageInUse)
    ));
    drop(inventory);
}

#[test]
fn exclusive_reconciliation_lock_excludes_inventory_and_import() {
    let (directory, mut store) = initialize();
    let service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("hold exclusive reconciliation lock");
    let inventory_store = ArtifactStateStore::open(&directory.path().join("inventory.sqlite3"))
        .expect("open inventory state");
    assert!(matches!(
        ArtifactInventoryService::open(
            storage(&directory),
            &inventory_store,
            ArtifactInventoryLimits {
                maximum_state_entries: 8,
                maximum_storage_entries: 8,
                maximum_artifact_bytes: 4_096,
                maximum_total_verification_bytes: 8_192,
            }
        ),
        Err(crate::ArtifactInventoryError::StorageInUse)
    ));
    let mut import_store = ArtifactStateStore::open(&directory.path().join("import.sqlite3"))
        .expect("open import state");
    assert!(matches!(
        OfflineArtifactImportService::open(
            storage(&directory),
            &mut import_store,
            ArtifactImportLimits {
                maximum_artifact_bytes: 4_096,
                maximum_storage_entries: 32,
            }
        ),
        Err(crate::ArtifactImportError::StorageInUse)
    ));
    drop(service);
}

#[test]
fn cancellation_during_hashing_registers_no_state() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    let cancellation = CancellationToken::new();
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    let result = service.reconcile(&request(&value), &cancellation, |event| {
        if event.stage == ArtifactOrphanReconciliationStage::VerifyingOrphan
            && event.completed_bytes > 0
        {
            cancellation.cancel();
        }
    });
    assert!(matches!(
        result,
        Err(ArtifactReconciliationError::Cancelled)
    ));
    drop(service);
    assert!(
        store
            .artifact_inventory(8)
            .expect("inspect state")
            .is_empty()
    );
}

#[test]
fn cancellation_before_missing_target_lookup_wins_over_not_found() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    let cancellation = CancellationToken::new();
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    let result = service.reconcile(&request(&value), &cancellation, |event| {
        if event.stage == ArtifactOrphanReconciliationStage::VerifyingOrphan {
            cancellation.cancel();
        }
    });
    assert!(matches!(
        result,
        Err(ArtifactReconciliationError::Cancelled)
    ));
}

#[test]
fn last_byte_callback_hard_link_alias_fails_before_state() {
    let (directory, mut store) = initialize();
    let value = manifest(BYTES);
    write_orphan(&directory, &value, BYTES);
    let target = artifacts(&directory).join(value.artifact_digest.as_str());
    let alias = directory.path().join("late-alias");
    let mut service = ArtifactOrphanReconciliationService::open_existing(
        storage(&directory),
        &mut store,
        limits(),
    )
    .expect("open reconciliation");
    let result = service.reconcile(&request(&value), &CancellationToken::new(), |event| {
        if event.stage == ArtifactOrphanReconciliationStage::VerifyingOrphan
            && event.completed_bytes == event.total_bytes
        {
            fs::hard_link(&target, &alias).expect("create late alias");
        }
    });
    assert!(matches!(
        result,
        Err(ArtifactReconciliationError::StorageChanged)
    ));
    drop(service);
    assert!(
        store
            .artifact_inventory(8)
            .expect("inspect state")
            .is_empty()
    );
}
