use tempfile::tempdir;

use rewrite_model::{ActivationId, ArtifactRole};
use rewrite_types::Digest;

use super::{fixture, populate, qualification_id};
use crate::{ArtifactStateStore, StoreError};

#[test]
fn indexed_binding_corruption_blocks_reads_recovery_and_removal() {
    let directory = tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let fixture = fixture();
    populate(&store, &fixture);
    store
        .activate(
            ActivationId::from_digest(Digest::sha256(b"activation")),
            ArtifactRole::Generation,
            &fixture.installed,
            &qualification_id(&fixture),
        )
        .expect("activate");
    store
        .connection()
        .pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys for corruption fixture");
    store
        .connection()
        .execute(
            "UPDATE active_bindings SET artifact_id = ?1 WHERE role = 'generation'",
            [Digest::sha256(b"other").as_str()],
        )
        .expect("corrupt indexed artifact identity");
    store
        .connection()
        .pragma_update(None, "foreign_keys", true)
        .expect("restore foreign keys");

    assert!(matches!(
        store.active_binding(ArtifactRole::Generation, |_| true),
        Err(StoreError::CorruptRecord)
    ));
    assert!(matches!(
        store.recover_active_bindings(|_| true),
        Err(StoreError::CorruptRecord)
    ));
    assert!(matches!(
        store.remove_installed(&fixture.installed.artifact_id),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn schema_bounds_serialized_records_by_utf8_bytes() {
    let directory = tempdir().expect("temporary directory");
    let store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let oversized_multibyte_text = "é".repeat(524_289);
    assert!(
        store
            .connection()
            .execute(
                "INSERT INTO artifact_manifests (artifact_id, record_json) VALUES (?1, ?2)",
                rusqlite::params![
                    Digest::sha256(b"oversized").as_str(),
                    oversized_multibyte_text
                ],
            )
            .is_err()
    );
}
