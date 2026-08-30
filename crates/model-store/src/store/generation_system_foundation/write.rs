use rewrite_model::{EffectivePackageEvidenceV2, GenerationSystemRecordV1};
use rusqlite::{Connection, params};

use crate::{StoreError, StoreResult, WriteDisposition};

use super::read::{read_evidence_row, read_system_row};

pub(super) fn insert_effective_package_evidence_v2(
    connection: &Connection,
    evidence: &EffectivePackageEvidenceV2,
    canonical_json: &[u8],
) -> StoreResult<WriteDisposition> {
    let id = evidence.effective_package_evidence_v2_id();
    let changed = connection.execute(
        "INSERT INTO effective_package_evidence_v2 (
             effective_package_evidence_v2_id, artifact_set_id, runtime_build_id,
             effective_runtime_state_id, member_count, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(effective_package_evidence_v2_id) DO NOTHING",
        params![
            id.digest().as_str(),
            evidence.artifact_set_id().digest().as_str(),
            evidence.runtime_build_id().digest().as_str(),
            evidence.effective_runtime_state_id().digest().as_str(),
            i64::try_from(evidence.member_evidence().len())
                .map_err(|_| StoreError::RecordTooLarge)?,
            canonical_json,
        ],
    )?;
    if changed == 1 {
        return Ok(WriteDisposition::Inserted);
    }
    let row =
        read_evidence_row(connection, id.digest().as_str())?.ok_or(StoreError::CorruptRecord)?;
    if row.exactly_matches(evidence, canonical_json) {
        Ok(WriteDisposition::AlreadyPresent)
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

pub(super) fn insert_generation_system(
    connection: &Connection,
    system: &GenerationSystemRecordV1,
    canonical_json: &[u8],
) -> StoreResult<WriteDisposition> {
    let changed = connection.execute(
        "INSERT INTO generation_system_records (
             generation_system_id, runtime_package_manifest_id, runtime_build_id,
             effective_runtime_state_id, model_artifact_set_id, model_package_manifest_id,
             model_artifact_id, effective_package_evidence_v2_id, canonical_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(generation_system_id) DO NOTHING",
        params![
            system.generation_system_id().digest().as_str(),
            system.runtime_package_manifest_id().digest().as_str(),
            system.runtime_build_id().digest().as_str(),
            system.effective_runtime_state_id().digest().as_str(),
            system.model_artifact_set_id().digest().as_str(),
            system.model_package_manifest_id().digest().as_str(),
            system.model_artifact_id().digest().as_str(),
            system.effective_package_evidence_v2_id().digest().as_str(),
            canonical_json,
        ],
    )?;
    if changed == 1 {
        return Ok(WriteDisposition::Inserted);
    }
    let row = read_system_row(connection, system.generation_system_id().digest().as_str())?
        .ok_or(StoreError::CorruptRecord)?;
    if row.exactly_matches(system, canonical_json) {
        Ok(WriteDisposition::AlreadyPresent)
    } else {
        Err(StoreError::ImmutableConflict)
    }
}
