use rewrite_model::{
    EffectivePackageEvidenceV2, GenerationSystemId, GenerationSystemRecordV1,
    GenerationSystemRecordV1Relations, MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES,
    MAX_GENERATION_SYSTEM_JSON_BYTES,
};
use rusqlite::{Connection, OptionalExtension as _, params};

use crate::store::bounded_text::read_required_digest_text;
use crate::{StoreError, StoreResult};

use super::GenerationSystemFoundationV1;

pub(super) struct EvidenceRow {
    artifact_set_id: String,
    runtime_build_id: String,
    runtime_state_id: String,
    member_count: i64,
    canonical_json: Vec<u8>,
}

impl EvidenceRow {
    pub(super) fn exactly_matches(
        &self,
        evidence: &EffectivePackageEvidenceV2,
        encoded: &[u8],
    ) -> bool {
        self.artifact_set_id == evidence.artifact_set_id().digest().as_str()
            && self.runtime_build_id == evidence.runtime_build_id().digest().as_str()
            && self.runtime_state_id == evidence.effective_runtime_state_id().digest().as_str()
            && usize::try_from(self.member_count).ok() == Some(evidence.member_evidence().len())
            && self.canonical_json == encoded
    }
}

pub(super) struct SystemRow {
    runtime_package_id: String,
    runtime_build_id: String,
    runtime_state_id: String,
    model_artifact_set_id: String,
    model_package_id: String,
    model_artifact_id: String,
    effective_package_id: String,
    canonical_json: Vec<u8>,
}

impl SystemRow {
    pub(super) fn exactly_matches(
        &self,
        system: &GenerationSystemRecordV1,
        encoded: &[u8],
    ) -> bool {
        self.runtime_package_id == system.runtime_package_manifest_id().digest().as_str()
            && self.runtime_build_id == system.runtime_build_id().digest().as_str()
            && self.runtime_state_id == system.effective_runtime_state_id().digest().as_str()
            && self.model_artifact_set_id == system.model_artifact_set_id().digest().as_str()
            && self.model_package_id == system.model_package_manifest_id().digest().as_str()
            && self.model_artifact_id == system.model_artifact_id().digest().as_str()
            && self.effective_package_id
                == system.effective_package_evidence_v2_id().digest().as_str()
            && self.canonical_json == encoded
    }
}

