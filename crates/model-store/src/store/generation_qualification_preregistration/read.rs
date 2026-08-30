use rewrite_model::{
    GenerationQualificationOperationPolicyV1, GenerationQualificationRequestProjectionV1,
    MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_JSON_BYTES,
    MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_JSON_BYTES,
};
use rusqlite::{Connection, OptionalExtension as _, params};

use super::{
    GenerationQualificationPreregistrationReadInput, StoredGenerationQualificationPreregistration,
    projection_relations, require_blob_bound,
};
use crate::GenerationQualificationPlanFoundationV1;
use crate::{StoreError, StoreResult};

pub(super) fn load_pair(
    connection: &Connection,
    input: GenerationQualificationPreregistrationReadInput<'_>,
    plan_foundation: GenerationQualificationPlanFoundationV1,
) -> StoreResult<Option<StoredGenerationQualificationPreregistration>> {
    let policy_key = input.operation_policy_id.digest().as_str();
    let projection_key = input.request_projection_id.digest().as_str();
    let policy_row = load_policy_row(connection, policy_key)?;
    let projection_row = load_projection_row(connection, projection_key)?;
    match (policy_row, projection_row) {
        (None, None) => Ok(None),
        (Some(policy_row), Some(projection_row)) => {
            let policy = GenerationQualificationOperationPolicyV1::from_json_bytes(
                &policy_row.canonical_json,
                input.operation_policy_relations,
                input.operation_policy_input,
            )
            .map_err(|_| StoreError::CorruptRecord)?;
            require_policy_indexes(&policy_row, &policy, policy_key)?;
            let projection = GenerationQualificationRequestProjectionV1::from_json_bytes(
                &projection_row.canonical_json,
                projection_relations(&policy, input.operation_policy_relations),
                input.request_projection_entry_inputs,
            )
            .map_err(|_| StoreError::CorruptRecord)?;
            require_projection_indexes(&projection_row, &projection, projection_key, policy_key)?;
            Ok(Some(StoredGenerationQualificationPreregistration {
                plan_foundation,
                operation_policy: policy,
                request_projection: projection,
            }))
        }
        _ => Err(StoreError::CorruptRecord),
    }
}

pub(super) fn require_exact_policy_row(
    connection: &Connection,
    expected: &PolicyRow,
) -> StoreResult<()> {
    match load_policy_row(connection, &expected.record_id)? {
        Some(actual) if actual == *expected => Ok(()),
        Some(_) => Err(StoreError::ImmutableConflict),
        None => Err(StoreError::CorruptRecord),
    }
}

