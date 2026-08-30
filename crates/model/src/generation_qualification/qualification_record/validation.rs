use super::{
    CandidateGenerationAttemptOutcomeV1, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationQualificationId, GenerationQualificationLicenseDecisionV1,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseStatusV1,
    GenerationQualificationPlatformStatusV1, GenerationQualificationRecordV1,
    GenerationQualificationRecordV1Error, GenerationQualificationRecordV1Relations,
    GenerationQualificationStatusV1, GenerationRepeatabilityEvidenceManifestV1,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityTerminalStageV1,
    GenerationResourceEvidenceManifestV1, MAX_GENERATION_QUALIFICATION_RECORD_CANONICAL_BYTES,
    codec,
};

impl GenerationQualificationRecordV1 {
    pub(super) fn build(
        relations: &GenerationQualificationRecordV1Relations<'_>,
        wire: Option<&codec::QualificationRecordWire>,
    ) -> Result<Self, GenerationQualificationRecordV1Error> {
        validate_complete_closure(relations)?;
        let receipt_relations = relations.operation_receipt_relations;
        let policy = receipt_relations.operation_policy;
        let status = derive_status(relations)?;
        let mut value = Self {
            schema_version: super::super::GENERATION_QUALIFICATION_SCHEMA_VERSION,
            target_generation_system_id: policy.target_generation_system_id().clone(),
            baseline_generation_system_id: policy.baseline_generation_system_id().clone(),
            operation_policy_id: policy.operation_policy_id().clone(),
            request_projection_id: receipt_relations
                .request_projection
                .request_projection_id()
                .clone(),
            platform_evidence_id: receipt_relations
                .platform_evidence
                .platform_evidence_id()
                .clone(),
            license_evidence_id: receipt_relations
                .license_evidence
                .license_evidence_id()
                .clone(),
            operation_receipt_id: relations.operation_receipt.operation_receipt_id().clone(),
            status,
            id: GenerationQualificationId::from_canonical_bytes(b"uninitialized qualification"),
        };
        if wire.is_some_and(|expected| !expected.matches(&value)) {
            return Err(GenerationQualificationRecordV1Error::RelationshipMismatch);
        }
        let canonical = value.canonical_bytes();
        if canonical.len() > MAX_GENERATION_QUALIFICATION_RECORD_CANONICAL_BYTES {
            return Err(GenerationQualificationRecordV1Error::CanonicalEncodingTooLarge);
        }
        value.id = GenerationQualificationId::from_canonical_bytes(&canonical);
        Ok(value)
    }
}

fn validate_complete_closure(
    relations: &GenerationQualificationRecordV1Relations<'_>,
) -> Result<(), GenerationQualificationRecordV1Error> {
    let receipt_relations = relations.operation_receipt_relations;
    receipt_relations
        .operation_policy
        .validate_against(
            relations.operation_policy_relations,
            relations.operation_policy_input,
        )
        .map_err(GenerationQualificationRecordV1Error::InvalidOperationEvidence)?;
    receipt_relations
        .request_projection
        .validate_against(
            relations.request_projection_relations,
            relations.request_projection_entry_inputs,
        )
        .map_err(GenerationQualificationRecordV1Error::InvalidOperationEvidence)?;
    receipt_relations
        .platform_evidence
        .validate_against(
            relations.platform_evidence_relations,
            relations.platform_evidence_input,
        )
        .map_err(GenerationQualificationRecordV1Error::InvalidOperationEvidence)?;
    receipt_relations
        .license_evidence
        .validate_against(
            relations.license_evidence_relations,
            relations.license_evidence_input,
        )
        .map_err(GenerationQualificationRecordV1Error::InvalidOperationEvidence)?;
    receipt_relations
        .attempt_ledger_manifest
        .validate_against(relations.attempt_ledger_relations)
        .map_err(GenerationQualificationRecordV1Error::InvalidPhaseEvidence)?;
    validate_repeatability_results(relations)?;
    let expected_repeatability =
        GenerationRepeatabilityEvidenceManifestV1::new(relations.repeatability_manifest_relations)
            .map_err(GenerationQualificationRecordV1Error::InvalidPhaseEvidence)?;
    if &expected_repeatability != receipt_relations.repeatability_manifest {
        return Err(GenerationQualificationRecordV1Error::RelationshipMismatch);
    }
    let expected_resource =
        GenerationResourceEvidenceManifestV1::new(relations.resource_manifest_relations)
            .map_err(GenerationQualificationRecordV1Error::InvalidPhaseEvidence)?;
    if &expected_resource != receipt_relations.resource_manifest {
        return Err(GenerationQualificationRecordV1Error::RelationshipMismatch);
    }
    let expected_human = GenerationHumanAdjudicationEvidenceManifestV1::new(
        relations.human_adjudication_manifest_relations,
    )
    .map_err(GenerationQualificationRecordV1Error::InvalidPhaseEvidence)?;
    if &expected_human != receipt_relations.human_adjudication_manifest {
        return Err(GenerationQualificationRecordV1Error::RelationshipMismatch);
    }
    relations
        .operation_receipt
        .validate_against(receipt_relations, relations.operation_receipt_input)
        .map_err(GenerationQualificationRecordV1Error::InvalidOperationEvidence)
}

