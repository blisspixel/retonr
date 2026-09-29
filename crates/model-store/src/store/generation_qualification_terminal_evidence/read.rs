use rewrite_model::{
    GenerationAttemptLedgerManifestV1, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationLicenseEvidenceV1, GenerationQualificationLicenseEvidenceV1Relations,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationPhaseInterruptionRecordV1,
    GenerationQualificationPhaseInterruptionRecordV1Relations,
    GenerationQualificationPhaseStatusV1, GenerationQualificationPlatformEvidenceV1,
    GenerationRepeatabilityEvidenceManifestV1, GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityResultRecordV1Relations,
    GenerationRepeatabilityTerminalStageV1, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations,
};
use rewrite_types::Digest;
use rusqlite::Connection;
use serde::Deserialize;

use super::context::{self, Anchor};
use super::rows::{self, ResultRow};
use super::{
    GenerationQualificationTerminalEvidenceV1ReadInput,
    StoredGenerationQualificationTerminalEvidenceV1,
};
use crate::{StoreError, StoreResult};

mod check;

struct Keys<'a> {
    policy: &'a str,
    plan: &'a str,
    system: &'a str,
}

struct Evidence {
    platform: GenerationQualificationPlatformEvidenceV1,
    license: GenerationQualificationLicenseEvidenceV1,
    ledger: GenerationAttemptLedgerManifestV1,
    results: Vec<GenerationRepeatabilityResultRecordV1>,
    repeatability: GenerationRepeatabilityEvidenceManifestV1,
    resource: GenerationResourceEvidenceManifestV1,
    human: GenerationHumanAdjudicationEvidenceManifestV1,
}

struct Closure {
    receipt: GenerationQualificationOperationReceiptV1,
    interruption: Option<GenerationQualificationPhaseInterruptionRecordV1>,
}

