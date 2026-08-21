use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use crate::{StoreError, StoreResult};

mod shape;

pub(super) use shape::{
    validate_schema_five, validate_schema_four, validate_schema_one, validate_schema_shape,
    validate_schema_three, validate_schema_two,
};

pub(super) const STORE_SCHEMA_VERSION: i64 = 6;

pub(super) fn migrate_existing_transaction(
    connection: &Connection,
    expected_version: i64,
) -> StoreResult<()> {
    let observed: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if observed != expected_version {
        return Err(StoreError::CorruptRecord);
    }
    crate::integrity::validate_database_integrity(connection)?;
    migrate_supported_schema(connection, observed)?;
    validate_schema_shape(connection)?;
    crate::integrity::validate_database_integrity(connection)
}

fn migrate_supported_schema(connection: &Connection, version: i64) -> StoreResult<()> {
    match version {
        0 => {
            return Err(StoreError::MigrationRequired {
                found: version,
                current: STORE_SCHEMA_VERSION,
            });
        }
        1 => {
            validate_schema_one(connection)?;
            migrate_schema_one(connection)?;
            validate_schema_two(connection)?;
            migrate_schema_two(connection)?;
            validate_schema_three(connection)?;
            migrate_schema_three(connection)?;
            validate_schema_four(connection)?;
            migrate_schema_four(connection)?;
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
        }
        2 => {
            validate_schema_two(connection)?;
            migrate_schema_two(connection)?;
            validate_schema_three(connection)?;
            migrate_schema_three(connection)?;
            validate_schema_four(connection)?;
            migrate_schema_four(connection)?;
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
        }
        3 => {
            validate_schema_three(connection)?;
            migrate_schema_three(connection)?;
            validate_schema_four(connection)?;
            migrate_schema_four(connection)?;
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
        }
        4 => {
            validate_schema_four(connection)?;
            migrate_schema_four(connection)?;
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
        }
        5 => {
            validate_schema_five(connection)?;
            migrate_schema_five(connection)?;
        }
        STORE_SCHEMA_VERSION => validate_schema_shape(connection)?,
        value if !(0..=STORE_SCHEMA_VERSION).contains(&value) => {
            return Err(StoreError::UnsupportedSchema(value));
        }
        value => {
            return Err(StoreError::MigrationRequired {
                found: value,
                current: STORE_SCHEMA_VERSION,
            });
        }
    }
    Ok(())
}

fn create_current_schema(connection: &Connection) -> StoreResult<()> {
    create_schema_two(connection)?;
    migrate_schema_two(connection)?;
    migrate_schema_three(connection)?;
    migrate_schema_four(connection)?;
    migrate_schema_five(connection)
}

