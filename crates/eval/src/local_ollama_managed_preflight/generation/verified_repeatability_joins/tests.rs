use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateGenerationAttemptRecordV1,
    CandidateGenerationReceiptSetV1, CandidateJudgeChoiceV1, CandidateJudgeJoinRecordV1,
    GenerationAttemptLedgerManifestV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationQualificationOperationPolicyV1, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationQualificationPlanV1,
    GenerationRepeatabilityResultRecordV1Relations, GenerationRepetitionRecordV1,
    GenerationSuiteManifestV1, GenerationSystemId, GenerationSystemRecordV1,
    PlannedCandidateAttemptV1,
};
use rewrite_types::{CancellationToken, Digest};

use super::super::managed_local_judge_receipt::ManagedLocalJudgeReceipt;
use super::super::verified_candidate_judge_join::{
    VerifiedCandidateJudgeJoinCompiler,
    test_support::{managed_receipt, observation_batch, portable_outputs},
};
use super::*;
use crate::candidate_judge_preparation::tests::{ready_config, request_branches::with_input};
use crate::generation_case_material::verified_material_test_support::Fixture;
use crate::{CandidateJudgePreparationOutcome, prepare_candidate_judge};

fn target_id(label: &str) -> GenerationSystemId {
    serde_json::from_str(&format!("\"{}\"", Digest::sha256(label.as_bytes())))
        .expect("transparent generation-system ID")
}

pub(super) struct RealJoinFixture<'store> {
    pub(super) join: VerifiedCandidateJudgeJoin<'store, 'static, 'static, 'static>,
    pub(super) qualification_plan: GenerationQualificationPlanV1,
    pub(super) suite: GenerationSuiteManifestV1,
    pub(super) repetition: GenerationRepetitionRecordV1,
    pub(super) planned_attempts: Vec<PlannedCandidateAttemptV1>,
    pub(super) target_system: GenerationSystemRecordV1,
    pub(super) target_attempt_records: Vec<CandidateGenerationAttemptRecordV1>,
    pub(super) target_receipt_set: CandidateGenerationReceiptSetV1,
    pub(super) deterministic: CandidateDeterministicEvaluationRecordV1,
    pub(super) operation_policy: Option<GenerationQualificationOperationPolicyV1>,
}

fn real_join_fixture(material: &Fixture, choice: CandidateJudgeChoiceV1) -> RealJoinFixture<'_> {
    real_join_fixture_for(material, "ordered-repeatability-join", choice)
}

pub(super) fn real_qualification_join_fixture(
    material: &Fixture,
    choice: CandidateJudgeChoiceV1,
) -> RealJoinFixture<'_> {
    real_join_fixture_for(material, "qualification-closure", choice)
}

pub(super) fn foreign_qualification_join_fixture(
    material: &Fixture,
    choice: CandidateJudgeChoiceV1,
) -> RealJoinFixture<'_> {
    real_join_fixture_for(material, "foreign-qualification-closure", choice)
}

fn real_join_fixture_for<'store>(
    material: &'store Fixture,
    suffix: &str,
    choice: CandidateJudgeChoiceV1,
) -> RealJoinFixture<'store> {
    with_input(material, suffix, ready_config(vec![0]), |input| {
        let cancellation = CancellationToken::new();
        let qualification_plan = input.plan_relations.qualification_plan.clone();
        let suite = input.plan_relations.suite.clone();
        let repetition = input.plan_relations.repetition.clone();
        let planned_attempts = input.plan_relations.planned_attempts.to_vec();
        let target_system = input.plan_relations.candidate_a_system.clone();
        let pair = crate::verified_candidate_batch_set::tests::offline_paired_authorities(
            &material.suite,
            &material.cases,
            suffix,
        );
        let operation_policy = pair.operation_policy;
        let target_attempt_records = input
            .candidate_a
            .attempt_records_for_test(&cancellation)
            .expect("target attempt records");
        let target_receipt_set = input
            .candidate_a
            .receipt_set(&cancellation)
            .expect("target receipt set")
            .clone();
        let CandidateJudgePreparationOutcome::Ready(ready) =
            prepare_candidate_judge(input, &cancellation).expect("judge preparation")
        else {
            panic!("passed deterministic fixture must prepare")
        };
        let deterministic = ready.deterministic_evaluation().clone();
        let handoff = ready
            .into_runner_handoff(&cancellation)
            .expect("runner handoff");
        let (responses, _) = portable_outputs(&handoff);
        let observations = observation_batch(&handoff, &responses, choice);
        let portable_receipt = managed_receipt(&handoff, &responses, &observations);
        let receipt = ManagedLocalJudgeReceipt::from_portable_closure_for_test(
            handoff,
            responses,
            observations,
            portable_receipt,
        );
        let join = VerifiedCandidateJudgeJoinCompiler::compile(receipt, &cancellation)
            .expect("real opaque join");
        RealJoinFixture {
            join,
            qualification_plan,
            suite,
            repetition,
            planned_attempts,
            target_system,
            target_attempt_records,
            target_receipt_set,
            deterministic,
            operation_policy,
        }
    })
}

