use rewrite_model::{
    CandidateGenerationAttemptOutcomeV1, CandidateGenerationAttemptRecordV1,
    GenerationAttemptLedgerManifestV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationHumanAdjudicationEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1Relations,
    GenerationQualificationInterruptedPhaseV1, GenerationQualificationLicenseDecisionV1,
    GenerationQualificationLicenseEvidenceV1, GenerationQualificationLicenseEvidenceV1Input,
    GenerationQualificationLicenseEvidenceV1Relations, GenerationQualificationLicenseReasonV1,
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationReceiptV1, GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseCheckpointV1,
    GenerationQualificationPhaseInterruptionReasonV1,
    GenerationQualificationPhaseInterruptionRecordV1,
    GenerationQualificationPhaseInterruptionRecordV1Input,
    GenerationQualificationPhaseInterruptionRecordV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationQualificationPlatformEvidenceV1,
    GenerationQualificationPlatformEvidenceV1Input,
    GenerationQualificationPlatformEvidenceV1Relations, GenerationQualificationPlatformReasonV1,
    GenerationQualificationPlatformStatusV1, GenerationRepeatabilityEvidenceManifestV1,
    GenerationRepeatabilityEvidenceManifestV1Relations, GenerationRepeatabilityResultRecordV1,
    GenerationRepeatabilityResultRecordV1Relations, GenerationRepeatabilityTerminalStageV1,
    GenerationResourceEvidenceManifestV1, GenerationResourceEvidenceManifestV1Relations,
    ManagedOllamaCandidateGenerationEvidenceV2Input, ModelLicenseControlId,
    ModelPackageFoundationId,
};
use rewrite_types::Digest;

use crate::store::candidate_generation_execution::tests::support as execution;
use crate::store::generation_qualification_preregistration::tests::support::Fixture;
use crate::store::generation_qualification_terminal_evidence::{
    GenerationQualificationTerminalEvidenceV1Input,
    GenerationQualificationTerminalEvidenceV1ReadInput,
};

#[derive(Clone, Copy)]
pub(super) struct CohortSpec<'a> {
    pub(super) attempts: &'a [CandidateGenerationAttemptRecordV1],
    pub(super) managed: &'a [ManagedOllamaCandidateGenerationEvidenceV2Input],
    pub(super) include_failed_result: bool,
    pub(super) receipt_input: GenerationQualificationOperationReceiptV1Input,
    pub(super) interruption_input:
        Option<&'a GenerationQualificationPhaseInterruptionRecordV1Input>,
}

pub(super) struct Cohort {
    pub(super) platform: GenerationQualificationPlatformEvidenceV1,
    platform_input: GenerationQualificationPlatformEvidenceV1Input,
    pub(super) license: GenerationQualificationLicenseEvidenceV1,
    license_input: GenerationQualificationLicenseEvidenceV1Input,
    foundation_id: ModelPackageFoundationId,
    control_id: ModelLicenseControlId,
    managed: Vec<ManagedOllamaCandidateGenerationEvidenceV2Input>,
    pub(super) ledger: GenerationAttemptLedgerManifestV1,
    pub(super) results: Vec<GenerationRepeatabilityResultRecordV1>,
    pub(super) repeatability: GenerationRepeatabilityEvidenceManifestV1,
    pub(super) resource: GenerationResourceEvidenceManifestV1,
    pub(super) human: GenerationHumanAdjudicationEvidenceManifestV1,
    resource_digests: Vec<Digest>,
    human_digests: Vec<Digest>,
    pub(super) receipt: GenerationQualificationOperationReceiptV1,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
    pub(super) interruption: Option<GenerationQualificationPhaseInterruptionRecordV1>,
    interruption_input: Option<GenerationQualificationPhaseInterruptionRecordV1Input>,
}

impl Cohort {
    pub(super) fn input<'a>(
        &'a self,
        fixture: &'a Fixture,
    ) -> GenerationQualificationTerminalEvidenceV1Input<'a> {
        GenerationQualificationTerminalEvidenceV1Input {
            preregistration: execution::read_input(fixture),
            platform_evidence: &self.platform,
            platform_input: self.platform_input,
            license_evidence: &self.license,
            license_input: self.license_input,
            model_package_foundation_id: &self.foundation_id,
            model_license_control_id: &self.control_id,
            managed_evidence_inputs: &self.managed,
            attempt_ledger_manifest: &self.ledger,
            repeatability_results: &self.results,
            repeatability_manifest: &self.repeatability,
            resource_manifest: &self.resource,
            resource_evidence_record_digests: &self.resource_digests,
            human_manifest: &self.human,
            human_evidence_record_digests: &self.human_digests,
            receipt: &self.receipt,
            receipt_input: self.receipt_input,
            phase_interruption: self.interruption.as_ref(),
            phase_interruption_input: self.interruption_input.as_ref(),
        }
    }

    pub(super) fn read<'a>(
        &'a self,
        fixture: &'a Fixture,
    ) -> GenerationQualificationTerminalEvidenceV1ReadInput<'a> {
        let input = self.input(fixture);
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
}

