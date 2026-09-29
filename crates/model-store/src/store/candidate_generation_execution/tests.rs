use std::cell::Cell;

use rewrite_model::{
    CandidateGenerationAttemptCleanupDispositionV1, CandidateGenerationAttemptFailureCategoryV1,
    CandidateGenerationAttemptFailurePhaseV1, CandidateGenerationAttemptFailureV1Input,
    CandidateGenerationAttemptRecordV1,
};
use tempfile::tempdir;

use super::*;
use crate::store::generation_qualification_preregistration::tests::support as prereg_support;

#[path = "tests/support.rs"]
pub(crate) mod support;

#[test]
fn completed_execution_is_atomic_cold_readable_and_exactly_idempotent() {
    let directory = tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let completed = support::completed(&fixture);
    let mut store = support::prepared_store(&directory.path().join("completed.db"), &fixture);
    support::checkpoint_precursor(&mut store, &fixture, &completed.precursor);
    let gate_calls = Cell::new(0);
    let (validated, first) = store
        .transact_candidate_generation_execution_v1(
            support::completed_input(&fixture, &completed),
            || {
                gate_calls.set(gate_calls.get() + 1);
                Ok::<_, ()>(())
            },
            |readback| {
                assert!(matches!(
                    readback.execution(),
                    StoredCandidateGenerationExecutionV1::Completed { attempt, .. }
                        if attempt == &completed.attempt
                ));
                Ok::<_, ()>("validated")
            },
        )
        .expect("completed transaction");
    assert_eq!(validated, "validated");
    assert_eq!(gate_calls.get(), 3);
    assert_eq!(first.precursor, Some(WriteDisposition::AlreadyPresent));
    assert_eq!(first.managed_evidence, Some(WriteDisposition::Inserted));
    assert_eq!(first.attempt, WriteDisposition::Inserted);

    let stored = store
        .candidate_generation_execution_v1(CandidateGenerationExecutionV1ReadInput::Completed {
            preregistration: support::read_input(&fixture),
            planned_attempt_id: fixture.attempts[0].planned_attempt_id(),
            managed_evidence_input: &completed.managed_input,
        })
        .expect("cold read")
        .expect("completed execution present");
    assert!(matches!(
        stored,
        StoredCandidateGenerationExecutionV1::Completed { attempt, .. }
            if attempt == completed.attempt
    ));

    let ((), replay) = store
        .transact_candidate_generation_execution_v1(
            support::completed_input(&fixture, &completed),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("exact replay");
    assert_eq!(
        replay.managed_evidence,
        Some(WriteDisposition::AlreadyPresent)
    );
    assert_eq!(replay.cleanup, Some(WriteDisposition::AlreadyPresent));
    assert_eq!(replay.bundle, Some(WriteDisposition::AlreadyPresent));
    assert_eq!(replay.storage, Some(WriteDisposition::AlreadyPresent));
    assert_eq!(replay.readback, Some(WriteDisposition::AlreadyPresent));
    assert_eq!(replay.receipt, Some(WriteDisposition::AlreadyPresent));
    assert_eq!(replay.attempt, WriteDisposition::AlreadyPresent);
}

#[test]
fn failed_execution_without_precursor_is_terminal_and_idempotent() {
    let directory = tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let mut store = support::prepared_store(&directory.path().join("failed.db"), &fixture);
    let attempt = failed_attempt(&fixture, None);
    let input = || CandidateGenerationExecutionV1Input::Failed {
        preregistration: support::read_input(&fixture),
        precursor: None,
        attempt: &attempt,
    };
    let ((), first) = store
        .transact_candidate_generation_execution_v1(
            input(),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("failed transaction");
    assert_eq!(first.precursor, None);
    assert_eq!(first.attempt, WriteDisposition::Inserted);
    let stored = store
        .candidate_generation_execution_v1(CandidateGenerationExecutionV1ReadInput::Failed {
            preregistration: support::read_input(&fixture),
            planned_attempt_id: fixture.attempts[0].planned_attempt_id(),
        })
        .expect("cold read")
        .expect("failed execution present");
    assert!(matches!(
        stored,
        StoredCandidateGenerationExecutionV1::Failed { precursor: None, attempt: value, .. }
            if value == attempt
    ));
    let ((), replay) = store
        .transact_candidate_generation_execution_v1(
            input(),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("failed replay");
    assert_eq!(replay.attempt, WriteDisposition::AlreadyPresent);
}

#[test]
fn failed_execution_with_checkpointed_precursor_retains_the_exact_link() {
    let directory = tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let precursor = support::precursor(&fixture);
    let attempt = failed_attempt(&fixture, Some(&precursor));
    let mut store =
        support::prepared_store(&directory.path().join("failed-precursor.db"), &fixture);
    support::checkpoint_precursor(&mut store, &fixture, &precursor);
    let ((), disposition) = store
        .transact_candidate_generation_execution_v1(
            CandidateGenerationExecutionV1Input::Failed {
                preregistration: support::read_input(&fixture),
                precursor: Some(&precursor),
                attempt: &attempt,
            },
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("failed transaction with precursor");
    assert_eq!(
        disposition.precursor,
        Some(WriteDisposition::AlreadyPresent)
    );
    let stored = store
        .candidate_generation_execution_v1(CandidateGenerationExecutionV1ReadInput::Failed {
            preregistration: support::read_input(&fixture),
            planned_attempt_id: fixture.attempts[0].planned_attempt_id(),
        })
        .expect("cold read")
        .expect("failed execution present");
    assert!(matches!(
        stored,
        StoredCandidateGenerationExecutionV1::Failed {
            precursor: Some(value),
            attempt: stored_attempt,
            ..
        } if value == precursor && stored_attempt == attempt
    ));
}

#[test]
fn validation_and_final_gate_rejection_roll_back_the_complete_cohort() {
    let directory = tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let completed = support::completed(&fixture);
    let mut store = support::prepared_store(&directory.path().join("rollback.db"), &fixture);
    support::checkpoint_precursor(&mut store, &fixture, &completed.precursor);
    let rejected = store.transact_candidate_generation_execution_v1(
        support::completed_input(&fixture, &completed),
        || Ok::<_, &'static str>(()),
        |_| Err::<(), _>("validation"),
    );
    assert!(matches!(
        rejected,
        Err(CandidateGenerationExecutionV1TransactionError::Validation(
            "validation"
        ))
    ));
    assert_terminal_tables_empty(&store);

    let calls = Cell::new(0);
    let rejected = store.transact_candidate_generation_execution_v1(
        support::completed_input(&fixture, &completed),
        || {
            calls.set(calls.get() + 1);
            if calls.get() == 3 {
                Err("gate")
            } else {
                Ok(())
            }
        },
        |_| Ok(()),
    );
    assert!(matches!(
        rejected,
        Err(CandidateGenerationExecutionV1TransactionError::Gate("gate"))
    ));
    assert_terminal_tables_empty(&store);
}

#[test]
fn completed_execution_refuses_to_heal_a_partial_terminal_cohort() {
    let directory = tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let completed = support::completed(&fixture);
    let mut store = support::prepared_store(&directory.path().join("partial.db"), &fixture);
    support::checkpoint_precursor(&mut store, &fixture, &completed.precursor);
    support::insert_managed_prefix(&store, &completed);
    assert!(matches!(
        store.transact_candidate_generation_execution_v1(
            support::completed_input(&fixture, &completed),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        ),
        Err(CandidateGenerationExecutionV1TransactionError::Store(
            StoreError::ImmutableConflict
        ))
    ));
    let managed_count: i64 = store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM managed_candidate_generation_evidence",
            [],
            |row| row.get(0),
        )
        .expect("count managed prefix");
    assert_eq!(managed_count, 1);
    for table in [
        "candidate_generation_cleanup_records",
        "generation_evidence_bundles",
        "generation_evidence_bundle_storage",
        "generation_evidence_bundle_readbacks",
        "candidate_generation_receipts",
        "candidate_generation_attempt_records",
    ] {
        assert_eq!(row_count(&store, table), 0, "{table} must not be healed");
    }
}

#[test]
fn cold_read_rejects_corrupt_managed_canonical_bytes() {
    let directory = tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let completed = support::completed(&fixture);
    let mut store = support::prepared_store(&directory.path().join("corrupt.db"), &fixture);
    support::checkpoint_precursor(&mut store, &fixture, &completed.precursor);
    store
        .transact_candidate_generation_execution_v1(
            support::completed_input(&fixture, &completed),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("persist completed execution");
    store
        .connection()
        .execute(
            "UPDATE managed_candidate_generation_evidence SET canonical_json = X'7b7d'",
            [],
        )
        .expect("corrupt managed bytes");
    assert!(matches!(
        store.candidate_generation_execution_v1(
            CandidateGenerationExecutionV1ReadInput::Completed {
                preregistration: support::read_input(&fixture),
                planned_attempt_id: fixture.attempts[0].planned_attempt_id(),
                managed_evidence_input: &completed.managed_input,
            }
        ),
        Err(StoreError::CorruptRecord)
    ));
}

fn failed_attempt(
    fixture: &prereg_support::Fixture,
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

fn assert_terminal_tables_empty(store: &ArtifactStateStore) {
    for table in [
        "managed_candidate_generation_evidence",
        "candidate_generation_cleanup_records",
        "generation_evidence_bundles",
        "generation_evidence_bundle_storage",
        "generation_evidence_bundle_readbacks",
        "candidate_generation_receipts",
        "candidate_generation_attempt_records",
    ] {
        assert_eq!(row_count(store, table), 0, "{table} must roll back");
    }
}

#[path = "tests/ledger.rs"]
mod ledger;

fn row_count(store: &ArtifactStateStore, table: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    store
        .connection()
        .query_row(&sql, [], |row| row.get(0))
        .expect("count terminal rows")
}