pub(super) fn passed_result(
    fixture: &RealJoinFixture<'_>,
    ledger: &GenerationAttemptLedgerManifestV1,
    ledger_relations: GenerationAttemptLedgerManifestV1Relations<'_>,
    join: &CandidateJudgeJoinRecordV1,
    terminal_evidence_digest: &Digest,
) -> GenerationRepeatabilityResultRecordV1 {
    terminal_result(
        fixture,
        ledger,
        ledger_relations,
        GenerationRepeatabilityTerminalStageV1::Passed,
        Some(join),
        terminal_evidence_digest,
    )
}

pub(super) fn judge_failed_result(
    fixture: &RealJoinFixture<'_>,
    ledger: &GenerationAttemptLedgerManifestV1,
    ledger_relations: GenerationAttemptLedgerManifestV1Relations<'_>,
    terminal_evidence_digest: &Digest,
) -> GenerationRepeatabilityResultRecordV1 {
    terminal_result(
        fixture,
        ledger,
        ledger_relations,
        GenerationRepeatabilityTerminalStageV1::JudgeFailed,
        None,
        terminal_evidence_digest,
    )
}

pub(super) fn complete_qualification_authority(
    material: &Fixture,
) -> VerifiedCompletePassedRepeatabilityJoins<'_, 'static, 'static, 'static> {
    let fixture = real_qualification_join_fixture(material, CandidateJudgeChoiceV1::Tie);
    let ledger_policy_digest = fixture
        .operation_policy
        .as_ref()
        .expect("qualification operation policy")
        .attempt_ledger_policy_digest();
    let ledger_relations = GenerationAttemptLedgerManifestV1Relations {
        scope: GenerationQualificationPhaseScopeV1 {
            generation_system: &fixture.target_system,
            qualification_plan: &fixture.qualification_plan,
            suite: &fixture.suite,
        },
        phase_policy_digest: ledger_policy_digest,
        planned_attempts: &fixture.planned_attempts,
        attempt_records: &fixture.target_attempt_records,
        status: GenerationQualificationPhaseStatusV1::Passed,
    };
    let ledger = GenerationAttemptLedgerManifestV1::new(ledger_relations)
        .expect("qualification attempt ledger");
    let join_record = fixture.join.record().clone();
    let result = passed_result(
        &fixture,
        &ledger,
        ledger_relations,
        &join_record,
        &Digest::sha256(b"qualification terminal evidence"),
    );
    let joins = verify_passed_repeatability_joins(
        fixture.target_system.generation_system_id(),
        std::slice::from_ref(&result),
        vec![fixture.join],
        &CancellationToken::new(),
    )
    .expect("qualification passed join");
    verify_complete_passed_repeatability_joins(
        CompletePassedRepeatabilityRelations {
            operation_policy: fixture
                .operation_policy
                .as_ref()
                .expect("qualification operation policy"),
            scope: GenerationQualificationPhaseScopeV1 {
                generation_system: &fixture.target_system,
                qualification_plan: &fixture.qualification_plan,
                suite: &fixture.suite,
            },
            planned_attempts: &fixture.planned_attempts,
            attempt_ledger_manifest: &ledger,
            preregistered_repetitions: std::slice::from_ref(&fixture.repetition),
            ordered_results: std::slice::from_ref(&result),
        },
        joins,
        &CancellationToken::new(),
    )
    .expect("complete qualification authority")
}

fn terminal_result(
    fixture: &RealJoinFixture<'_>,
    ledger: &GenerationAttemptLedgerManifestV1,
    ledger_relations: GenerationAttemptLedgerManifestV1Relations<'_>,
    terminal_stage: GenerationRepeatabilityTerminalStageV1,
    join: Option<&CandidateJudgeJoinRecordV1>,
    terminal_evidence_digest: &Digest,
) -> GenerationRepeatabilityResultRecordV1 {
    GenerationRepeatabilityResultRecordV1::new(GenerationRepeatabilityResultRecordV1Relations {
        scope: ledger_relations.scope,
        repetition: &fixture.repetition,
        attempt_ledger: ledger,
        attempt_ledger_relations: ledger_relations,
        terminal_stage,
        candidate_receipt_set: Some(&fixture.target_receipt_set),
        deterministic_evaluation: Some(&fixture.deterministic),
        candidate_judge_join: join,
        terminal_evidence_digest,
    })
    .expect("passed repeatability result")
}