fn create_schema_two(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE artifact_manifests (
             artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576)
         ) STRICT;

         CREATE TABLE installed_artifacts (
             artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
             installation_epoch INTEGER NOT NULL
                 CHECK(installation_epoch BETWEEN 1 AND 9223372036854775807),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
         ) STRICT;

         CREATE TABLE qualification_records (
             qualification_id TEXT PRIMARY KEY NOT NULL CHECK(length(qualification_id) = 64),
             artifact_id TEXT NOT NULL CHECK(length(artifact_id) = 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
         ) STRICT;

         CREATE TABLE qualification_invalidations (
             sequence INTEGER PRIMARY KEY,
             qualification_id TEXT NOT NULL CHECK(length(qualification_id) = 64),
             reason_code TEXT NOT NULL CHECK(length(reason_code) BETWEEN 1 AND 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(qualification_id)
                 REFERENCES qualification_records(qualification_id)
         ) STRICT;

         CREATE INDEX invalidations_by_qualification
             ON qualification_invalidations(qualification_id, sequence);

         CREATE TABLE activation_decisions (
             activation_id TEXT PRIMARY KEY NOT NULL CHECK(length(activation_id) = 64),
             role TEXT NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576)
         ) STRICT;

         CREATE TABLE active_bindings (
             role TEXT PRIMARY KEY NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
             artifact_id TEXT NOT NULL CHECK(length(artifact_id) = 64),
             qualification_id TEXT NOT NULL CHECK(length(qualification_id) = 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(artifact_id) REFERENCES installed_artifacts(artifact_id),
             FOREIGN KEY(qualification_id)
                 REFERENCES qualification_records(qualification_id)
         ) STRICT;

         CREATE TABLE artifact_removals (
             artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
             installation_epoch INTEGER NOT NULL
                 CHECK(installation_epoch BETWEEN 1 AND 9223372036854775807),
             phase TEXT NOT NULL CHECK(phase IN ('prepared', 'completed')),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
         ) STRICT;

         PRAGMA user_version = 2;",
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) fn create_schema_two_fixture(connection: &Connection) -> StoreResult<()> {
    create_schema_two(connection)
}

fn migrate_schema_two(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE artifact_set_manifests (
             artifact_set_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_set_id) = 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576)
         ) STRICT;

         CREATE TABLE runtime_build_identities (
             runtime_build_id TEXT PRIMARY KEY NOT NULL CHECK(length(runtime_build_id) = 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576)
         ) STRICT;

         CREATE TABLE effective_runtime_states (
             effective_runtime_state_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(effective_runtime_state_id) = 64),
             runtime_build_id TEXT NOT NULL CHECK(length(runtime_build_id) = 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(runtime_build_id)
                 REFERENCES runtime_build_identities(runtime_build_id)
         ) STRICT;

         CREATE TABLE effective_package_evidence (
             effective_package_evidence_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(effective_package_evidence_id) = 64),
             artifact_set_id TEXT NOT NULL CHECK(length(artifact_set_id) = 64),
             runtime_build_id TEXT NOT NULL CHECK(length(runtime_build_id) = 64),
             effective_runtime_state_id TEXT NOT NULL
                 CHECK(length(effective_runtime_state_id) = 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id),
             FOREIGN KEY(runtime_build_id)
                 REFERENCES runtime_build_identities(runtime_build_id),
             FOREIGN KEY(effective_runtime_state_id)
                 REFERENCES effective_runtime_states(effective_runtime_state_id)
         ) STRICT;

         CREATE TABLE qualification_v2_records (
             qualification_v2_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(qualification_v2_id) = 64),
             artifact_set_id TEXT NOT NULL CHECK(length(artifact_set_id) = 64),
             effective_package_evidence_id TEXT NOT NULL
                 CHECK(length(effective_package_evidence_id) = 64),
             runtime_build_id TEXT NOT NULL CHECK(length(runtime_build_id) = 64),
             effective_runtime_state_id TEXT NOT NULL
                 CHECK(length(effective_runtime_state_id) = 64),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id),
             FOREIGN KEY(effective_package_evidence_id)
                 REFERENCES effective_package_evidence(effective_package_evidence_id),
             FOREIGN KEY(runtime_build_id)
                 REFERENCES runtime_build_identities(runtime_build_id),
             FOREIGN KEY(effective_runtime_state_id)
                 REFERENCES effective_runtime_states(effective_runtime_state_id)
         ) STRICT;

         PRAGMA user_version = 3;",
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) fn create_schema_three_fixture(connection: &Connection) -> StoreResult<()> {
    create_schema_two(connection)?;
    migrate_schema_two(connection)
}

fn migrate_schema_three(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE installed_artifact_sets (
             artifact_set_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_set_id) = 64),
             storage_key TEXT NOT NULL COLLATE NOCASE UNIQUE
                 CHECK(length(storage_key) BETWEEN 1 AND 128),
             installation_epoch INTEGER NOT NULL
                 CHECK(installation_epoch BETWEEN 1 AND 9223372036854775807),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1024),
             FOREIGN KEY(artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id)
         ) STRICT;

         PRAGMA user_version = 4;",
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) fn create_schema_four_fixture(connection: &Connection) -> StoreResult<()> {
    create_schema_two(connection)?;
    migrate_schema_two(connection)?;
    migrate_schema_three(connection)
}

