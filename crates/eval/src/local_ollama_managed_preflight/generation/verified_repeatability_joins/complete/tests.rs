use rewrite_model::{
    CandidateJudgeChoiceV1, GenerationAttemptLedgerManifestV1,
    GenerationAttemptLedgerManifestV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationRepeatabilityResultRecordV1,
};
use rewrite_types::{CancellationToken, Digest};

use super::{
    CompletePassedRepeatabilityRelations, VerifiedCompletePassedRepeatabilityJoinsError,
    complete_result_closure_matches, verify_complete_passed_repeatability_joins,
};
use crate::generation_case_material::verified_material_test_support::Fixture;

use super::super::{
    tests::{
        RealJoinFixture, foreign_qualification_join_fixture, judge_failed_result, passed_result,
        real_qualification_join_fixture,
    },
    verify_passed_repeatability_joins,
};

#[test]
fn exact_full_closure_is_required_instead_of_any_subset() {
    assert!(complete_result_closure_matches(3, 3, [true, true, true]));
    assert!(!complete_result_closure_matches(3, 2, [true, true]));
    assert!(!complete_result_closure_matches(3, 2, [true, false]));
    assert!(!complete_result_closure_matches(3, 3, [true, false, true]));
    assert!(!complete_result_closure_matches(
        3,
        4,
        [true, true, true, true]
    ));
}

#[test]
fn real_complete_closure_retains_exact_policy_manifest_and_live_join() {
    let material = Fixture::judge_pair();
    let (fixture, ledger, result) = passed_fixture(&material);
    let joins = verify_passed_repeatability_joins(
        fixture.target_system.generation_system_id(),
        std::slice::from_ref(&result),
        vec![fixture.join],
        &CancellationToken::new(),
    )
    .expect("exact passed join");
    let mut verified = verify_complete_passed_repeatability_joins(
        relations(
            fixture.operation_policy.as_ref(),
            &fixture.target_system,
            &fixture.qualification_plan,
            &fixture.suite,
            &fixture.planned_attempts,
            &ledger,
            &fixture.repetition,
            std::slice::from_ref(&result),
        ),
        joins,
        &CancellationToken::new(),
    )
    .expect("complete passed closure");

    assert_eq!(verified.repetition_count(), 1);
    assert_eq!(
        verified.repeatability_manifest().status(),
        GenerationQualificationPhaseStatusV1::Passed
    );
    assert_eq!(
        verified.repeatability_manifest().phase_policy_digest(),
        fixture
            .operation_policy
            .as_ref()
            .expect("qualification operation policy")
            .repeatability_policy_digest()
    );
    verified
        .revalidate(&CancellationToken::new())
        .expect("fresh complete revalidation");
    assert!(format!("{verified:?}").contains("repetition_count"));
}