pub(super) fn require_exact_projection_row(
    connection: &Connection,
    expected: &ProjectionRow,
) -> StoreResult<()> {
    if let Some(actual) = load_projection_row(connection, &expected.record_id)? {
        return if actual == *expected {
            Ok(())
        } else {
            Err(StoreError::ImmutableConflict)
        };
    }
    let conflicting: bool = connection.query_row(
        "SELECT EXISTS(
             SELECT 1 FROM generation_qualification_request_projections
             WHERE operation_policy_id = ?1
         )",
        [&expected.operation_policy_id],
        |row| row.get(0),
    )?;
    if conflicting {
        Err(StoreError::ImmutableConflict)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

#[derive(Eq, PartialEq)]
pub(super) struct PolicyRow {
    pub(super) record_id: String,
    pub(super) plan_id: String,
    pub(super) suite_id: String,
    pub(super) target_system_id: String,
    pub(super) baseline_system_id: String,
    pub(super) canonical_json: Vec<u8>,
}

#[derive(Eq, PartialEq)]
pub(super) struct ProjectionRow {
    pub(super) record_id: String,
    pub(super) operation_policy_id: String,
    pub(super) plan_id: String,
    pub(super) suite_id: String,
    pub(super) target_system_id: String,
    pub(super) baseline_system_id: String,
    pub(super) entry_count: i64,
    pub(super) canonical_json: Vec<u8>,
}

fn load_policy_row(connection: &Connection, key: &str) -> StoreResult<Option<PolicyRow>> {
    let metadata = connection
        .query_row(
            "SELECT operation_policy_id, generation_qualification_plan_id, suite_manifest_id,
                    target_generation_system_id, baseline_generation_system_id,
                    typeof(canonical_json), length(canonical_json)
             FROM generation_qualification_operation_policies
             WHERE operation_policy_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((record_id, plan_id, suite_id, target_system_id, baseline_system_id, kind, length)) =
        metadata
    else {
        return Ok(None);
    };
    let canonical_json = load_blob(
        connection,
        "generation_qualification_operation_policies",
        "operation_policy_id",
        key,
        &kind,
        length,
        MAX_GENERATION_QUALIFICATION_OPERATION_POLICY_JSON_BYTES,
    )?;
    Ok(Some(PolicyRow {
        record_id,
        plan_id,
        suite_id,
        target_system_id,
        baseline_system_id,
        canonical_json,
    }))
}

fn load_projection_row(connection: &Connection, key: &str) -> StoreResult<Option<ProjectionRow>> {
    let metadata = connection
        .query_row(
            "SELECT request_projection_id, operation_policy_id,
                    generation_qualification_plan_id, suite_manifest_id,
                    target_generation_system_id, baseline_generation_system_id, entry_count,
                    typeof(canonical_json), length(canonical_json)
             FROM generation_qualification_request_projections
             WHERE request_projection_id = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((
        record_id,
        operation_policy_id,
        plan_id,
        suite_id,
        target_system_id,
        baseline_system_id,
        entry_count,
        kind,
        length,
    )) = metadata
    else {
        return Ok(None);
    };
    let canonical_json = load_blob(
        connection,
        "generation_qualification_request_projections",
        "request_projection_id",
        key,
        &kind,
        length,
        MAX_GENERATION_QUALIFICATION_REQUEST_PROJECTION_JSON_BYTES,
    )?;
    Ok(Some(ProjectionRow {
        record_id,
        operation_policy_id,
        plan_id,
        suite_id,
        target_system_id,
        baseline_system_id,
        entry_count,
        canonical_json,
    }))
}

pub(super) fn load_blob(
    connection: &Connection,
    table: &str,
    key_column: &str,
    key: &str,
    storage_class: &str,
    length: i64,
    maximum: usize,
) -> StoreResult<Vec<u8>> {
    let length = usize::try_from(length).map_err(|_| StoreError::CorruptRecord)?;
    if storage_class != "blob" {
        return Err(StoreError::CorruptRecord);
    }
    require_blob_bound(length, maximum).map_err(|_| StoreError::CorruptRecord)?;
    let bounded = i64::try_from(maximum + 1).map_err(|_| StoreError::InvalidLimit)?;
    let sql = format!(
        "SELECT CAST(substr(canonical_json, 1, ?2) AS BLOB)
         FROM {table} WHERE {key_column} = ?1"
    );
    let bytes: Vec<u8> = connection.query_row(&sql, params![key, bounded], |row| row.get(0))?;
    if bytes.len() == length {
        Ok(bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_policy_indexes(
    row: &PolicyRow,
    policy: &GenerationQualificationOperationPolicyV1,
    expected_key: &str,
) -> StoreResult<()> {
    let canonical = serde_json::to_vec(policy)?;
    if row.record_id == expected_key
        && row.plan_id == policy.generation_qualification_plan_id().digest().as_str()
        && row.suite_id == policy.suite_manifest_id().digest().as_str()
        && row.target_system_id == policy.target_generation_system_id().digest().as_str()
        && row.baseline_system_id == policy.baseline_generation_system_id().digest().as_str()
        && row.canonical_json == canonical
        && policy.operation_policy_id().digest().as_str() == expected_key
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn require_projection_indexes(
    row: &ProjectionRow,
    projection: &GenerationQualificationRequestProjectionV1,
    expected_key: &str,
    expected_policy_key: &str,
) -> StoreResult<()> {
    let canonical = serde_json::to_vec(projection)?;
    let entry_count =
        i64::try_from(projection.entry_count()).map_err(|_| StoreError::CorruptRecord)?;
    if row.record_id == expected_key
        && row.operation_policy_id == expected_policy_key
        && row.plan_id
            == projection
                .generation_qualification_plan_id()
                .digest()
                .as_str()
        && row.suite_id == projection.suite_manifest_id().digest().as_str()
        && row.target_system_id == projection.target_generation_system_id().digest().as_str()
        && row.baseline_system_id == projection.baseline_generation_system_id().digest().as_str()
        && row.entry_count == entry_count
        && row.canonical_json == canonical
        && projection.request_projection_id().digest().as_str() == expected_key
        && projection.operation_policy_id().digest().as_str() == expected_policy_key
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}
