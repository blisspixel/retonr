use rewrite_model::{
    GenerationAttemptLedgerManifestV1, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationQualificationLicenseEvidenceV1, GenerationQualificationOperationReceiptV1,
    GenerationQualificationPhaseInterruptionRecordV1, GenerationQualificationPhaseStatusV1,
    GenerationQualificationPlatformEvidenceV1, GenerationRepeatabilityEvidenceManifestV1,
    GenerationRepeatabilityResultRecordV1, GenerationResourceEvidenceManifestV1,
};
use rewrite_types::Digest;

use super::super::rows::{
    InterruptionRow, LicenseRow, PhaseManifestRow, PlatformRow, ReceiptRow, ResultRow,
};
use super::super::text;
use crate::{StoreError, StoreResult};

#[derive(Clone, Copy)]
pub(super) struct PhaseCheck<'a> {
    pub(super) id: &'a Digest,
    pub(super) system: &'a Digest,
    pub(super) plan: &'a Digest,
    pub(super) suite: &'a Digest,
    pub(super) count: u32,
    pub(super) status: GenerationQualificationPhaseStatusV1,
}

pub(super) fn platform(
    row: &PlatformRow,
    value: GenerationQualificationPlatformEvidenceV1,
) -> StoreResult<GenerationQualificationPlatformEvidenceV1> {
    same(&row.id, value.platform_evidence_id().digest())?;
    same(&row.policy_id, value.operation_policy_id().digest())?;
    same(&row.projection_id, value.request_projection_id().digest())?;
    same(&row.target_id, value.target_generation_system_id().digest())?;
    same_text(&row.status, text::platform_status(value.status()))?;
    same_text(&row.reason, text::platform_reason(value.reason()))?;
    Ok(value)
}

pub(super) fn license(
    row: &LicenseRow,
    value: GenerationQualificationLicenseEvidenceV1,
) -> StoreResult<GenerationQualificationLicenseEvidenceV1> {
    same(&row.id, value.license_evidence_id().digest())?;
    same(&row.policy_id, value.operation_policy_id().digest())?;
    same(&row.projection_id, value.request_projection_id().digest())?;
    same(&row.target_id, value.target_generation_system_id().digest())?;
    same_text(
        &row.permission,
        text::license_permission(value.permission()),
    )?;
    same_text(&row.decision, text::license_decision(value.decision()))?;
    same_text(&row.reason, text::license_reason(value.reason()))?;
    Ok(value)
}

pub(super) fn phase(row: &PhaseManifestRow, expected: PhaseCheck<'_>) -> StoreResult<()> {
    same(&row.id, expected.id)?;
    same(&row.system_id, expected.system)?;
    same(&row.plan_id, expected.plan)?;
    same(&row.suite_id, expected.suite)?;
    if row.evidence_item_count == i64::from(expected.count) {
        same_text(&row.status, text::phase_status(expected.status))
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(super) fn result(
    row: &ResultRow,
    value: GenerationRepeatabilityResultRecordV1,
) -> StoreResult<GenerationRepeatabilityResultRecordV1> {
    same(&row.id, value.repeatability_result_id().digest())?;
    same(&row.system_id, value.generation_system_id().digest())?;
    same(
        &row.plan_id,
        value.generation_qualification_plan_id().digest(),
    )?;
    same(&row.suite_id, value.suite_manifest_id().digest())?;
    same(&row.repetition_id, value.repetition_id().digest())?;
    same(&row.ledger_id, value.attempt_ledger_manifest_id().digest())?;
    same_text(
        &row.terminal_stage,
        text::repeatability_stage(value.terminal_stage())?,
    )?;
    if row.receipt_set_id.is_none() && row.deterministic_id.is_none() && row.judge_join_id.is_none()
    {
        Ok(value)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

pub(super) fn receipt(
    row: &ReceiptRow,
    value: GenerationQualificationOperationReceiptV1,
    ledger: &GenerationAttemptLedgerManifestV1,
    repeatability: &GenerationRepeatabilityEvidenceManifestV1,
    resource: &GenerationResourceEvidenceManifestV1,
    human: &GenerationHumanAdjudicationEvidenceManifestV1,
) -> StoreResult<GenerationQualificationOperationReceiptV1> {
    same(&row.id, value.operation_receipt_id().digest())?;
    same(&row.policy_id, value.operation_policy_id().digest())?;
    same(&row.projection_id, value.request_projection_id().digest())?;
    same(
        &row.plan_id,
        ledger.generation_qualification_plan_id().digest(),
    )?;
    same(&row.suite_id, ledger.suite_manifest_id().digest())?;
    same(&row.target_id, value.target_generation_system_id().digest())?;
    same(
        &row.baseline_id,
        value.baseline_generation_system_id().digest(),
    )?;
    same(&row.platform_id, value.platform_evidence_id().digest())?;
    same(&row.license_id, value.license_evidence_id().digest())?;
    same(&row.ledger_id, ledger.attempt_ledger_manifest_id().digest())?;
    same(
        &row.repeatability_id,
        repeatability.repeatability_evidence_manifest_id().digest(),
    )?;
    same(
        &row.resource_id,
        resource.resource_evidence_manifest_id().digest(),
    )?;
    same(
        &row.human_id,
        human.human_adjudication_evidence_manifest_id().digest(),
    )?;
    let elapsed =
        i64::try_from(value.elapsed_nanoseconds()).map_err(|_| StoreError::CorruptRecord)?;
    if row.elapsed_nanoseconds != elapsed
        || row.peak_concurrent_attempts != i64::from(value.peak_concurrent_attempts())
    {
        return Err(StoreError::CorruptRecord);
    }
    same_text(
        &row.terminal_status,
        text::terminal_status(value.terminal_status()),
    )?;
    same_text(
        &row.finalization_status,
        text::finalization_status(value.finalization_status()),
    )?;
    Ok(value)
}

pub(super) fn interruption(
    row: &InterruptionRow,
    value: GenerationQualificationPhaseInterruptionRecordV1,
    receipt: &GenerationQualificationOperationReceiptV1,
) -> StoreResult<GenerationQualificationPhaseInterruptionRecordV1> {
    same(&row.id, value.phase_interruption_record_id().digest())?;
    same(&row.receipt_id, value.operation_receipt_id().digest())?;
    same(&row.policy_id, value.operation_policy_id().digest())?;
    same(&row.target_id, value.target_generation_system_id().digest())?;
    same(
        &row.plan_id,
        value.generation_qualification_plan_id().digest(),
    )?;
    same(&row.suite_id, value.suite_manifest_id().digest())?;
    same_text(&row.phase, text::interrupted_phase(value.phase()))?;
    same_text(&row.checkpoint, text::checkpoint(value.checkpoint()))?;
    same_text(&row.reason, text::interruption_reason(value.reason()))?;
    same_text(
        &row.terminal_status,
        text::terminal_status(receipt.terminal_status()),
    )?;
    let planned = value
        .planned_attempt_id()
        .map(|planned| planned.digest().as_str());
    if row.planned_attempt_id.as_deref() == planned {
        Ok(value)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn same(actual: &str, expected: &Digest) -> StoreResult<()> {
    same_text(actual, expected.as_str())
}

fn same_text(actual: &str, expected: &str) -> StoreResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}
