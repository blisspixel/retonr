use rewrite_types::Digest;
use serde_json::Value;

use super::super::operation_policy::test_support;
use super::*;
use crate::generation_qualification::*;
use crate::{ModelLicenseControlId, ModelPackageFoundationId};

struct OperationFixture {
    base: test_support::Fixture,
    projection: GenerationQualificationRequestProjectionV1,
}

struct PhaseSet {
    ledger: GenerationAttemptLedgerManifestV1,
    repeatability: GenerationRepeatabilityEvidenceManifestV1,
    resource: GenerationResourceEvidenceManifestV1,
    human: GenerationHumanAdjudicationEvidenceManifestV1,
}

impl OperationFixture {
    fn new() -> Self {
        let base = test_support::fixture();
        let request_inputs = request_inputs(&base);
        let projection = GenerationQualificationRequestProjectionV1::new(
            projection_relations(&base),
            &request_inputs,
        )
        .expect("request projection");
        Self { base, projection }
    }

    fn platform(&self) -> GenerationQualificationPlatformEvidenceV1 {
        GenerationQualificationPlatformEvidenceV1::new(
            GenerationQualificationPlatformEvidenceV1Relations {
                operation_policy: &self.base.policy,
                request_projection: &self.projection,
                target_generation_system: &self.base.systems[0],
                target_generation_system_relations: self.base.system_fixture.relations(),
            },
            GenerationQualificationPlatformEvidenceV1Input {
                status: GenerationQualificationPlatformStatusV1::Supported,
                reason: GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu,
            },
        )
        .expect("platform evidence")
    }

    fn license(&self) -> GenerationQualificationLicenseEvidenceV1 {
        let foundation = ModelPackageFoundationId::from_derived_digest(digest("foundation"));
        let control = ModelLicenseControlId::from_derived_digest(digest("license control"));
        GenerationQualificationLicenseEvidenceV1::new(
            GenerationQualificationLicenseEvidenceV1Relations {
                operation_policy: &self.base.policy,
                request_projection: &self.projection,
                target_generation_system: &self.base.systems[0],
                target_generation_system_relations: self.base.system_fixture.relations(),
                model_package_foundation_id: &foundation,
                model_license_control_id: &control,
            },
            GenerationQualificationLicenseEvidenceV1Input {
                decision: GenerationQualificationLicenseDecisionV1::LocalUseOnly,
                reason: GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration,
            },
        )
        .expect("license evidence")
    }

    fn phases(&self, ledger_status: GenerationQualificationPhaseStatusV1) -> PhaseSet {
        let attempt_records = match ledger_status {
            GenerationQualificationPhaseStatusV1::Skipped => Vec::new(),
            GenerationQualificationPhaseStatusV1::Failed => {
                vec![failed_attempt(&self.base.attempts[0])]
            }
            GenerationQualificationPhaseStatusV1::Passed => {
                panic!("phase-interruption fixture does not need a passed ledger")
            }
        };
        let ledger =
            GenerationAttemptLedgerManifestV1::new(GenerationAttemptLedgerManifestV1Relations {
                scope: phase_scope(&self.base),
                phase_policy_digest: &self.base.policy_input.attempt_ledger_policy_digest,
                planned_attempts: &self.base.attempts,
                attempt_records: &attempt_records,
                status: ledger_status,
            })
            .expect("ledger manifest");
        let repeatability = GenerationRepeatabilityEvidenceManifestV1::new(
            GenerationRepeatabilityEvidenceManifestV1Relations {
                scope: phase_scope(&self.base),
                phase_policy_digest: &self.base.policy_input.repeatability_policy_digest,
                planned_attempts: &self.base.attempts,
                preregistered_repetitions: &self.base.repetitions,
                results: &[],
                status: GenerationQualificationPhaseStatusV1::Skipped,
            },
        )
        .expect("repeatability manifest");
        let resource = GenerationResourceEvidenceManifestV1::new(
            GenerationResourceEvidenceManifestV1Relations {
                scope: phase_scope(&self.base),
                phase_policy_digest: &self.base.policy_input.resource_policy_digest,
                evidence_record_digests: &[],
                status: GenerationQualificationPhaseStatusV1::Skipped,
            },
        )
        .expect("resource manifest");
        let human = GenerationHumanAdjudicationEvidenceManifestV1::new(
            GenerationHumanAdjudicationEvidenceManifestV1Relations {
                scope: phase_scope(&self.base),
                phase_policy_digest: &self.base.policy_input.human_adjudication_policy_digest,
                evidence_record_digests: &[],
                status: GenerationQualificationPhaseStatusV1::Skipped,
            },
        )
        .expect("human manifest");
        PhaseSet {
            ledger,
            repeatability,
            resource,
            human,
        }
    }
}

