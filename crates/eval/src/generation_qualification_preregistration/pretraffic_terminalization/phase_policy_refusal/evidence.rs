use std::time::Instant;

use rewrite_app::{
    GenerationQualificationPhasePolicySourceDisposition,
    VerifiedGenerationQualificationHumanAdjudicationPolicy,
    VerifiedGenerationQualificationResourcePolicy,
};
use rewrite_model::{
    GenerationAttemptLedgerManifestV1, GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1,
    GenerationHumanAdjudicationPolicyDenialRecordV1Relations,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1, GenerationRepeatabilityEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1, GenerationResourcePolicyDenialRecordV1,
    GenerationResourcePolicyDenialRecordV1Relations,
};
use rewrite_types::CancellationToken;

use super::GenerationQualificationPhasePolicyRefusalError;
use crate::generation_qualification_preregistration::{
    check_gate,
    prepared::PreparedGenerationQualificationValidationView,
    pretraffic_terminalization::{
        SkippedPhaseManifests, derive_skipped_manifests,
        relations::{phase_scope, receipt_relations},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DeniedPolicySelection {
    pub(super) resource: bool,
    pub(super) human: bool,
}

pub(super) struct PhasePolicyRefusalEvidence {
    pub(super) attempt_ledger_manifest: GenerationAttemptLedgerManifestV1,
    pub(super) repeatability_manifest: GenerationRepeatabilityEvidenceManifestV1,
    pub(super) resource_manifest: GenerationResourceEvidenceManifestV1,
    pub(super) human_adjudication_manifest: GenerationHumanAdjudicationEvidenceManifestV1,
    pub(super) resource_denial_record: Option<GenerationResourcePolicyDenialRecordV1>,
    pub(super) human_denial_record: Option<GenerationHumanAdjudicationPolicyDenialRecordV1>,
    pub(super) receipt_input: GenerationQualificationOperationReceiptV1Input,
    pub(super) receipt: GenerationQualificationOperationReceiptV1,
}

pub(super) fn derive_refusal_evidence(
    view: &PreparedGenerationQualificationValidationView<'_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    human_policy: &VerifiedGenerationQualificationHumanAdjudicationPolicy,
    cancellation: &CancellationToken,
) -> Result<PhasePolicyRefusalEvidence, GenerationQualificationPhasePolicyRefusalError> {
    let selection = validate_policies(view, resource_policy, human_policy)?;
    check_refusal_gate(view.deadline, cancellation)?;
    let manifests = derive_skipped_manifests(view)
        .map_err(|_| GenerationQualificationPhasePolicyRefusalError::PhaseClosureMismatch)?;
    let (resource_denial_record, human_denial_record) =
        derive_denial_records(view, resource_policy, human_policy, selection)?;
    let receipt_input = failed_receipt_input(checked_elapsed_nanoseconds(
        view.started,
        view.deadline,
        cancellation,
    )?);
    let receipt = GenerationQualificationOperationReceiptV1::new(
        receipt_relations(view, &manifests),
        receipt_input,
    )
    .map_err(|_| GenerationQualificationPhasePolicyRefusalError::ReceiptMismatch)?;
    check_refusal_gate(view.deadline, cancellation)?;
    Ok(evidence_from_parts(
        manifests,
        resource_denial_record,
        human_denial_record,
        receipt_input,
        receipt,
    ))
}

pub(super) fn validate_refusal_evidence(
    view: &PreparedGenerationQualificationValidationView<'_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    human_policy: &VerifiedGenerationQualificationHumanAdjudicationPolicy,
    evidence: &PhasePolicyRefusalEvidence,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPhasePolicyRefusalError> {
    let selection = validate_policies(view, resource_policy, human_policy)?;
    check_refusal_gate(view.deadline, cancellation)?;
    let expected_manifests = derive_skipped_manifests(view)
        .map_err(|_| GenerationQualificationPhasePolicyRefusalError::PhaseClosureMismatch)?;
    if !manifests_match(&expected_manifests, evidence) {
        return Err(GenerationQualificationPhasePolicyRefusalError::PhaseClosureMismatch);
    }
    validate_denial_records(view, resource_policy, human_policy, selection, evidence)?;
    evidence
        .receipt
        .validate_against(
            receipt_relations_from_evidence(view, evidence),
            evidence.receipt_input,
        )
        .map_err(|_| GenerationQualificationPhasePolicyRefusalError::ReceiptMismatch)?;
    check_refusal_gate(view.deadline, cancellation)
}

fn validate_policies(
    view: &PreparedGenerationQualificationValidationView<'_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    human_policy: &VerifiedGenerationQualificationHumanAdjudicationPolicy,
) -> Result<DeniedPolicySelection, GenerationQualificationPhasePolicyRefusalError> {
    resource_policy
        .revalidate_operation_policy(view.operation_policy)
        .map_err(|_| GenerationQualificationPhasePolicyRefusalError::ResourcePolicyMismatch)?;
    human_policy
        .revalidate_operation_policy(view.operation_policy)
        .map_err(|_| GenerationQualificationPhasePolicyRefusalError::HumanPolicyMismatch)?;
    denied_policy_selection(
        resource_policy.source_disposition(),
        human_policy.source_disposition(),
    )
}

pub(super) const fn denied_policy_selection(
    resource: GenerationQualificationPhasePolicySourceDisposition,
    human: GenerationQualificationPhasePolicySourceDisposition,
) -> Result<DeniedPolicySelection, GenerationQualificationPhasePolicyRefusalError> {
    let selection = DeniedPolicySelection {
        resource: matches!(
            resource,
            GenerationQualificationPhasePolicySourceDisposition::Denied
        ),
        human: matches!(
            human,
            GenerationQualificationPhasePolicySourceDisposition::Denied
        ),
    };
    if selection.resource || selection.human {
        Ok(selection)
    } else {
        Err(GenerationQualificationPhasePolicyRefusalError::BothPoliciesApproved)
    }
}

fn derive_denial_records(
    view: &PreparedGenerationQualificationValidationView<'_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    human_policy: &VerifiedGenerationQualificationHumanAdjudicationPolicy,
    selection: DeniedPolicySelection,
) -> Result<
    (
        Option<GenerationResourcePolicyDenialRecordV1>,
        Option<GenerationHumanAdjudicationPolicyDenialRecordV1>,
    ),
    GenerationQualificationPhasePolicyRefusalError,
> {
    let scope = phase_scope(view);
    let resource = selection
        .resource
        .then(|| {
            GenerationResourcePolicyDenialRecordV1::new(
                GenerationResourcePolicyDenialRecordV1Relations {
                    scope,
                    phase_policy_digest: resource_policy.policy_digest(),
                },
            )
        })
        .transpose()
        .map_err(|_| GenerationQualificationPhasePolicyRefusalError::DenialRecordMismatch)?;
    let human = selection
        .human
        .then(|| {
            GenerationHumanAdjudicationPolicyDenialRecordV1::new(
                GenerationHumanAdjudicationPolicyDenialRecordV1Relations {
                    scope,
                    phase_policy_digest: human_policy.policy_digest(),
                },
            )
        })
        .transpose()
        .map_err(|_| GenerationQualificationPhasePolicyRefusalError::DenialRecordMismatch)?;
    Ok((resource, human))
}

fn validate_denial_records(
    view: &PreparedGenerationQualificationValidationView<'_>,
    resource_policy: &VerifiedGenerationQualificationResourcePolicy,
    human_policy: &VerifiedGenerationQualificationHumanAdjudicationPolicy,
    selection: DeniedPolicySelection,
    evidence: &PhasePolicyRefusalEvidence,
) -> Result<(), GenerationQualificationPhasePolicyRefusalError> {
    let expected = derive_denial_records(view, resource_policy, human_policy, selection)?;
    if expected.0 == evidence.resource_denial_record && expected.1 == evidence.human_denial_record {
        Ok(())
    } else {
        Err(GenerationQualificationPhasePolicyRefusalError::DenialRecordMismatch)
    }
}

fn evidence_from_parts(
    manifests: SkippedPhaseManifests,
    resource_denial_record: Option<GenerationResourcePolicyDenialRecordV1>,
    human_denial_record: Option<GenerationHumanAdjudicationPolicyDenialRecordV1>,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
    receipt: GenerationQualificationOperationReceiptV1,
) -> PhasePolicyRefusalEvidence {
    PhasePolicyRefusalEvidence {
        attempt_ledger_manifest: manifests.attempt_ledger,
        repeatability_manifest: manifests.repeatability,
        resource_manifest: manifests.resource,
        human_adjudication_manifest: manifests.human_adjudication,
        resource_denial_record,
        human_denial_record,
        receipt_input,
        receipt,
    }
}

fn manifests_match(
    expected: &SkippedPhaseManifests,
    evidence: &PhasePolicyRefusalEvidence,
) -> bool {
    expected.attempt_ledger == evidence.attempt_ledger_manifest
        && expected.repeatability == evidence.repeatability_manifest
        && expected.resource == evidence.resource_manifest
        && expected.human_adjudication == evidence.human_adjudication_manifest
}

fn receipt_relations_from_evidence<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
    evidence: &'a PhasePolicyRefusalEvidence,
) -> GenerationQualificationOperationReceiptV1Relations<'a> {
    GenerationQualificationOperationReceiptV1Relations {
        operation_policy: view.operation_policy,
        request_projection: view.request_projection,
        platform_evidence: view.platform_evidence,
        license_evidence: view.license_evidence,
        attempt_ledger_manifest: &evidence.attempt_ledger_manifest,
        repeatability_manifest: &evidence.repeatability_manifest,
        resource_manifest: &evidence.resource_manifest,
        human_adjudication_manifest: &evidence.human_adjudication_manifest,
    }
}

const fn failed_receipt_input(
    elapsed_nanoseconds: u64,
) -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds,
        peak_concurrent_attempts: 0,
        terminal_status: GenerationQualificationOperationTerminalStatusV1::Failed,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    }
}

pub(super) fn checked_elapsed_nanoseconds(
    started: Instant,
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<u64, GenerationQualificationPhasePolicyRefusalError> {
    check_refusal_gate(deadline, cancellation)?;
    let observed = Instant::now();
    let elapsed = observed
        .checked_duration_since(started)
        .ok_or(GenerationQualificationPhasePolicyRefusalError::ElapsedUnavailable)?;
    let elapsed = u64::try_from(elapsed.as_nanos())
        .map_err(|_| GenerationQualificationPhasePolicyRefusalError::ElapsedUnavailable)?;
    if observed >= deadline {
        Err(GenerationQualificationPhasePolicyRefusalError::DeadlineExceeded)
    } else if cancellation.is_cancelled() {
        Err(GenerationQualificationPhasePolicyRefusalError::Cancelled)
    } else {
        Ok(elapsed)
    }
}

fn check_refusal_gate(
    deadline: Instant,
    cancellation: &CancellationToken,
) -> Result<(), GenerationQualificationPhasePolicyRefusalError> {
    check_gate(deadline, cancellation).map_err(super::map_preparation_error)
}