fn validate_repeatability_results(
    relations: &GenerationQualificationRecordV1Relations<'_>,
) -> Result<(), GenerationQualificationRecordV1Error> {
    let results = relations.repeatability_manifest_relations.results;
    if results.len() != relations.repeatability_result_relations.len() {
        return Err(GenerationQualificationRecordV1Error::RelationshipMismatch);
    }
    let operation_ledger = relations
        .operation_receipt_relations
        .attempt_ledger_manifest;
    for (record, result_relations) in results.iter().zip(relations.repeatability_result_relations) {
        if result_relations.attempt_ledger != operation_ledger {
            return Err(GenerationQualificationRecordV1Error::RelationshipMismatch);
        }
        let expected = GenerationRepeatabilityResultRecordV1::new(*result_relations)
            .map_err(GenerationQualificationRecordV1Error::InvalidPhaseEvidence)?;
        if &expected != record {
            return Err(GenerationQualificationRecordV1Error::RelationshipMismatch);
        }
    }
    Ok(())
}

fn derive_status(
    relations: &GenerationQualificationRecordV1Relations<'_>,
) -> Result<GenerationQualificationStatusV1, GenerationQualificationRecordV1Error> {
    derive_status_from_facts(decision_facts(relations))
}

#[derive(Clone, Copy)]
pub(super) struct DecisionFacts {
    pub(super) terminal_status: GenerationQualificationOperationTerminalStatusV1,
    pub(super) finalization_status: GenerationQualificationOperationFinalizationStatusV1,
    pub(super) peak_concurrent_attempts: u32,
    pub(super) under_deadline: bool,
    pub(super) platform_status: GenerationQualificationPlatformStatusV1,
    pub(super) license_decision: GenerationQualificationLicenseDecisionV1,
    pub(super) phase_statuses: [GenerationQualificationPhaseStatusV1; 4],
    pub(super) ledger_has_actual_failure: bool,
    pub(super) repeatability_has_actual_failure: bool,
}

fn decision_facts(relations: &GenerationQualificationRecordV1Relations<'_>) -> DecisionFacts {
    let receipt = relations.operation_receipt;
    let receipt_relations = relations.operation_receipt_relations;
    let deadline_nanoseconds = u64::from(
        receipt_relations
            .operation_policy
            .limits()
            .maximum_elapsed_milliseconds(),
    ) * 1_000_000;
    DecisionFacts {
        terminal_status: receipt.terminal_status(),
        finalization_status: receipt.finalization_status(),
        peak_concurrent_attempts: receipt.peak_concurrent_attempts(),
        under_deadline: receipt.elapsed_nanoseconds() < deadline_nanoseconds,
        platform_status: receipt_relations.platform_evidence.status(),
        license_decision: receipt_relations.license_evidence.decision(),
        phase_statuses: phase_statuses(receipt_relations),
        ledger_has_actual_failure: relations
            .attempt_ledger_relations
            .attempt_records
            .iter()
            .any(|record| {
                matches!(
                    record.outcome(),
                    CandidateGenerationAttemptOutcomeV1::Failed { .. }
                )
            }),
        repeatability_has_actual_failure: relations
            .repeatability_manifest_relations
            .results
            .iter()
            .any(|result| {
                result.terminal_stage() != GenerationRepeatabilityTerminalStageV1::Passed
            }),
    }
}