fn request_inputs(
    fixture: &test_support::Fixture,
) -> Vec<GenerationQualificationRequestProjectionEntryV1Input> {
    fixture
        .attempts
        .iter()
        .enumerate()
        .map(
            |(index, attempt)| GenerationQualificationRequestProjectionEntryV1Input {
                structured_completion_request_binding_id:
                    StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                        "phase interruption structured request {index}"
                    ))),
                complete_input_byte_count: attempt.source_byte_count() + 100,
                context_token_limit: fixture.policy.limits().maximum_context_tokens(),
                output_token_limit: fixture.policy.limits().maximum_output_tokens(),
            },
        )
        .collect()
}

fn projection_relations(
    fixture: &test_support::Fixture,
) -> GenerationQualificationRequestProjectionV1Relations<'_> {
    GenerationQualificationRequestProjectionV1Relations {
        operation_policy: &fixture.policy,
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
        planned_attempts: &fixture.attempts,
    }
}

fn phase_scope(fixture: &test_support::Fixture) -> GenerationQualificationPhaseScopeV1<'_> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[0],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
    }
}

fn failed_attempt(attempt: &PlannedCandidateAttemptV1) -> CandidateGenerationAttemptRecordV1 {
    CandidateGenerationAttemptRecordV1::failed(
        attempt,
        None,
        CandidateGenerationAttemptFailureV1Input {
            failure_phase: CandidateGenerationAttemptFailurePhaseV1::RequestCompilation,
            failure_category: CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
            traffic_observed: false,
            output_observed: false,
            cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1::NotRequired,
        },
    )
    .expect("failed attempt")
}

fn receipt_relations<'a>(
    fixture: &'a OperationFixture,
    platform: &'a GenerationQualificationPlatformEvidenceV1,
    license: &'a GenerationQualificationLicenseEvidenceV1,
    phases: &'a PhaseSet,
) -> GenerationQualificationOperationReceiptV1Relations<'a> {
    GenerationQualificationOperationReceiptV1Relations {
        operation_policy: &fixture.base.policy,
        request_projection: &fixture.projection,
        platform_evidence: platform,
        license_evidence: license,
        attempt_ledger_manifest: &phases.ledger,
        repeatability_manifest: &phases.repeatability,
        resource_manifest: &phases.resource,
        human_adjudication_manifest: &phases.human,
    }
}

const fn receipt_input(
    elapsed_nanoseconds: u64,
    peak_concurrent_attempts: u32,
    terminal_status: GenerationQualificationOperationTerminalStatusV1,
    finalization_status: GenerationQualificationOperationFinalizationStatusV1,
) -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds,
        peak_concurrent_attempts,
        terminal_status,
        finalization_status,
    }
}

fn interruption_relations<'a>(
    fixture: &'a OperationFixture,
    receipt: &'a GenerationQualificationOperationReceiptV1,
    receipt_relations: GenerationQualificationOperationReceiptV1Relations<'a>,
    receipt_input: GenerationQualificationOperationReceiptV1Input,
) -> GenerationQualificationPhaseInterruptionRecordV1Relations<'a> {
    GenerationQualificationPhaseInterruptionRecordV1Relations {
        operation_policy: &fixture.base.policy,
        operation_policy_relations: fixture.base.relations(),
        operation_policy_input: &fixture.base.policy_input,
        operation_receipt: receipt,
        operation_receipt_relations: receipt_relations,
        operation_receipt_input: receipt_input,
    }
}

fn interruption_input(
    fixture: &OperationFixture,
    checkpoint: GenerationQualificationPhaseCheckpointV1,
    reason: GenerationQualificationPhaseInterruptionReasonV1,
) -> GenerationQualificationPhaseInterruptionRecordV1Input {
    GenerationQualificationPhaseInterruptionRecordV1Input {
        phase: GenerationQualificationInterruptedPhaseV1::AttemptLedger,
        checkpoint,
        planned_attempt_id: Some(fixture.base.attempts[0].planned_attempt_id().clone()),
        reason,
    }
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn decode_value(
    value: &Value,
    relations: &GenerationQualificationPhaseInterruptionRecordV1Relations<'_>,
    input: &GenerationQualificationPhaseInterruptionRecordV1Input,
) -> Result<
    GenerationQualificationPhaseInterruptionRecordV1,
    GenerationQualificationOperationContractError,
> {
    GenerationQualificationPhaseInterruptionRecordV1::from_json_bytes(
        &serde_json::to_vec(value).expect("interruption JSON"),
        relations,
        input,
    )
}

fn reorder_first_two_fields(bytes: &[u8]) -> Vec<u8> {
    let value = String::from_utf8(bytes.to_vec()).expect("UTF-8 JSON");
    let first_comma = value.find(',').expect("first comma");
    let second_comma = value[first_comma + 1..]
        .find(',')
        .map(|index| index + first_comma + 1)
        .expect("second comma");
    format!(
        "{{{},{},{}",
        &value[first_comma + 1..second_comma],
        &value[1..first_comma],
        &value[second_comma + 1..]
    )
    .into_bytes()
}

#[path = "tests/closure.rs"]
mod closure;
#[path = "tests/codec.rs"]
mod codec;
