use std::cell::Cell;

use rewrite_model::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureCategoryV1,
    CandidateGenerationAttemptFailurePhaseV1, CandidateGenerationAttemptFailureV1Input,
    CandidateGenerationAttemptRecordV1, GenerationQualificationOperationContractError,
    GenerationQualificationOperationReceiptV1Input,
    GenerationQualificationPhaseInterruptionRecordV1Input,
};
use tempfile::tempdir;

use super::{
    GenerationQualificationTerminalEvidenceV1TransactionError,
    GenerationQualificationTerminalEvidenceV1WriteDisposition,
    StoredGenerationQualificationTerminalEvidenceV1,
};
use crate::store::candidate_generation_execution::CandidateGenerationExecutionV1Input;
use crate::store::candidate_generation_execution::tests::support as execution;
use crate::store::generation_qualification_preregistration::tests::support::{
    self as prereg, Fixture,
};
use crate::{ArtifactStateStore, StoreError, WriteDisposition};

#[path = "tests/build.rs"]
pub(crate) mod build;

const COHORT_TABLES: [&str; 9] = [
    "generation_qualification_platform_evidence",
    "generation_qualification_license_evidence",
    "generation_attempt_ledger_manifests",
    "generation_repeatability_result_records",
    "generation_repeatability_evidence_manifests",
    "generation_resource_evidence_manifests",
    "generation_human_adjudication_evidence_manifests",
    "generation_qualification_operation_receipts",
    "generation_qualification_phase_interruption_records",
];

#[test]
fn skipped_cohort_is_absent_until_insert_and_does_not_qualify() {
    let (_directory, fixture, mut store) = open("skipped.db");
    let cohort = build::cohort(
        &fixture,
        without_result(&[], build::skipped_receipt(), None),
    );
    assert!(
        read_unchanged(&store, &fixture, &cohort)
            .expect("absent read")
            .is_none()
    );
    let inserted = write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())).expect("insert");
    assert_uniform(inserted, WriteDisposition::Inserted, false, false);
    assert_shape(&store, 0, 0, 1);
    let stored = read_unchanged(&store, &fixture, &cohort)
        .expect("read")
        .expect("present");
    assert_cohort(&stored, &fixture, &cohort);
}

#[test]
fn failed_receipt_without_interruption_replays_without_new_rows() {
    let (_directory, fixture, mut store) = open("failed.db");
    let attempt = failed_attempt(&fixture, None);
    persist_failed(&mut store, &fixture, &attempt);
    let cohort = build::cohort(
        &fixture,
        with_result(std::slice::from_ref(&attempt), build::failed_receipt()),
    );
    let inserted = write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())).expect("insert");
    assert_uniform(inserted, WriteDisposition::Inserted, true, false);
    assert_shape(&store, 1, 0, 1);
    let stored = read_unchanged(&store, &fixture, &cohort)
        .expect("read")
        .expect("present");
    assert_cohort(&stored, &fixture, &cohort);
    let counts = cohort_counts(&store);
    let replay = write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())).expect("replay");
    assert_uniform(replay, WriteDisposition::AlreadyPresent, true, false);
    assert_eq!(cohort_counts(&store), counts);
}

#[test]
fn conflicting_receipt_json_is_an_immutable_conflict() {
    let (_directory, fixture, mut store) = open("conflict.db");
    let attempt = failed_attempt(&fixture, None);
    persist_failed(&mut store, &fixture, &attempt);
    let attempts = std::slice::from_ref(&attempt);
    let cohort = build::cohort(&fixture, with_result(attempts, build::failed_receipt()));
    write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())).expect("insert");
    let stored_json = serde_json::to_vec(&cohort.receipt).expect("receipt json");
    assert_eq!(receipt_json(&store, &fixture), stored_json);
    let conflict = build::cohort(
        &fixture,
        with_result(attempts, build::receipt_at(2_000_000)),
    );
    assert!(matches!(
        write(&mut store, &fixture, &conflict, || Ok::<_, &str>(())),
        Err(
            GenerationQualificationTerminalEvidenceV1TransactionError::Store(
                StoreError::ImmutableConflict
            )
        )
    ));
    assert_eq!(receipt_json(&store, &fixture), stored_json);
    assert_shape(&store, 1, 0, 1);
}

