use rewrite_model::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureCategoryV1,
    CandidateGenerationAttemptFailurePhaseV1, CandidateGenerationAttemptFailureV1Input,
    CandidateGenerationAttemptRecordV1, GenerationQualificationRecordV1,
    GenerationQualificationStatusV1,
};

use super::super::{
    GenerationQualificationRecordV1Input, GenerationQualificationRecordV1ReadInput,
    GenerationQualificationRecordV1TransactionError,
};
use crate::store::candidate_generation_execution::CandidateGenerationExecutionV1Input;
use crate::store::candidate_generation_execution::tests::support as execution;
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};
use crate::store::generation_qualification_terminal_evidence::tests::build::{self, Cohort};
use crate::{ArtifactStateStore, StoreError, WriteDisposition};

pub(super) const TABLE: &str = "generation_qualification_records";

pub(crate) struct Session {
    pub(super) _directory: tempfile::TempDir,
    pub(crate) store: ArtifactStateStore,
    pub(crate) record: GenerationQualificationRecordV1,
}

pub(crate) fn session() -> Session {
    open(true, true)
}

pub(super) fn without_cohort() -> Session {
    open(false, false)
}

pub(super) fn without_receipt() -> Session {
    open(true, false)
}

pub(super) fn read_input(session: &Session) -> GenerationQualificationRecordV1ReadInput<'_> {
    GenerationQualificationRecordV1ReadInput {
        record: &session.record,
    }
}

pub(crate) fn commit(session: &mut Session) -> WriteDisposition {
    session
        .store
        .transact_generation_qualification_record_v1(
            GenerationQualificationRecordV1Input {
                record: &session.record,
            },
            || Ok::<_, ()>(()),
        )
        .expect("qualification record")
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
    record: &GenerationQualificationRecordV1,
    expected: &StoreError,
) {
    let error = store
        .transact_generation_qualification_record_v1(
            GenerationQualificationRecordV1Input { record },
            || Ok::<_, ()>(()),
        )
        .expect_err("refused write");
    assert_eq!(
        format!("{error:?}"),
        "GenerationQualificationRecordV1TransactionError::Store"
    );
    assert_eq!(
        error.to_string(),
        "generation qualification record storage failed"
    );
    assert!(matches!(
        error,
        GenerationQualificationRecordV1TransactionError::Store(value)
            if same_store(&value, expected)
    ));
}

pub(super) fn expect_read(
    store: &ArtifactStateStore,
    record: &GenerationQualificationRecordV1,
    expected: &StoreError,
) {
    let error = store
        .generation_qualification_record_v1(GenerationQualificationRecordV1ReadInput { record })
        .expect_err("refused read");
    assert!(same_store(&error, expected));
}

fn open(persist_cohort: bool, keep_receipt: bool) -> Session {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store = execution::prepared_store(&directory.path().join("state.db"), &fixture);
    let attempt = failed_attempt(&fixture);
    let cohort = build::cohort(
        &fixture,
        build::CohortSpec {
            attempts: std::slice::from_ref(&attempt),
            managed: &[],
            include_failed_result: true,
            receipt_input: build::completed_receipt(),
            interruption_input: None,
        },
    );
    let record = cohort.qualification_record(&fixture, std::slice::from_ref(&attempt));
    assert_eq!(record.status(), GenerationQualificationStatusV1::Rejected);
    if persist_cohort {
        persist(&mut store, &fixture, &cohort, &attempt);
        if !keep_receipt {
            store
                .connection()
                .execute(
                    "DELETE FROM generation_qualification_operation_receipts",
                    [],
                )
                .expect("delete receipt");
        }
    }
    Session {
        _directory: directory,
        store,
        record,
    }
}

fn persist(
    store: &mut ArtifactStateStore,
    fixture: &Fixture,
    cohort: &Cohort,
    attempt: &CandidateGenerationAttemptRecordV1,
) {
    store
        .transact_candidate_generation_execution_v1(
            CandidateGenerationExecutionV1Input::Failed {
                preregistration: execution::read_input(fixture),
                precursor: None,
                attempt,
            },
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("failed attempt");
    store
        .transact_generation_qualification_terminal_evidence_v1(cohort.input(fixture), || {
            Ok::<_, ()>(())
        })
        .expect("terminal cohort");
}

fn failed_attempt(fixture: &Fixture) -> CandidateGenerationAttemptRecordV1 {
    CandidateGenerationAttemptRecordV1::failed(
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
    .expect("failed attempt")
}

fn same_store(actual: &StoreError, expected: &StoreError) -> bool {
    std::mem::discriminant(actual) == std::mem::discriminant(expected)
}
