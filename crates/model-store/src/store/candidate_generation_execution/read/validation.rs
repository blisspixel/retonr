use rewrite_model::ManagedOllamaCandidateGenerationEvidenceV2;
use rusqlite::Connection;

use super::rows;
use crate::{StoreError, StoreResult};

pub(super) fn validate_managed_row(
    row: &rows::JsonRow,
    value: &ManagedOllamaCandidateGenerationEvidenceV2,
) -> StoreResult<()> {
    require_indexes(
        row,
        value.managed_evidence_v2_id().digest().as_str(),
        &[
            value.precursor_id().digest().as_str(),
            value.bracket_observation_v1_id().digest().as_str(),
            value.effective_package_evidence_v2_id().digest().as_str(),
            value.effective_runtime_state_id().digest().as_str(),
            value.effective_runtime_state_join_id().digest().as_str(),
            value.generation_request_binding_id().digest().as_str(),
            value.structured_request_binding_id().digest().as_str(),
            value.response_id().digest().as_str(),
        ],
    )?;
    require_exact_json(value, &row.canonical_json)
}

pub(super) fn required_json_row(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    columns: &[&str],
    maximum: usize,
) -> StoreResult<rows::JsonRow> {
    rows::load_json_row(connection, table, key_column, key, columns, maximum)?
        .ok_or(StoreError::CorruptRecord)
}

pub(super) fn require_indexes(row: &rows::JsonRow, id: &str, indexes: &[&str]) -> StoreResult<()> {
    if row.id == id
        && row.indexes.len() == indexes.len()
        && row
            .indexes
            .iter()
            .map(String::as_str)
            .eq(indexes.iter().copied())
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(super) fn require_id(actual: &str, expected: &str) -> StoreResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(super) fn require_exact_json<T: serde::Serialize>(value: &T, stored: &[u8]) -> StoreResult<()> {
    if serde_json::to_vec(value)? == stored {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}
