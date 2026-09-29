use rusqlite::Connection;
use serde_json::Value;

use super::super::codec::PreparedCohort;
use super::super::rows;
use crate::{StoreError, StoreResult};

pub(super) fn stored(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    confirm_platform(connection, prepared)?;
    confirm_license(connection, prepared)?;
    confirm_ledger(connection, prepared)?;
    confirm_results(connection, prepared)?;
    confirm_repeatability(connection, prepared)?;
    confirm_resource(connection, prepared)?;
    confirm_human(connection, prepared)?;
    confirm_receipt(connection, prepared)?;
    confirm_interruption(connection, prepared)
}

fn confirm_platform(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    let row = rows::load_platform(
        connection,
        prepared
            .records
            .platform
            .operation_policy_id()
            .digest()
            .as_str(),
    )?
    .ok_or(StoreError::CorruptRecord)?;
    require_json(&row.bytes, &prepared.encoded.platform)
}

fn confirm_license(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    let row = rows::load_license(
        connection,
        prepared
            .records
            .license
            .operation_policy_id()
            .digest()
            .as_str(),
    )?
    .ok_or(StoreError::CorruptRecord)?;
    require_json(&row.bytes, &prepared.encoded.license)
}

fn confirm_ledger(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    confirm_phase(
        connection,
        "generation_attempt_ledger_manifests",
        "generation_attempt_ledger_manifest_id",
        prepared
            .records
            .ledger
            .generation_qualification_plan_id()
            .digest()
            .as_str(),
        prepared
            .records
            .ledger
            .generation_system_id()
            .digest()
            .as_str(),
        &prepared.encoded.ledger,
    )
}

fn confirm_repeatability(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<()> {
    confirm_phase(
        connection,
        "generation_repeatability_evidence_manifests",
        "generation_repeatability_evidence_manifest_id",
        prepared
            .records
            .repeatability
            .generation_qualification_plan_id()
            .digest()
            .as_str(),
        prepared
            .records
            .repeatability
            .generation_system_id()
            .digest()
            .as_str(),
        &prepared.encoded.repeatability,
    )
}

fn confirm_resource(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    confirm_phase(
        connection,
        "generation_resource_evidence_manifests",
        "generation_resource_evidence_manifest_id",
        prepared
            .records
            .resource
            .generation_qualification_plan_id()
            .digest()
            .as_str(),
        prepared
            .records
            .resource
            .generation_system_id()
            .digest()
            .as_str(),
        &prepared.encoded.resource,
    )
}

fn confirm_human(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    confirm_phase(
        connection,
        "generation_human_adjudication_evidence_manifests",
        "generation_human_adjudication_evidence_manifest_id",
        prepared
            .records
            .human
            .generation_qualification_plan_id()
            .digest()
            .as_str(),
        prepared
            .records
            .human
            .generation_system_id()
            .digest()
            .as_str(),
        &prepared.encoded.human,
    )
}

fn confirm_phase(
    connection: &Connection,
    table: &str,
    identifier: &str,
    plan_id: &str,
    system_id: &str,
    bytes: &[u8],
) -> StoreResult<()> {
    let row = rows::load_phase_manifest(connection, table, identifier, plan_id, system_id)?
        .ok_or(StoreError::CorruptRecord)?;
    require_json(&row.bytes, bytes)
}

fn confirm_results(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    let plan_id = prepared
        .records
        .ledger
        .generation_qualification_plan_id()
        .digest()
        .as_str();
    let system_id = prepared
        .records
        .ledger
        .generation_system_id()
        .digest()
        .as_str();
    if prepared.encoded.results.is_empty() {
        return if rows::load_results(connection, plan_id, system_id)?.is_empty() {
            Ok(())
        } else {
            Err(StoreError::CorruptRecord)
        };
    }
    for (result, bytes) in prepared
        .records
        .results
        .iter()
        .zip(&prepared.encoded.results)
    {
        let row = rows::load_result(
            connection,
            plan_id,
            result.repetition_id().digest().as_str(),
        )?
        .ok_or(StoreError::CorruptRecord)?;
        require_json(&row.bytes, bytes)?;
    }
    Ok(())
}

fn confirm_receipt(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    let row = rows::load_receipt(
        connection,
        prepared
            .records
            .receipt
            .operation_policy_id()
            .digest()
            .as_str(),
    )?
    .ok_or(StoreError::CorruptRecord)?;
    require_json(&row.bytes, &prepared.encoded.receipt)
}

fn confirm_interruption(connection: &Connection, prepared: &PreparedCohort<'_>) -> StoreResult<()> {
    let row = rows::load_interruption(
        connection,
        prepared
            .records
            .receipt
            .operation_policy_id()
            .digest()
            .as_str(),
    )?;
    match (&prepared.encoded.interruption, row) {
        (None, None) => Ok(()),
        (Some(bytes), Some(row)) => require_json(&row.bytes, bytes),
        (None, Some(_)) | (Some(_), None) => Err(StoreError::CorruptRecord),
    }
}

fn require_json(stored: &[u8], expected: &[u8]) -> StoreResult<()> {
    if stored == expected {
        Ok(())
    } else if serde_json::from_slice::<Value>(stored).is_ok() {
        Err(StoreError::ImmutableConflict)
    } else {
        Err(StoreError::CorruptRecord)
    }
}