#[test]
fn completed_receipt_with_interruption_is_rejected_before_insert() {
    let (_directory, fixture, mut store) = open("completed-interruption.db");
    let attempt = failed_attempt(&fixture, None);
    persist_failed(&mut store, &fixture, &attempt);
    let attempts = std::slice::from_ref(&attempt);
    let facts = build::interruption_facts(&fixture);
    let legal = build::cohort(
        &fixture,
        without_result(attempts, build::cancelled_receipt(), Some(&facts)),
    );
    let completed = build::cohort(&fixture, with_result(attempts, build::completed_receipt()));
    let legal_input = legal.input(&fixture);
    let mut input = completed.input(&fixture);
    input.phase_interruption = legal_input.phase_interruption;
    input.phase_interruption_input = legal_input.phase_interruption_input;
    assert!(matches!(
        store.transact_generation_qualification_terminal_evidence_v1(input, || Ok::<_, &str>(())),
        Err(
            GenerationQualificationTerminalEvidenceV1TransactionError::Store(
                StoreError::InvalidGenerationQualificationPreregistration(
                    GenerationQualificationOperationContractError::InvalidInterruptionClosure
                )
            )
        )
    ));
    assert_shape(&store, 0, 0, 0);
}

#[test]
fn cancelled_interruption_round_trips_and_replays() {
    let (_directory, fixture, mut store) = open("interruption.db");
    let attempt = failed_attempt(&fixture, None);
    persist_failed(&mut store, &fixture, &attempt);
    let facts = build::interruption_facts(&fixture);
    let cohort = build::cohort(
        &fixture,
        without_result(
            std::slice::from_ref(&attempt),
            build::cancelled_receipt(),
            Some(&facts),
        ),
    );
    let inserted = write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())).expect("insert");
    assert_uniform(inserted, WriteDisposition::Inserted, false, true);
    assert_shape(&store, 0, 1, 1);
    let stored = read_unchanged(&store, &fixture, &cohort)
        .expect("read")
        .expect("present");
    assert_cohort(&stored, &fixture, &cohort);
    let counts = cohort_counts(&store);
    let replay = write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())).expect("replay");
    assert_uniform(replay, WriteDisposition::AlreadyPresent, false, true);
    assert_eq!(cohort_counts(&store), counts);
}

#[test]
fn extra_managed_input_is_corrupt_before_insert() {
    let (_directory, fixture, mut store) = open("extra-managed.db");
    let completed = execution::completed(&fixture);
    let attempt = failed_attempt(&fixture, None);
    persist_failed(&mut store, &fixture, &attempt);
    let managed = [completed.managed_input];
    let cohort = build::cohort(
        &fixture,
        build::CohortSpec {
            attempts: std::slice::from_ref(&attempt),
            managed: &managed,
            include_failed_result: true,
            receipt_input: build::failed_receipt(),
            interruption_input: None,
        },
    );
    assert!(matches!(
        write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())),
        Err(
            GenerationQualificationTerminalEvidenceV1TransactionError::Store(
                StoreError::CorruptRecord
            )
        )
    ));
    assert_shape(&store, 0, 0, 0);
}

#[test]
fn ledger_mismatch_is_an_immutable_conflict_before_insert() {
    let (_directory, fixture, mut store) = open("ledger.db");
    let attempt = failed_attempt(&fixture, None);
    persist_failed(&mut store, &fixture, &attempt);
    let skipped = build::cohort(
        &fixture,
        without_result(&[], build::skipped_receipt(), None),
    );
    assert!(matches!(
        write(&mut store, &fixture, &skipped, || Ok::<_, &str>(())),
        Err(
            GenerationQualificationTerminalEvidenceV1TransactionError::Store(
                StoreError::ImmutableConflict
            )
        )
    ));
    assert_shape(&store, 0, 0, 0);
}

#[test]
fn passed_ledger_stores_an_empty_skipped_repeatability_manifest() {
    let (_directory, fixture, mut store) = open("passed.db");
    let completed = execution::completed(&fixture);
    persist_completed(&mut store, &fixture, &completed);
    let cohort = build::cohort(
        &fixture,
        build::CohortSpec {
            attempts: std::slice::from_ref(&completed.attempt),
            managed: std::slice::from_ref(&completed.managed_input),
            include_failed_result: false,
            receipt_input: build::failed_receipt(),
            interruption_input: None,
        },
    );
    let inserted = write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())).expect("insert");
    assert_uniform(inserted, WriteDisposition::Inserted, false, false);
    assert_shape(&store, 0, 0, 1);
    let stored = read_unchanged(&store, &fixture, &cohort)
        .expect("read")
        .expect("present");
    assert_cohort(&stored, &fixture, &cohort);
}

