use rusqlite::{Connection, OptionalExtension as _};

use rewrite_model::{
    MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES, MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES,
    ModelPackageManifest, NativeLoadObservation, PackageTransformation, RuntimePackageManifest,
};

use crate::{StoreError, StoreResult};

use super::super::bounded_text::{
    BoundedTextCell, read_nullable_digest_text, read_required_bounded_text,
    read_required_digest_text,
};

pub(in crate::store) fn load_runtime_package(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<RuntimePackageManifest>> {
    let Some((artifact_set_id, source_artifact_set_id, encoded)) = load_package_row(
        connection,
        "runtime_package_manifests",
        "runtime_package_manifest_id",
        key,
        MAX_RUNTIME_PACKAGE_MANIFEST_JSON_BYTES,
    )?
    else {
        return Ok(None);
    };
    let artifact_set =
        super::super::evidence::read::load_artifact_set(connection, &artifact_set_id)?
            .ok_or(StoreError::CorruptRecord)?;
    let record = RuntimePackageManifest::from_json_bytes(encoded.as_bytes(), &artifact_set)
        .map_err(|_| StoreError::CorruptRecord)?;
    require_exact(
        &serde_json::to_string(&record)?,
        &encoded,
        record.runtime_package_manifest_id().digest().as_str(),
        key,
        record.artifact_set_id().digest().as_str(),
        &artifact_set_id,
    )?;
    require_source_artifact_set(
        connection,
        record.transformation(),
        source_artifact_set_id.as_deref(),
    )?;
    Ok(Some(record))
}

pub(in crate::store) fn load_model_package(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<ModelPackageManifest>> {
    let Some((artifact_set_id, source_artifact_set_id, encoded)) = load_package_row(
        connection,
        "model_package_manifests",
        "model_package_manifest_id",
        key,
        MAX_MODEL_PACKAGE_MANIFEST_JSON_BYTES,
    )?
    else {
        return Ok(None);
    };
    let artifact_set =
        super::super::evidence::read::load_artifact_set(connection, &artifact_set_id)?
            .ok_or(StoreError::CorruptRecord)?;
    let record = ModelPackageManifest::from_json_bytes(encoded.as_bytes(), &artifact_set)
        .map_err(|_| StoreError::CorruptRecord)?;
    require_exact(
        &serde_json::to_string(&record)?,
        &encoded,
        record.model_package_manifest_id().digest().as_str(),
        key,
        record.artifact_set_id().digest().as_str(),
        &artifact_set_id,
    )?;
    require_source_artifact_set(
        connection,
        record.transformation(),
        source_artifact_set_id.as_deref(),
    )?;
    Ok(Some(record))
}

fn load_package_row(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    maximum_json_bytes: usize,
) -> StoreResult<Option<(String, Option<String>, String)>> {
    let Some(artifact_set_id) =
        read_required_digest_text(connection, table, key_column, key, "artifact_set_id")?
    else {
        return Ok(None);
    };
    let source_artifact_set_id = match read_nullable_digest_text(
        connection,
        table,
        key_column,
        key,
        "source_artifact_set_id",
    )? {
        BoundedTextCell::MissingRow => return Err(StoreError::CorruptRecord),
        BoundedTextCell::Null => None,
        BoundedTextCell::Text(value) => Some(value),
    };
    let encoded = read_required_bounded_text(
        connection,
        table,
        key_column,
        key,
        "record_json",
        maximum_json_bytes,
    )?
    .ok_or(StoreError::CorruptRecord)?;
    Ok(Some((artifact_set_id, source_artifact_set_id, encoded)))
}

fn require_source_artifact_set(
    connection: &Connection,
    transformation: &PackageTransformation,
    indexed_source_id: Option<&str>,
) -> StoreResult<()> {
    let declared_source_id = match transformation {
        PackageTransformation::Untransformed { .. } => None,
        PackageTransformation::Transformed {
            source_artifact_set_id,
            ..
        } => Some(source_artifact_set_id.digest().as_str()),
    };
    if declared_source_id != indexed_source_id {
        return Err(StoreError::CorruptRecord);
    }
    if let Some(source_id) = declared_source_id {
        super::super::evidence::read::load_artifact_set(connection, source_id)?
            .ok_or(StoreError::CorruptRecord)?;
    }
    Ok(())
}

pub(super) fn load_native_load(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<NativeLoadObservation>> {
    let Some((runtime_package_id, encoded)) = load_row(
        connection,
        "native_load_observations",
        "native_load_observation_id",
        "runtime_package_manifest_id",
        key,
    )?
    else {
        return Ok(None);
    };
    let package =
        load_runtime_package(connection, &runtime_package_id)?.ok_or(StoreError::CorruptRecord)?;
    let record = NativeLoadObservation::from_json_bytes(encoded.as_bytes(), &package)
        .map_err(|_| StoreError::CorruptRecord)?;
    require_exact(
        &serde_json::to_string(&record)?,
        &encoded,
        record.native_load_observation_id().digest().as_str(),
        key,
        record.runtime_package_manifest_id().digest().as_str(),
        &runtime_package_id,
    )?;
    Ok(Some(record))
}

fn load_row(
    connection: &Connection,
    table: &str,
    key_column: &str,
    dependency_column: &str,
    key: &str,
) -> StoreResult<Option<(String, String)>> {
    let sql =
        format!("SELECT {dependency_column}, record_json FROM {table} WHERE {key_column} = ?1");
    connection
        .query_row(&sql, [key], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()
        .map_err(StoreError::Database)
}

fn require_exact(
    canonical: &str,
    encoded: &str,
    actual_key: &str,
    indexed_key: &str,
    actual_dependency: &str,
    indexed_dependency: &str,
) -> StoreResult<()> {
    if canonical == encoded && actual_key == indexed_key && actual_dependency == indexed_dependency
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}