pub(super) fn derive_status_from_facts(
    facts: DecisionFacts,
) -> Result<GenerationQualificationStatusV1, GenerationQualificationRecordV1Error> {
    if facts.terminal_status != GenerationQualificationOperationTerminalStatusV1::Completed {
        return Err(GenerationQualificationRecordV1Error::IneligibleOperation);
    }
    match facts.finalization_status {
        GenerationQualificationOperationFinalizationStatusV1::Failed => {
            Err(GenerationQualificationRecordV1Error::IneligibleOperation)
        }
        GenerationQualificationOperationFinalizationStatusV1::NotRequired => {
            derive_pretraffic_rejection(facts)
        }
        GenerationQualificationOperationFinalizationStatusV1::Passed => {
            derive_managed_decision(facts)
        }
    }
}

fn derive_pretraffic_rejection(
    facts: DecisionFacts,
) -> Result<GenerationQualificationStatusV1, GenerationQualificationRecordV1Error> {
    let negative_gate = facts.platform_status == GenerationQualificationPlatformStatusV1::Rejected
        || facts.license_decision == GenerationQualificationLicenseDecisionV1::Rejected;
    let all_skipped = facts
        .phase_statuses
        .into_iter()
        .all(|status| status == GenerationQualificationPhaseStatusV1::Skipped);
    if facts.peak_concurrent_attempts == 0 && facts.under_deadline && negative_gate && all_skipped {
        Ok(GenerationQualificationStatusV1::Rejected)
    } else {
        Err(GenerationQualificationRecordV1Error::InvalidDecisionClosure)
    }
}

fn derive_managed_decision(
    facts: DecisionFacts,
) -> Result<GenerationQualificationStatusV1, GenerationQualificationRecordV1Error> {
    if facts.peak_concurrent_attempts != 1
        || !facts.under_deadline
        || facts.platform_status != GenerationQualificationPlatformStatusV1::Supported
        || facts.license_decision != GenerationQualificationLicenseDecisionV1::LocalUseOnly
    {
        return Err(GenerationQualificationRecordV1Error::InvalidDecisionClosure);
    }
    if facts
        .phase_statuses
        .into_iter()
        .all(|status| status == GenerationQualificationPhaseStatusV1::Passed)
    {
        return Ok(GenerationQualificationStatusV1::Qualified);
    }
    if has_actual_policy_failure(facts) {
        Ok(GenerationQualificationStatusV1::Rejected)
    } else {
        Err(GenerationQualificationRecordV1Error::InvalidDecisionClosure)
    }
}

fn phase_statuses(
    relations: GenerationQualificationOperationReceiptV1Relations<'_>,
) -> [GenerationQualificationPhaseStatusV1; 4] {
    [
        relations.attempt_ledger_manifest.status(),
        relations.repeatability_manifest.status(),
        relations.resource_manifest.status(),
        relations.human_adjudication_manifest.status(),
    ]
}

fn has_actual_policy_failure(facts: DecisionFacts) -> bool {
    use GenerationQualificationPhaseStatusV1::{Failed, Passed, Skipped};
    match facts.phase_statuses {
        [Failed, Failed | Skipped, Skipped, Skipped] => facts.ledger_has_actual_failure,
        [Passed, Failed, Skipped, Skipped] => facts.repeatability_has_actual_failure,
        [Passed, Passed, Failed, Skipped] | [Passed, Passed, Passed, Failed] => true,
        _ => false,
    }
}
