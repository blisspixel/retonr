use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationRecordV1Input,
    CandidateDeterministicReportRelationshipV1, CandidateDeterministicReportSummaryV1,
    CandidateDeterministicTransformationCoverageV1, CandidateGenerationAttemptCleanupDispositionV1,
    CandidateGenerationAttemptFailureCategoryV1, CandidateGenerationAttemptFailurePhaseV1,
    CandidateGenerationAttemptFailureV1Input, CandidateGenerationAttemptRecordV1,
    CandidateGenerationReceiptSetV1, CandidateGenerationReceiptSetV1Relations,
    GenerationAttemptLedgerManifestV1, GenerationAttemptLedgerManifestV1Relations,
    GenerationQualificationPhaseScopeV1, GenerationQualificationPhaseStatusV1,
    GenerationRepeatabilityResultRecordV1, GenerationRepeatabilityResultRecordV1Relations,
    GenerationRepeatabilityTerminalStageV1,
};
use rewrite_types::Digest;
use rusqlite::params;

use super::super::{
    GenerationRepeatabilityTerminalResultV1Input, GenerationRepeatabilityTerminalResultV1ReadInput,
    GenerationRepeatabilityTerminalResultV1TransactionError,
};
use crate::store::candidate_deterministic_evaluation::CandidateDeterministicEvaluationV1Input;
use crate::store::candidate_generation_execution::tests::support as execution;
use crate::store::candidate_generation_receipt_set::CandidateGenerationReceiptSetV1Input;
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};
use crate::{ArtifactStateStore, StoreError, WriteDisposition};

pub(super) const TABLE: &str = "generation_repeatability_terminal_result_records";

pub(super) struct Session {
    pub(super) _directory: tempfile::TempDir,
    pub(super) store: ArtifactStateStore,
    pub(super) record: GenerationRepeatabilityResultRecordV1,
    pub(super) judge_failed: GenerationRepeatabilityResultRecordV1,
    pub(super) generation_failed: GenerationRepeatabilityResultRecordV1,
}

pub(super) fn session() -> Session {
    open(true)
}

pub(super) fn without_ledger() -> Session {
    open(false)
}

pub(super) fn read_input(
    session: &Session,
) -> GenerationRepeatabilityTerminalResultV1ReadInput<'_> {
    GenerationRepeatabilityTerminalResultV1ReadInput {
        record: &session.record,
    }
}

pub(super) fn commit(session: &mut Session) -> WriteDisposition {
    session
        .store
        .transact_generation_repeatability_terminal_result_v1(
            GenerationRepeatabilityTerminalResultV1Input {
                record: &session.record,
            },
            || Ok::<_, ()>(()),
        )
        .expect("repeatability result")
}

pub(super) fn count(store: &ArtifactStateStore, table: &str) -> i64 {
    store
        .connection()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count")
}

pub(super) fn canonical_json(session: &Session) -> Vec<u8> {
    session
        .store
        .connection()
        .query_row(&format!("SELECT canonical_json FROM {TABLE}"), [], |row| {
            row.get(0)
        })
        .expect("canonical json")
}

pub(super) fn expect_store(
    store: &mut ArtifactStateStore,
    record: &GenerationRepeatabilityResultRecordV1,
    expected: &StoreError,
) {
    let error = store
        .transact_generation_repeatability_terminal_result_v1(
            GenerationRepeatabilityTerminalResultV1Input { record },
            || Ok::<_, ()>(()),
        )
        .expect_err("refused write");
    assert_eq!(
        format!("{error:?}"),
        "GenerationRepeatabilityTerminalResultV1TransactionError::Store"
    );
    assert_eq!(
        error.to_string(),
        "generation repeatability terminal result storage failed"
    );
    assert!(matches!(
        error,
        GenerationRepeatabilityTerminalResultV1TransactionError::Store(value)
            if same_store(&value, expected)
    ));
}

pub(super) fn expect_read(
    store: &ArtifactStateStore,
    record: &GenerationRepeatabilityResultRecordV1,
    expected: &StoreError,
) {
    let error = store
        .generation_repeatability_terminal_result_v1(
            GenerationRepeatabilityTerminalResultV1ReadInput { record },
        )
        .expect_err("refused read");
    assert!(same_store(&error, expected));
}