#[test]
fn same_scope_result_from_foreign_attempt_ledger_policy_is_rejected() {
    let material = Fixture::judge_pair();
    let fixture = real_qualification_join_fixture(&material, CandidateJudgeChoiceV1::Tie);
    let (_, exact_ledger_relations) = ledger(&fixture);
    let foreign_policy_digest = Digest::sha256(b"foreign attempt ledger policy");
    let foreign_ledger_relations = GenerationAttemptLedgerManifestV1Relations {
        phase_policy_digest: &foreign_policy_digest,
        ..exact_ledger_relations
    };
    let foreign_ledger = GenerationAttemptLedgerManifestV1::new(foreign_ledger_relations)
        .expect("same-scope foreign-policy ledger");
    let join = fixture.join.record().clone();
    let foreign_result = passed_result(
        &fixture,
        &foreign_ledger,
        foreign_ledger_relations,
        &join,
        &Digest::sha256(b"foreign-ledger repeatability terminal evidence"),
    );
    let joins = verify_passed_repeatability_joins(
        fixture.target_system.generation_system_id(),
        std::slice::from_ref(&foreign_result),
        vec![fixture.join],
        &CancellationToken::new(),
    )
    .expect("same-scope foreign-ledger result still has an exact live join");

    assert_eq!(
        foreign_ledger.generation_system_id(),
        fixture.target_system.generation_system_id()
    );
    assert_ne!(
        foreign_ledger.phase_policy_digest(),
        fixture
            .operation_policy
            .as_ref()
            .expect("qualification operation policy")
            .attempt_ledger_policy_digest()
    );
    assert!(matches!(
        verify_complete_passed_repeatability_joins(
            relations(
                fixture.operation_policy.as_ref(),
                &fixture.target_system,
                &fixture.qualification_plan,
                &fixture.suite,
                &fixture.planned_attempts,
                &foreign_ledger,
                &fixture.repetition,
                std::slice::from_ref(&foreign_result),
            ),
            joins,
            &CancellationToken::new(),
        ),
        Err(VerifiedCompletePassedRepeatabilityJoinsError::Relationship)
    ));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one adversarial test keeps every complete-closure rejection visible"
)]
fn incomplete_failed_foreign_and_cancelled_closures_fail_before_release() {
    let material = Fixture::judge_pair();

    let (missing_fixture, missing_ledger, missing_result) = passed_fixture(&material);
    let missing_joins = verify_passed_repeatability_joins(
        missing_fixture.target_system.generation_system_id(),
        std::slice::from_ref(&missing_result),
        vec![missing_fixture.join],
        &CancellationToken::new(),
    )
    .expect("stored exact passed join");
    assert!(matches!(
        verify_complete_passed_repeatability_joins(
            relations(
                missing_fixture.operation_policy.as_ref(),
                &missing_fixture.target_system,
                &missing_fixture.qualification_plan,
                &missing_fixture.suite,
                &missing_fixture.planned_attempts,
                &missing_ledger,
                &missing_fixture.repetition,
                &[],
            ),
            missing_joins,
            &CancellationToken::new(),
        ),
        Err(VerifiedCompletePassedRepeatabilityJoinsError::Relationship)
    ));

    let (extra_fixture, extra_ledger, extra_result) = passed_fixture(&material);
    let extra_joins = verify_passed_repeatability_joins(
        extra_fixture.target_system.generation_system_id(),
        std::slice::from_ref(&extra_result),
        vec![extra_fixture.join],
        &CancellationToken::new(),
    )
    .expect("stored exact passed join");
    assert!(matches!(
        verify_complete_passed_repeatability_joins(
            relations(
                extra_fixture.operation_policy.as_ref(),
                &extra_fixture.target_system,
                &extra_fixture.qualification_plan,
                &extra_fixture.suite,
                &extra_fixture.planned_attempts,
                &extra_ledger,
                &extra_fixture.repetition,
                &[extra_result.clone(), extra_result],
            ),
            extra_joins,
            &CancellationToken::new(),
        ),
        Err(VerifiedCompletePassedRepeatabilityJoinsError::Relationship)
    ));

    let (failed_fixture, _passed_ledger, passed) = passed_fixture(&material);
    let (failed_ledger, failed) = failed_result(&failed_fixture);
    assert_ne!(
        failed.repeatability_result_id(),
        passed.repeatability_result_id()
    );
    let failed_joins = verify_passed_repeatability_joins(
        failed_fixture.target_system.generation_system_id(),
        std::slice::from_ref(&failed),
        Vec::new(),
        &CancellationToken::new(),
    )
    .expect("non-Passed result needs no Passed join");
    assert!(matches!(
        verify_complete_passed_repeatability_joins(
            relations(
                failed_fixture.operation_policy.as_ref(),
                &failed_fixture.target_system,
                &failed_fixture.qualification_plan,
                &failed_fixture.suite,
                &failed_fixture.planned_attempts,
                &failed_ledger,
                &failed_fixture.repetition,
                std::slice::from_ref(&failed),
            ),
            failed_joins,
            &CancellationToken::new(),
        ),
        Err(VerifiedCompletePassedRepeatabilityJoinsError::Relationship)
    ));

    let (foreign_fixture, foreign_ledger, foreign_result) = passed_fixture(&material);
    let foreign_policy = foreign_qualification_join_fixture(&material, CandidateJudgeChoiceV1::Tie)
        .operation_policy
        .expect("foreign operation policy");
    let foreign_joins = verify_passed_repeatability_joins(
        foreign_fixture.target_system.generation_system_id(),
        std::slice::from_ref(&foreign_result),
        vec![foreign_fixture.join],
        &CancellationToken::new(),
    )
    .expect("stored exact passed join");
    let mut foreign_relations = relations(
        foreign_fixture.operation_policy.as_ref(),
        &foreign_fixture.target_system,
        &foreign_fixture.qualification_plan,
        &foreign_fixture.suite,
        &foreign_fixture.planned_attempts,
        &foreign_ledger,
        &foreign_fixture.repetition,
        std::slice::from_ref(&foreign_result),
    );
    foreign_relations.operation_policy = &foreign_policy;
    assert!(matches!(
        verify_complete_passed_repeatability_joins(
            foreign_relations,
            foreign_joins,
            &CancellationToken::new(),
        ),
        Err(VerifiedCompletePassedRepeatabilityJoinsError::Relationship)
    ));

    let (cancelled_fixture, cancelled_ledger, cancelled_result) = passed_fixture(&material);
    let cancelled_joins = verify_passed_repeatability_joins(
        cancelled_fixture.target_system.generation_system_id(),
        std::slice::from_ref(&cancelled_result),
        vec![cancelled_fixture.join],
        &CancellationToken::new(),
    )
    .expect("stored exact passed join");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(matches!(
        verify_complete_passed_repeatability_joins(
            relations(
                cancelled_fixture.operation_policy.as_ref(),
                &cancelled_fixture.target_system,
                &cancelled_fixture.qualification_plan,
                &cancelled_fixture.suite,
                &cancelled_fixture.planned_attempts,
                &cancelled_ledger,
                &cancelled_fixture.repetition,
                std::slice::from_ref(&cancelled_result),
            ),
            cancelled_joins,
            &cancellation,
        ),
        Err(VerifiedCompletePassedRepeatabilityJoinsError::Cancelled)
    ));
}

