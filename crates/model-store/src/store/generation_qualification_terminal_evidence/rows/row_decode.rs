use rusqlite::Row;

use super::{InterruptionRow, LicenseRow, PhaseManifestRow, PlatformRow, ReceiptRow, ResultRow};
use crate::{StoreError, StoreResult};

struct BlobParts {
    kind: String,
    length: i64,
    bytes: Vec<u8>,
}

fn require_blob(parts: BlobParts) -> StoreResult<Vec<u8>> {
    let bounded = usize::try_from(parts.length)
        .ok()
        .is_some_and(|value| (1..=super::MAX_JSON_BYTES).contains(&value));
    if parts.kind == "blob"
        && bounded
        && i64::try_from(parts.bytes.len()).ok() == Some(parts.length)
    {
        Ok(parts.bytes)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn blob_at(row: &Row, index: usize) -> rusqlite::Result<BlobParts> {
    Ok(BlobParts {
        kind: row.get(index)?,
        length: row.get(index + 1)?,
        bytes: row.get(index + 2)?,
    })
}

pub(super) struct PlatformRaw {
    id: String,
    policy_id: String,
    projection_id: String,
    target_id: String,
    status: String,
    reason: String,
    blob: BlobParts,
}

pub(super) fn map_platform(row: &Row) -> rusqlite::Result<PlatformRaw> {
    Ok(PlatformRaw {
        id: row.get(0)?,
        policy_id: row.get(1)?,
        projection_id: row.get(2)?,
        target_id: row.get(3)?,
        status: row.get(4)?,
        reason: row.get(5)?,
        blob: blob_at(row, 6)?,
    })
}

pub(super) fn finish_platform(raw: PlatformRaw) -> StoreResult<PlatformRow> {
    Ok(PlatformRow {
        id: raw.id,
        policy_id: raw.policy_id,
        projection_id: raw.projection_id,
        target_id: raw.target_id,
        status: raw.status,
        reason: raw.reason,
        bytes: require_blob(raw.blob)?,
    })
}

pub(super) struct LicenseRaw {
    id: String,
    policy_id: String,
    projection_id: String,
    target_id: String,
    permission: String,
    decision: String,
    reason: String,
    blob: BlobParts,
}

pub(super) fn map_license(row: &Row) -> rusqlite::Result<LicenseRaw> {
    Ok(LicenseRaw {
        id: row.get(0)?,
        policy_id: row.get(1)?,
        projection_id: row.get(2)?,
        target_id: row.get(3)?,
        permission: row.get(4)?,
        decision: row.get(5)?,
        reason: row.get(6)?,
        blob: blob_at(row, 7)?,
    })
}

pub(super) fn finish_license(raw: LicenseRaw) -> StoreResult<LicenseRow> {
    Ok(LicenseRow {
        id: raw.id,
        policy_id: raw.policy_id,
        projection_id: raw.projection_id,
        target_id: raw.target_id,
        permission: raw.permission,
        decision: raw.decision,
        reason: raw.reason,
        bytes: require_blob(raw.blob)?,
    })
}

pub(super) struct PhaseRaw {
    id: String,
    system_id: String,
    plan_id: String,
    suite_id: String,
    evidence_item_count: i64,
    status: String,
    blob: BlobParts,
}

pub(super) fn map_phase_manifest(row: &Row) -> rusqlite::Result<PhaseRaw> {
    Ok(PhaseRaw {
        id: row.get(0)?,
        system_id: row.get(1)?,
        plan_id: row.get(2)?,
        suite_id: row.get(3)?,
        evidence_item_count: row.get(4)?,
        status: row.get(5)?,
        blob: blob_at(row, 6)?,
    })
}

pub(super) fn finish_phase_manifest(raw: PhaseRaw) -> StoreResult<PhaseManifestRow> {
    Ok(PhaseManifestRow {
        id: raw.id,
        system_id: raw.system_id,
        plan_id: raw.plan_id,
        suite_id: raw.suite_id,
        evidence_item_count: raw.evidence_item_count,
        status: raw.status,
        bytes: require_blob(raw.blob)?,
    })
}

pub(super) struct ResultRaw {
    id: String,
    system_id: String,
    plan_id: String,
    suite_id: String,
    repetition_id: String,
    ledger_id: String,
    terminal_stage: String,
    receipt_set_id: Option<String>,
    deterministic_id: Option<String>,
    judge_join_id: Option<String>,
    blob: BlobParts,
}

pub(super) fn map_result(row: &Row) -> rusqlite::Result<ResultRaw> {
    Ok(ResultRaw {
        id: row.get(0)?,
        system_id: row.get(1)?,
        plan_id: row.get(2)?,
        suite_id: row.get(3)?,
        repetition_id: row.get(4)?,
        ledger_id: row.get(5)?,
        terminal_stage: row.get(6)?,
        receipt_set_id: row.get(7)?,
        deterministic_id: row.get(8)?,
        judge_join_id: row.get(9)?,
        blob: blob_at(row, 10)?,
    })
}

pub(super) fn finish_result(raw: ResultRaw) -> StoreResult<ResultRow> {
    Ok(ResultRow {
        id: raw.id,
        system_id: raw.system_id,
        plan_id: raw.plan_id,
        suite_id: raw.suite_id,
        repetition_id: raw.repetition_id,
        ledger_id: raw.ledger_id,
        terminal_stage: raw.terminal_stage,
        receipt_set_id: raw.receipt_set_id,
        deterministic_id: raw.deterministic_id,
        judge_join_id: raw.judge_join_id,
        bytes: require_blob(raw.blob)?,
    })
}

pub(super) struct ReceiptRaw {
    id: String,
    policy_id: String,
    projection_id: String,
    plan_id: String,
    suite_id: String,
    target_id: String,
    baseline_id: String,
    platform_id: String,
    license_id: String,
    ledger_id: String,
    repeatability_id: String,
    resource_id: String,
    human_id: String,
    elapsed_nanoseconds: i64,
    peak_concurrent_attempts: i64,
    terminal_status: String,
    finalization_status: String,
    blob: BlobParts,
}

pub(super) fn map_receipt(row: &Row) -> rusqlite::Result<ReceiptRaw> {
    Ok(ReceiptRaw {
        id: row.get(0)?,
        policy_id: row.get(1)?,
        projection_id: row.get(2)?,
        plan_id: row.get(3)?,
        suite_id: row.get(4)?,
        target_id: row.get(5)?,
        baseline_id: row.get(6)?,
        platform_id: row.get(7)?,
        license_id: row.get(8)?,
        ledger_id: row.get(9)?,
        repeatability_id: row.get(10)?,
        resource_id: row.get(11)?,
        human_id: row.get(12)?,
        elapsed_nanoseconds: row.get(13)?,
        peak_concurrent_attempts: row.get(14)?,
        terminal_status: row.get(15)?,
        finalization_status: row.get(16)?,
        blob: blob_at(row, 17)?,
    })
}

pub(super) fn finish_receipt(raw: ReceiptRaw) -> StoreResult<ReceiptRow> {
    Ok(ReceiptRow {
        id: raw.id,
        policy_id: raw.policy_id,
        projection_id: raw.projection_id,
        plan_id: raw.plan_id,
        suite_id: raw.suite_id,
        target_id: raw.target_id,
        baseline_id: raw.baseline_id,
        platform_id: raw.platform_id,
        license_id: raw.license_id,
        ledger_id: raw.ledger_id,
        repeatability_id: raw.repeatability_id,
        resource_id: raw.resource_id,
        human_id: raw.human_id,
        elapsed_nanoseconds: raw.elapsed_nanoseconds,
        peak_concurrent_attempts: raw.peak_concurrent_attempts,
        terminal_status: raw.terminal_status,
        finalization_status: raw.finalization_status,
        bytes: require_blob(raw.blob)?,
    })
}

pub(super) struct InterruptionRaw {
    id: String,
    receipt_id: String,
    policy_id: String,
    target_id: String,
    plan_id: String,
    suite_id: String,
    phase: String,
    checkpoint: String,
    planned_attempt_id: Option<String>,
    reason: String,
    terminal_status: String,
    blob: BlobParts,
}

pub(super) fn map_interruption(row: &Row) -> rusqlite::Result<InterruptionRaw> {
    Ok(InterruptionRaw {
        id: row.get(0)?,
        receipt_id: row.get(1)?,
        policy_id: row.get(2)?,
        target_id: row.get(3)?,
        plan_id: row.get(4)?,
        suite_id: row.get(5)?,
        phase: row.get(6)?,
        checkpoint: row.get(7)?,
        planned_attempt_id: row.get(8)?,
        reason: row.get(9)?,
        terminal_status: row.get(10)?,
        blob: blob_at(row, 11)?,
    })
}

pub(super) fn finish_interruption(raw: InterruptionRaw) -> StoreResult<InterruptionRow> {
    Ok(InterruptionRow {
        id: raw.id,
        receipt_id: raw.receipt_id,
        policy_id: raw.policy_id,
        target_id: raw.target_id,
        plan_id: raw.plan_id,
        suite_id: raw.suite_id,
        phase: raw.phase,
        checkpoint: raw.checkpoint,
        planned_attempt_id: raw.planned_attempt_id,
        reason: raw.reason,
        terminal_status: raw.terminal_status,
        bytes: require_blob(raw.blob)?,
    })
}

pub(super) const PLATFORM_SQL: &str = "SELECT generation_qualification_platform_evidence_id,
       operation_policy_id, request_projection_id, target_generation_system_id,
       status, reason, typeof(canonical_json), length(canonical_json),
       CAST(substr(canonical_json, 1, 16384) AS BLOB)
FROM generation_qualification_platform_evidence
WHERE operation_policy_id = ?1";

pub(super) const LICENSE_SQL: &str = "SELECT generation_qualification_license_evidence_id,
       operation_policy_id, request_projection_id, target_generation_system_id,
       permission, decision, reason, typeof(canonical_json), length(canonical_json),
       CAST(substr(canonical_json, 1, 16384) AS BLOB)
FROM generation_qualification_license_evidence
WHERE operation_policy_id = ?1";

pub(super) const RESULT_SQL: &str =
    "SELECT generation_repeatability_result_id, generation_system_id,
       generation_qualification_plan_id, generation_suite_manifest_id, generation_repetition_id,
       generation_attempt_ledger_manifest_id, terminal_stage,
       candidate_generation_receipt_set_id, candidate_deterministic_evaluation_id,
       candidate_judge_join_id, typeof(canonical_json), length(canonical_json),
       CAST(substr(canonical_json, 1, 16384) AS BLOB)
FROM generation_repeatability_result_records
WHERE generation_qualification_plan_id = ?1 AND generation_repetition_id = ?2";

pub(super) const RESULT_LIST_SQL: &str =
    "SELECT generation_repeatability_result_id, generation_system_id,
       generation_qualification_plan_id, generation_suite_manifest_id, generation_repetition_id,
       generation_attempt_ledger_manifest_id, terminal_stage,
       candidate_generation_receipt_set_id, candidate_deterministic_evaluation_id,
       candidate_judge_join_id, typeof(canonical_json), length(canonical_json),
       CAST(substr(canonical_json, 1, 16384) AS BLOB)
FROM generation_repeatability_result_records
WHERE generation_qualification_plan_id = ?1 AND generation_system_id = ?2";

pub(super) const RECEIPT_SQL: &str = "SELECT generation_qualification_operation_receipt_id,
       operation_policy_id, request_projection_id, generation_qualification_plan_id,
       generation_suite_manifest_id, target_generation_system_id, baseline_generation_system_id,
       generation_qualification_platform_evidence_id, generation_qualification_license_evidence_id,
       generation_attempt_ledger_manifest_id, generation_repeatability_evidence_manifest_id,
       generation_resource_evidence_manifest_id, generation_human_adjudication_evidence_manifest_id,
       elapsed_nanoseconds, peak_concurrent_attempts, terminal_status, finalization_status,
       typeof(canonical_json), length(canonical_json),
       CAST(substr(canonical_json, 1, 16384) AS BLOB)
FROM generation_qualification_operation_receipts
WHERE operation_policy_id = ?1";

pub(super) const INTERRUPTION_SQL: &str =
    "SELECT generation_qualification_phase_interruption_record_id,
       generation_qualification_operation_receipt_id, operation_policy_id,
       target_generation_system_id, generation_qualification_plan_id,
       generation_suite_manifest_id, phase, checkpoint, planned_candidate_attempt_id, reason,
       terminal_status, typeof(canonical_json), length(canonical_json),
       CAST(substr(canonical_json, 1, 16384) AS BLOB)
FROM generation_qualification_phase_interruption_records
WHERE operation_policy_id = ?1";
