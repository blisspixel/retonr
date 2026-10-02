//! Full typed negative decision derived only inside the retained live projection.

use super::{
    CancellationToken, Error, GenerationQualificationPreregistrationRepository,
    JudgeSettlementContext, map_preparation_error,
};
use crate::generation_qualification_preregistration::prepared::PreparedGenerationQualificationValidationView;
use rewrite_model::{
    GenerationAttemptLedgerManifestV1Relations, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationQualificationRecordV1,
    GenerationQualificationRecordV1Relations, GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationRepeatabilityResultRecordV1Relations, GenerationRepeatabilityTerminalStageV1,
    GenerationRepetitionRecordV1, GenerationResourceEvidenceManifestV1Relations,
};
use rewrite_model_store::{
    GenerationResourcePhaseV1Input, GenerationResourceRejectionV1Input, WriteDisposition,
};

pub(super) fn publish(
    context: &JudgeSettlementContext<'_>,
    prepared: &PreparedGenerationQualificationValidationView<'_>,
    repository: &mut GenerationQualificationPreregistrationRepository,
    input: GenerationResourcePhaseV1Input<'_>,
    peak: u32,
    cancellation: &CancellationToken,
) -> Result<
    (
        GenerationQualificationOperationReceiptV1,
        GenerationQualificationRecordV1,
        WriteDisposition,
    ),
    Error,
> {
    with_evidence(context, prepared, input, peak, |input| {
        let disposition = repository
            .persist_resource_rejection(input, prepared.deadline, cancellation)
            .map_err(map_preparation_error)?;
        Ok((
            input.qualification_relations.operation_receipt.clone(),
            input.record.clone(),
            disposition,
        ))
    })
}

pub(super) fn with_evidence<T>(
    context: &JudgeSettlementContext<'_>,
    prepared: &PreparedGenerationQualificationValidationView<'_>,
    input: GenerationResourcePhaseV1Input<'_>,
    peak: u32,
    callback: impl FnOnce(GenerationResourceRejectionV1Input<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let policy = prepared.operation_policy;
    let frozen = prepared.operation_policy_relations;
    let scope = GenerationQualificationPhaseScopeV1 {
        generation_system: frozen.target_system.generation_system,
        qualification_plan: frozen.plan,
        suite: frozen.suite,
    };
    let ledger_relations = GenerationAttemptLedgerManifestV1Relations {
        scope,
        phase_policy_digest: policy.attempt_ledger_policy_digest(),
        planned_attempts: frozen.planned_attempts,
        attempt_records: context.ledger.target_attempt_records(),
        status: GenerationQualificationPhaseStatusV1::Passed,
    };
    let repeat = input.repeatability;
    let repeatability_relations = GenerationRepeatabilityEvidenceManifestV1Relations {
        scope,
        phase_policy_digest: policy.repeatability_policy_digest(),
        planned_attempts: frozen.planned_attempts,
        preregistered_repetitions: frozen.repetitions,
        results: repeat.ordered_results,
        status: GenerationQualificationPhaseStatusV1::Passed,
    };
    let result_relations =
        passed_result_relations(input, scope, ledger_relations, frozen.repetitions);
    let digests = input
        .ordered_results
        .iter()
        .map(|result| result.resource_attempt_result_id().digest().clone())
        .collect::<Vec<_>>();
    let resource_relations = GenerationResourceEvidenceManifestV1Relations {
        scope,
        phase_policy_digest: policy.resource_policy_digest(),
        evidence_record_digests: &digests,
        status: GenerationQualificationPhaseStatusV1::Failed,
    };
    let human_relations = GenerationHumanAdjudicationEvidenceManifestV1Relations {
        scope,
        phase_policy_digest: policy.human_adjudication_policy_digest(),
        evidence_record_digests: &[],
        status: GenerationQualificationPhaseStatusV1::Skipped,
    };
    let human = GenerationHumanAdjudicationEvidenceManifestV1::new(human_relations)
        .map_err(|_| Error::OperationScope)?;
    let receipt_relations = GenerationQualificationOperationReceiptV1Relations {
        operation_policy: policy,
        request_projection: prepared.request_projection,
        platform_evidence: prepared.platform_evidence,
        license_evidence: prepared.license_evidence,
        attempt_ledger_manifest: context.ledger.manifest(),
        repeatability_manifest: repeat.manifest,
        resource_manifest: input.manifest,
        human_adjudication_manifest: &human,
    };
    let receipt_input = GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds: u64::try_from(prepared.started.elapsed().as_nanos())
            .map_err(|_| Error::DeadlineExceeded)?,
        peak_concurrent_attempts: peak,
        terminal_status: GenerationQualificationOperationTerminalStatusV1::Completed,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::Passed,
    };
    let receipt = GenerationQualificationOperationReceiptV1::new(receipt_relations, receipt_input)
        .map_err(|_| Error::OperationScope)?;
    let relations = GenerationQualificationRecordV1Relations {
        operation_receipt: &receipt,
        operation_receipt_relations: receipt_relations,
        operation_receipt_input: receipt_input,
        operation_policy_relations: frozen,
        operation_policy_input: prepared.operation_policy_input,
        request_projection_relations: prepared.request_projection_relations,
        request_projection_entry_inputs: prepared.request_projection_entry_inputs,
        platform_evidence_relations: prepared.platform_evidence_relations,
        platform_evidence_input: prepared.platform_evidence_input,
        license_evidence_relations: prepared.license_evidence_relations,
        license_evidence_input: prepared.license_evidence_input,
        attempt_ledger_relations: ledger_relations,
        repeatability_manifest_relations: repeatability_relations,
        repeatability_result_relations: &result_relations,
        resource_manifest_relations: resource_relations,
        human_adjudication_manifest_relations: human_relations,
    };
    let record =
        GenerationQualificationRecordV1::new(&relations).map_err(|_| Error::OperationScope)?;
    callback(GenerationResourceRejectionV1Input {
        resource_phase: input,
        qualification_relations: &relations,
        record: &record,
    })
}

fn passed_result_relations<'a>(
    input: GenerationResourcePhaseV1Input<'a>,
    scope: GenerationQualificationPhaseScopeV1<'a>,
    ledger_relations: GenerationAttemptLedgerManifestV1Relations<'a>,
    repetitions: &'a [GenerationRepetitionRecordV1],
) -> Vec<GenerationRepeatabilityResultRecordV1Relations<'a>> {
    input
        .repeatability
        .ordered_results
        .iter()
        .zip(input.repeatability.expected_parents)
        .zip(repetitions)
        .map(
            |((result, parent), repetition)| GenerationRepeatabilityResultRecordV1Relations {
                scope,
                repetition,
                attempt_ledger: input.repeatability.ledger.manifest,
                attempt_ledger_relations: ledger_relations,
                terminal_stage: GenerationRepeatabilityTerminalStageV1::Passed,
                candidate_receipt_set: Some(&parent.target_receipt_set),
                deterministic_evaluation: Some(&parent.deterministic_evaluation),
                candidate_judge_join: Some(parent.judge_execution.join()),
                terminal_evidence_digest: result.terminal_evidence_digest(),
            },
        )
        .collect()
}
