use rewrite_model::{
    GenerationAttemptLedgerManifestV1, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationRepeatabilityEvidenceManifestV1, GenerationRepeatabilityResultRecordV1,
    GenerationResourceEvidenceManifestV1,
};
use rusqlite::{Connection, params};

use super::GenerationQualificationTerminalEvidenceV1WriteDisposition;
use super::codec::PreparedCohort;
use super::text;

mod confirm;
use crate::{StoreError, StoreResult, WriteDisposition};

#[derive(Clone, Copy)]
struct PhaseInsert<'a> {
    table: &'a str,
    identifier: &'a str,
    id: &'a str,
    system_id: &'a str,
    plan_id: &'a str,
    suite_id: &'a str,
    count: i64,
    status: &'a str,
    bytes: &'a [u8],
}

pub(super) fn insert_cohort(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<GenerationQualificationTerminalEvidenceV1WriteDisposition> {
    let elapsed = i64::try_from(prepared.records.receipt.elapsed_nanoseconds())
        .map_err(|_| StoreError::CorruptRecord)?;
    if prepared.records.results.len() != prepared.encoded.results.len()
        || prepared.records.interruption.is_some() != prepared.encoded.interruption.is_some()
    {
        return Err(StoreError::CorruptRecord);
    }
    let disposition = write_rows(connection, prepared, elapsed)?;
    confirm::stored(connection, prepared)?;
    Ok(disposition)
}

fn write_rows(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
    elapsed: i64,
) -> StoreResult<GenerationQualificationTerminalEvidenceV1WriteDisposition> {
    let mut written = Vec::new();
    written.push(insert_platform(connection, prepared)?);
    written.push(insert_license(connection, prepared)?);
    written.push(insert_ledger(connection, prepared)?);
    let results = insert_results(connection, prepared)?;
    written.extend(results.iter().copied());
    written.push(insert_repeatability(connection, prepared)?);
    written.push(insert_resource(connection, prepared)?);
    written.push(insert_human(connection, prepared)?);
    written.push(insert_receipt(connection, prepared, elapsed)?);
    let interruption = insert_interruption(connection, prepared)?;
    if let Some(value) = interruption {
        written.push(value);
    }
    let disposition = require_uniform(&written)?;
    Ok(GenerationQualificationTerminalEvidenceV1WriteDisposition {
        platform: disposition,
        license: disposition,
        attempt_ledger: disposition,
        repeatability_results: results.first().copied(),
        repeatability_manifest: disposition,
        resource: disposition,
        human: disposition,
        receipt: disposition,
        phase_interruption: interruption,
    })
}

fn insert_platform(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let value = &prepared.records.platform;
    insert(
        connection,
        "INSERT OR IGNORE INTO generation_qualification_platform_evidence (
            generation_qualification_platform_evidence_id, operation_policy_id,
            request_projection_id, target_generation_system_id, status, reason, canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            value.platform_evidence_id().digest().as_str(),
            value.operation_policy_id().digest().as_str(),
            value.request_projection_id().digest().as_str(),
            value.target_generation_system_id().digest().as_str(),
            text::platform_status(value.status()),
            text::platform_reason(value.reason()),
            &prepared.encoded.platform,
        ],
    )
}

fn insert_license(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    let value = &prepared.records.license;
    insert(
        connection,
        "INSERT OR IGNORE INTO generation_qualification_license_evidence (
            generation_qualification_license_evidence_id, operation_policy_id,
            request_projection_id, target_generation_system_id, permission, decision, reason,
            canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            value.license_evidence_id().digest().as_str(),
            value.operation_policy_id().digest().as_str(),
            value.request_projection_id().digest().as_str(),
            value.target_generation_system_id().digest().as_str(),
            text::license_permission(value.permission()),
            text::license_decision(value.decision()),
            text::license_reason(value.reason()),
            &prepared.encoded.license,
        ],
    )
}

fn insert_ledger(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    phase_insert(
        connection,
        &prepared.records.ledger,
        "generation_attempt_ledger_manifests",
        "generation_attempt_ledger_manifest_id",
        prepared
            .records
            .ledger
            .attempt_ledger_manifest_id()
            .digest()
            .as_str(),
        &prepared.encoded.ledger,
    )
}

fn insert_repeatability(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    phase_insert(
        connection,
        &prepared.records.repeatability,
        "generation_repeatability_evidence_manifests",
        "generation_repeatability_evidence_manifest_id",
        prepared
            .records
            .repeatability
            .repeatability_evidence_manifest_id()
            .digest()
            .as_str(),
        &prepared.encoded.repeatability,
    )
}

fn insert_resource(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    phase_insert(
        connection,
        &prepared.records.resource,
        "generation_resource_evidence_manifests",
        "generation_resource_evidence_manifest_id",
        prepared
            .records
            .resource
            .resource_evidence_manifest_id()
            .digest()
            .as_str(),
        &prepared.encoded.resource,
    )
}

