use rewrite_model::{
    GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationLicenseEvidenceV1, GenerationQualificationLicenseEvidenceV1Relations,
    GenerationQualificationOperationContractError, GenerationQualificationOperationReceiptV1,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationPhaseInterruptionRecordV1,
    GenerationQualificationPhaseInterruptionRecordV1Relations,
    GenerationQualificationPhaseStatusV1, GenerationQualificationPlatformEvidenceV1,
    GenerationRepeatabilityEvidenceManifestV1, GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityResultRecordV1Relations,
    GenerationRepeatabilityTerminalStageV1, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations,
    MAX_GENERATION_QUALIFICATION_LICENSE_EVIDENCE_JSON_BYTES,
    MAX_GENERATION_QUALIFICATION_OPERATION_RECEIPT_JSON_BYTES,
    MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_JSON_BYTES,
    MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
    MAX_GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_JSON_BYTES,
    MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES,
};
use rusqlite::Connection;
use serde::Serialize;

use super::context::{self, Anchor};
use super::{
    GenerationQualificationTerminalEvidenceV1Input,
    GenerationQualificationTerminalEvidenceV1ReadInput,
    StoredGenerationQualificationTerminalEvidenceV1,
};
use crate::store::generation_qualification_preregistration::StoredGenerationQualificationPreregistration;
use crate::{StoreError, StoreResult};

pub(super) struct PreparedCohort<'a> {
    pub(super) encoded: EncodedCohort,
    pub(super) records: CohortRecords,
    pub(super) preregistration: StoredGenerationQualificationPreregistration,
    pub(super) read: GenerationQualificationTerminalEvidenceV1ReadInput<'a>,
}

pub(super) struct EncodedCohort {
    pub(super) platform: Vec<u8>,
    pub(super) license: Vec<u8>,
    pub(super) ledger: Vec<u8>,
    pub(super) results: Vec<Vec<u8>>,
    pub(super) repeatability: Vec<u8>,
    pub(super) resource: Vec<u8>,
    pub(super) human: Vec<u8>,
    pub(super) receipt: Vec<u8>,
    pub(super) interruption: Option<Vec<u8>>,
}

pub(super) struct CohortRecords {
    pub(super) platform: GenerationQualificationPlatformEvidenceV1,
    pub(super) license: GenerationQualificationLicenseEvidenceV1,
    pub(super) ledger: rewrite_model::GenerationAttemptLedgerManifestV1,
    pub(super) results: Vec<GenerationRepeatabilityResultRecordV1>,
    pub(super) repeatability: GenerationRepeatabilityEvidenceManifestV1,
    pub(super) resource: GenerationResourceEvidenceManifestV1,
    pub(super) human: GenerationHumanAdjudicationEvidenceManifestV1,
    pub(super) receipt: GenerationQualificationOperationReceiptV1,
    pub(super) interruption: Option<GenerationQualificationPhaseInterruptionRecordV1>,
}

struct PhaseSet<'a> {
    platform: &'a GenerationQualificationPlatformEvidenceV1,
    license: &'a GenerationQualificationLicenseEvidenceV1,
    repeatability: &'a GenerationRepeatabilityEvidenceManifestV1,
    resource: &'a GenerationResourceEvidenceManifestV1,
    human: &'a GenerationHumanAdjudicationEvidenceManifestV1,
}

pub(super) fn prepare<'a>(
    connection: &Connection,
    input: &GenerationQualificationTerminalEvidenceV1Input<'a>,
) -> StoreResult<PreparedCohort<'a>> {
    let anchor = context::load_anchor(
        connection,
        input.preregistration,
        input.managed_evidence_inputs,
    )?;
    if input.attempt_ledger_manifest != &anchor.ledger.manifest {
        return Err(StoreError::ImmutableConflict);
    }
    require_repeatability_stage(input)?;
    let records = build_records(&anchor, input)?;
    let encoded = encode_records(&records)?;
    let read = read_input(input);
    Ok(PreparedCohort {
        encoded,
        records,
        preregistration: anchor.preregistration,
        read,
    })
}