#[test]
fn corrupt_receipt_bytes_are_not_an_absent_cohort() {
    let (_directory, fixture, mut store) = open("corrupt.db");
    let attempt = failed_attempt(&fixture, None);
    persist_failed(&mut store, &fixture, &attempt);
    let cohort = build::cohort(
        &fixture,
        with_result(std::slice::from_ref(&attempt), build::failed_receipt()),
    );
    write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())).expect("insert");
    store
        .connection()
        .execute(
            "UPDATE generation_qualification_operation_receipts SET canonical_json = X'7B7D'",
            [],
        )
        .expect("corrupt receipt");
    assert!(matches!(
        read_unchanged(&store, &fixture, &cohort),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn gate_rejection_rolls_back_without_revealing_the_gate() {
    let (_directory, fixture, mut store) = open("gate.db");
    let attempt = failed_attempt(&fixture, None);
    persist_failed(&mut store, &fixture, &attempt);
    let cohort = build::cohort(
        &fixture,
        with_result(std::slice::from_ref(&attempt), build::failed_receipt()),
    );
    let calls = Cell::new(0_u8);
    let secret = "terminal evidence gate secret";
    let error = write(&mut store, &fixture, &cohort, || {
        let call = calls.get();
        calls.set(call + 1);
        if call == 2 { Err(secret) } else { Ok(()) }
    })
    .expect_err("gate");
    assert!(matches!(
        error,
        GenerationQualificationTerminalEvidenceV1TransactionError::Gate(value) if value == secret
    ));
    assert_eq!(
        format!("{error:?}"),
        "GenerationQualificationTerminalEvidenceV1TransactionError::Gate"
    );
    assert_eq!(
        error.to_string(),
        "generation qualification terminal evidence gate rejected the commit"
    );
    assert_eq!(calls.get(), 3);
    assert_shape(&store, 0, 0, 0);
}

#[test]
fn missing_plan_foundation_is_missing_record() {
    let directory = tempdir().expect("temporary directory");
    let fixture = prereg::fixture();
    let mut store = ArtifactStateStore::open(&directory.path().join("missing.db")).expect("open");
    let cohort = build::cohort(
        &fixture,
        without_result(&[], build::skipped_receipt(), None),
    );
    assert!(matches!(
        write(&mut store, &fixture, &cohort, || Ok::<_, &str>(())),
        Err(
            GenerationQualificationTerminalEvidenceV1TransactionError::Store(
                StoreError::MissingRecord
            )
        )
    ));
    assert!(matches!(
        read_unchanged(&store, &fixture, &cohort),
        Err(StoreError::MissingRecord)
    ));
}

fn open(name: &str) -> (tempfile::TempDir, Fixture, ArtifactStateStore) {
    let directory = tempdir().expect("temporary directory");
    let fixture = prereg::fixture();
    let store = execution::prepared_store(&directory.path().join(name), &fixture);
    (directory, fixture, store)
}

fn with_result(
    attempts: &[CandidateGenerationAttemptRecordV1],
    receipt: GenerationQualificationOperationReceiptV1Input,
) -> build::CohortSpec<'_> {
    build::CohortSpec {
        attempts,
        managed: &[],
        include_failed_result: true,
        receipt_input: receipt,
        interruption_input: None,
    }
}

fn without_result<'a>(
    attempts: &'a [CandidateGenerationAttemptRecordV1],
    receipt: GenerationQualificationOperationReceiptV1Input,
    interruption: Option<&'a GenerationQualificationPhaseInterruptionRecordV1Input>,
) -> build::CohortSpec<'a> {
    build::CohortSpec {
        attempts,
        managed: &[],
        include_failed_result: false,
        receipt_input: receipt,
        interruption_input: interruption,
    }
}

fn write<E>(
    store: &mut ArtifactStateStore,
    fixture: &Fixture,
    cohort: &build::Cohort,
    gate: impl FnMut() -> Result<(), E>,
) -> Result<
    GenerationQualificationTerminalEvidenceV1WriteDisposition,
    GenerationQualificationTerminalEvidenceV1TransactionError<E>,
> {
    store.transact_generation_qualification_terminal_evidence_v1(cohort.input(fixture), gate)
}

fn read_unchanged(
    store: &ArtifactStateStore,
    fixture: &Fixture,
    cohort: &build::Cohort,
) -> Result<Option<StoredGenerationQualificationTerminalEvidenceV1>, StoreError> {
    let before = store.connection().total_changes();
    let stored = store.generation_qualification_terminal_evidence_v1(cohort.read(fixture));
    assert_eq!(store.connection().total_changes(), before);
    stored
}