pub(super) fn cohort(fixture: &Fixture, spec: CohortSpec<'_>) -> Cohort {
    let (platform, platform_input, license, license_input, foundation_id, control_id) =
        platform_and_license(fixture);
    let (ledger, results) = ledger_and_results(fixture, &spec);
    let (repeatability, resource, human) = manifests(fixture, &results);
    let closure = Closure {
        fixture,
        platform: &platform,
        license: &license,
        ledger: &ledger,
        repeatability: &repeatability,
        resource: &resource,
        human: &human,
    };
    let receipt = GenerationQualificationOperationReceiptV1::new(
        receipt_relations(&closure),
        spec.receipt_input,
    )
    .expect("operation receipt");
    let interruption = spec.interruption_input.map(|facts| {
        GenerationQualificationPhaseInterruptionRecordV1::new(
            &interruption_relations(&closure, &receipt, spec.receipt_input),
            facts.clone(),
        )
        .expect("phase interruption")
    });
    Cohort {
        platform,
        platform_input,
        license,
        license_input,
        foundation_id,
        control_id,
        managed: spec.managed.to_vec(),
        ledger,
        results,
        repeatability,
        resource,
        human,
        resource_digests: Vec::new(),
        human_digests: Vec::new(),
        receipt,
        receipt_input: spec.receipt_input,
        interruption,
        interruption_input: spec.interruption_input.cloned(),
    }
}

pub(super) const fn failed_receipt() -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds: 1_000_000,
        peak_concurrent_attempts: 1,
        terminal_status: GenerationQualificationOperationTerminalStatusV1::Failed,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::Failed,
    }
}

pub(super) const fn cancelled_receipt() -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds: 1_000_000,
        peak_concurrent_attempts: 1,
        terminal_status: GenerationQualificationOperationTerminalStatusV1::Cancelled,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::Failed,
    }
}

pub(super) const fn completed_receipt() -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds: 1_000_000,
        peak_concurrent_attempts: 1,
        terminal_status: GenerationQualificationOperationTerminalStatusV1::Completed,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::Passed,
    }
}

pub(super) const fn skipped_receipt() -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds: 1_000_000,
        peak_concurrent_attempts: 0,
        terminal_status: GenerationQualificationOperationTerminalStatusV1::Failed,
        finalization_status: GenerationQualificationOperationFinalizationStatusV1::NotRequired,
    }
}

pub(super) fn receipt_at(
    elapsed_nanoseconds: u64,
) -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds,
        ..failed_receipt()
    }
}

pub(super) fn interruption_facts(
    fixture: &Fixture,
) -> GenerationQualificationPhaseInterruptionRecordV1Input {
    GenerationQualificationPhaseInterruptionRecordV1Input {
        phase: GenerationQualificationInterruptedPhaseV1::AttemptLedger,
        checkpoint: GenerationQualificationPhaseCheckpointV1::EvidenceAcquisition,
        planned_attempt_id: Some(fixture.attempts[0].planned_attempt_id().clone()),
        reason: GenerationQualificationPhaseInterruptionReasonV1::Cancelled,
    }
}

fn platform_and_license(
    fixture: &Fixture,
) -> (
    GenerationQualificationPlatformEvidenceV1,
    GenerationQualificationPlatformEvidenceV1Input,
    GenerationQualificationLicenseEvidenceV1,
    GenerationQualificationLicenseEvidenceV1Input,
    ModelPackageFoundationId,
    ModelLicenseControlId,
) {
    let platform_input = GenerationQualificationPlatformEvidenceV1Input {
        status: GenerationQualificationPlatformStatusV1::Supported,
        reason: GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu,
    };
    let platform = GenerationQualificationPlatformEvidenceV1::new(
        GenerationQualificationPlatformEvidenceV1Relations {
            operation_policy: &fixture.policy,
            request_projection: &fixture.projection,
            target_generation_system: &fixture.systems[0],
            target_generation_system_relations: fixture.system.relations(),
        },
        platform_input,
    )
    .expect("platform evidence");
    let license_input = GenerationQualificationLicenseEvidenceV1Input {
        decision: GenerationQualificationLicenseDecisionV1::LocalUseOnly,
        reason: GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration,
    };
    let foundation_id =
        ModelPackageFoundationId::from_derived_digest(Digest::sha256(b"model package foundation"));
    let control_id =
        ModelLicenseControlId::from_derived_digest(Digest::sha256(b"model license control"));
    let license = GenerationQualificationLicenseEvidenceV1::new(
        GenerationQualificationLicenseEvidenceV1Relations {
            operation_policy: &fixture.policy,
            request_projection: &fixture.projection,
            target_generation_system: &fixture.systems[0],
            target_generation_system_relations: fixture.system.relations(),
            model_package_foundation_id: &foundation_id,
            model_license_control_id: &control_id,
        },
        license_input,
    )
    .expect("license evidence");
    (
        platform,
        platform_input,
        license,
        license_input,
        foundation_id,
        control_id,
    )
}

