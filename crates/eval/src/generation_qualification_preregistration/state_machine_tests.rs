use std::time::Duration;

use rewrite_app::{
    GenerationQualificationAssessmentPolicySourceDisposition,
    GenerationQualificationLicenseAssessmentCompiler,
    GenerationQualificationPlatformAssessmentError, SyntheticGenerationQualificationScenario,
    with_synthetic_generation_qualification_fixture,
};
use rewrite_model::GenerationQualificationOperationLimitsV1;
use rewrite_types::{CancellationToken, Digest};
use tempfile::tempdir;

use super::{
    GenerationQualificationActivationError, GenerationQualificationOperationDraft,
    GenerationQualificationPreparationDisposition, GenerationQualificationPreparationError,
    GenerationQualificationPreregistrationRepository,
};

#[test]
fn traffic_eligible_prepared_operation_has_one_active_owner() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::TrafficEligible,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            assert_eq!(production_policy.approved_control_count(), 1);
            assert_eq!(
                license_policy.source_disposition(),
                GenerationQualificationAssessmentPolicySourceDisposition::Approved
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

            assert_eq!(
                prepared.disposition(),
                GenerationQualificationPreparationDisposition::TrafficEligible
            );
            assert_eq!(
                prepared.plan_foundation().plan(),
                input.foundation.plan_foundation.plan
            );
            let evidence =
                rewrite_app::CandidateGenerationEvidenceRepository::initialize(directory.path())
                    .expect("evidence root");
            let mut active = prepared
                .activate(&repository, &evidence, &cancellation)
                .expect("active owner");
            assert_eq!(
                active.request_projection().operation_policy_id(),
                active.operation_policy().operation_policy_id()
            );
            assert!(!active.ever_acquired_live_authority());
            assert_eq!(active.peak_live_authorities(), 0);
            assert_eq!(active.settled_candidate_attempts(), 0);
            assert_eq!(active.executed_candidate_attempts(), 0);
            assert!(!active.has_pending_completed_candidate());
            assert_eq!(active.completed_judge_repetitions(), 0);
            active
                .revalidate(&cancellation)
                .expect("active revalidation");
            let debug = format!("{active:?}");
            assert!(debug.contains("operation_policy_id"));
            assert!(!debug.contains("Retain Acme"));
            cancellation.cancel();
            assert!(matches!(
                active.revalidate(&cancellation),
                Err(GenerationQualificationActivationError::Cancelled)
            ));
        },
    );
}

#[test]
fn rejected_prepared_operation_cannot_become_active() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::LicenseRejected,
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

            let evidence =
                rewrite_app::CandidateGenerationEvidenceRepository::initialize(directory.path())
                    .expect("evidence root");
            std::fs::write(
                directory
                    .path()
                    .join("generation-evidence")
                    .join(".staging")
                    .join("leftover"),
                b"x",
            )
            .expect("stage leftover");
            assert!(matches!(
                prepared.activate(&repository, &evidence, &cancellation),
                Err(GenerationQualificationActivationError::LicenseRejected)
            ));
            assert!(
                directory
                    .path()
                    .join("generation-evidence")
                    .join(".staging")
                    .join("leftover")
                    .is_file()
            );
        },
    );
}

#[test]
fn denied_license_prepares_exact_readbacks_and_revalidates() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |input, platform_owners, license_proof, license_policy, production_policy| {
            assert_eq!(production_policy.approved_control_count(), 0);
            assert_eq!(
                license_policy.source_disposition(),
                GenerationQualificationAssessmentPolicySourceDisposition::Denied
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
            let mut prepared = projected
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

            assert_eq!(
                prepared.disposition(),
                GenerationQualificationPreparationDisposition::LicenseRejected
            );
            assert_eq!(
                prepared.request_projection().operation_policy_id(),
                prepared.operation_policy().operation_policy_id()
            );
            prepared.revalidate(&cancellation).expect("revalidation");
            let debug = format!("{prepared:?}");
            assert!(debug.contains("operation_policy_id"));
            assert!(!debug.contains("Retain Acme"));
        },
    );
}

#[test]
fn mandatory_finalization_remains_available_after_deadline_and_cancellation() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::LicenseRejected,
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
            let mut prepared = projected
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

            prepared.expire_deadline_for_test();
            cancellation.cancel();
            assert!(matches!(
                prepared.revalidate(&cancellation),
                Err(GenerationQualificationPreparationError::DeadlineExceeded)
            ));
            prepared
                .revalidate_for_mandatory_finalization()
                .expect("fresh mandatory finalization validation");
            let disposition = prepared
                .with_mandatory_finalization_validated_view(|view| Ok::<_, ()>(view.disposition))
                .expect("freshly bracketed finalization view");
            assert_eq!(
                disposition,
                GenerationQualificationPreparationDisposition::LicenseRejected
            );
        },
    );
}

#[test]
fn platform_rejection_has_precedence_when_both_assessments_reject() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::BothRejected,
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
            assert_eq!(
                prepared.disposition(),
                GenerationQualificationPreparationDisposition::PlatformRejected
            );
        },
    );
}

#[test]
fn cancellation_stops_the_state_machine_before_policy_construction() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::BothRejected,
        |input, _platform_owners, _license_proof, _license_policy, _production_policy| {
            let cancellation = CancellationToken::new();
            cancellation.cancel();
            let result = GenerationQualificationOperationDraft::begin(
                input.operation_policy_relations,
                input.operation_policy_input,
                &cancellation,
            );
            assert!(matches!(
                result,
                Err(GenerationQualificationPreparationError::Cancelled)
            ));
        },
    );
}

#[test]
fn operation_relationship_substitution_fails_closed() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |mut input, _platform_owners, _license_proof, _license_policy, _production_policy| {
            input.operation_policy_input.resource_policy_digest =
                Digest::sha256(b"substituted resource policy");
            let result = GenerationQualificationOperationDraft::begin(
                input.operation_policy_relations,
                input.operation_policy_input,
                &CancellationToken::new(),
            );
            assert!(matches!(
                result,
                Err(GenerationQualificationPreparationError::OperationPolicyMismatch)
            ));
        },
    );
}

#[test]
fn original_deadline_precedes_cancellation_during_projection() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::BothRejected,
        |mut input, _platform_owners, _license_proof, _license_policy, _production_policy| {
            input.operation_policy_input.limits =
                limits_with_elapsed(input.operation_policy_input.limits, 2_000);
            let cancellation = CancellationToken::new();
            let draft = GenerationQualificationOperationDraft::begin(
                input.operation_policy_relations,
                input.operation_policy_input,
                &cancellation,
            )
            .expect("draft before deadline");
            std::thread::sleep(Duration::from_millis(2_250));
            cancellation.cancel();
            let result = draft.project(input.case_authorities, &cancellation);
            assert!(matches!(
                result,
                Err(GenerationQualificationPreparationError::DeadlineExceeded)
            ));
        },
    );
}

#[test]
fn platform_owner_rejects_a_substituted_projection_closure() {
    with_synthetic_generation_qualification_fixture(
        SyntheticGenerationQualificationScenario::LicenseRejected,
        |input, platform_owners, _license_proof, _license_policy, _production_policy| {
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
            let mut substituted = projected.platform_portable_relations();
            substituted.request_projection_entry_inputs = &[];
            let result = platform_owners.assess(substituted, &cancellation);
            assert!(matches!(
                result,
                Err(GenerationQualificationPlatformAssessmentError::InvalidPortableClosure)
            ));
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
