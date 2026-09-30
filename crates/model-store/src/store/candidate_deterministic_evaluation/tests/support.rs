use rewrite_model::{
    CandidateDeterministicEvaluationRecordV1, CandidateDeterministicEvaluationRecordV1Input,
    CandidateDeterministicReportRelationshipV1, CandidateDeterministicReportSummaryV1,
    CandidateDeterministicTransformationCoverageV1, CandidateGenerationReceiptSetV1,
    CandidateGenerationReceiptSetV1Relations,
};
use rewrite_types::Digest;

use super::super::{
    CandidateDeterministicEvaluationV1Input, CandidateDeterministicEvaluationV1ReadInput,
    CandidateDeterministicEvaluationV1TransactionError,
};
use crate::store::candidate_generation_execution::tests::support as execution;
use crate::store::candidate_generation_receipt_set::CandidateGenerationReceiptSetV1Input;
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};
use crate::{ArtifactStateStore, StoreError, WriteDisposition};

pub(super) const TABLE: &str = "candidate_deterministic_evaluation_records";

pub(super) struct Session {
    pub(super) _directory: tempfile::TempDir,
    pub(super) store: ArtifactStateStore,
    pub(super) record: CandidateDeterministicEvaluationRecordV1,
    pub(super) alternate: CandidateDeterministicEvaluationRecordV1,
}

pub(super) struct ParentIds {
    pub(super) receipt_a: String,
    pub(super) receipt_b: String,
}

pub(super) fn session() -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let materials = materials();
    let mut store =
        execution::prepared_store(&directory.path().join("state.db"), &materials.fixture);
    persist_executions(&mut store, &materials);
    for receipt_set in &materials.receipt_sets {
        persist_receipt_set(&mut store, receipt_set);
    }
    Session {
        _directory: directory,
        store,
        record: materials.record,
        alternate: materials.alternate,
    }
}

pub(super) fn bare() -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let materials = materials();
    Session {
        _directory: directory,
        store,
        record: materials.record,
        alternate: materials.alternate,
    }
}

pub(super) fn one_receipt_set() -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let materials = materials();
    let mut store =
        execution::prepared_store(&directory.path().join("state.db"), &materials.fixture);
    persist_executions(&mut store, &materials);
    persist_receipt_set(&mut store, &materials.receipt_sets[0]);
    Session {
        _directory: directory,
        store,
        record: materials.record,
        alternate: materials.alternate,
    }
}

pub(super) fn read_input(session: &Session) -> CandidateDeterministicEvaluationV1ReadInput<'_> {
    CandidateDeterministicEvaluationV1ReadInput {
        record: &session.record,
    }
}

pub(super) fn commit(session: &mut Session) -> WriteDisposition {
    session
        .store
        .transact_candidate_deterministic_evaluation_v1(
            CandidateDeterministicEvaluationV1Input {
                record: &session.record,
            },
            || Ok::<_, ()>(()),
        )
        .expect("deterministic evaluation")
}

pub(super) fn parent_ids(session: &Session) -> ParentIds {
    ParentIds {
        receipt_a: digest(session.record.candidate_a_receipt_set_id().digest()),
        receipt_b: digest(session.record.candidate_b_receipt_set_id().digest()),
    }
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
    record: &CandidateDeterministicEvaluationRecordV1,
    expected: &StoreError,
) {
    let error = store
        .transact_candidate_deterministic_evaluation_v1(
            CandidateDeterministicEvaluationV1Input { record },
            || Ok::<_, ()>(()),
        )
        .expect_err("refused write");
    assert_eq!(
        format!("{error:?}"),
        "CandidateDeterministicEvaluationV1TransactionError::Store"
    );
    assert_eq!(
        error.to_string(),
        "candidate deterministic evaluation storage failed"
    );
    assert!(matches!(
        error,
        CandidateDeterministicEvaluationV1TransactionError::Store(value)
            if same_store(&value, expected)
    ));
}

pub(super) fn expect_read(
    store: &ArtifactStateStore,
    record: &CandidateDeterministicEvaluationRecordV1,
    expected: &StoreError,
) {
    let error = store
        .candidate_deterministic_evaluation_v1(CandidateDeterministicEvaluationV1ReadInput {
            record,
        })
        .expect_err("refused read");
    assert!(same_store(&error, expected));
}

struct Materials {
    fixture: Fixture,
    completed: [execution::CompletedFixture; 2],
    receipt_sets: [CandidateGenerationReceiptSetV1; 2],
    record: CandidateDeterministicEvaluationRecordV1,
    alternate: CandidateDeterministicEvaluationRecordV1,
}

fn materials() -> Materials {
    let fixture = support::fixture();
    let completed = [
        execution::completed_at(&fixture, 0),
        execution::completed_at(&fixture, 1),
    ];
    let receipt_sets = [
        receipt_set(&fixture, &completed[0], 0),
        receipt_set(&fixture, &completed[1], 1),
    ];
    let record = evaluation(&receipt_sets[0], &receipt_sets[1], true);
    let alternate = evaluation(&receipt_sets[0], &receipt_sets[1], false);
    Materials {
        fixture,
        completed,
        receipt_sets,
        record,
        alternate,
    }
}

fn persist_executions(store: &mut ArtifactStateStore, materials: &Materials) {
    for completed in &materials.completed {
        execution::checkpoint_precursor(store, &materials.fixture, &completed.precursor);
        store
            .transact_candidate_generation_execution_v1(
                execution::completed_input(&materials.fixture, completed),
                || Ok::<_, ()>(()),
                |_| Ok::<_, ()>(()),
            )
            .expect("persist completed execution");
    }
}

fn persist_receipt_set(store: &mut ArtifactStateStore, record: &CandidateGenerationReceiptSetV1) {
    store
        .transact_candidate_generation_receipt_set_v1(
            CandidateGenerationReceiptSetV1Input { record },
            || Ok::<_, ()>(()),
        )
        .expect("persist receipt set");
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

fn digest(value: &rewrite_types::Digest) -> String {
    value.as_str().to_owned()
}

fn same_store(actual: &StoreError, expected: &StoreError) -> bool {
    std::mem::discriminant(actual) == std::mem::discriminant(expected)
}
