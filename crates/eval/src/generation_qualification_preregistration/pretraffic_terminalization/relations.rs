use rewrite_model::{
    GenerationAttemptLedgerManifestV1Relations,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationQualificationRecordV1Relations,
    GenerationRepeatabilityEvidenceManifestV1Relations,
    GenerationResourceEvidenceManifestV1Relations,
};

use super::{
    PreparedGenerationQualificationValidationView, RejectedPretrafficEvidence,
    SkippedPhaseManifests,
};

pub(super) fn phase_scope<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
) -> GenerationQualificationPhaseScopeV1<'a> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: view
            .operation_policy_relations
            .target_system
            .generation_system,
        qualification_plan: view.operation_policy_relations.plan,
        suite: view.operation_policy_relations.suite,
    }
}

pub(super) fn receipt_relations<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
    manifests: &'a SkippedPhaseManifests,
) -> GenerationQualificationOperationReceiptV1Relations<'a> {
    GenerationQualificationOperationReceiptV1Relations {
        operation_policy: view.operation_policy,
        request_projection: view.request_projection,
        platform_evidence: view.platform_evidence,
        license_evidence: view.license_evidence,
        attempt_ledger_manifest: &manifests.attempt_ledger,
        repeatability_manifest: &manifests.repeatability,
        resource_manifest: &manifests.resource,
        human_adjudication_manifest: &manifests.human_adjudication,
    }
}

pub(super) fn receipt_relations_from_evidence<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
    evidence: &'a RejectedPretrafficEvidence,
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

pub(super) fn record_relations<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
    manifests: &'a SkippedPhaseManifests,
    receipt: &'a GenerationQualificationOperationReceiptV1,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
) -> GenerationQualificationRecordV1Relations<'a> {
    GenerationQualificationRecordV1Relations {
        operation_receipt: receipt,
        operation_receipt_relations: receipt_relations(view, manifests),
        operation_receipt_input: receipt_input,
        operation_policy_relations: view.operation_policy_relations,
        operation_policy_input: view.operation_policy_input,
        request_projection_relations: view.request_projection_relations,
        request_projection_entry_inputs: view.request_projection_entry_inputs,
        platform_evidence_relations: view.platform_evidence_relations,
        platform_evidence_input: view.platform_evidence_input,
        license_evidence_relations: view.license_evidence_relations,
        license_evidence_input: view.license_evidence_input,
        attempt_ledger_relations: skipped_attempt_ledger_relations(view),
        repeatability_manifest_relations: skipped_repeatability_relations(view),
        repeatability_result_relations: &[],
        resource_manifest_relations: skipped_resource_relations(view),
        human_adjudication_manifest_relations: skipped_human_relations(view),
    }
}

pub(super) fn record_relations_from_evidence<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
    evidence: &'a RejectedPretrafficEvidence,
) -> GenerationQualificationRecordV1Relations<'a> {
    GenerationQualificationRecordV1Relations {
        operation_receipt: &evidence.receipt,
        operation_receipt_relations: receipt_relations_from_evidence(view, evidence),
        operation_receipt_input: evidence.receipt_input,
        operation_policy_relations: view.operation_policy_relations,
        operation_policy_input: view.operation_policy_input,
        request_projection_relations: view.request_projection_relations,
        request_projection_entry_inputs: view.request_projection_entry_inputs,
        platform_evidence_relations: view.platform_evidence_relations,
        platform_evidence_input: view.platform_evidence_input,
        license_evidence_relations: view.license_evidence_relations,
        license_evidence_input: view.license_evidence_input,
        attempt_ledger_relations: skipped_attempt_ledger_relations(view),
        repeatability_manifest_relations: skipped_repeatability_relations(view),
        repeatability_result_relations: &[],
        resource_manifest_relations: skipped_resource_relations(view),
        human_adjudication_manifest_relations: skipped_human_relations(view),
    }
}

fn skipped_attempt_ledger_relations<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
) -> GenerationAttemptLedgerManifestV1Relations<'a> {
    GenerationAttemptLedgerManifestV1Relations {
        scope: phase_scope(view),
        phase_policy_digest: view.operation_policy.attempt_ledger_policy_digest(),
        planned_attempts: view.operation_policy_relations.planned_attempts,
        attempt_records: &[],
        status: GenerationQualificationPhaseStatusV1::Skipped,
    }
}

fn skipped_repeatability_relations<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
) -> GenerationRepeatabilityEvidenceManifestV1Relations<'a> {
    GenerationRepeatabilityEvidenceManifestV1Relations {
        scope: phase_scope(view),
        phase_policy_digest: view.operation_policy.repeatability_policy_digest(),
        planned_attempts: view.operation_policy_relations.planned_attempts,
        preregistered_repetitions: view.operation_policy_relations.repetitions,
        results: &[],
        status: GenerationQualificationPhaseStatusV1::Skipped,
    }
}

fn skipped_resource_relations<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
) -> GenerationResourceEvidenceManifestV1Relations<'a> {
    GenerationResourceEvidenceManifestV1Relations {
        scope: phase_scope(view),
        phase_policy_digest: view.operation_policy.resource_policy_digest(),
        evidence_record_digests: &[],
        status: GenerationQualificationPhaseStatusV1::Skipped,
    }
}

fn skipped_human_relations<'a>(
    view: &'a PreparedGenerationQualificationValidationView<'a>,
) -> GenerationHumanAdjudicationEvidenceManifestV1Relations<'a> {
    GenerationHumanAdjudicationEvidenceManifestV1Relations {
        scope: phase_scope(view),
        phase_policy_digest: view.operation_policy.human_adjudication_policy_digest(),
        evidence_record_digests: &[],
        status: GenerationQualificationPhaseStatusV1::Skipped,
    }
}