fn passed_fixture(
    material: &Fixture,
) -> (
    RealJoinFixture<'_>,
    GenerationAttemptLedgerManifestV1,
    GenerationRepeatabilityResultRecordV1,
) {
    let fixture = real_qualification_join_fixture(material, CandidateJudgeChoiceV1::Tie);
    let (ledger, ledger_relations) = ledger(&fixture);
    let join = fixture.join.record().clone();
    let result = passed_result(
        &fixture,
        &ledger,
        ledger_relations,
        &join,
        &Digest::sha256(b"complete repeatability terminal evidence"),
    );
    (fixture, ledger, result)
}

fn failed_result(
    fixture: &RealJoinFixture<'_>,
) -> (
    GenerationAttemptLedgerManifestV1,
    GenerationRepeatabilityResultRecordV1,
) {
    let (ledger, ledger_relations) = ledger(fixture);
    let result = judge_failed_result(
        fixture,
        &ledger,
        ledger_relations,
        &Digest::sha256(b"failed repeatability terminal evidence"),
    );
    (ledger, result)
}

fn ledger<'a>(
    fixture: &'a RealJoinFixture<'_>,
) -> (
    GenerationAttemptLedgerManifestV1,
    GenerationAttemptLedgerManifestV1Relations<'a>,
) {
    let relations = GenerationAttemptLedgerManifestV1Relations {
        scope: GenerationQualificationPhaseScopeV1 {
            generation_system: &fixture.target_system,
            qualification_plan: &fixture.qualification_plan,
            suite: &fixture.suite,
        },
        phase_policy_digest: fixture
            .operation_policy
            .as_ref()
            .expect("qualification operation policy")
            .attempt_ledger_policy_digest(),
        planned_attempts: &fixture.planned_attempts,
        attempt_records: &fixture.target_attempt_records,
        status: GenerationQualificationPhaseStatusV1::Passed,
    };
    (
        GenerationAttemptLedgerManifestV1::new(relations).expect("complete attempt ledger"),
        relations,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the complete-closure fixture keeps every exact retained relationship explicit"
)]
fn relations<'a>(
    operation_policy: Option<&'a rewrite_model::GenerationQualificationOperationPolicyV1>,
    target_system: &'a rewrite_model::GenerationSystemRecordV1,
    qualification_plan: &'a rewrite_model::GenerationQualificationPlanV1,
    suite: &'a rewrite_model::GenerationSuiteManifestV1,
    planned_attempts: &'a [rewrite_model::PlannedCandidateAttemptV1],
    attempt_ledger_manifest: &'a GenerationAttemptLedgerManifestV1,
    repetition: &'a rewrite_model::GenerationRepetitionRecordV1,
    results: &'a [GenerationRepeatabilityResultRecordV1],
) -> CompletePassedRepeatabilityRelations<'a> {
    CompletePassedRepeatabilityRelations {
        operation_policy: operation_policy.expect("qualification operation policy"),
        scope: GenerationQualificationPhaseScopeV1 {
            generation_system: target_system,
            qualification_plan,
            suite,
        },
        planned_attempts,
        attempt_ledger_manifest,
        preregistered_repetitions: std::slice::from_ref(repetition),
        ordered_results: results,
    }
}
