use rusqlite::{Connection, OptionalExtension as _};

use rewrite_model::{
    ArtifactSetManifest, EffectivePackageEvidence, EffectiveRuntimeState,
    MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES, MAX_RUNTIME_IDENTITY_JSON_BYTES, QualificationRecordV2,
    RuntimeBuildIdentity,
};

use crate::{StoreError, StoreResult};

use super::super::bounded_text::{read_required_bounded_text, read_required_digest_text};

pub(super) struct QualificationDependencies {
    pub(super) artifact_set: ArtifactSetManifest,
    pub(super) build: RuntimeBuildIdentity,
    pub(super) state: EffectiveRuntimeState,
    pub(super) package: EffectivePackageEvidence,
}

pub(in crate::store) fn load_artifact_set(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<ArtifactSetManifest>> {
    let Some(encoded) = load_json(
        connection,
        "artifact_set_manifests",
        "artifact_set_id",
        key,
        MAX_ARTIFACT_SET_MANIFEST_JSON_BYTES,
    )?
    else {
        return Ok(None);
    };
    let record = ArtifactSetManifest::from_json_bytes(encoded.as_bytes())
        .map_err(|_| StoreError::CorruptRecord)?;
    require_canonical_and_key(
        &record.canonical_json(),
        &encoded,
        record.artifact_set_id().digest().as_str(),
        key,
    )?;
    Ok(Some(record))
}

pub(in crate::store) fn load_runtime_build(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<RuntimeBuildIdentity>> {
    let Some(encoded) = load_json(
        connection,
        "runtime_build_identities",
        "runtime_build_id",
        key,
        MAX_RUNTIME_IDENTITY_JSON_BYTES,
    )?
    else {
        return Ok(None);
    };
    let record = RuntimeBuildIdentity::from_json_bytes(encoded.as_bytes())
        .map_err(|_| StoreError::CorruptRecord)?;
    let canonical = serde_json::to_string(&record)?;
    require_canonical_and_key(
        &canonical,
        &encoded,
        record.runtime_build_id().digest().as_str(),
        key,
    )?;
    Ok(Some(record))
}

pub(in crate::store) fn load_runtime_state(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<EffectiveRuntimeState>> {
    let Some(stored_build_id) = read_required_digest_text(
        connection,
        "effective_runtime_states",
        "effective_runtime_state_id",
        key,
        "runtime_build_id",
    )?
    else {
        return Ok(None);
    };
    let encoded = read_required_bounded_text(
        connection,
        "effective_runtime_states",
        "effective_runtime_state_id",
        key,
        "record_json",
        MAX_RUNTIME_IDENTITY_JSON_BYTES,
    )?
    .ok_or(StoreError::CorruptRecord)?;
    let record = EffectiveRuntimeState::from_json_bytes(encoded.as_bytes())
        .map_err(|_| StoreError::CorruptRecord)?;
    let canonical = serde_json::to_string(&record)?;
    require_canonical_and_key(
        &canonical,
        &encoded,
        record.effective_runtime_state_id().digest().as_str(),
        key,
    )?;
    if stored_build_id != record.runtime_build_id().digest().as_str() {
        return Err(StoreError::CorruptRecord);
    }
    load_runtime_build(connection, &stored_build_id)?.ok_or(StoreError::CorruptRecord)?;
    Ok(Some(record))
}

pub(super) fn load_package_evidence(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<EffectivePackageEvidence>> {
    let row = connection
        .query_row(
            "SELECT artifact_set_id, runtime_build_id, effective_runtime_state_id, record_json
             FROM effective_package_evidence WHERE effective_package_evidence_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((artifact_set_id, runtime_build_id, runtime_state_id, encoded)) = row else {
        return Ok(None);
    };
    let artifact_set =
        load_artifact_set(connection, &artifact_set_id)?.ok_or(StoreError::CorruptRecord)?;
    let build =
        load_runtime_build(connection, &runtime_build_id)?.ok_or(StoreError::CorruptRecord)?;
    let state =
        load_runtime_state(connection, &runtime_state_id)?.ok_or(StoreError::CorruptRecord)?;
    let record = EffectivePackageEvidence::from_json_bytes(
        encoded.as_bytes(),
        &artifact_set,
        &build,
        &state,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    let canonical = serde_json::to_string(&record)?;
    require_canonical_and_key(
        &canonical,
        &encoded,
        record.effective_package_evidence_id().digest().as_str(),
        key,
    )?;
    if artifact_set_id != record.artifact_set_id().digest().as_str()
        || runtime_build_id != record.runtime_build_id().digest().as_str()
        || runtime_state_id != record.effective_runtime_state_id().digest().as_str()
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(record))
}

pub(super) fn load_qualification_v2(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<QualificationRecordV2>> {
    let row = connection
        .query_row(
            "SELECT artifact_set_id, effective_package_evidence_id,
                    runtime_build_id, effective_runtime_state_id, record_json
             FROM qualification_v2_records WHERE qualification_v2_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((artifact_set_id, package_id, build_id, state_id, encoded)) = row else {
        return Ok(None);
    };
    let dependencies = load_qualification_dependencies(
        connection,
        &artifact_set_id,
        &build_id,
        &state_id,
        &package_id,
    )
    .map_err(|error| match error {
        StoreError::MissingRecord => StoreError::CorruptRecord,
        other => other,
    })?;
    let record = QualificationRecordV2::from_json_bytes(
        encoded.as_bytes(),
        &dependencies.artifact_set,
        &dependencies.build,
        &dependencies.state,
        &dependencies.package,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    let canonical = serde_json::to_string(&record)?;
    require_canonical_and_key(
        &canonical,
        &encoded,
        record.qualification_v2_id().digest().as_str(),
        key,
    )?;
    if artifact_set_id != record.artifact_set_id().digest().as_str()
        || package_id != record.effective_package_evidence_id().digest().as_str()
        || build_id != record.runtime_build_id().digest().as_str()
        || state_id != record.effective_runtime_state_id().digest().as_str()
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(record))
}

pub(super) fn load_qualification_dependencies(
    connection: &Connection,
    artifact_set_id: &str,
    runtime_build_id: &str,
    runtime_state_id: &str,
    package_id: &str,
) -> StoreResult<QualificationDependencies> {
    Ok(QualificationDependencies {
        artifact_set: load_artifact_set(connection, artifact_set_id)?
            .ok_or(StoreError::MissingRecord)?,
        build: load_runtime_build(connection, runtime_build_id)?
            .ok_or(StoreError::MissingRecord)?,
        state: load_runtime_state(connection, runtime_state_id)?
            .ok_or(StoreError::MissingRecord)?,
        package: load_package_evidence(connection, package_id)?.ok_or(StoreError::MissingRecord)?,
    })
}

fn load_json(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    maximum_bytes: usize,
) -> StoreResult<Option<String>> {
    read_required_bounded_text(
        connection,
        table,
        key_column,
        key,
        "record_json",
        maximum_bytes,
    )
}

fn require_canonical_and_key(
    canonical: &str,
    encoded: &str,
    actual_key: &str,
    indexed_key: &str,
) -> StoreResult<()> {
    if canonical == encoded && actual_key == indexed_key {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}
