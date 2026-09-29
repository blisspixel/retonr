use rewrite_model::{
    GenerationQualificationPhaseScopeV1, GenerationResourceAttemptResultRecordV1,
    GenerationResourceAttemptResultRecordV1Input, GenerationResourceAttemptResultRecordV1Relations,
    GenerationResourceExceededLimitV1,
};
use rewrite_types::Digest;

use super::super::{
    GenerationResourceAttemptResultV1Input, GenerationResourceAttemptResultV1ReadInput,
    GenerationResourceAttemptResultV1TransactionError,
};
use crate::store::candidate_generation_execution::tests::support as execution;
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};
use crate::{ArtifactStateStore, StoreError, WriteDisposition};

pub(super) const TABLE: &str = "generation_resource_attempt_result_records";

pub(super) struct Session {
    pub(super) _directory: tempfile::TempDir,
    pub(super) store: ArtifactStateStore,
    pub(super) fixture: Fixture,
    pub(super) completed: execution::CompletedFixture,
    pub(super) record: GenerationResourceAttemptResultRecordV1,
}

pub(super) struct ParentIds {
    pub(super) system: String,
    pub(super) other_system: String,
    pub(super) plan: String,
    pub(super) suite: String,
    pub(super) case_id: String,
    pub(super) repetition: String,
    pub(super) planned: String,
    pub(super) attempt_record: String,
    pub(super) receipt: String,
    pub(super) policy: String,
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
        fixture: built.fixture,
        completed: built.completed,
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
        fixture: built.fixture,
        completed: built.completed,
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
        fixture: built.fixture,
        completed: built.completed,
        record: built.record,
    }
}

pub(super) fn read_input(session: &Session) -> GenerationResourceAttemptResultV1ReadInput<'_> {
    GenerationResourceAttemptResultV1ReadInput {
        record: &session.record,
    }
}

pub(super) fn other_record(session: &Session) -> GenerationResourceAttemptResultRecordV1 {
    record(&session.fixture, &session.completed, other_observations())
}

pub(super) fn commit(session: &mut Session) -> WriteDisposition {
    session
        .store
        .transact_generation_resource_attempt_result_v1(
            GenerationResourceAttemptResultV1Input {
                record: &session.record,
            },
            || Ok::<_, ()>(()),
        )
        .expect("resource result")
}

pub(super) fn parent_ids(session: &Session) -> ParentIds {
    ParentIds {
        system: digest(session.fixture.systems[0].generation_system_id().digest()),
        other_system: digest(session.fixture.systems[1].generation_system_id().digest()),
        plan: digest(session.fixture.plan.qualification_plan_id().digest()),
        suite: digest(session.fixture.suite.suite_manifest_id().digest()),
        case_id: digest(session.fixture.cases[0].case_id().digest()),
        repetition: digest(session.fixture.repetitions[0].repetition_id().digest()),
        planned: digest(session.fixture.attempts[0].planned_attempt_id().digest()),
        attempt_record: digest(session.completed.attempt.attempt_record_id().digest()),
        receipt: digest(session.completed.receipt.receipt_id().digest()),
        policy: digest(session.record.resource_policy_digest()),
    }
}

pub(super) fn scope(fixture: &Fixture) -> GenerationQualificationPhaseScopeV1<'_> {
    GenerationQualificationPhaseScopeV1 {
        generation_system: &fixture.systems[0],
        qualification_plan: &fixture.plan,
        suite: &fixture.suite,
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
    record: &GenerationResourceAttemptResultRecordV1,
    expected: &StoreError,
) {
    let error = store
        .transact_generation_resource_attempt_result_v1(
            GenerationResourceAttemptResultV1Input { record },
            || Ok::<_, ()>(()),
        )
        .expect_err("refused write");
    assert_eq!(
        format!("{error:?}"),
        "GenerationResourceAttemptResultV1TransactionError::Store"
    );
    assert_eq!(error.to_string(), "resource attempt result storage failed");
    assert!(matches!(
        error,
        GenerationResourceAttemptResultV1TransactionError::Store(value) if same_store(&value, expected)
    ));
}

