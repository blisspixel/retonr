use rusqlite::Connection;
use tempfile::tempdir;

use super::ArtifactStateStore;
use crate::StoreError;

#[test]
fn newer_schema_is_rejected_without_migration() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("future.db");
    let connection = Connection::open(&path).expect("create database");
    connection
        .pragma_update(None, "user_version", 7)
        .expect("set future version");
    drop(connection);
    assert!(matches!(
        ArtifactStateStore::open(&path),
        Err(StoreError::UnsupportedSchema(7))
    ));
}