pub(super) fn load(
    connection: &Connection,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
) -> StoreResult<Option<StoredGenerationQualificationTerminalEvidenceV1>> {
    let before = connection.total_changes();
    let stored = load_cohort(connection, input)?;
    if connection.total_changes() == before {
        Ok(stored)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn load_cohort(
    connection: &Connection,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
) -> StoreResult<Option<StoredGenerationQualificationTerminalEvidenceV1>> {
    let anchor = context::load_anchor(
        connection,
        input.preregistration,
        input.managed_evidence_inputs,
    )?;
    if rows::cohort_absent(connection, &anchor)? {
        return Ok(None);
    }
    let keys = keys(&anchor)?;
    let evidence = decode_evidence(connection, &anchor, input, &keys)?;
    let closure = decode_closure(connection, &anchor, input, &keys, &evidence)?;
    Ok(Some(StoredGenerationQualificationTerminalEvidenceV1 {
        preregistration: anchor.preregistration,
        platform_evidence: evidence.platform,
        license_evidence: evidence.license,
        attempt_ledger_manifest: evidence.ledger,
        repeatability_results: evidence.results,
        repeatability_manifest: evidence.repeatability,
        resource_manifest: evidence.resource,
        human_manifest: evidence.human,
        receipt: closure.receipt,
        phase_interruption: closure.interruption,
    }))
}

fn keys<'a>(anchor: &'a Anchor<'_>) -> StoreResult<Keys<'a>> {
    let policy = anchor.preregistration.operation_policy();
    let foundation = anchor.preregistration.plan_foundation();
    let system = foundation
        .generation_systems()
        .get(anchor.target_index)
        .ok_or(StoreError::CorruptRecord)?;
    Ok(Keys {
        policy: policy.operation_policy_id().digest().as_str(),
        plan: foundation.plan().qualification_plan_id().digest().as_str(),
        system: system.generation_system_id().digest().as_str(),
    })
}

fn decode_evidence(
    connection: &Connection,
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
    keys: &Keys<'_>,
) -> StoreResult<Evidence> {
    let platform = decode_platform(connection, anchor, input, keys)?;
    let license = decode_license(connection, anchor, input, keys)?;
    let ledger = decode_ledger(connection, anchor, keys)?;
    let results = decode_results(connection, anchor, keys)?;
    let repeatability = decode_repeatability(connection, anchor, &results, keys)?;
    let resource = decode_resource(connection, anchor, input, keys)?;
    let human = decode_human(connection, anchor, input, keys)?;
    Ok(Evidence {
        platform,
        license,
        ledger,
        results,
        repeatability,
        resource,
        human,
    })
}

fn decode_platform(
    connection: &Connection,
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
    keys: &Keys<'_>,
) -> StoreResult<GenerationQualificationPlatformEvidenceV1> {
    let row = rows::load_platform(connection, keys.policy)?.ok_or(StoreError::CorruptRecord)?;
    let value = GenerationQualificationPlatformEvidenceV1::from_json_bytes(
        &row.bytes,
        context::platform_relations(anchor)?,
        input.platform_input,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    check::platform(&row, value)
}

fn decode_license(
    connection: &Connection,
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
    keys: &Keys<'_>,
) -> StoreResult<GenerationQualificationLicenseEvidenceV1> {
    let row = rows::load_license(connection, keys.policy)?.ok_or(StoreError::CorruptRecord)?;
    let scope = context::scope(anchor)?;
    let value = GenerationQualificationLicenseEvidenceV1::from_json_bytes(
        &row.bytes,
        GenerationQualificationLicenseEvidenceV1Relations {
            operation_policy: anchor.preregistration.operation_policy(),
            request_projection: anchor.preregistration.request_projection(),
            target_generation_system: scope.generation_system,
            target_generation_system_relations: anchor.system_relations,
            model_package_foundation_id: input.model_package_foundation_id,
            model_license_control_id: input.model_license_control_id,
        },
        input.license_input,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    check::license(&row, value)
}

fn decode_ledger(
    connection: &Connection,
    anchor: &Anchor<'_>,
    keys: &Keys<'_>,
) -> StoreResult<GenerationAttemptLedgerManifestV1> {
    let row = load_manifest(
        connection,
        "generation_attempt_ledger_manifests",
        "generation_attempt_ledger_manifest_id",
        keys,
    )?;
    let value = GenerationAttemptLedgerManifestV1::from_json_bytes(
        &row.bytes,
        context::ledger_relations(anchor)?,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    check::phase(&row, phase_check_ledger(&value))?;
    Ok(value)
}

fn decode_results(
    connection: &Connection,
    anchor: &Anchor<'_>,
    keys: &Keys<'_>,
) -> StoreResult<Vec<GenerationRepeatabilityResultRecordV1>> {
    let mut rows = rows::load_results(connection, keys.plan, keys.system)?;
    let repetitions = anchor.preregistration.plan_foundation().repetitions();
    let mut decoded = Vec::new();
    let mut accepting = true;
    for repetition in repetitions {
        let repetition_id = repetition.repetition_id().digest().as_str();
        if let Some(position) = rows
            .iter()
            .position(|row| row.repetition_id == repetition_id)
        {
            if !accepting {
                return Err(StoreError::CorruptRecord);
            }
            decoded.push(decode_result(
                anchor,
                repetition,
                &rows.swap_remove(position),
            )?);
        } else {
            accepting = false;
        }
    }
    if rows.is_empty() {
        Ok(decoded)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn decode_result(
    anchor: &Anchor<'_>,
    repetition: &rewrite_model::GenerationRepetitionRecordV1,
    row: &ResultRow,
) -> StoreResult<GenerationRepeatabilityResultRecordV1> {
    let peek = peek_result(&row.bytes)?;
    if peek.terminal_stage != GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed {
        return Err(StoreError::CorruptRecord);
    }
    let value = GenerationRepeatabilityResultRecordV1::from_json_bytes(
        &row.bytes,
        GenerationRepeatabilityResultRecordV1Relations {
            scope: context::scope(anchor)?,
            repetition,
            attempt_ledger: &anchor.ledger.manifest,
            attempt_ledger_relations: context::ledger_relations(anchor)?,
            terminal_stage: GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
            candidate_receipt_set: None,
            deterministic_evaluation: None,
            candidate_judge_join: None,
            terminal_evidence_digest: &peek.terminal_evidence_digest,
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    check::result(row, value)
}

fn decode_repeatability(
    connection: &Connection,
    anchor: &Anchor<'_>,
    results: &[GenerationRepeatabilityResultRecordV1],
    keys: &Keys<'_>,
) -> StoreResult<GenerationRepeatabilityEvidenceManifestV1> {
    let row = load_manifest(
        connection,
        "generation_repeatability_evidence_manifests",
        "generation_repeatability_evidence_manifest_id",
        keys,
    )?;
    let foundation = anchor.preregistration.plan_foundation();
    let value = GenerationRepeatabilityEvidenceManifestV1::from_json_bytes(
        &row.bytes,
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope: context::scope(anchor)?,
            phase_policy_digest: anchor
                .preregistration
                .operation_policy()
                .repeatability_policy_digest(),
            planned_attempts: foundation.planned_attempts(),
            preregistered_repetitions: foundation.repetitions(),
            results,
            status: peeked_status(&row)?,
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    check::phase(&row, phase_check_repeatability(&value))?;
    Ok(value)
}

fn decode_resource(
    connection: &Connection,
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
    keys: &Keys<'_>,
) -> StoreResult<GenerationResourceEvidenceManifestV1> {
    let row = load_manifest(
        connection,
        "generation_resource_evidence_manifests",
        "generation_resource_evidence_manifest_id",
        keys,
    )?;
    let value = GenerationResourceEvidenceManifestV1::from_json_bytes(
        &row.bytes,
        GenerationResourceEvidenceManifestV1Relations {
            scope: context::scope(anchor)?,
            phase_policy_digest: anchor
                .preregistration
                .operation_policy()
                .resource_policy_digest(),
            evidence_record_digests: input.resource_evidence_record_digests,
            status: peeked_status(&row)?,
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    check::phase(&row, phase_check_resource(&value))?;
    Ok(value)
}

fn decode_human(
    connection: &Connection,
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
    keys: &Keys<'_>,
) -> StoreResult<GenerationHumanAdjudicationEvidenceManifestV1> {
    let row = load_manifest(
        connection,
        "generation_human_adjudication_evidence_manifests",
        "generation_human_adjudication_evidence_manifest_id",
        keys,
    )?;
    let value = GenerationHumanAdjudicationEvidenceManifestV1::from_json_bytes(
        &row.bytes,
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope: context::scope(anchor)?,
            phase_policy_digest: anchor
                .preregistration
                .operation_policy()
                .human_adjudication_policy_digest(),
            evidence_record_digests: input.human_evidence_record_digests,
            status: peeked_status(&row)?,
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    check::phase(&row, phase_check_human(&value))?;
    Ok(value)
}

fn decode_closure(
    connection: &Connection,
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
    keys: &Keys<'_>,
    evidence: &Evidence,
) -> StoreResult<Closure> {
    let row = rows::load_receipt(connection, keys.policy)?.ok_or(StoreError::CorruptRecord)?;
    let receipt = GenerationQualificationOperationReceiptV1::from_json_bytes(
        &row.bytes,
        receipt_relations(anchor, evidence),
        input.receipt_input,
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    let receipt = check::receipt(
        &row,
        receipt,
        &evidence.ledger,
        &evidence.repeatability,
        &evidence.resource,
        &evidence.human,
    )?;
    let interruption = decode_interruption(connection, anchor, input, keys, evidence, &receipt)?;
    Ok(Closure {
        receipt,
        interruption,
    })
}

fn decode_interruption(
    connection: &Connection,
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'_>,
    keys: &Keys<'_>,
    evidence: &Evidence,
    receipt: &GenerationQualificationOperationReceiptV1,
) -> StoreResult<Option<GenerationQualificationPhaseInterruptionRecordV1>> {
    match (
        rows::load_interruption(connection, keys.policy)?,
        input.phase_interruption_input,
    ) {
        (None, None) => Ok(None),
        (Some(row), Some(facts)) => {
            let value = GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(
                &row.bytes,
                &interruption_relations(anchor, input, evidence, receipt),
                facts,
            )
            .map_err(|_| StoreError::CorruptRecord)?;
            check::interruption(&row, value, receipt).map(Some)
        }
        (Some(_), None) | (None, Some(_)) => Err(StoreError::CorruptRecord),
    }
}

fn receipt_relations<'a>(
    anchor: &'a Anchor<'_>,
    evidence: &'a Evidence,
) -> GenerationQualificationOperationReceiptV1Relations<'a> {
    GenerationQualificationOperationReceiptV1Relations {
        operation_policy: anchor.preregistration.operation_policy(),
        request_projection: anchor.preregistration.request_projection(),
        platform_evidence: &evidence.platform,
        license_evidence: &evidence.license,
        attempt_ledger_manifest: &evidence.ledger,
        repeatability_manifest: &evidence.repeatability,
        resource_manifest: &evidence.resource,
        human_adjudication_manifest: &evidence.human,
    }
}

fn interruption_relations<'a>(
    anchor: &'a Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1ReadInput<'a>,
    evidence: &'a Evidence,
    receipt: &'a GenerationQualificationOperationReceiptV1,
) -> GenerationQualificationPhaseInterruptionRecordV1Relations<'a> {
    GenerationQualificationPhaseInterruptionRecordV1Relations {
        operation_policy: anchor.preregistration.operation_policy(),
        operation_policy_relations: input.preregistration.operation_policy_relations,
        operation_policy_input: input.preregistration.operation_policy_input,
        operation_receipt: receipt,
        operation_receipt_relations: receipt_relations(anchor, evidence),
        operation_receipt_input: input.receipt_input,
    }
}

fn load_manifest(
    connection: &Connection,
    table: &str,
    identifier: &str,
    keys: &Keys<'_>,
) -> StoreResult<super::rows::PhaseManifestRow> {
    rows::load_phase_manifest(connection, table, identifier, keys.plan, keys.system)?
        .ok_or(StoreError::CorruptRecord)
}

fn peeked_status(
    row: &super::rows::PhaseManifestRow,
) -> StoreResult<GenerationQualificationPhaseStatusV1> {
    let status = serde_json::from_slice::<StatusPeek>(&row.bytes)
        .map(|peek| peek.status)
        .map_err(|_| StoreError::CorruptRecord)?;
    if super::text::phase_status(status) == row.status {
        Ok(status)
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn peek_result(bytes: &[u8]) -> StoreResult<ResultPeek> {
    serde_json::from_slice(bytes).map_err(|_| StoreError::CorruptRecord)
}

fn phase_check_ledger(value: &GenerationAttemptLedgerManifestV1) -> check::PhaseCheck<'_> {
    check::PhaseCheck {
        id: value.attempt_ledger_manifest_id().digest(),
        system: value.generation_system_id().digest(),
        plan: value.generation_qualification_plan_id().digest(),
        suite: value.suite_manifest_id().digest(),
        count: value.evidence_item_count(),
        status: value.status(),
    }
}

fn phase_check_repeatability(
    value: &GenerationRepeatabilityEvidenceManifestV1,
) -> check::PhaseCheck<'_> {
    check::PhaseCheck {
        id: value.repeatability_evidence_manifest_id().digest(),
        system: value.generation_system_id().digest(),
        plan: value.generation_qualification_plan_id().digest(),
        suite: value.suite_manifest_id().digest(),
        count: value.evidence_item_count(),
        status: value.status(),
    }
}

fn phase_check_resource(value: &GenerationResourceEvidenceManifestV1) -> check::PhaseCheck<'_> {
    check::PhaseCheck {
        id: value.resource_evidence_manifest_id().digest(),
        system: value.generation_system_id().digest(),
        plan: value.generation_qualification_plan_id().digest(),
        suite: value.suite_manifest_id().digest(),
        count: value.evidence_item_count(),
        status: value.status(),
    }
}

fn phase_check_human(
    value: &GenerationHumanAdjudicationEvidenceManifestV1,
) -> check::PhaseCheck<'_> {
    check::PhaseCheck {
        id: value.human_adjudication_evidence_manifest_id().digest(),
        system: value.generation_system_id().digest(),
        plan: value.generation_qualification_plan_id().digest(),
        suite: value.suite_manifest_id().digest(),
        count: value.evidence_item_count(),
        status: value.status(),
    }
}

#[derive(Deserialize)]
struct StatusPeek {
    status: GenerationQualificationPhaseStatusV1,
}

#[derive(Deserialize)]
struct ResultPeek {
    terminal_stage: GenerationRepeatabilityTerminalStageV1,
    terminal_evidence_digest: Digest,
}