fn ledger_and_results(
    fixture: &Fixture,
    spec: &CohortSpec<'_>,
) -> (
    GenerationAttemptLedgerManifestV1,
    Vec<GenerationRepeatabilityResultRecordV1>,
) {
    let status = ledger_status(fixture, spec.attempts);
    let relations = GenerationAttemptLedgerManifestV1Relations {
        scope: scope(fixture),
        phase_policy_digest: fixture.policy.attempt_ledger_policy_digest(),
        planned_attempts: &fixture.attempts,
        attempt_records: spec.attempts,
        status,
    };
    let ledger = GenerationAttemptLedgerManifestV1::new(relations).expect("attempt ledger");
    let digest = Digest::sha256(b"repeatability terminal");
    let results = if spec.include_failed_result {
        vec![
            GenerationRepeatabilityResultRecordV1::new(
                GenerationRepeatabilityResultRecordV1Relations {
                    scope: scope(fixture),
                    repetition: &fixture.repetitions[0],
                    attempt_ledger: &ledger,
                    attempt_ledger_relations: relations,
                    terminal_stage:
                        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
                    candidate_receipt_set: None,
                    deterministic_evaluation: None,
                    candidate_judge_join: None,
                    terminal_evidence_digest: &digest,
                },
            )
            .expect("repeatability result"),
        ]
    } else {
        Vec::new()
    };
    (ledger, results)
}

fn manifests(
    fixture: &Fixture,
    results: &[GenerationRepeatabilityResultRecordV1],
) -> (
    GenerationRepeatabilityEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1,
    GenerationHumanAdjudicationEvidenceManifestV1,
) {
    let repeatability_status = if results.is_empty() {
        GenerationQualificationPhaseStatusV1::Skipped
    } else {
        GenerationQualificationPhaseStatusV1::Failed
    };
    let repeatability = GenerationRepeatabilityEvidenceManifestV1::new(
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope: scope(fixture),
            phase_policy_digest: fixture.policy.repeatability_policy_digest(),
            planned_attempts: &fixture.attempts,
            preregistered_repetitions: &fixture.repetitions,
            results,
            status: repeatability_status,
        },
    )
    .expect("repeatability manifest");
    let resource =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope: scope(fixture),
            phase_policy_digest: fixture.policy.resource_policy_digest(),
            evidence_record_digests: &[],
            status: GenerationQualificationPhaseStatusV1::Skipped,
        })
        .expect("resource manifest");
    let human = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope: scope(fixture),
            phase_policy_digest: fixture.policy.human_adjudication_policy_digest(),
            evidence_record_digests: &[],
            status: GenerationQualificationPhaseStatusV1::Skipped,
        },
    )
    .expect("human manifest");
    (repeatability, resource, human)
}

struct Closure<'a> {
    fixture: &'a Fixture,
    platform: &'a GenerationQualificationPlatformEvidenceV1,
    license: &'a GenerationQualificationLicenseEvidenceV1,
    ledger: &'a GenerationAttemptLedgerManifestV1,
    repeatability: &'a GenerationRepeatabilityEvidenceManifestV1,
    resource: &'a GenerationResourceEvidenceManifestV1,
    human: &'a GenerationHumanAdjudicationEvidenceManifestV1,
}

fn receipt_relations<'a>(
    closure: &Closure<'a>,
) -> GenerationQualificationOperationReceiptV1Relations<'a> {
    GenerationQualificationOperationReceiptV1Relations {
        operation_policy: &closure.fixture.policy,
        request_projection: &closure.fixture.projection,
        platform_evidence: closure.platform,
        license_evidence: closure.license,
        attempt_ledger_manifest: closure.ledger,
        repeatability_manifest: closure.repeatability,
        resource_manifest: closure.resource,
        human_adjudication_manifest: closure.human,
    }
}

fn interruption_relations<'a>(
    closure: &'a Closure<'a>,
    receipt: &'a GenerationQualificationOperationReceiptV1,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
) -> GenerationQualificationPhaseInterruptionRecordV1Relations<'a> {
    GenerationQualificationPhaseInterruptionRecordV1Relations {
        operation_policy: &closure.fixture.policy,
        operation_policy_relations: closure.fixture.relations(),
        operation_policy_input: &closure.fixture.policy_input,
        operation_receipt: receipt,
        operation_receipt_relations: receipt_relations(closure),
        operation_receipt_input: receipt_input,
    }
}

fn scope(fixture: &Fixture) -> GenerationQualificationPhaseScopeV1<'_> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[0],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
    }
}

fn ledger_status(
    fixture: &Fixture,
    attempts: &[CandidateGenerationAttemptRecordV1],
) -> GenerationQualificationPhaseStatusV1 {
    let target_count = fixture
        .attempts
        .iter()
        .filter(|attempt| {
            attempt.generation_system_id() == fixture.systems[0].generation_system_id()
        })
        .count();
    let all_completed = attempts.iter().all(|attempt| {
        matches!(
            attempt.outcome(),
            CandidateGenerationAttemptOutcomeV1::Completed { .. }
        )
    });
    if attempts.is_empty() {
        GenerationQualificationPhaseStatusV1::Skipped
    } else if all_completed && attempts.len() == target_count {
        GenerationQualificationPhaseStatusV1::Passed
    } else {
        GenerationQualificationPhaseStatusV1::Failed
    }
}