pub(super) fn matches_prepared(
    stored: &StoredGenerationQualificationTerminalEvidenceV1,
    prepared: &PreparedCohort<'_>,
) -> bool {
    stored.preregistration() == &prepared.preregistration
        && stored.platform_evidence() == &prepared.records.platform
        && stored.license_evidence() == &prepared.records.license
        && stored.attempt_ledger_manifest() == &prepared.records.ledger
        && stored.repeatability_results() == prepared.records.results.as_slice()
        && stored.repeatability_manifest() == &prepared.records.repeatability
        && stored.resource_manifest() == &prepared.records.resource
        && stored.human_manifest() == &prepared.records.human
        && stored.receipt() == &prepared.records.receipt
        && stored.phase_interruption() == prepared.records.interruption.as_ref()
}

fn read_input<'a>(
    input: &GenerationQualificationTerminalEvidenceV1Input<'a>,
) -> GenerationQualificationTerminalEvidenceV1ReadInput<'a> {
    GenerationQualificationTerminalEvidenceV1ReadInput {
        preregistration: input.preregistration,
        platform_input: input.platform_input,
        license_input: input.license_input,
        model_package_foundation_id: input.model_package_foundation_id,
        model_license_control_id: input.model_license_control_id,
        managed_evidence_inputs: input.managed_evidence_inputs,
        resource_evidence_record_digests: input.resource_evidence_record_digests,
        human_evidence_record_digests: input.human_evidence_record_digests,
        receipt_input: input.receipt_input,
        phase_interruption_input: input.phase_interruption_input,
    }
}

fn require_repeatability_stage(
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
) -> StoreResult<()> {
    let failed = GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed;
    if input.repeatability_results.is_empty() {
        if input.repeatability_manifest.status() == GenerationQualificationPhaseStatusV1::Skipped {
            Ok(())
        } else {
            Err(StoreError::CorruptRecord)
        }
    } else if input
        .repeatability_results
        .iter()
        .all(|result| result.terminal_stage() == failed)
    {
        Ok(())
    } else {
        Err(StoreError::CorruptRecord)
    }
}

fn build_records(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
) -> StoreResult<CohortRecords> {
    let platform = build_platform(anchor, input)?;
    let license = build_license(anchor, input)?;
    let results = build_results(anchor, input)?;
    let repeatability = build_repeatability(anchor, input, &results)?;
    let resource = build_resource(anchor, input)?;
    let human = build_human(anchor, input)?;
    let phases = PhaseSet {
        platform: &platform,
        license: &license,
        repeatability: &repeatability,
        resource: &resource,
        human: &human,
    };
    let receipt = bind_receipt(anchor, input, &phases)?;
    let interruption = bind_interruption(anchor, input, &receipt, &phases)?;
    Ok(CohortRecords {
        platform,
        license,
        ledger: anchor.ledger.manifest.clone(),
        results,
        repeatability,
        resource,
        human,
        receipt,
        interruption,
    })
}

fn build_platform(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
) -> StoreResult<GenerationQualificationPlatformEvidenceV1> {
    let value = GenerationQualificationPlatformEvidenceV1::new(
        context::platform_relations(anchor)?,
        input.platform_input,
    )
    .map_err(map_operation)?;
    require_same(value, input.platform_evidence)
}

fn build_license(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
) -> StoreResult<GenerationQualificationLicenseEvidenceV1> {
    let scope = context::scope(anchor)?;
    let relations = GenerationQualificationLicenseEvidenceV1Relations {
        operation_policy: anchor.preregistration.operation_policy(),
        request_projection: anchor.preregistration.request_projection(),
        target_generation_system: scope.generation_system,
        target_generation_system_relations: anchor.system_relations,
        model_package_foundation_id: input.model_package_foundation_id,
        model_license_control_id: input.model_license_control_id,
    };
    let value = GenerationQualificationLicenseEvidenceV1::new(relations, input.license_input)
        .map_err(map_operation)?;
    require_same(value, input.license_evidence)
}