pub(super) fn read_evidence_row(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<EvidenceRow>> {
    let Some(artifact_set_id) = read_required_digest_text(
        connection,
        "effective_package_evidence_v2",
        "effective_package_evidence_v2_id",
        key,
        "artifact_set_id",
    )?
    else {
        return Ok(None);
    };
    let runtime_build_id = read_required_digest_text(
        connection,
        "effective_package_evidence_v2",
        "effective_package_evidence_v2_id",
        key,
        "runtime_build_id",
    )?
    .ok_or(StoreError::CorruptRecord)?;
    let runtime_state_id = read_required_digest_text(
        connection,
        "effective_package_evidence_v2",
        "effective_package_evidence_v2_id",
        key,
        "effective_runtime_state_id",
    )?
    .ok_or(StoreError::CorruptRecord)?;
    let metadata = connection
        .query_row(
            "SELECT member_count, typeof(canonical_json), length(canonical_json)
             FROM effective_package_evidence_v2
             WHERE effective_package_evidence_v2_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((member_count, kind, length)) = metadata else {
        return Err(StoreError::CorruptRecord);
    };
    let canonical_json = read_bounded_blob(
        connection,
        "effective_package_evidence_v2",
        "effective_package_evidence_v2_id",
        key,
        &kind,
        length,
        MAX_EFFECTIVE_PACKAGE_EVIDENCE_V2_JSON_BYTES,
    )?;
    Ok(Some(EvidenceRow {
        artifact_set_id,
        runtime_build_id,
        runtime_state_id,
        member_count,
        canonical_json,
    }))
}

pub(super) fn read_system_row(
    connection: &Connection,
    key: &str,
) -> StoreResult<Option<SystemRow>> {
    let Some(runtime_package_id) = read_required_digest_text(
        connection,
        "generation_system_records",
        "generation_system_id",
        key,
        "runtime_package_manifest_id",
    )?
    else {
        return Ok(None);
    };
    let runtime_build_id = read_system_digest(connection, key, "runtime_build_id")?;
    let runtime_state_id = read_system_digest(connection, key, "effective_runtime_state_id")?;
    let model_artifact_set_id = read_system_digest(connection, key, "model_artifact_set_id")?;
    let model_package_id = read_system_digest(connection, key, "model_package_manifest_id")?;
    let model_artifact_id = read_system_digest(connection, key, "model_artifact_id")?;
    let effective_package_id =
        read_system_digest(connection, key, "effective_package_evidence_v2_id")?;
    let metadata = connection
        .query_row(
            "SELECT typeof(canonical_json), length(canonical_json)
             FROM generation_system_records WHERE generation_system_id = ?1",
            [key],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let Some((kind, length)) = metadata else {
        return Err(StoreError::CorruptRecord);
    };
    let canonical_json = read_bounded_blob(
        connection,
        "generation_system_records",
        "generation_system_id",
        key,
        &kind,
        length,
        MAX_GENERATION_SYSTEM_JSON_BYTES,
    )?;
    Ok(Some(SystemRow {
        runtime_package_id,
        runtime_build_id,
        runtime_state_id,
        model_artifact_set_id,
        model_package_id,
        model_artifact_id,
        effective_package_id,
        canonical_json,
    }))
}

fn read_system_digest(connection: &Connection, key: &str, column: &str) -> StoreResult<String> {
    read_required_digest_text(
        connection,
        "generation_system_records",
        "generation_system_id",
        key,
        column,
    )?
    .ok_or(StoreError::CorruptRecord)
}

pub(crate) fn read_bounded_blob(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    kind: &str,
    length: i64,
    maximum: usize,
) -> StoreResult<Vec<u8>> {
    if kind != "blob"
        || length < 1
        || usize::try_from(length)
            .ok()
            .is_none_or(|value| value > maximum)
    {
        return Err(StoreError::CorruptRecord);
    }
    let sql = format!("SELECT substr(canonical_json, 1, ?2) FROM {table} WHERE {key_column} = ?1");
    let bytes: Vec<u8> = connection.query_row(
        &sql,
        params![
            key,
            i64::try_from(maximum).map_err(|_| StoreError::RecordTooLarge)?
        ],
        |row| row.get(0),
    )?;
    if i64::try_from(bytes.len()).ok() != Some(length) {
        return Err(StoreError::CorruptRecord);
    }
    Ok(bytes)
}

pub(crate) fn load_foundation(
    connection: &Connection,
    generation_system_id: &GenerationSystemId,
) -> StoreResult<Option<GenerationSystemFoundationV1>> {
    use crate::store::evidence::read::{load_artifact_set, load_runtime_build, load_runtime_state};
    use crate::store::package_contracts::read::{load_model_package, load_runtime_package};

    let Some(system_row) = read_system_row(connection, generation_system_id.digest().as_str())?
    else {
        return Ok(None);
    };
    let runtime_package = load_runtime_package(connection, &system_row.runtime_package_id)?
        .ok_or(StoreError::CorruptRecord)?;
    let runtime_build = load_runtime_build(connection, &system_row.runtime_build_id)?
        .ok_or(StoreError::CorruptRecord)?;
    let runtime_state = load_runtime_state(connection, &system_row.runtime_state_id)?
        .ok_or(StoreError::CorruptRecord)?;
    let model_artifact_set = load_artifact_set(connection, &system_row.model_artifact_set_id)?
        .ok_or(StoreError::CorruptRecord)?;
    let model_package = load_model_package(connection, &system_row.model_package_id)?
        .ok_or(StoreError::CorruptRecord)?;
    let evidence_row = read_evidence_row(connection, &system_row.effective_package_id)?
        .ok_or(StoreError::CorruptRecord)?;
    let evidence = EffectivePackageEvidenceV2::from_json_bytes(
        &evidence_row.canonical_json,
        &model_artifact_set,
        &runtime_build,
        &runtime_state,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    if !evidence_row.exactly_matches(&evidence, &serde_json::to_vec(&evidence)?)
        || evidence
            .effective_package_evidence_v2_id()
            .digest()
            .as_str()
            != system_row.effective_package_id
    {
        return Err(StoreError::CorruptRecord);
    }
    let relations = GenerationSystemRecordV1Relations {
        runtime_package_manifest: &runtime_package,
        runtime_build: &runtime_build,
        effective_runtime_state: &runtime_state,
        model_artifact_set: &model_artifact_set,
        model_package_manifest: &model_package,
        effective_package_evidence_v2: &evidence,
    };
    let system = GenerationSystemRecordV1::from_json_bytes(&system_row.canonical_json, relations)
        .map_err(|_| StoreError::CorruptRecord)?;
    if system.generation_system_id() != generation_system_id
        || !system_row.exactly_matches(&system, &serde_json::to_vec(&system)?)
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(Some(GenerationSystemFoundationV1 {
        effective_package_evidence_v2: evidence,
        generation_system: system,
    }))
}
