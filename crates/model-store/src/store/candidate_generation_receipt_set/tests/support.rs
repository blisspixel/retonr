use rewrite_model::{CandidateGenerationReceiptSetV1, CandidateGenerationReceiptSetV1Relations};

use super::super::{
    CandidateGenerationReceiptSetV1Input, CandidateGenerationReceiptSetV1ReadInput,
    CandidateGenerationReceiptSetV1TransactionError,
};
use crate::store::candidate_generation_execution::tests::support as execution;
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};
use crate::{ArtifactStateStore, StoreError, WriteDisposition};

pub(super) const TABLE: &str = "candidate_generation_receipt_sets";

pub(super) struct Session {
    pub(super) _directory: tempfile::TempDir,
    pub(super) store: ArtifactStateStore,
    pub(super) record: CandidateGenerationReceiptSetV1,
}

pub(super) struct ParentIds {
    pub(super) system: String,
    pub(super) plan: String,
    pub(super) suite: String,
    pub(super) repetition: String,
    pub(super) policy: String,
    pub(super) attempt_record: String,
}

pub(super) fn session() -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let built = build();
    let mut store = execution::prepared_store(&directory.path().join("state.db"), &built.fixture);
    execution::checkpoint_precursor(&mut store, &built.fixture, &built.completed.precursor);
    store
        .transact_candidate_generation_execution_v1(
            execution::completed_input(&built.fixture, &built.completed),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("persist completed execution");
    Session {
        _directory: directory,
        store,
        record: built.record,
    }
}

pub(super) fn foundation() -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let mut store =
        ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let built = build();
    support::persist_plan_foundation(&mut store, &built.fixture);
    Session {
        _directory: directory,
        store,
        record: built.record,
    }
}

pub(super) fn bare() -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let store = ArtifactStateStore::open(&directory.path().join("state.db")).expect("open store");
    let built = build();
    Session {
        _directory: directory,
        store,
        record: built.record,
    }
}

pub(super) fn read_input(session: &Session) -> CandidateGenerationReceiptSetV1ReadInput<'_> {
    CandidateGenerationReceiptSetV1ReadInput {
        record: &session.record,
    }
}

pub(super) fn commit(session: &mut Session) -> WriteDisposition {
    session
        .store
        .transact_candidate_generation_receipt_set_v1(
            CandidateGenerationReceiptSetV1Input {
                record: &session.record,
            },
            || Ok::<_, ()>(()),
        )
        .expect("receipt set")
}

pub(super) fn parent_ids(session: &Session) -> ParentIds {
    ParentIds {
        system: digest(session.record.generation_system_id().digest()),
        plan: digest(session.record.qualification_plan_id().digest()),
        suite: digest(session.record.suite_manifest_id().digest()),
        repetition: digest(session.record.repetition_id().digest()),
        policy: digest(session.record.selection_policy_id().digest()),
        attempt_record: digest(session.record.entries()[0].attempt_record_id().digest()),
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
    record: &CandidateGenerationReceiptSetV1,
    expected: &StoreError,
) {
    let error = store
        .transact_candidate_generation_receipt_set_v1(
            CandidateGenerationReceiptSetV1Input { record },
            || Ok::<_, ()>(()),
        )
        .expect_err("refused write");
    assert_eq!(
        format!("{error:?}"),
        "CandidateGenerationReceiptSetV1TransactionError::Store"
    );
    assert_eq!(
        error.to_string(),
        "candidate generation receipt set storage failed"
    );
    assert!(matches!(
        error,
        CandidateGenerationReceiptSetV1TransactionError::Store(value) if same_store(&value, expected)
    ));
}

pub(super) fn expect_read(
    store: &ArtifactStateStore,
    record: &CandidateGenerationReceiptSetV1,
    expected: &StoreError,
) {
    let error = store
        .candidate_generation_receipt_set_v1(CandidateGenerationReceiptSetV1ReadInput { record })
        .expect_err("refused read");
    assert!(same_store(&error, expected));
}

struct Built {
    fixture: Fixture,
    completed: execution::CompletedFixture,
    record: CandidateGenerationReceiptSetV1,
}

fn build() -> Built {
    let fixture = support::fixture();
    let completed = execution::completed(&fixture);
    let record = CandidateGenerationReceiptSetV1::new(CandidateGenerationReceiptSetV1Relations {
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
        repetition: &fixture.repetitions[0],
        generation_system: &fixture.systems[0],
        selection_policy: &fixture.selection_policy,
        planned_attempts: &fixture.attempts,
        attempt_records: std::slice::from_ref(&completed.attempt),
        receipts: std::slice::from_ref(&completed.receipt),
    })
    .expect("receipt set");
    Built {
        fixture,
        completed,
        record,
    }
}

fn digest(value: &rewrite_types::Digest) -> String {
    value.as_str().to_owned()
}

fn same_store(actual: &StoreError, expected: &StoreError) -> bool {
    std::mem::discriminant(actual) == std::mem::discriminant(expected)
}