pub(super) fn expect_read(
    store: &ArtifactStateStore,
    record: &GenerationResourceAttemptResultRecordV1,
    expected: &StoreError,
) {
    let error = store
        .generation_resource_attempt_result_v1(GenerationResourceAttemptResultV1ReadInput {
            record,
        })
        .expect_err("refused read");
    assert!(same_store(&error, expected));
}

struct Built {
    fixture: Fixture,
    completed: execution::CompletedFixture,
    record: GenerationResourceAttemptResultRecordV1,
}

fn build() -> Built {
    let fixture = support::fixture();
    assert_eq!(
        fixture.policy.target_generation_system_id(),
        fixture.systems[0].generation_system_id()
    );
    assert!(fixture.systems.len() >= 2, "plan carries a second system");
    let completed = execution::completed(&fixture);
    let record = record(&fixture, &completed, observations());
    Built {
        fixture,
        completed,
        record,
    }
}

fn record(
    fixture: &Fixture,
    completed: &execution::CompletedFixture,
    input: GenerationResourceAttemptResultRecordV1Input,
) -> GenerationResourceAttemptResultRecordV1 {
    GenerationResourceAttemptResultRecordV1::new(relations(fixture, completed), input)
        .expect("resource result")
}

fn relations<'a>(
    fixture: &'a Fixture,
    completed: &'a execution::CompletedFixture,
) -> GenerationResourceAttemptResultRecordV1Relations<'a> {
    GenerationResourceAttemptResultRecordV1Relations {
        scope: scope(fixture),
        operation_policy: &fixture.policy,
        case: &fixture.cases[0],
        repetition: &fixture.repetitions[0],
        planned_attempt: &fixture.attempts[0],
        attempt_record: &completed.attempt,
        candidate_generation_receipt: &completed.receipt,
    }
}

fn observations() -> GenerationResourceAttemptResultRecordV1Input {
    GenerationResourceAttemptResultRecordV1Input {
        prompt_token_count: 2,
        generated_token_count: 3,
        total_duration_nanoseconds: 5_000,
        load_duration_nanoseconds: 0,
        prompt_evaluation_duration_nanoseconds: 0,
        evaluation_duration_nanoseconds: 5_000,
        attempt_elapsed_nanoseconds: 5_000,
        first_response_elapsed_nanoseconds: 0,
        cleanup_elapsed_nanoseconds: 0,
        worker_high_water_resident_bytes: 0,
        runtime_installed_payload_bytes: 0,
        model_installed_payload_bytes: 0,
        installed_footprint_bytes: 0,
        exceeded_limits: Vec::new(),
    }
}

fn other_observations() -> GenerationResourceAttemptResultRecordV1Input {
    GenerationResourceAttemptResultRecordV1Input {
        prompt_token_count: 2,
        generated_token_count: 3,
        total_duration_nanoseconds: 6_000,
        load_duration_nanoseconds: 0,
        prompt_evaluation_duration_nanoseconds: 0,
        evaluation_duration_nanoseconds: 5_000,
        attempt_elapsed_nanoseconds: 8_000,
        first_response_elapsed_nanoseconds: 1_000,
        cleanup_elapsed_nanoseconds: 1_000,
        worker_high_water_resident_bytes: 10,
        runtime_installed_payload_bytes: 1,
        model_installed_payload_bytes: 1,
        installed_footprint_bytes: 2,
        exceeded_limits: vec![GenerationResourceExceededLimitV1::AttemptElapsed],
    }
}

fn digest(value: &Digest) -> String {
    value.as_str().to_owned()
}

fn same_store(actual: &StoreError, expected: &StoreError) -> bool {
    std::mem::discriminant(actual) == std::mem::discriminant(expected)
}