fn open(persist_ledger: bool) -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let built = build();
    let mut store = execution::prepared_store(&directory.path().join("state.db"), &built.fixture);
    for completed in &built.completed {
        execution::checkpoint_precursor(&mut store, &built.fixture, &completed.precursor);
        store
            .transact_candidate_generation_execution_v1(
                execution::completed_input(&built.fixture, completed),
                || Ok::<_, ()>(()),
                |_| Ok::<_, ()>(()),
            )
            .expect("persist completed execution");
    }
    for receipt_set in &built.receipt_sets {
        store
            .transact_candidate_generation_receipt_set_v1(
                CandidateGenerationReceiptSetV1Input {
                    record: receipt_set,
                },
                || Ok::<_, ()>(()),
            )
            .expect("persist receipt set");
    }
    for evaluation in [&built.failed_evaluation, &built.passed_evaluation] {
        store
            .transact_candidate_deterministic_evaluation_v1(
                CandidateDeterministicEvaluationV1Input { record: evaluation },
                || Ok::<_, ()>(()),
            )
            .expect("persist evaluation");
    }
    if persist_ledger {
        insert_ledger(&store, &built.ledger);
    }
    Session {
        _directory: directory,
        store,
        record: built.deterministic_failed,
        judge_failed: built.judge_failed,
        generation_failed: built.generation_failed,
    }
}

struct Built {
    fixture: Fixture,
    completed: [execution::CompletedFixture; 2],
    receipt_sets: [CandidateGenerationReceiptSetV1; 2],
    failed_evaluation: CandidateDeterministicEvaluationRecordV1,
    passed_evaluation: CandidateDeterministicEvaluationRecordV1,
    ledger: GenerationAttemptLedgerManifestV1,
    deterministic_failed: GenerationRepeatabilityResultRecordV1,
    judge_failed: GenerationRepeatabilityResultRecordV1,
    generation_failed: GenerationRepeatabilityResultRecordV1,
}

fn build() -> Built {
    let fixture = support::fixture();
    let completed = [
        execution::completed_at(&fixture, 0),
        execution::completed_at(&fixture, 1),
    ];
    let receipt_sets = [
        receipt_set(&fixture, &completed[0], 0),
        receipt_set(&fixture, &completed[1], 1),
    ];
    let failed_evaluation = evaluation(&receipt_sets[0], &receipt_sets[1], false);
    let passed_evaluation = evaluation(&receipt_sets[0], &receipt_sets[1], true);
    let passed_attempts = std::slice::from_ref(&completed[0].attempt);
    let ledger = passed_ledger(&fixture, &completed[0].attempt);
    let deterministic_failed = result(
        &fixture,
        &ledger,
        passed_attempts,
        GenerationRepeatabilityTerminalStageV1::DeterministicFailed,
        Some(&receipt_sets[0]),
        Some(&failed_evaluation),
    );
    let judge_failed = result(
        &fixture,
        &ledger,
        passed_attempts,
        GenerationRepeatabilityTerminalStageV1::JudgeFailed,
        Some(&receipt_sets[0]),
        Some(&passed_evaluation),
    );
    let generation_failed = generation_failed_result(&fixture);
    Built {
        fixture,
        completed,
        receipt_sets,
        failed_evaluation,
        passed_evaluation,
        ledger,
        deterministic_failed,
        judge_failed,
        generation_failed,
    }
}

fn generation_failed_result(fixture: &Fixture) -> GenerationRepeatabilityResultRecordV1 {
    let failed_attempt = CandidateGenerationAttemptRecordV1::failed(
        &fixture.attempts[0],
        None,
        CandidateGenerationAttemptFailureV1Input {
            failure_phase: CandidateGenerationAttemptFailurePhaseV1::RequestCompilation,
            failure_category: CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
            traffic_observed: false,
            output_observed: false,
            cleanup_disposition: CandidateGenerationAttemptCleanupDispositionV1::NotRequired,
        },
    )
    .expect("failed attempt");
    let attempts = [failed_attempt];
    let ledger = GenerationAttemptLedgerManifestV1::new(ledger_relations(
        fixture,
        &attempts,
        GenerationQualificationPhaseStatusV1::Failed,
    ))
    .expect("failed ledger");
    result(
        fixture,
        &ledger,
        &attempts,
        GenerationRepeatabilityTerminalStageV1::CandidateGenerationFailed,
        None,
        None,
    )
}

fn result(
    fixture: &Fixture,
    ledger: &GenerationAttemptLedgerManifestV1,
    attempt_records: &[CandidateGenerationAttemptRecordV1],
    terminal_stage: GenerationRepeatabilityTerminalStageV1,
    receipt_set: Option<&CandidateGenerationReceiptSetV1>,
    evaluation: Option<&CandidateDeterministicEvaluationRecordV1>,
) -> GenerationRepeatabilityResultRecordV1 {
    let evidence = Digest::sha256(b"terminal evidence");
    GenerationRepeatabilityResultRecordV1::new(GenerationRepeatabilityResultRecordV1Relations {
        scope: scope(fixture),
        repetition: &fixture.repetitions[0],
        attempt_ledger: ledger,
        attempt_ledger_relations: ledger_relations(fixture, attempt_records, ledger.status()),
        terminal_stage,
        candidate_receipt_set: receipt_set,
        deterministic_evaluation: evaluation,
        candidate_judge_join: None,
        terminal_evidence_digest: &evidence,
    })
    .expect("repeatability result")
}