fn valid_binding_facts() -> BindingFacts {
    BindingFacts {
        result: RelationMatch::Exact,
        target_receipt: RelationMatch::Exact,
        deterministic: RelationMatch::Exact,
        join: RelationMatch::Exact,
        evidence_class: RelationMatch::Exact,
        claims: RelationMatch::Exact,
    }
}

#[test]
fn binding_requires_every_exact_identity_and_keeps_all_claims_false() {
    assert!(valid_binding_facts().is_valid());
    let base = valid_binding_facts();
    for invalid in [
        BindingFacts {
            result: RelationMatch::Mismatch,
            ..base
        },
        BindingFacts {
            target_receipt: RelationMatch::Mismatch,
            ..base
        },
        BindingFacts {
            deterministic: RelationMatch::Mismatch,
            ..base
        },
        BindingFacts {
            join: RelationMatch::Mismatch,
            ..base
        },
        BindingFacts {
            evidence_class: RelationMatch::Mismatch,
            ..base
        },
        BindingFacts {
            claims: RelationMatch::Mismatch,
            ..base
        },
    ] {
        assert!(!invalid.is_valid());
    }
}

#[test]
fn empty_nonpassing_closure_is_a_nonqualifying_revalidatable_capability() {
    let target = target_id("empty repeatability target");
    let mut verified =
        verify_passed_repeatability_joins(&target, &[], Vec::new(), &CancellationToken::new())
            .expect("empty passed-join closure");
    assert_eq!(verified.target_generation_system_id(), &target);
    assert_eq!(verified.result_count(), 0);
    assert_eq!(verified.passed_join_count(), 0);
    verified
        .revalidate(&target, &[], &CancellationToken::new())
        .expect("empty closure revalidation");
    assert_eq!(
        format!("{verified:?}"),
        "VerifiedPassedRepeatabilityJoins { result_count: 0, passed_join_count: 0, .. }"
    );
}

#[test]
fn cancellation_and_target_substitution_fail_even_without_joins() {
    let target = target_id("empty repeatability target");
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let cancelled = verify_passed_repeatability_joins(&target, &[], Vec::new(), &cancellation)
        .expect_err("cancelled closure");
    assert_eq!(
        cancelled.kind(),
        VerifiedPassedRepeatabilityJoinsErrorKind::Cancelled
    );

    let mut verified =
        verify_passed_repeatability_joins(&target, &[], Vec::new(), &CancellationToken::new())
            .expect("empty passed-join closure");
    let foreign = target_id("foreign repeatability target");
    let error = verified
        .revalidate(&foreign, &[], &CancellationToken::new())
        .expect_err("target substitution");
    assert_eq!(
        error.kind(),
        VerifiedPassedRepeatabilityJoinsErrorKind::Relationship
    );
    assert_eq!(error.relationship_failure_count(), 1);
    assert_eq!(error.initial_authority_failure_count(), 0);
    assert_eq!(error.final_authority_failure_count(), 0);
}

#[test]
fn failure_aggregation_preserves_initial_relationship_and_final_counts() {
    use super::super::managed_schedule_runner::ManagedJudgeScheduleAuthorityFailures;

    let mut failures = ValidationFailures::default();
    failures.record(Err(ManagedJudgeScheduleExecutionAuthorityError::Initial(
        ManagedJudgeScheduleAuthorityFailures::cancelled_for_test(),
    )));
    failures.record(Err(
        ManagedJudgeScheduleExecutionAuthorityError::InitialAndFinal {
            initial: ManagedJudgeScheduleAuthorityFailures::cancelled_for_test(),
            final_validation: ManagedJudgeScheduleAuthorityFailures::cancelled_for_test(),
        },
    ));
    failures.record(Err(ManagedJudgeScheduleExecutionAuthorityError::Callback(
        JoinBindingError,
    )));
    failures.record(Err(ManagedJudgeScheduleExecutionAuthorityError::Final(
        ManagedJudgeScheduleAuthorityFailures::cancelled_for_test(),
    )));
    failures.record(Err(
        ManagedJudgeScheduleExecutionAuthorityError::CallbackAndFinal {
            callback: JoinBindingError,
            final_validation: ManagedJudgeScheduleAuthorityFailures::cancelled_for_test(),
        },
    ));
    let error = failures.finish(false).expect_err("aggregate failures");
    assert_eq!(
        error.kind(),
        VerifiedPassedRepeatabilityJoinsErrorKind::Relationship
    );
    assert_eq!(error.initial_authority_failure_count(), 2);
    assert_eq!(error.relationship_failure_count(), 2);
    assert_eq!(error.final_authority_failure_count(), 3);
    let rendered = format!("{error:?} {error}");
    for forbidden in ["candidate", "prompt", "response", "rationale"] {
        assert!(!rendered.contains(forbidden));
    }
}

