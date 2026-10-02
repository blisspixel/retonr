use super::super::codec::PreparedCohort;
use super::{check, rows};
use crate::{StoreError, StoreResult};
use rusqlite::Connection;

pub(in crate::store::generation_qualification_terminal_evidence) fn confirm(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<bool> {
    let p = &prepared.records;
    let policy = p.receipt.operation_policy_id().digest().as_str();
    let platform = rows::load_platform(connection, policy)?;
    let license = rows::load_license(connection, policy)?;
    let human = rows::load_phase_manifest(
        connection,
        "generation_human_adjudication_evidence_manifests",
        "generation_human_adjudication_evidence_manifest_id",
        p.human.generation_qualification_plan_id().digest().as_str(),
        p.human.generation_system_id().digest().as_str(),
    )?;
    let receipt = rows::load_receipt(connection, policy)?;
    if rows::load_interruption(connection, policy)?.is_some() {
        return Err(StoreError::CorruptRecord);
    }
    if platform.is_none() && license.is_none() && human.is_none() && receipt.is_none() {
        return Ok(false);
    }
    let platform = platform.ok_or(StoreError::MissingRecord)?;
    let license = license.ok_or(StoreError::MissingRecord)?;
    let human = human.ok_or(StoreError::MissingRecord)?;
    let receipt = receipt.ok_or(StoreError::MissingRecord)?;
    if platform.bytes != prepared.encoded.platform
        || license.bytes != prepared.encoded.license
        || human.bytes != prepared.encoded.human
        || receipt.bytes != prepared.encoded.receipt
    {
        return Err(StoreError::CorruptRecord);
    }
    check::platform(&platform, p.platform.clone())?;
    check::license(&license, p.license.clone())?;
    check::phase(
        &human,
        check::PhaseCheck {
            id: p.human.human_adjudication_evidence_manifest_id().digest(),
            system: p.human.generation_system_id().digest(),
            plan: p.human.generation_qualification_plan_id().digest(),
            suite: p.human.suite_manifest_id().digest(),
            count: 0,
            status: rewrite_model::GenerationQualificationPhaseStatusV1::Skipped,
        },
    )?;
    check::receipt(
        &receipt,
        p.receipt.clone(),
        &p.ledger,
        &p.repeatability,
        &p.resource,
        &p.human,
    )?;
    Ok(true)
}