fn assert_uniform(
    value: GenerationQualificationTerminalEvidenceV1WriteDisposition,
    expected: WriteDisposition,
    results: bool,
    interruption: bool,
) {
    assert_eq!(value.platform, expected);
    assert_eq!(value.license, expected);
    assert_eq!(value.attempt_ledger, expected);
    assert_eq!(value.repeatability_results, results.then_some(expected));
    assert_eq!(value.repeatability_manifest, expected);
    assert_eq!(value.resource, expected);
    assert_eq!(value.human, expected);
    assert_eq!(value.receipt, expected);
    assert_eq!(value.phase_interruption, interruption.then_some(expected));
}

fn assert_cohort(
    stored: &StoredGenerationQualificationTerminalEvidenceV1,
    fixture: &Fixture,
    cohort: &build::Cohort,
) {
    assert_eq!(stored.preregistration().operation_policy(), &fixture.policy);
    assert_eq!(stored.platform_evidence(), &cohort.platform);
    assert_eq!(stored.license_evidence(), &cohort.license);
    assert_eq!(stored.attempt_ledger_manifest(), &cohort.ledger);
    assert_eq!(stored.repeatability_results(), cohort.results.as_slice());
    assert_eq!(stored.repeatability_manifest(), &cohort.repeatability);
    assert_eq!(stored.resource_manifest(), &cohort.resource);
    assert_eq!(stored.human_manifest(), &cohort.human);
    assert_eq!(stored.receipt(), &cohort.receipt);
    assert_eq!(stored.phase_interruption(), cohort.interruption.as_ref());
}

fn assert_shape(store: &ArtifactStateStore, results: i64, interruption: i64, other: i64) {
    for table in COHORT_TABLES {
        let expected = match table {
            "generation_repeatability_result_records" => results,
            "generation_qualification_phase_interruption_records" => interruption,
            _ => other,
        };
        assert_eq!(row_count(store, table), expected, "{table}");
    }
    for table in ["qualification_records", "qualification_v2_records"] {
        assert_eq!(row_count(store, table), 0, "{table}");
    }
}

fn cohort_counts(store: &ArtifactStateStore) -> [i64; 9] {
    COHORT_TABLES.map(|table| row_count(store, table))
}

fn receipt_json(store: &ArtifactStateStore, fixture: &Fixture) -> Vec<u8> {
    store
        .connection()
        .query_row(
            "SELECT canonical_json FROM generation_qualification_operation_receipts
             WHERE operation_policy_id = ?1",
            [fixture.policy.operation_policy_id().digest().as_str()],
            |row| row.get(0),
        )
        .expect("stored receipt")
}

fn persist_failed(
    store: &mut ArtifactStateStore,
    fixture: &Fixture,
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
}

fn persist_completed(
    store: &mut ArtifactStateStore,
    fixture: &Fixture,
    completed: &execution::CompletedFixture,
) {
    execution::checkpoint_precursor(store, fixture, &completed.precursor);
    store
        .transact_candidate_generation_execution_v1(
            execution::completed_input(fixture, completed),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("completed attempt");
}

fn failed_attempt(
    fixture: &Fixture,
    precursor: Option<&rewrite_model::CandidateGenerationAttemptPrecursorV1>,
) -> CandidateGenerationAttemptRecordV1 {
    let (failure_phase, failure_category, cleanup_disposition) = if precursor.is_some() {
        (
            CandidateGenerationAttemptFailurePhaseV1::Launch,
            CandidateGenerationAttemptFailureCategoryV1::LaunchFailed,
            CandidateGenerationAttemptCleanupDispositionV1::Succeeded,
        )
    } else {
        (
            CandidateGenerationAttemptFailurePhaseV1::RequestCompilation,
            CandidateGenerationAttemptFailureCategoryV1::RequestInvalid,
            CandidateGenerationAttemptCleanupDispositionV1::NotRequired,
        )
    };
    CandidateGenerationAttemptRecordV1::failed(
        &fixture.attempts[0],
        precursor,
        CandidateGenerationAttemptFailureV1Input {
            failure_phase,
            failure_category,
            traffic_observed: false,
            output_observed: false,
            cleanup_disposition,
        },
    )
    .expect("failed attempt")
}

fn row_count(store: &ArtifactStateStore, table: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    store
        .connection()
        .query_row(&sql, [], |row| row.get(0))
        .expect("count rows")
}
