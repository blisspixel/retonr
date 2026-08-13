use rusqlite::{Connection, OptionalExtension as _, Transaction, params};
use serde::{Serialize, de::DeserializeOwned};

use rewrite_model::{ArtifactManifest, InstalledArtifact};

use crate::{StoreError, StoreResult, store::WriteDisposition};

const MAX_RECORD_BYTES: usize = 1_048_576;

pub(super) fn encode_record<T: Serialize>(record: &T) -> StoreResult<String> {
    let encoded = serde_json::to_string(record)?;
    if encoded.len() > MAX_RECORD_BYTES {
        return Err(StoreError::RecordTooLarge);
    }
    Ok(encoded)
}

pub(super) fn decode_record<T: DeserializeOwned>(encoded: &str) -> StoreResult<T> {
    if encoded.len() > MAX_RECORD_BYTES {
        return Err(StoreError::RecordTooLarge);
    }
    Ok(serde_json::from_str(encoded)?)
}

pub(super) fn insert_immutable(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    encoded: &str,
) -> StoreResult<WriteDisposition> {
    let sql = format!(
        "INSERT INTO {table} ({key_column}, record_json) VALUES (?1, ?2)
         ON CONFLICT({key_column}) DO NOTHING"
    );
    let changed = connection.execute(&sql, params![key, encoded])?;
    immutable_disposition(connection, table, key_column, key, encoded, changed)
}

pub(super) fn immutable_disposition(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    encoded: &str,
    changed: usize,
) -> StoreResult<WriteDisposition> {
    if changed == 1 {
        return Ok(WriteDisposition::Inserted);
    }
    let sql = format!("SELECT record_json FROM {table} WHERE {key_column} = ?1");
    let existing: String = connection.query_row(&sql, [key], |row| row.get(0))?;
    if existing == encoded {
        Ok(WriteDisposition::AlreadyPresent)
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

pub(super) fn load_record<T: DeserializeOwned>(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
) -> StoreResult<Option<T>> {
    let sql = format!("SELECT record_json FROM {table} WHERE {key_column} = ?1");
    let encoded = connection
        .query_row(&sql, [key], |row| row.get::<_, String>(0))
        .optional()?;
    encoded.map(|value| decode_record(&value)).transpose()
}

pub(super) fn load_required<T: DeserializeOwned>(
    transaction: &Transaction<'_>,
    table: &str,
    key_column: &str,
    key: &str,
) -> StoreResult<T> {
    load_record(transaction, table, key_column, key)?.ok_or(StoreError::MissingRecord)
}

pub(super) fn validate_existing_installation(
    indexed_id: &str,
    manifest: Option<&ArtifactManifest>,
    installed: Option<&InstalledArtifact>,
) -> StoreResult<()> {
    if installed.is_some() && manifest.is_none() {
        return Err(StoreError::CorruptRecord);
    }
    if let Some(manifest) = manifest {
        manifest.validate().map_err(|_| StoreError::CorruptRecord)?;
        if manifest.artifact_id.digest().as_str() != indexed_id {
            return Err(StoreError::CorruptRecord);
        }
    }
    if let Some(installed) = installed {
        installed
            .validate()
            .map_err(|_| StoreError::CorruptRecord)?;
        if installed.artifact_id.digest().as_str() != indexed_id {
            return Err(StoreError::CorruptRecord);
        }
        let manifest = manifest.ok_or(StoreError::CorruptRecord)?;
        if installed.artifact_id != manifest.artifact_id
            || installed.artifact_digest != manifest.artifact_digest
            || installed.byte_size != manifest.byte_size
        {
            return Err(StoreError::CorruptRecord);
        }
    }
    Ok(())
}