fn insert_human(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<WriteDisposition> {
    phase_insert(
        connection,
        &prepared.records.human,
        "generation_human_adjudication_evidence_manifests",
        "generation_human_adjudication_evidence_manifest_id",
        prepared
            .records
            .human
            .human_adjudication_evidence_manifest_id()
            .digest()
            .as_str(),
        &prepared.encoded.human,
    )
}

fn phase_insert(
    connection: &Connection,
    manifest: &impl PhaseManifest,
    table: &str,
    identifier: &str,
    id: &str,
    bytes: &[u8],
) -> StoreResult<WriteDisposition> {
    insert_phase(
        connection,
        PhaseInsert {
            table,
            identifier,
            id,
            system_id: manifest.system_id(),
            plan_id: manifest.plan_id(),
            suite_id: manifest.suite_id(),
            count: i64::from(manifest.item_count()),
            status: text::phase_status(manifest.phase_status()),
            bytes,
        },
    )
}

trait PhaseManifest {
    fn system_id(&self) -> &str;
    fn plan_id(&self) -> &str;
    fn suite_id(&self) -> &str;
    fn item_count(&self) -> u32;
    fn phase_status(&self) -> rewrite_model::GenerationQualificationPhaseStatusV1;
}

impl PhaseManifest for GenerationAttemptLedgerManifestV1 {
    fn system_id(&self) -> &str {
        self.generation_system_id().digest().as_str()
    }
    fn plan_id(&self) -> &str {
        self.generation_qualification_plan_id().digest().as_str()
    }
    fn suite_id(&self) -> &str {
        self.suite_manifest_id().digest().as_str()
    }
    fn item_count(&self) -> u32 {
        self.evidence_item_count()
    }
    fn phase_status(&self) -> rewrite_model::GenerationQualificationPhaseStatusV1 {
        self.status()
    }
}

impl PhaseManifest for GenerationRepeatabilityEvidenceManifestV1 {
    fn system_id(&self) -> &str {
        self.generation_system_id().digest().as_str()
    }
    fn plan_id(&self) -> &str {
        self.generation_qualification_plan_id().digest().as_str()
    }
    fn suite_id(&self) -> &str {
        self.suite_manifest_id().digest().as_str()
    }
    fn item_count(&self) -> u32 {
        self.evidence_item_count()
    }
    fn phase_status(&self) -> rewrite_model::GenerationQualificationPhaseStatusV1 {
        self.status()
    }
}

impl PhaseManifest for GenerationResourceEvidenceManifestV1 {
    fn system_id(&self) -> &str {
        self.generation_system_id().digest().as_str()
    }
    fn plan_id(&self) -> &str {
        self.generation_qualification_plan_id().digest().as_str()
    }
    fn suite_id(&self) -> &str {
        self.suite_manifest_id().digest().as_str()
    }
    fn item_count(&self) -> u32 {
        self.evidence_item_count()
    }
    fn phase_status(&self) -> rewrite_model::GenerationQualificationPhaseStatusV1 {
        self.status()
    }
}

impl PhaseManifest for GenerationHumanAdjudicationEvidenceManifestV1 {
    fn system_id(&self) -> &str {
        self.generation_system_id().digest().as_str()
    }
    fn plan_id(&self) -> &str {
        self.generation_qualification_plan_id().digest().as_str()
    }
    fn suite_id(&self) -> &str {
        self.suite_manifest_id().digest().as_str()
    }
    fn item_count(&self) -> u32 {
        self.evidence_item_count()
    }
    fn phase_status(&self) -> rewrite_model::GenerationQualificationPhaseStatusV1 {
        self.status()
    }
}

fn insert_results(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<Vec<WriteDisposition>> {
    let mut dispositions = Vec::with_capacity(prepared.records.results.len());
    for (result, bytes) in prepared
        .records
        .results
        .iter()
        .zip(&prepared.encoded.results)
    {
        dispositions.push(insert_result(connection, result, bytes)?);
    }
    Ok(dispositions)
}

fn insert_result(
    connection: &Connection,
    value: &GenerationRepeatabilityResultRecordV1,
    bytes: &[u8],
) -> StoreResult<WriteDisposition> {
    insert(
        connection,
        "INSERT OR IGNORE INTO generation_repeatability_result_records (
            generation_repeatability_result_id, generation_system_id,
            generation_qualification_plan_id, generation_suite_manifest_id,
            generation_repetition_id, generation_attempt_ledger_manifest_id, terminal_stage,
            candidate_generation_receipt_set_id, candidate_deterministic_evaluation_id,
            candidate_judge_join_id, canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, NULL, ?8)",
        params![
            value.repeatability_result_id().digest().as_str(),
            value.generation_system_id().digest().as_str(),
            value.generation_qualification_plan_id().digest().as_str(),
            value.suite_manifest_id().digest().as_str(),
            value.repetition_id().digest().as_str(),
            value.attempt_ledger_manifest_id().digest().as_str(),
            text::repeatability_stage(value.terminal_stage())?,
            bytes,
        ],
    )
}

fn insert_receipt(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
    elapsed: i64,
) -> StoreResult<WriteDisposition> {
    let receipt = &prepared.records.receipt;
    let policy = prepared.preregistration.operation_policy();
    let (ledger_id, _) = receipt.attempt_ledger_manifest();
    let (repeatability_id, _) = receipt.repeatability_evidence_manifest();
    let (resource_id, _) = receipt.resource_evidence_manifest();
    let (human_id, _) = receipt.human_adjudication_evidence_manifest();
    insert(
        connection,
        "INSERT OR IGNORE INTO generation_qualification_operation_receipts (
            generation_qualification_operation_receipt_id, operation_policy_id,
            request_projection_id, generation_qualification_plan_id, generation_suite_manifest_id,
            target_generation_system_id, baseline_generation_system_id,
            generation_qualification_platform_evidence_id,
            generation_qualification_license_evidence_id, generation_attempt_ledger_manifest_id,
            generation_repeatability_evidence_manifest_id, generation_resource_evidence_manifest_id,
            generation_human_adjudication_evidence_manifest_id, elapsed_nanoseconds,
            peak_concurrent_attempts, terminal_status, finalization_status, canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            receipt.operation_receipt_id().digest().as_str(),
            receipt.operation_policy_id().digest().as_str(),
            receipt.request_projection_id().digest().as_str(),
            policy.generation_qualification_plan_id().digest().as_str(),
            policy.suite_manifest_id().digest().as_str(),
            receipt.target_generation_system_id().digest().as_str(),
            receipt.baseline_generation_system_id().digest().as_str(),
            receipt.platform_evidence_id().digest().as_str(),
            receipt.license_evidence_id().digest().as_str(),
            ledger_id.digest().as_str(),
            repeatability_id.digest().as_str(),
            resource_id.digest().as_str(),
            human_id.digest().as_str(),
            elapsed,
            i64::from(receipt.peak_concurrent_attempts()),
            text::terminal_status(receipt.terminal_status()),
            text::finalization_status(receipt.finalization_status()),
            &prepared.encoded.receipt,
        ],
    )
}

