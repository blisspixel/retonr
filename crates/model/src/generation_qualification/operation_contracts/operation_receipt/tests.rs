use rewrite_types::Digest;
use serde_json::Value;

use super::super::operation_policy::test_support;
use super::*;
use crate::generation_qualification::*;
use crate::{ArtifactId, ArtifactSetRelativePath, ModelLicenseControlId, ModelPackageFoundationId};

struct OperationFixture {
    base: test_support::Fixture,
    request_inputs: Vec<GenerationQualificationRequestProjectionEntryV1Input>,
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
        Self {
            base,
            request_inputs,
            projection,
        }
    }

    fn platform(
        &self,
        status: GenerationQualificationPlatformStatusV1,
    ) -> GenerationQualificationPlatformEvidenceV1 {
        let reason = match status {
            GenerationQualificationPlatformStatusV1::Supported => {
                GenerationQualificationPlatformReasonV1::ReviewedManagedLinuxNativeCpu
            }
            GenerationQualificationPlatformStatusV1::Rejected => {
                GenerationQualificationPlatformReasonV1::AssessmentPolicyDenied
            }
        };
        GenerationQualificationPlatformEvidenceV1::new(
            GenerationQualificationPlatformEvidenceV1Relations {
                operation_policy: &self.base.policy,
                request_projection: &self.projection,
                target_generation_system: &self.base.systems[0],
                target_generation_system_relations: self.base.system_fixture.relations(),
            },
            GenerationQualificationPlatformEvidenceV1Input { status, reason },
        )
        .expect("platform evidence")
    }

    fn license(
        &self,
        decision: GenerationQualificationLicenseDecisionV1,
    ) -> GenerationQualificationLicenseEvidenceV1 {
        let reason = match decision {
            GenerationQualificationLicenseDecisionV1::LocalUseOnly => {
                GenerationQualificationLicenseReasonV1::ApprovedLocalGeneration
            }
            GenerationQualificationLicenseDecisionV1::Rejected => {
                GenerationQualificationLicenseReasonV1::ApprovalPolicyDenied
            }
        };
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
            GenerationQualificationLicenseEvidenceV1Input { decision, reason },
        )
        .expect("license evidence")
    }

    fn skipped_phases(&self) -> PhaseSet {
        phases(
            &self.base,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Skipped,
        )
    }

    fn failed_ledger_phases(&self) -> PhaseSet {
        phases(
            &self.base,
            GenerationQualificationPhaseStatusV1::Failed,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Skipped,
        )
    }

    fn passed_ledger_then_skipped_phases(&self) -> PhaseSet {
        phases(
            &self.base,
            GenerationQualificationPhaseStatusV1::Passed,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Skipped,
            GenerationQualificationPhaseStatusV1::Skipped,
        )
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
                        "structured request {index}"
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
    peak_concurrent_attempts: u32,
    terminal_status: GenerationQualificationOperationTerminalStatusV1,
    finalization_status: GenerationQualificationOperationFinalizationStatusV1,
    elapsed_nanoseconds: u64,
) -> GenerationQualificationOperationReceiptV1Input {
    GenerationQualificationOperationReceiptV1Input {
        elapsed_nanoseconds,
        peak_concurrent_attempts,
        terminal_status,
        finalization_status,
    }
}

fn phase_scope(fixture: &test_support::Fixture) -> GenerationQualificationPhaseScopeV1<'_> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[0],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
    }
}