fn passed_ledger(
    fixture: &Fixture,
    attempt: &CandidateGenerationAttemptRecordV1,
) -> GenerationAttemptLedgerManifestV1 {
    GenerationAttemptLedgerManifestV1::new(ledger_relations(
        fixture,
        std::slice::from_ref(attempt),
        GenerationQualificationPhaseStatusV1::Passed,
    ))
    .expect("passed ledger")
}

fn ledger_relations<'a>(
    fixture: &'a Fixture,
    attempt_records: &'a [CandidateGenerationAttemptRecordV1],
    status: GenerationQualificationPhaseStatusV1,
) -> GenerationAttemptLedgerManifestV1Relations<'a> {
    GenerationAttemptLedgerManifestV1Relations {
        scope: scope(fixture),
        phase_policy_digest: fixture.policy.attempt_ledger_policy_digest(),
        planned_attempts: &fixture.attempts,
        attempt_records,
        status,
    }
}

fn scope(fixture: &Fixture) -> GenerationQualificationPhaseScopeV1<'_> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[0],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
    }
}

fn insert_ledger(store: &ArtifactStateStore, ledger: &GenerationAttemptLedgerManifestV1) {
    let json = serde_json::to_vec(ledger).expect("ledger json");
    store
        .connection()
        .execute(
            "INSERT INTO generation_attempt_ledger_manifests (
                generation_attempt_ledger_manifest_id, generation_system_id,
                generation_qualification_plan_id, generation_suite_manifest_id,
                evidence_item_count, status, canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'passed', ?6)",
            params![
                ledger.attempt_ledger_manifest_id().digest().as_str(),
                ledger.generation_system_id().digest().as_str(),
                ledger.generation_qualification_plan_id().digest().as_str(),
                ledger.suite_manifest_id().digest().as_str(),
                i64::from(ledger.evidence_item_count()),
                json,
            ],
        )
        .expect("insert ledger");
}

fn receipt_set(
    fixture: &Fixture,
    completed: &execution::CompletedFixture,
    index: usize,
) -> CandidateGenerationReceiptSetV1 {
    CandidateGenerationReceiptSetV1::new(CandidateGenerationReceiptSetV1Relations {
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
        repetition: &fixture.repetitions[0],
        generation_system: &fixture.systems[index],
        selection_policy: &fixture.selection_policy,
        planned_attempts: &fixture.attempts,
        attempt_records: std::slice::from_ref(&completed.attempt),
        receipts: std::slice::from_ref(&completed.receipt),
    })
    .expect("receipt set")
}

fn evaluation(
    receipt_a: &CandidateGenerationReceiptSetV1,
    receipt_b: &CandidateGenerationReceiptSetV1,
    passed: bool,
) -> CandidateDeterministicEvaluationRecordV1 {
    let summary_a = summary(1, 1, 0);
    let summary_b = if passed {
        summary(1, 0, 0)
    } else {
        summary(0, 0, 0)
    };
    let report_b: &[u8] = if passed {
        br#"{"schema_version":1,"candidate":"b"}"#
    } else {
        br#"{"schema_version":1,"candidate":"b","passed":0}"#
    };
    let reports = CandidateDeterministicReportRelationshipV1::new(
        br#"{"schema_version":1,"candidate":"a"}"#,
        report_b,
        summary_a,
        summary_b,
    )
    .expect("report relationship");
    let case_material = Digest::sha256(b"case material");
    let suite_pair = Digest::sha256(b"suite pair");
    CandidateDeterministicEvaluationRecordV1::new(
        receipt_a,
        receipt_b,
        CandidateDeterministicEvaluationRecordV1Input {
            case_material_set_digest: &case_material,
            suite_pair_digest: &suite_pair,
            report_relationship: &reports,
        },
    )
    .expect("evaluation record")
}

fn summary(passed: u32, acceptable: u32, rewritten: u32) -> CandidateDeterministicReportSummaryV1 {
    let coverage = CandidateDeterministicTransformationCoverageV1::new(acceptable, rewritten)
        .expect("coverage");
    CandidateDeterministicReportSummaryV1::new(1, passed, coverage).expect("summary")
}

fn same_store(actual: &StoreError, expected: &StoreError) -> bool {
    std::mem::discriminant(actual) == std::mem::discriminant(expected)
}
