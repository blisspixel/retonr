use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use crate::{StoreError, StoreResult};

pub(super) const STORE_SCHEMA_VERSION: i64 = 1;

pub(super) fn initialize(connection: &mut Connection) -> StoreResult<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA trusted_schema = OFF;
         PRAGMA synchronous = FULL;",
    )?;

    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > STORE_SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(version));
    }
    if version == STORE_SCHEMA_VERSION {
        return Ok(());
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(
        "CREATE TABLE artifact_manifests (
             artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
             record_json TEXT NOT NULL CHECK(length(record_json) <= 1048576)
         ) STRICT;

         CREATE TABLE installed_artifacts (
             artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
             record_json TEXT NOT NULL CHECK(length(record_json) <= 1048576),
             FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
         ) STRICT;

         CREATE TABLE qualification_records (
             qualification_id TEXT PRIMARY KEY NOT NULL CHECK(length(qualification_id) = 64),
             artifact_id TEXT NOT NULL CHECK(length(artifact_id) = 64),
             record_json TEXT NOT NULL CHECK(length(record_json) <= 1048576),
             FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
         ) STRICT;

         CREATE TABLE qualification_invalidations (
             sequence INTEGER PRIMARY KEY,
             qualification_id TEXT NOT NULL CHECK(length(qualification_id) = 64),
             reason_code TEXT NOT NULL CHECK(length(reason_code) BETWEEN 1 AND 64),
             record_json TEXT NOT NULL CHECK(length(record_json) <= 1048576),
             FOREIGN KEY(qualification_id)
                 REFERENCES qualification_records(qualification_id)
         ) STRICT;

         CREATE INDEX invalidations_by_qualification
             ON qualification_invalidations(qualification_id, sequence);

         CREATE TABLE activation_decisions (
             activation_id TEXT PRIMARY KEY NOT NULL CHECK(length(activation_id) = 64),
             role TEXT NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
             record_json TEXT NOT NULL CHECK(length(record_json) <= 1048576)
         ) STRICT;

         CREATE TABLE active_bindings (
             role TEXT PRIMARY KEY NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
             artifact_id TEXT NOT NULL CHECK(length(artifact_id) = 64),
             qualification_id TEXT NOT NULL CHECK(length(qualification_id) = 64),
             record_json TEXT NOT NULL CHECK(length(record_json) <= 1048576),
             FOREIGN KEY(artifact_id) REFERENCES installed_artifacts(artifact_id),
             FOREIGN KEY(qualification_id)
                 REFERENCES qualification_records(qualification_id)
         ) STRICT;

         PRAGMA user_version = 1;",
    )?;
    transaction.commit()?;
    Ok(())
}