fn migrate_schema_four(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE artifact_set_removals (
             artifact_set_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_set_id) = 64),
             installation_epoch INTEGER NOT NULL
                 CHECK(installation_epoch BETWEEN 1 AND 9223372036854775807),
             phase TEXT NOT NULL CHECK(phase IN ('prepared', 'completed')),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1024),
             FOREIGN KEY(artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id)
         ) STRICT;

         PRAGMA user_version = 5;",
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) fn create_schema_five_fixture(connection: &Connection) -> StoreResult<()> {
    create_schema_two(connection)?;
    migrate_schema_two(connection)?;
    migrate_schema_three(connection)?;
    migrate_schema_four(connection)
}

fn migrate_schema_five(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "CREATE TABLE runtime_package_manifests (
             runtime_package_manifest_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(runtime_package_manifest_id) = 64
                     AND runtime_package_manifest_id NOT GLOB '*[^0-9a-f]*'),
             artifact_set_id TEXT NOT NULL
                 CHECK(length(artifact_set_id) = 64
                     AND artifact_set_id NOT GLOB '*[^0-9a-f]*'),
             source_artifact_set_id TEXT
                 CHECK(source_artifact_set_id IS NULL
                     OR (length(source_artifact_set_id) = 64
                         AND source_artifact_set_id NOT GLOB '*[^0-9a-f]*')),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id),
             FOREIGN KEY(source_artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id)
         ) STRICT;

         CREATE TABLE model_package_manifests (
             model_package_manifest_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(model_package_manifest_id) = 64
                     AND model_package_manifest_id NOT GLOB '*[^0-9a-f]*'),
             artifact_set_id TEXT NOT NULL
                 CHECK(length(artifact_set_id) = 64
                     AND artifact_set_id NOT GLOB '*[^0-9a-f]*'),
             source_artifact_set_id TEXT
                 CHECK(source_artifact_set_id IS NULL
                     OR (length(source_artifact_set_id) = 64
                         AND source_artifact_set_id NOT GLOB '*[^0-9a-f]*')),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id),
             FOREIGN KEY(source_artifact_set_id)
                 REFERENCES artifact_set_manifests(artifact_set_id)
         ) STRICT;

         CREATE TABLE native_load_observations (
             native_load_observation_id TEXT PRIMARY KEY NOT NULL
                 CHECK(length(native_load_observation_id) = 64
                     AND native_load_observation_id NOT GLOB '*[^0-9a-f]*'),
             runtime_package_manifest_id TEXT NOT NULL
                 CHECK(length(runtime_package_manifest_id) = 64
                     AND runtime_package_manifest_id NOT GLOB '*[^0-9a-f]*'),
             record_json TEXT NOT NULL
                 CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
             FOREIGN KEY(runtime_package_manifest_id)
                 REFERENCES runtime_package_manifests(runtime_package_manifest_id)
         ) STRICT;

         PRAGMA user_version = 6;",
    )?;
    Ok(())
}

const SCHEMA_ONE_SQL: &str = "CREATE TABLE artifact_manifests (
         artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
         record_json TEXT NOT NULL
             CHECK(length(CAST(record_json AS BLOB)) <= 1048576)
     ) STRICT;

     CREATE TABLE installed_artifacts (
         artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
         record_json TEXT NOT NULL
             CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
         FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
     ) STRICT;

     CREATE TABLE qualification_records (
         qualification_id TEXT PRIMARY KEY NOT NULL CHECK(length(qualification_id) = 64),
         artifact_id TEXT NOT NULL CHECK(length(artifact_id) = 64),
         record_json TEXT NOT NULL
             CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
         FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
     ) STRICT;

     CREATE TABLE qualification_invalidations (
         sequence INTEGER PRIMARY KEY,
         qualification_id TEXT NOT NULL CHECK(length(qualification_id) = 64),
         reason_code TEXT NOT NULL CHECK(length(reason_code) BETWEEN 1 AND 64),
         record_json TEXT NOT NULL
             CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
         FOREIGN KEY(qualification_id)
             REFERENCES qualification_records(qualification_id)
     ) STRICT;

     CREATE INDEX invalidations_by_qualification
         ON qualification_invalidations(qualification_id, sequence);

     CREATE TABLE activation_decisions (
         activation_id TEXT PRIMARY KEY NOT NULL CHECK(length(activation_id) = 64),
         role TEXT NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
         record_json TEXT NOT NULL
             CHECK(length(CAST(record_json AS BLOB)) <= 1048576)
     ) STRICT;

     CREATE TABLE active_bindings (
         role TEXT PRIMARY KEY NOT NULL CHECK(length(role) BETWEEN 1 AND 64),
         artifact_id TEXT NOT NULL CHECK(length(artifact_id) = 64),
         qualification_id TEXT NOT NULL CHECK(length(qualification_id) = 64),
         record_json TEXT NOT NULL
             CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
         FOREIGN KEY(artifact_id) REFERENCES installed_artifacts(artifact_id),
         FOREIGN KEY(qualification_id)
             REFERENCES qualification_records(qualification_id)
     ) STRICT;

     PRAGMA user_version = 1;";