fn phases(
    fixture: &test_support::Fixture,
    ledger_status: GenerationQualificationPhaseStatusV1,
    repeatability_status: GenerationQualificationPhaseStatusV1,
    resource_status: GenerationQualificationPhaseStatusV1,
    human_status: GenerationQualificationPhaseStatusV1,
) -> PhaseSet {
    let target_attempts = fixture
        .attempts
        .iter()
        .filter(|attempt| {
            attempt.generation_system_id() == fixture.systems[0].generation_system_id()
        })
        .collect::<Vec<_>>();
    let attempt_records = match ledger_status {
        GenerationQualificationPhaseStatusV1::Skipped => Vec::new(),
        GenerationQualificationPhaseStatusV1::Failed => vec![failed_attempt(target_attempts[0])],
        GenerationQualificationPhaseStatusV1::Passed => target_attempts
            .iter()
            .map(|attempt| completed_attempt(fixture, attempt))
            .collect(),
    };
    let ledger_relations = GenerationAttemptLedgerManifestV1Relations {
        scope: phase_scope(fixture),
        phase_policy_digest: &fixture.policy_input.attempt_ledger_policy_digest,
        planned_attempts: &fixture.attempts,
        attempt_records: &attempt_records,
        status: ledger_status,
    };
    let ledger = GenerationAttemptLedgerManifestV1::new(ledger_relations).expect("ledger manifest");

    let terminal_digest = digest("repeatability terminal");
    let repeatability_results = match repeatability_status {
        GenerationQualificationPhaseStatusV1::Skipped => Vec::new(),
        GenerationQualificationPhaseStatusV1::Failed => vec![
            GenerationRepeatabilityResultRecordV1::new(
                GenerationRepeatabilityResultRecordV1Relations {
                    scope: phase_scope(fixture),
                    repetition: &fixture.repetitions[0],
                    attempt_ledger: &ledger,
                    attempt_ledger_relations: ledger_relations,
                    terminal_stage:
                        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
                    candidate_receipt_set: None,
                    deterministic_evaluation: None,
                    candidate_judge_join: None,
                    terminal_evidence_digest: &terminal_digest,
                },
            )
            .expect("failed repetition result"),
        ],
        GenerationQualificationPhaseStatusV1::Passed => {
            panic!("the receipt fixture does not synthesize opaque passed judge joins")
        }
    };
    let repeatability = GenerationRepeatabilityEvidenceManifestV1::new(
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope: phase_scope(fixture),
            phase_policy_digest: &fixture.policy_input.repeatability_policy_digest,
            planned_attempts: &fixture.attempts,
            preregistered_repetitions: &fixture.repetitions,
            results: &repeatability_results,
            status: repeatability_status,
        },
    )
    .expect("repeatability manifest");

    let resource_digests = evidence_digests(resource_status, "resource evidence");
    let resource =
        GenerationResourceEvidenceManifestV1::new(GenerationResourceEvidenceManifestV1Relations {
            scope: phase_scope(fixture),
            phase_policy_digest: &fixture.policy_input.resource_policy_digest,
            evidence_record_digests: &resource_digests,
            status: resource_status,
        })
        .expect("resource manifest");
    let human_digests = evidence_digests(human_status, "human evidence");
    let human = GenerationHumanAdjudicationEvidenceManifestV1::new(
        GenerationHumanAdjudicationEvidenceManifestV1Relations {
            scope: phase_scope(fixture),
            phase_policy_digest: &fixture.policy_input.human_adjudication_policy_digest,
            evidence_record_digests: &human_digests,
            status: human_status,
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

fn evidence_digests(status: GenerationQualificationPhaseStatusV1, label: &str) -> Vec<Digest> {
    match status {
        GenerationQualificationPhaseStatusV1::Skipped => Vec::new(),
        GenerationQualificationPhaseStatusV1::Passed
        | GenerationQualificationPhaseStatusV1::Failed => vec![digest(label)],
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

#[expect(
    clippy::too_many_lines,
    reason = "the fixture constructs the complete successful receipt closure"
)]
fn completed_attempt(
    fixture: &test_support::Fixture,
    attempt: &PlannedCandidateAttemptV1,
) -> CandidateGenerationAttemptRecordV1 {
    let case = fixture
        .cases
        .iter()
        .find(|case| case.case_id() == attempt.case_id())
        .expect("attempt case");
    let repetition = fixture
        .repetitions
        .iter()
        .find(|repetition| repetition.repetition_id() == attempt.repetition_id())
        .expect("attempt repetition");
    let precursor = CandidateGenerationAttemptPrecursorV1::new(
        &fixture.plan,
        attempt,
        &fixture.systems[0],
        CandidateGenerationAttemptPrecursorV1Input {
            runtime_installation_generation: u64::from(attempt.attempt_ordinal()) + 1,
            model_installation_generation: u64::from(attempt.attempt_ordinal()) + 100,
            structured_request_binding_id:
                StructuredCompletionRequestBindingId::from_derived_digest(digest(&format!(
                    "completed request {}",
                    attempt.attempt_ordinal()
                ))),
        },
    )
    .expect("attempt precursor");
    let managed = ManagedOllamaCandidateGenerationEvidenceV2::new(
        ManagedOllamaCandidateGenerationEvidenceV2Relations {
            precursor: &precursor,
            planned_attempt: attempt,
            generation_system: &fixture.systems[0],
            effective_package_evidence_v2: &fixture.system_fixture.effective_package,
        },
        ManagedOllamaCandidateGenerationEvidenceV2Input {
            bracket_observation_v1_id:
                ManagedOllamaGenerationBracketObservationV1Id::from_derived_digest(digest(
                    "completed bracket",
                )),
            effective_runtime_state_join_id:
                ManagedOllamaEffectiveRuntimeStateJoinId::from_derived_digest(digest(
                    "completed runtime join",
                )),
            response_id: OllamaRetainedSessionResponseId::from_derived_digest(digest(
                "completed response",
            )),
        },
    )
    .expect("managed evidence");
    let cleanup = CandidateGenerationCleanupRecordV1::new(
        &precursor,
        &managed,
        CandidateGenerationCleanupRecordV1Input {
            process_cleanup_status: CandidateGenerationProcessCleanupStatusV1::Succeeded,
            runtime_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
            model_package_revalidation_status:
                CandidateGenerationPackageRevalidationStatusV1::Verified,
        },
    )
    .expect("cleanup");
    let response = format!("response {}", attempt.attempt_ordinal()).into_bytes();
    let response_artifact = StructuredResponseArtifactV1Input::new(
        ArtifactId::from_digest(Digest::sha256(&response)),
        u64::try_from(response.len()).expect("response length"),
    )
    .expect("response artifact");
    let candidate_bytes = format!("candidate {}", attempt.attempt_ordinal()).into_bytes();
    let candidate = CandidateArtifactEntryV1::new(
        &precursor,
        attempt,
        case,
        0,
        path("candidates/000.txt"),
        &candidate_bytes,
    )
    .expect("candidate artifact");
    let mut entries = vec![
        record_entry(
            "records/planned.json",
            CandidateGenerationEvidenceBundleRoleV1::PlannedAttempt,
            attempt,
        ),
        record_entry(
            "records/precursor.json",
            CandidateGenerationEvidenceBundleRoleV1::AttemptPrecursor,
            &precursor,
        ),
        record_entry(
            "records/managed.json",
            CandidateGenerationEvidenceBundleRoleV1::ManagedGenerationEvidence,
            &managed,
        ),
        CandidateGenerationEvidenceBundleEntryV1::new(
            path("records/response.json"),
            CandidateGenerationEvidenceBundleRoleV1::StructuredResponse,
            response_artifact.artifact_id().clone(),
            u64::try_from(response.len()).expect("response length"),
        ),
        record_entry(
            "records/cleanup.json",
            CandidateGenerationEvidenceBundleRoleV1::CleanupRecord,
            &cleanup,
        ),
        CandidateGenerationEvidenceBundleEntryV1::new(
            candidate.relative_path().clone(),
            CandidateGenerationEvidenceBundleRoleV1::Candidate,
            candidate.artifact_id().clone(),
            candidate.byte_count(),
        ),
    ];
    entries.sort_by(|left, right| left.relative_path().cmp(right.relative_path()));
    let bundle = CandidateGenerationEvidenceBundleManifestV1::new(
        CandidateGenerationEvidenceBundleManifestV1Relations {
            qualification_plan: &fixture.plan,
            planned_attempt: attempt,
            precursor: &precursor,
            managed_evidence: &managed,
            cleanup: &cleanup,
            structured_response_artifact: &response_artifact,
        },
        entries,
        vec![candidate],
    )
    .expect("evidence bundle");
    let readback = CandidateGenerationEvidenceBundleReadbackV1::new(&bundle).expect("readback");
    let receipt = CandidateGenerationReceiptV1::new(
        CandidateGenerationReceiptV1Relations {
            qualification_plan: &fixture.plan,
            suite: &fixture.suite,
            case,
            cluster: &fixture.cluster,
            repetition,
            planned_attempt: attempt,
            precursor: &precursor,
            generation_system: &fixture.systems[0],
            managed_evidence: &managed,
            cleanup: &cleanup,
            bundle: &bundle,
            readback: &readback,
        },
        CandidateGenerationUsageObservationV1::new(Some(2), Some(3), Some(5)),
    )
    .expect("candidate receipt");
    CandidateGenerationAttemptRecordV1::completed(attempt, &precursor, &receipt)
        .expect("completed attempt")
}

fn record_entry<T: serde::Serialize>(
    relative_path: &str,
    role: CandidateGenerationEvidenceBundleRoleV1,
    record: &T,
) -> CandidateGenerationEvidenceBundleEntryV1 {
    let bytes = serde_json::to_vec(record).expect("record JSON");
    CandidateGenerationEvidenceBundleEntryV1::new(
        path(relative_path),
        role,
        ArtifactId::from_digest(Digest::sha256(&bytes)),
        u64::try_from(bytes.len()).expect("record length"),
    )
}

fn path(value: &str) -> ArtifactSetRelativePath {
    ArtifactSetRelativePath::new(value).expect("fixture path")
}

fn digest(label: &str) -> Digest {
    Digest::sha256(label.as_bytes())
}

fn receipt_value(receipt: &GenerationQualificationOperationReceiptV1) -> Value {
    serde_json::to_value(receipt).expect("receipt value")
}

fn decode_value(
    value: &Value,
    relations: GenerationQualificationOperationReceiptV1Relations<'_>,
    input: GenerationQualificationOperationReceiptV1Input,
) -> Result<GenerationQualificationOperationReceiptV1, GenerationQualificationOperationContractError>
{
    GenerationQualificationOperationReceiptV1::from_json_bytes(
        &serde_json::to_vec(value).expect("receipt JSON"),
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

#[path = "tests/construction.rs"]
mod construction;
#[path = "tests/terminal.rs"]
mod terminal;
