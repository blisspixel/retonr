use rusqlite::{Connection, OptionalExtension as _, Row};

use rewrite_model::{ActiveArtifactBinding, ArtifactRole};

use crate::{StoreError, StoreResult, record::decode_record};

struct StoredBindingRow {
    role: String,
    artifact_id: String,
    qualification_id: String,
    record_json: String,
}

pub(super) fn load_active_binding(
    connection: &Connection,
    role: ArtifactRole,
) -> StoreResult<Option<ActiveArtifactBinding>> {
    connection
        .query_row(
            "SELECT role, artifact_id, qualification_id, record_json
             FROM active_bindings WHERE role = ?1",
            [role_key(role)],
            read_stored_binding,
        )
        .optional()?
        .map(|row| decode_stored_binding(&row))
        .transpose()
}

pub(super) fn load_active_bindings(
    connection: &Connection,
) -> StoreResult<Vec<ActiveArtifactBinding>> {
    let mut statement = connection.prepare(
        "SELECT role, artifact_id, qualification_id, record_json
         FROM active_bindings ORDER BY role ASC",
    )?;
    let rows = statement.query_map([], read_stored_binding)?;
    let mut bindings = Vec::new();
    for row in rows {
        bindings.push(decode_stored_binding(&row?)?);
    }
    Ok(bindings)
}

fn read_stored_binding(row: &Row<'_>) -> rusqlite::Result<StoredBindingRow> {
    Ok(StoredBindingRow {
        role: row.get(0)?,
        artifact_id: row.get(1)?,
        qualification_id: row.get(2)?,
        record_json: row.get(3)?,
    })
}

fn decode_stored_binding(row: &StoredBindingRow) -> StoreResult<ActiveArtifactBinding> {
    let binding = decode_record::<ActiveArtifactBinding>(&row.record_json)?;
    if row.role != role_key(binding.role)
        || row.artifact_id != binding.artifact_id.digest().as_str()
        || row.qualification_id != binding.qualification_id.digest().as_str()
    {
        return Err(StoreError::CorruptRecord);
    }
    Ok(binding)
}

pub(super) const fn role_key(role: ArtifactRole) -> &'static str {
    match role {
        ArtifactRole::Generation => "generation",
        ArtifactRole::Embedding => "embedding",
        ArtifactRole::SpeechRecognition => "speech_recognition",
        ArtifactRole::VoiceActivityDetection => "voice_activity_detection",
        ArtifactRole::SpeechSynthesis => "speech_synthesis",
        ArtifactRole::Voice => "voice",
    }
}