fn create_schema_one(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(SCHEMA_ONE_SQL)?;
    Ok(())
}

#[cfg(test)]
pub(super) fn create_schema_one_fixture(connection: &Connection) -> StoreResult<()> {
    create_schema_one(connection)
}

fn migrate_schema_one(connection: &Connection) -> StoreResult<()> {
    connection.execute_batch(
        "ALTER TABLE installed_artifacts
                 ADD COLUMN installation_epoch INTEGER NOT NULL DEFAULT 1
                 CHECK(installation_epoch BETWEEN 1 AND 9223372036854775807);

             CREATE TABLE artifact_removals (
                 artifact_id TEXT PRIMARY KEY NOT NULL CHECK(length(artifact_id) = 64),
                 installation_epoch INTEGER NOT NULL
                     CHECK(installation_epoch BETWEEN 1 AND 9223372036854775807),
                 phase TEXT NOT NULL CHECK(phase IN ('prepared', 'completed')),
                 record_json TEXT NOT NULL
                     CHECK(length(CAST(record_json AS BLOB)) <= 1048576),
                 FOREIGN KEY(artifact_id) REFERENCES artifact_manifests(artifact_id)
             ) STRICT;

             PRAGMA user_version = 2;",
    )?;
    Ok(())
}

pub(super) fn initialize_empty(connection: &mut Connection) -> StoreResult<()> {
    configure(connection, true)?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version == STORE_SCHEMA_VERSION {
        validate_schema_shape(&transaction)?;
        transaction.commit()?;
        return Ok(());
    }
    if version != 0 {
        return if version < STORE_SCHEMA_VERSION {
            Err(StoreError::MigrationRequired {
                found: version,
                current: STORE_SCHEMA_VERSION,
            })
        } else {
            Err(StoreError::UnsupportedSchema(version))
        };
    }
    require_empty_schema(&transaction)?;
    create_current_schema(&transaction)?;
    validate_schema_shape(&transaction)?;
    transaction.commit()?;
    Ok(())
}

fn require_empty_schema(connection: &Connection) -> StoreResult<()> {
    let schema_objects: i64 = connection.query_row(
        "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    if schema_objects != 0 {
        return Err(StoreError::MigrationRequired {
            found: 0,
            current: STORE_SCHEMA_VERSION,
        });
    }
    Ok(())
}

pub(super) fn validate_exact(connection: &Connection) -> StoreResult<()> {
    configure(connection, false)?;
    validate_exact_version(connection)
}

pub(super) fn validate_exact_writable(connection: &Connection) -> StoreResult<()> {
    configure(connection, true)?;
    validate_exact_version(connection)
}

fn validate_exact_version(connection: &Connection) -> StoreResult<()> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 0 {
        return Err(StoreError::UnsupportedSchema(version));
    }
    if version < STORE_SCHEMA_VERSION {
        return Err(StoreError::MigrationRequired {
            found: version,
            current: STORE_SCHEMA_VERSION,
        });
    }
    if version > STORE_SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchema(version));
    }
    validate_schema_shape(connection)
}

pub(super) fn configure(connection: &Connection, writable: bool) -> StoreResult<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA trusted_schema = OFF;",
    )?;
    if writable {
        connection.execute_batch("PRAGMA synchronous = FULL;")?;
    }
    Ok(())
}
