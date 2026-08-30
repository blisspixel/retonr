use rewrite_app::{
    GenerationQualificationLicenseAssessmentCompiler, SyntheticGenerationQualificationScenario,
    with_synthetic_generation_qualification_fixture,
};
use rewrite_model::{
    GenerationQualificationOperationFinalizationStatusV1,
    GenerationQualificationOperationTerminalStatusV1, GenerationQualificationPhaseStatusV1,
};
use rewrite_types::CancellationToken;
use tempfile::tempdir;

use super::*;
use crate::{
    GenerationQualificationOperationDraft, GenerationQualificationPreregistrationRepository,
};

#[test]
fn terminalization_disposition_preserves_independent_failures() {
    assert!(CandidateCloseoutInterruptionDisposition::Available.available());
    assert!(
        CandidateCloseoutInterruptionDisposition::EvidenceCompilationFailed.compilation_failed()
    );
    assert!(
        CandidateCloseoutInterruptionDisposition::MandatoryFinalizationFailed.finalization_failed()
    );
    let both = CandidateCloseoutInterruptionDisposition::PrimaryAndFinalizationFailed;
    assert!(both.compilation_failed());
    assert!(both.finalization_failed());
    assert!(!both.available());
}

#[test]
fn interruption_error_debug_is_content_free() {
    let error = ActiveGenerationQualificationOperationInterruptionError::EvidenceCompilation;
    assert_eq!(
        error.kind(),
        ActiveGenerationQualificationOperationInterruptionErrorKind::EvidenceCompilation
    );
    assert_eq!(
        format!("{error:?}"),
        "ActiveGenerationQualificationOperationInterruptionError { kind: EvidenceCompilation, .. }"
    );
}

#[test]
fn active_compiles_and_freshly_revalidates_exact_interruption_closure() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
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
            let mut active = prepared.activate(&cancellation).expect("active operation");
            let planned_attempt_id = input.operation_policy_relations.planned_attempts[0]
                .planned_attempt_id()
                .clone();
            assert_exact_interruption(&mut active, &planned_attempt_id);
        },
    );
}

fn assert_exact_interruption(
    active: &mut ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    planned_attempt_id: &PlannedCandidateAttemptId,
) {
    active.lifecycle.mark_authority_acquired_for_test();
    active.next_candidate_attempt = 1;
    active.terminal = true;
    let disposition =
        active.compile_candidate_closeout_interruption(CandidateCloseoutInterruptionFacts {
            planned_attempt_id: planned_attempt_id.clone(),
            reason: GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed,
            terminal_status: GenerationQualificationOperationTerminalStatusV1::Failed,
            observed_at: Instant::now(),
        });
    assert_eq!(
        disposition,
        CandidateCloseoutInterruptionDisposition::Available
    );
    let interruption = active
        .operation_interruption()
        .expect("operation interruption");
    for status in [
        interruption.attempt_ledger().manifest().status(),
        interruption.repeatability_manifest().status(),
        interruption.resource_manifest().status(),
        interruption.human_adjudication_manifest().status(),
    ] {
        assert_eq!(status, GenerationQualificationPhaseStatusV1::Skipped);
    }
    assert_eq!(
        interruption.receipt().terminal_status(),
        GenerationQualificationOperationTerminalStatusV1::Failed
    );
    assert_eq!(
        interruption.receipt().finalization_status(),
        GenerationQualificationOperationFinalizationStatusV1::Passed
    );
    assert_eq!(interruption.receipt().peak_concurrent_attempts(), 1);
    assert_eq!(
        interruption.interruption().planned_attempt_id(),
        Some(planned_attempt_id)
    );
    assert_eq!(
        interruption.interruption().reason(),
        GenerationQualificationPhaseInterruptionReasonV1::EvidenceCompilationFailed
    );
    active
        .revalidate_operation_interruption()
        .expect("fresh operation interruption validation");
    assert_eq!(
        active
            .seal_target_attempt_ledger()
            .expect_err("interruption already owns the sealed ledger")
            .kind(),
        crate::ActiveGenerationQualificationAttemptLedgerErrorKind::AlreadySealed
    );
}
