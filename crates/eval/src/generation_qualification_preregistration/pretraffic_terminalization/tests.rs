use std::time::Duration;

use rewrite_app::{
    GenerationQualificationLicenseAssessmentCompiler, SyntheticGenerationQualificationScenario,
    with_synthetic_generation_qualification_fixture,
};
use rewrite_model::{
    GenerationQualificationOperationContractError, GenerationQualificationOperationLimitsV1,
    GenerationQualificationOperationReceiptV1Relations,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseStatusV1,
    GenerationQualificationStatusV1, GenerationResourceEvidenceManifestV1,
    GenerationResourceEvidenceManifestV1Relations,
};
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::*;
use crate::{
    GenerationQualificationOperationDraft, GenerationQualificationPreregistrationRepository,
};

const TEST_MAXIMUM_ELAPSED_MILLISECONDS: u32 = 60_000;

#[test]
fn license_rejection_closes_exact_empty_phases_receipt_and_record() {
    with_finalized(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |finalized| {
            assert_exact_rejected_terminal(finalized);
            finalized
                .revalidate(&CancellationToken::new())
                .expect("fresh terminal revalidation");
        },
    );
}

#[test]
fn platform_rejection_closes_exact_empty_phases_receipt_and_record() {
    with_finalized(
        SyntheticGenerationQualificationScenario::BothRejected,
        |finalized| {
            assert_exact_rejected_terminal(finalized);
            assert_eq!(
                finalized.record().status(),
                GenerationQualificationStatusV1::Rejected
            );
        },
    );
}

#[test]
fn traffic_eligible_disposition_is_refused() {
    assert_eq!(
        ensure_rejected_disposition(GenerationQualificationPreparationDisposition::TrafficEligible),
        Err(GenerationQualificationPretrafficTerminalizationError::TrafficEligible)
    );
}

#[test]
fn substituted_phase_policy_cannot_revalidate_the_receipt() {
    with_finalized(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |finalized| {
            let FinalizedRejectedPretrafficGenerationQualification { prepared, evidence } =
                finalized;
            prepared
                .with_validated_view(&CancellationToken::new(), |view| {
                    let substituted_digest = Digest::sha256(b"substituted resource phase policy");
                    let substituted = GenerationResourceEvidenceManifestV1::new(
                        GenerationResourceEvidenceManifestV1Relations {
                            scope: relations::phase_scope(&view),
                            phase_policy_digest: &substituted_digest,
                            evidence_record_digests: &[],
                            status: GenerationQualificationPhaseStatusV1::Skipped,
                        },
                    )
                    .expect("structurally valid foreign resource manifest");
                    let relations = GenerationQualificationOperationReceiptV1Relations {
                        operation_policy: view.operation_policy,
                        request_projection: view.request_projection,
                        platform_evidence: view.platform_evidence,
                        license_evidence: view.license_evidence,
                        attempt_ledger_manifest: &evidence.attempt_ledger_manifest,
                        repeatability_manifest: &evidence.repeatability_manifest,
                        resource_manifest: &substituted,
                        human_adjudication_manifest: &evidence.human_adjudication_manifest,
                    };
                    assert_eq!(
                        evidence
                            .receipt
                            .validate_against(relations, evidence.receipt_input),
                        Err(GenerationQualificationOperationContractError::ScopeMismatch)
                    );
                    Ok::<_, ()>(())
                })
                .expect("outer Prepared validation remains exact");
        },
    );
}

#[test]
fn active_deadline_observes_cancellation() {
    with_prepared(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |prepared| {
            let cancellation = CancellationToken::new();
            cancellation.cancel();
            assert!(matches!(
                prepared.finalize_rejected_pretraffic(&cancellation),
                Err(GenerationQualificationPretrafficTerminalizationError::ValidationAggregation)
            ));
        },
    );
}

