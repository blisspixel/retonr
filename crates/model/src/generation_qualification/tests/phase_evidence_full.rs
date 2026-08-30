use super::candidate_judge::fixture::{judge_fixture, judge_fixture_from_pair};
use super::deterministic_evaluation::fixture::{
    input, mixed_repetition_pair_fixture, pair_fixture, report_relationship,
};
use super::*;

fn scope(
    pair: &deterministic_evaluation::fixture::PairFixture,
) -> GenerationQualificationPhaseScopeV1<'_> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: &pair.systems[0],
        qualification_plan: &pair.plan,
        suite: &pair.suite,
    }
}

fn ledger_relations<'a>(
    pair: &'a deterministic_evaluation::fixture::PairFixture,
    phase_policy_digest: &'a Digest,
    records: &'a [CandidateGenerationAttemptRecordV1],
    status: GenerationQualificationPhaseStatusV1,
) -> GenerationAttemptLedgerManifestV1Relations<'a> {
    GenerationAttemptLedgerManifestV1Relations {
        scope: scope(pair),
        phase_policy_digest,
        planned_attempts: &pair.planned,
        attempt_records: records,
        status,
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

fn failed_deterministic(
    pair: &deterministic_evaluation::fixture::PairFixture,
) -> CandidateDeterministicEvaluationRecordV1 {
    let reports = report_relationship(0);
    let case_material = digest("failed deterministic material");
    let suite_pair = digest("failed deterministic suite pair");
    CandidateDeterministicEvaluationRecordV1::new(
        &pair.candidate_a,
        &pair.candidate_b,
        input(&case_material, &suite_pair, &reports),
    )
    .expect("failed deterministic")
}

fn assert_typed_substitutions(
    pair: &deterministic_evaluation::fixture::PairFixture,
    cases: &[GenerationRepeatabilityResultRecordV1Relations<'_>; 4],
    deterministic_failed: &CandidateDeterministicEvaluationRecordV1,
) {
    assert_eq!(
        GenerationRepeatabilityResultRecordV1::new(
            GenerationRepeatabilityResultRecordV1Relations {
                candidate_receipt_set: Some(&pair.candidate_b),
                ..cases[1]
            }
        ),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
    let foreign = judge_fixture_from_pair(pair_fixture("foreign join failure policy"));
    assert_eq!(
        GenerationRepeatabilityResultRecordV1::new(
            GenerationRepeatabilityResultRecordV1Relations {
                candidate_judge_join: Some(&foreign.join),
                ..cases[3]
            }
        ),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
    assert_eq!(
        GenerationRepeatabilityResultRecordV1::new(
            GenerationRepeatabilityResultRecordV1Relations {
                deterministic_evaluation: Some(deterministic_failed),
                ..cases[3]
            }
        ),
        Err(GenerationQualificationPhaseEvidenceError::RelationshipMismatch)
    );
}

#[test]
fn all_four_terminal_records_construct_round_trip_and_reject_typed_substitution() {
    let fixture = judge_fixture();
    let pair = &fixture.pair;
    let ledger_policy = digest("full ledger policy");
    let passed_records = [pair.completed_attempts[0].clone()];
    let passed_relations = ledger_relations(
        pair,
        &ledger_policy,
        &passed_records,
        GenerationQualificationPhaseStatusV1::Passed,
    );
    let passed_ledger =
        GenerationAttemptLedgerManifestV1::new(passed_relations).expect("passed ledger");
    let failed_records = [failed_attempt(&pair.planned[0])];
    let failed_relations = ledger_relations(
        pair,
        &ledger_policy,
        &failed_records,
        GenerationQualificationPhaseStatusV1::Failed,
    );
    let failed_ledger =
        GenerationAttemptLedgerManifestV1::new(failed_relations).expect("failed ledger");
    let deterministic_failed = failed_deterministic(pair);
    let terminal_digest = digest("full terminal evidence");
    let cases = [
        GenerationRepeatabilityResultRecordV1Relations {
            scope: scope(pair),
            repetition: &pair.repetition,
            attempt_ledger: &failed_ledger,
            attempt_ledger_relations: failed_relations,
            terminal_stage: GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
            candidate_receipt_set: None,
            deterministic_evaluation: None,
            candidate_judge_join: None,
            terminal_evidence_digest: &terminal_digest,
        },
        GenerationRepeatabilityResultRecordV1Relations {
            scope: scope(pair),
            repetition: &pair.repetition,
            attempt_ledger: &passed_ledger,
            attempt_ledger_relations: passed_relations,
            terminal_stage: GenerationRepeatabilityTerminalStageV1::DeterministicFailed,
            candidate_receipt_set: Some(&pair.candidate_a),
            deterministic_evaluation: Some(&deterministic_failed),
            candidate_judge_join: None,
            terminal_evidence_digest: &terminal_digest,
        },
        GenerationRepeatabilityResultRecordV1Relations {
            scope: scope(pair),
            repetition: &pair.repetition,
            attempt_ledger: &passed_ledger,
            attempt_ledger_relations: passed_relations,
            terminal_stage: GenerationRepeatabilityTerminalStageV1::JudgeFailed,
            candidate_receipt_set: Some(&pair.candidate_a),
            deterministic_evaluation: Some(&fixture.deterministic),
            candidate_judge_join: None,
            terminal_evidence_digest: &terminal_digest,
        },
        GenerationRepeatabilityResultRecordV1Relations {
            scope: scope(pair),
            repetition: &pair.repetition,
            attempt_ledger: &passed_ledger,
            attempt_ledger_relations: passed_relations,
            terminal_stage: GenerationRepeatabilityTerminalStageV1::Passed,
            candidate_receipt_set: Some(&pair.candidate_a),
            deterministic_evaluation: Some(&fixture.deterministic),
            candidate_judge_join: Some(&fixture.join),
            terminal_evidence_digest: &terminal_digest,
        },
    ];
    for relations in cases {
        let record =
            GenerationRepeatabilityResultRecordV1::new(relations).expect("terminal record");
        assert_eq!(
            GenerationRepeatabilityResultRecordV1::from_json_bytes(
                &serde_json::to_vec(&record).expect("result JSON"),
                relations,
            )
            .expect("result round trip"),
            record
        );
    }

    assert_typed_substitutions(pair, &cases, &deterministic_failed);
}

#[test]
fn global_failed_ledger_supports_passed_then_candidate_failed_repetitions() {
    let fixture = judge_fixture_from_pair(mixed_repetition_pair_fixture());
    let pair = &fixture.pair;
    let records = [
        pair.completed_attempts[0].clone(),
        failed_attempt(&pair.planned[2]),
    ];
    let ledger_policy = digest("mixed ledger policy");
    let ledger_closure = ledger_relations(
        pair,
        &ledger_policy,
        &records,
        GenerationQualificationPhaseStatusV1::Failed,
    );
    let ledger =
        GenerationAttemptLedgerManifestV1::new(ledger_closure).expect("mixed global ledger");
    GenerationAttemptLedgerManifestV1::new(ledger_relations(
        pair,
        &ledger_policy,
        &records[..1],
        GenerationQualificationPhaseStatusV1::Failed,
    ))
    .expect("completed abort prefix ledger");
    let terminal_digest = digest("mixed terminal evidence");
    let passed = GenerationRepeatabilityResultRecordV1::new(
        GenerationRepeatabilityResultRecordV1Relations {
            scope: scope(pair),
            repetition: &pair.repetition,
            attempt_ledger: &ledger,
            attempt_ledger_relations: ledger_closure,
            terminal_stage: GenerationRepeatabilityTerminalStageV1::Passed,
            candidate_receipt_set: Some(&pair.candidate_a),
            deterministic_evaluation: Some(&fixture.deterministic),
            candidate_judge_join: Some(&fixture.join),
            terminal_evidence_digest: &terminal_digest,
        },
    )
    .expect("passed first repetition");
    let second_repetition =
        GenerationRepetitionRecordV1::new(&pair.suite, 1, digest("pair repetition 1"))
            .expect("second repetition");
    let failed = GenerationRepeatabilityResultRecordV1::new(
        GenerationRepeatabilityResultRecordV1Relations {
            scope: scope(pair),
            repetition: &second_repetition,
            attempt_ledger: &ledger,
            attempt_ledger_relations: ledger_closure,
            terminal_stage: GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
            candidate_receipt_set: None,
            deterministic_evaluation: None,
            candidate_judge_join: None,
            terminal_evidence_digest: &terminal_digest,
        },
    )
    .expect("failed second repetition");
    let repetitions = [pair.repetition.clone(), second_repetition];
    let results = [passed, failed];
    let manifest = GenerationRepeatabilityEvidenceManifestV1::new(
        GenerationRepeatabilityEvidenceManifestV1Relations {
            scope: scope(pair),
            phase_policy_digest: &digest("mixed repeatability policy"),
            planned_attempts: &pair.planned,
            preregistered_repetitions: &repetitions,
            results: &results,
            status: GenerationQualificationPhaseStatusV1::Failed,
        },
    )
    .expect("mixed repeatability manifest");
    assert_eq!(manifest.evidence_item_count(), 2);
    assert_eq!(
        manifest.status(),
        GenerationQualificationPhaseStatusV1::Failed
    );
}