fn insert_interruption(
    connection: &Connection,
    prepared: &PreparedCohort<'_>,
) -> StoreResult<Option<WriteDisposition>> {
    let (Some(value), Some(bytes)) = (
        prepared.records.interruption.as_ref(),
        prepared.encoded.interruption.as_ref(),
    ) else {
        return Ok(None);
    };
    let planned = value
        .planned_attempt_id()
        .map(|planned| planned.digest().as_str().to_owned());
    let receipt = &prepared.records.receipt;
    insert(
        connection,
        "INSERT OR IGNORE INTO generation_qualification_phase_interruption_records (
            generation_qualification_phase_interruption_record_id,
            generation_qualification_operation_receipt_id, operation_policy_id,
            target_generation_system_id, generation_qualification_plan_id,
            generation_suite_manifest_id, phase, checkpoint, planned_candidate_attempt_id, reason,
            terminal_status, canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            value.phase_interruption_record_id().digest().as_str(),
            value.operation_receipt_id().digest().as_str(),
            value.operation_policy_id().digest().as_str(),
            value.target_generation_system_id().digest().as_str(),
            value.generation_qualification_plan_id().digest().as_str(),
            value.suite_manifest_id().digest().as_str(),
            text::interrupted_phase(value.phase()),
            text::checkpoint(value.checkpoint()),
            planned,
            text::interruption_reason(value.reason()),
            text::terminal_status(receipt.terminal_status()),
            bytes,
        ],
    )
    .map(Some)
}

fn insert_phase(connection: &Connection, value: PhaseInsert<'_>) -> StoreResult<WriteDisposition> {
    let sql = format!(
        "INSERT OR IGNORE INTO {table} (
            {identifier}, generation_system_id, generation_qualification_plan_id,
            generation_suite_manifest_id, evidence_item_count, status, canonical_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        table = value.table,
        identifier = value.identifier,
    );
    insert(
        connection,
        &sql,
        params![
            value.id,
            value.system_id,
            value.plan_id,
            value.suite_id,
            value.count,
            value.status,
            value.bytes,
        ],
    )
}

fn insert(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> StoreResult<WriteDisposition> {
    match connection.execute(sql, parameters)? {
        1 => Ok(WriteDisposition::Inserted),
        0 => Ok(WriteDisposition::AlreadyPresent),
        _ => Err(StoreError::CorruptRecord),
    }
}

fn require_uniform(values: &[WriteDisposition]) -> StoreResult<WriteDisposition> {
    let first = values.first().copied().ok_or(StoreError::CorruptRecord)?;
    if values.iter().all(|value| *value == first) {
        Ok(first)
    } else {
        Err(StoreError::ImmutableConflict)
    }
}