#[test]
fn prepared_bracket_preserves_callback_and_final_failures() {
    with_prepared(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |mut prepared| {
            let cancellation = CancellationToken::new();
            let result = prepared.with_validated_view(&cancellation, |_view| {
                cancellation.cancel();
                Err::<(), _>("callback failure")
            });
            assert!(matches!(
                result,
                Err(
                    PreparedGenerationQualificationValidationError::CallbackAndFinal {
                        callback: "callback failure",
                        final_validation: GenerationQualificationPreparationError::Cancelled,
                    }
                )
            ));
        },
    );
}

#[test]
fn prepared_bracket_preserves_initial_and_final_failures() {
    with_prepared(
        SyntheticGenerationQualificationScenario::BothRejected,
        |mut prepared| {
            let cancellation = CancellationToken::new();
            cancellation.cancel();
            let result: Result<(), PreparedGenerationQualificationValidationError<()>> = prepared
                .with_validated_view(&cancellation, |_view| {
                    panic!("callback must not run after failed initial validation")
                });
            assert!(matches!(
                result,
                Err(
                    PreparedGenerationQualificationValidationError::InitialAndFinal {
                        initial: GenerationQualificationPreparationError::Cancelled,
                        final_validation: GenerationQualificationPreparationError::Cancelled,
                    }
                )
            ));
        },
    );
}

#[test]
fn elapsed_deadline_precedes_cancellation() {
    with_prepared(
        SyntheticGenerationQualificationScenario::BothRejected,
        |mut prepared| {
            prepared.expire_deadline_for_test();
            let cancellation = CancellationToken::new();
            cancellation.cancel();
            let result: Result<(), PreparedGenerationQualificationValidationError<()>> =
                prepared.with_validated_view(&cancellation, |_view| Ok(()));
            assert!(matches!(
                result,
                Err(
                    PreparedGenerationQualificationValidationError::InitialAndFinal {
                        initial: GenerationQualificationPreparationError::DeadlineExceeded,
                        final_validation: GenerationQualificationPreparationError::DeadlineExceeded,
                    }
                )
            ));
        },
    );
}

#[test]
fn checked_elapsed_rejects_a_reversed_monotonic_interval() {
    let cancellation = CancellationToken::new();
    let started = std::time::Instant::now() + Duration::from_secs(1);
    assert_eq!(
        checked_elapsed_nanoseconds(started, started + Duration::from_secs(1), &cancellation,),
        Err(GenerationQualificationPretrafficTerminalizationError::ElapsedUnavailable)
    );
}

fn assert_exact_rejected_terminal(
    finalized: &FinalizedRejectedPretrafficGenerationQualification<'_, '_, '_, '_, '_>,
) {
    for (count, status) in [
        (
            finalized.attempt_ledger_manifest().evidence_item_count(),
            finalized.attempt_ledger_manifest().status(),
        ),
        (
            finalized.repeatability_manifest().evidence_item_count(),
            finalized.repeatability_manifest().status(),
        ),
        (
            finalized.resource_manifest().evidence_item_count(),
            finalized.resource_manifest().status(),
        ),
        (
            finalized
                .human_adjudication_manifest()
                .evidence_item_count(),
            finalized.human_adjudication_manifest().status(),
        ),
    ] {
        assert_eq!(count, 0);
        assert_eq!(status, GenerationQualificationPhaseStatusV1::Skipped);
    }
    let receipt = finalized.receipt();
    let deadline_nanoseconds = u64::from(
        finalized
            .prepared
            .operation_policy()
            .limits()
            .maximum_elapsed_milliseconds(),
    ) * 1_000_000;
    assert!(receipt.elapsed_nanoseconds() < deadline_nanoseconds);
    assert_eq!(receipt.peak_concurrent_attempts(), 0);
    assert_eq!(
        receipt.terminal_status(),
        GenerationQualificationOperationTerminalStatusV1::Completed
    );
    assert_eq!(
        receipt.finalization_status(),
        GenerationQualificationOperationFinalizationStatusV1::NotRequired
    );
    let record = finalized.record();
    assert_eq!(record.status(), GenerationQualificationStatusV1::Rejected);
    assert_eq!(record.operation_policy_id(), receipt.operation_policy_id());
    assert_eq!(
        record.request_projection_id(),
        receipt.request_projection_id()
    );
    assert_eq!(
        record.platform_evidence_id(),
        receipt.platform_evidence_id()
    );
    assert_eq!(record.license_evidence_id(), receipt.license_evidence_id());
    assert_eq!(
        record.operation_receipt_id(),
        receipt.operation_receipt_id()
    );
}