fn build_results(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
) -> StoreResult<Vec<GenerationRepeatabilityResultRecordV1>> {
    let repetitions = anchor.preregistration.plan_foundation().repetitions();
    if input.repeatability_results.len() > repetitions.len() {
        return Err(StoreError::CorruptRecord);
    }
    let mut results = Vec::with_capacity(input.repeatability_results.len());
    for (index, caller) in input.repeatability_results.iter().enumerate() {
        let value = GenerationRepeatabilityResultRecordV1::new(
            GenerationRepeatabilityResultRecordV1Relations {
                scope: context::scope(anchor)?,
                repetition: &repetitions[index],
                attempt_ledger: &anchor.ledger.manifest,
                attempt_ledger_relations: context::ledger_relations(anchor)?,
                terminal_stage: GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
                candidate_receipt_set: None,
                deterministic_evaluation: None,
                candidate_judge_join: None,
                terminal_evidence_digest: caller.terminal_evidence_digest(),
            },
        )
        .map_err(|_| StoreError::CorruptRecord)?;
        results.push(require_same(value, caller)?);
    }
    Ok(results)
}

fn build_repeatability(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
    results: &[GenerationRepeatabilityResultRecordV1],
) -> StoreResult<GenerationRepeatabilityEvidenceManifestV1> {
    let foundation = anchor.preregistration.plan_foundation();
    let value = GenerationRepeatabilityEvidenceManifestV1::new(
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope: context::scope(anchor)?,
            phase_policy_digest: anchor
                .preregistration
                .operation_policy()
                .repeatability_policy_digest(),
            planned_attempts: foundation.planned_attempts(),
            preregistered_repetitions: foundation.repetitions(),
            results,
            status: input.repeatability_manifest.status(),
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    require_same(value, input.repeatability_manifest)
}

fn build_resource(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
) -> StoreResult<GenerationResourceEvidenceManifestV1> {
    let value =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope: context::scope(anchor)?,
            phase_policy_digest: anchor
                .preregistration
                .operation_policy()
                .resource_policy_digest(),
            evidence_record_digests: input.resource_evidence_record_digests,
            status: input.resource_manifest.status(),
        })
        .map_err(|_| StoreError::CorruptRecord)?;
    require_same(value, input.resource_manifest)
}

fn build_human(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
) -> StoreResult<GenerationHumanAdjudicationEvidenceManifestV1> {
    let value = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope: context::scope(anchor)?,
            phase_policy_digest: anchor
                .preregistration
                .operation_policy()
                .human_adjudication_policy_digest(),
            evidence_record_digests: input.human_evidence_record_digests,
            status: input.human_manifest.status(),
        },
    )
    .map_err(|_| StoreError::CorruptRecord)?;
    require_same(value, input.human_manifest)
}

fn bind_receipt(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
    phases: &PhaseSet<'_>,
) -> StoreResult<GenerationQualificationOperationReceiptV1> {
    let bytes = encode_record(
        input.receipt,
        MAX_GENERATION_QUALIFICATION_OPERATION_RECEIPT_JSON_BYTES,
    )?;
    let value = GenerationQualificationOperationReceiptV1::from_json_bytes(
        &bytes,
        receipt_relations(anchor, phases),
        input.receipt_input,
    )
    .map_err(map_operation)?;
    require_same(value, input.receipt)
}

fn bind_interruption(
    anchor: &Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'_>,
    receipt: &GenerationQualificationOperationReceiptV1,
    phases: &PhaseSet<'_>,
) -> StoreResult<Option<GenerationQualificationPhaseInterruptionRecordV1>> {
    match (input.phase_interruption, input.phase_interruption_input) {
        (None, None) => Ok(None),
        (Some(caller), Some(facts)) => {
            let bytes = encode_record(
                caller,
                MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_JSON_BYTES,
            )?;
            let value = GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(
                &bytes,
                &interruption_relations(anchor, input, receipt, phases),
                facts,
            )
            .map_err(map_operation)?;
            require_same(value, caller).map(Some)
        }
        (Some(_), None) | (None, Some(_)) => Err(StoreError::CorruptRecord),
    }
}