#[test]
fn real_opaque_join_requires_the_exact_passed_result_and_revalidates_fresh() {
    let material = Fixture::judge_pair();
    let first = real_join_fixture(&material, CandidateJudgeChoiceV1::Tie);
    let second = real_join_fixture(&material, CandidateJudgeChoiceV1::First);
    assert_eq!(first.qualification_plan, second.qualification_plan);
    assert_eq!(first.target_system, second.target_system);
    assert_eq!(first.deterministic, second.deterministic);
    assert_ne!(first.join.record(), second.join.record());

    let ledger_policy_digest = Digest::sha256(b"real join passed ledger policy");
    let scope = GenerationQualificationPhaseScopeV1 {
        generation_system: &first.target_system,
        qualification_plan: &first.qualification_plan,
        suite: &first.suite,
    };
    let ledger_relations = GenerationAttemptLedgerManifestV1Relations {
        scope,
        phase_policy_digest: &ledger_policy_digest,
        planned_attempts: &first.planned_attempts,
        attempt_records: &first.target_attempt_records,
        status: GenerationQualificationPhaseStatusV1::Passed,
    };
    let ledger = GenerationAttemptLedgerManifestV1::new(ledger_relations)
        .expect("complete passed target ledger");
    let first_join_record = first.join.record().clone();
    let second_join_record = second.join.record().clone();
    let terminal_evidence_digest = Digest::sha256(b"real opaque join terminal evidence");
    let first_result = passed_result(
        &first,
        &ledger,
        ledger_relations,
        &first_join_record,
        &terminal_evidence_digest,
    );
    let second_result = passed_result(
        &first,
        &ledger,
        ledger_relations,
        &second_join_record,
        &terminal_evidence_digest,
    );

    let mismatch = verify_passed_repeatability_joins(
        first.target_system.generation_system_id(),
        std::slice::from_ref(&first_result),
        vec![second.join],
        &CancellationToken::new(),
    )
    .expect_err("foreign passed-result join");
    assert_eq!(
        mismatch.kind(),
        VerifiedPassedRepeatabilityJoinsErrorKind::Relationship
    );
    assert_eq!(mismatch.relationship_failure_count(), 1);

    let mut verified = verify_passed_repeatability_joins(
        first.target_system.generation_system_id(),
        std::slice::from_ref(&first_result),
        vec![first.join],
        &CancellationToken::new(),
    )
    .expect("exact passed-result join");
    assert_eq!(verified.result_count(), 1);
    assert_eq!(verified.passed_join_count(), 1);
    verified
        .revalidate(
            first.target_system.generation_system_id(),
            std::slice::from_ref(&first_result),
            &CancellationToken::new(),
        )
        .expect("fresh exact revalidation");
    let substitution = verified
        .revalidate(
            first.target_system.generation_system_id(),
            std::slice::from_ref(&second_result),
            &CancellationToken::new(),
        )
        .expect_err("stored result substitution");
    assert_eq!(
        substitution.kind(),
        VerifiedPassedRepeatabilityJoinsErrorKind::Relationship
    );

    let target_id = first.target_system.generation_system_id();
    let baseline_id = if first_join_record.candidate_a_generation_system_id() == target_id {
        first_join_record.candidate_b_generation_system_id()
    } else {
        first_join_record.candidate_a_generation_system_id()
    };
    let ordinary_error = verified
        .collect_target_resource_results(
            target_id,
            baseline_id,
            std::slice::from_ref(&first_result),
            &CancellationToken::new(),
        )
        .expect_err("ordinary target set has no strict resource closure");
    assert_eq!(
        ordinary_error.kind(),
        VerifiedPassedRepeatabilityJoinsErrorKind::Relationship
    );
    assert_eq!(ordinary_error.relationship_failure_count(), 1);
}

#[test]
fn invalid_count_has_stable_precedence_without_losing_failure_counts() {
    let failures = ValidationFailures {
        invalid_count: true,
        relationship: 1,
        final_authority: 1,
        ..ValidationFailures::default()
    };
    let error = failures.finish(false).expect_err("invalid count");
    assert_eq!(
        error.kind(),
        VerifiedPassedRepeatabilityJoinsErrorKind::InvalidCount
    );
    assert_eq!(error.relationship_failure_count(), 1);
    assert_eq!(error.final_authority_failure_count(), 1);
}