fn with_finalized(
    scenario: SyntheticGenerationQualificationScenario,
    use_finalized: impl for<'records, 'store, 'platform, 'proof, 'lease> FnOnce(
        &mut FinalizedRejectedPretrafficGenerationQualification<
            'records,
            'store,
            'platform,
            'proof,
            'lease,
        >,
    ),
) {
    with_prepared(scenario, |prepared| {
        let mut finalized = prepared
            .finalize_rejected_pretraffic(&CancellationToken::new())
            .expect("rejected pretraffic terminal");
        use_finalized(&mut finalized);
    });
}

fn with_prepared(
    scenario: SyntheticGenerationQualificationScenario,
    use_prepared: impl for<'records, 'store, 'platform, 'proof, 'lease> FnOnce(
        PreparedGenerationQualificationOperation<'records, 'store, 'platform, 'proof, 'lease>,
    ),
) {
    with_synthetic_generation_qualification_fixture(
        scenario,
        |mut input, platform_owners, license_proof, license_policy, production_policy| {
            input.operation_policy_input.limits = limits_with_elapsed(
                input.operation_policy_input.limits,
                TEST_MAXIMUM_ELAPSED_MILLISECONDS,
            );
            let cancellation = CancellationToken::new();
            let draft = GenerationQualificationOperationDraft::begin(
                input.operation_policy_relations,
                input.operation_policy_input,
                &cancellation,
            )
            .expect("draft");
            let projected = draft
                .project(input.case_authorities, &cancellation)
                .expect("projection");
            let platform = platform_owners
                .assess(projected.platform_portable_relations(), &cancellation)
                .expect("platform assessment");
            let assessment_input = projected.license_assessment_input(
                &license_policy,
                license_proof.control(),
                license_proof.selected_lease(),
                &production_policy,
            );
            let license = GenerationQualificationLicenseAssessmentCompiler::compile(
                &assessment_input,
                &cancellation,
            )
            .expect("license assessment");
            let directory = tempdir().expect("temporary repository directory");
            let mut repository = GenerationQualificationPreregistrationRepository::open(
                &directory.path().join("qualification.db"),
            )
            .expect("durable repository");
            let prepared = projected
                .finish(
                    &mut repository,
                    input.foundation,
                    platform,
                    license,
                    license_policy,
                    production_policy,
                    &cancellation,
                )
                .expect("prepared operation");
            use_prepared(prepared);
        },
    );
}

fn limits_with_elapsed(
    limits: GenerationQualificationOperationLimitsV1,
    maximum_elapsed_milliseconds: u32,
) -> GenerationQualificationOperationLimitsV1 {
    GenerationQualificationOperationLimitsV1::new(
        limits.maximum_source_bytes(),
        limits.maximum_complete_input_bytes(),
        limits.maximum_context_tokens(),
        limits.maximum_output_tokens(),
        limits.maximum_output_bytes(),
        limits.maximum_candidates_per_completion(),
        limits.maximum_candidate_bytes(),
        limits.maximum_aggregate_candidate_bytes(),
        limits.maximum_predeclared_attempts(),
        limits.maximum_concurrent_attempts(),
        maximum_elapsed_milliseconds,
    )
    .expect("valid synthetic operation limits")
}