fn receipt_relations<'a>(
    anchor: &'a Anchor<'_>,
    phases: &PhaseSet<'a>,
) -> GenerationQualificationOperationReceiptV1Relations<'a> {
    GenerationQualificationOperationReceiptV1Relations {
        operation_policy: anchor.preregistration.operation_policy(),
        request_projection: anchor.preregistration.request_projection(),
        platform_evidence: phases.platform,
        license_evidence: phases.license,
        attempt_ledger_manifest: &anchor.ledger.manifest,
        repeatability_manifest: phases.repeatability,
        resource_manifest: phases.resource,
        human_adjudication_manifest: phases.human,
    }
}

fn interruption_relations<'a>(
    anchor: &'a Anchor<'_>,
    input: &GenerationQualificationTerminalEvidenceV1Input<'a>,
    receipt: &'a GenerationQualificationOperationReceiptV1,
    phases: &PhaseSet<'a>,
) -> GenerationQualificationPhaseInterruptionRecordV1Relations<'a> {
    GenerationQualificationPhaseInterruptionRecordV1Relations {
        operation_policy: anchor.preregistration.operation_policy(),
        operation_policy_relations: input.preregistration.operation_policy_relations,
        operation_policy_input: input.preregistration.operation_policy_input,
        operation_receipt: receipt,
        operation_receipt_relations: receipt_relations(anchor, phases),
        operation_receipt_input: input.receipt_input,
    }
}

pub(super) fn encode_records(records: &CohortRecords) -> StoreResult<EncodedCohort> {
    let mut results = Vec::with_capacity(records.results.len());
    for result in &records.results {
        results.push(encode_record(
            result,
            MAX_GENERATION_REPEATABILITY_RESULT_JSON_BYTES,
        )?);
    }
    Ok(EncodedCohort {
        platform: encode_record(
            &records.platform,
            MAX_GENERATION_QUALIFICATION_PLATFORM_EVIDENCE_JSON_BYTES,
        )?,
        license: encode_record(
            &records.license,
            MAX_GENERATION_QUALIFICATION_LICENSE_EVIDENCE_JSON_BYTES,
        )?,
        ledger: encode_record(
            &records.ledger,
            MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
        )?,
        results,
        repeatability: encode_record(
            &records.repeatability,
            MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
        )?,
        resource: encode_record(
            &records.resource,
            MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
        )?,
        human: encode_record(
            &records.human,
            MAX_GENERATION_QUALIFICATION_PHASE_MANIFEST_JSON_BYTES,
        )?,
        receipt: encode_record(
            &records.receipt,
            MAX_GENERATION_QUALIFICATION_OPERATION_RECEIPT_JSON_BYTES,
        )?,
        interruption: records
            .interruption
            .as_ref()
            .map(|value| {
                encode_record(
                    value,
                    MAX_GENERATION_QUALIFICATION_PHASE_INTERRUPTION_JSON_BYTES,
                )
            })
            .transpose()?,
    })
}

fn encode_record<T: Serialize>(value: &T, maximum: usize) -> StoreResult<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.is_empty() {
        Err(StoreError::CorruptRecord)
    } else if bytes.len() > maximum {
        Err(StoreError::RecordTooLarge)
    } else {
        Ok(bytes)
    }
}

fn require_same<T: PartialEq>(reconstructed: T, caller: &T) -> StoreResult<T> {
    if &reconstructed == caller {
        Ok(reconstructed)
    } else {
        Err(StoreError::ImmutableConflict)
    }
}

fn map_operation(error: GenerationQualificationOperationContractError) -> StoreError {
    match error {
        GenerationQualificationOperationContractError::NonCanonicalEncoding => {
            StoreError::CorruptRecord
        }
        GenerationQualificationOperationContractError::EncodedRecordTooLarge => {
            StoreError::RecordTooLarge
        }
        other => StoreError::InvalidGenerationQualificationPreregistration(other),
    }
}
