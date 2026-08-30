use rusqlite::Connection;
use tempfile::tempdir;

use super::{input, read_input, support};
use crate::{
    ArtifactStateStore, GenerationQualificationPreregistrationTransactionError, StoreError,
};

#[test]
fn cold_reads_reject_every_wrong_projection_index() {
    for assignment in [
        "generation_qualification_plan_id = lower(hex(randomblob(32)))",
        "suite_manifest_id = lower(hex(randomblob(32)))",
        "target_generation_system_id = lower(hex(randomblob(32)))",
        "entry_count = entry_count + 1",
    ] {
        assert_projection_corrupt(assignment);
    }
}

#[test]
fn cold_reads_reject_noncanonical_and_malformed_bytes() {
    for assignment in [
        "canonical_json = CAST(X'207B7D' AS BLOB)",
        "canonical_json = X'7B'",
    ] {
        assert_projection_corrupt(assignment);
    }
}

#[test]
fn same_projection_id_with_different_bytes_is_an_immutable_conflict() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("projection-bytes.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    support::persist_plan_foundation(&mut store, &fixture);
    store
        .transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("insert pair");
    store
        .connection()
        .execute(
            "UPDATE generation_qualification_request_projections
             SET canonical_json = X'7B7D' WHERE request_projection_id = ?1",
            [fixture.projection.request_projection_id().digest().as_str()],
        )
        .expect("replace projection bytes");
    assert!(matches!(
        store.transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        ),
        Err(
            GenerationQualificationPreregistrationTransactionError::Store(
                StoreError::ImmutableConflict
            )
        )
    ));
}

#[test]
fn bounded_blob_loader_rejects_a_non_blob_storage_class_before_fetch() {
    let connection = Connection::open_in_memory().expect("open memory database");
    assert!(matches!(
        super::super::read::load_blob(
            &connection,
            "unused_table",
            "unused_key",
            "unused",
            "text",
            2,
            16,
        ),
        Err(StoreError::CorruptRecord)
    ));
}

fn assert_projection_corrupt(assignment: &str) {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("projection-index.db");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    support::persist_plan_foundation(&mut store, &fixture);
    store
        .transact_generation_qualification_preregistration(
            input(&fixture),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("insert pair");
    store
        .connection()
        .pragma_update(None, "foreign_keys", false)
        .expect("disable fixture foreign keys");
    store
        .connection()
        .execute(
            &format!(
                "UPDATE generation_qualification_request_projections SET {assignment}
                 WHERE request_projection_id = ?1"
            ),
            [fixture.projection.request_projection_id().digest().as_str()],
        )
        .expect("mutate projection fixture");
    assert!(matches!(
        store.generation_qualification_preregistration(read_input(&fixture)),
        Err(StoreError::CorruptRecord)
    ));
}
