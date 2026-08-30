use rewrite_model::{GenerationQualificationPlanId, PlannedCandidateAttemptId};
use rewrite_types::Digest;
use rusqlite::params;
use serde::de::DeserializeOwned;

use super::CandidateGenerationExecutionV1State;
use crate::{ArtifactStateStore, StoreError};

#[test]
fn missing_attempt_has_no_terminal_state() {
    let fixture = StoreFixture::open("missing");
    assert_eq!(
        fixture
            .store
            .candidate_generation_execution_v1_state(&id("plan"), &id("attempt"))
            .expect("bounded state query"),
        CandidateGenerationExecutionV1State::Missing
    );
}

#[test]
fn failed_and_completed_states_do_not_decode_canonical_json() {
    let fixture = StoreFixture::open("terminal-states");
    let store = &fixture.store;
    let plan_id = id("plan");
    let failed_attempt_id = id("failed-attempt");
    let completed_attempt_id = id("completed-attempt");
    let precursor_id = digest("precursor");
    let receipt_id = digest("receipt");
    disable_foreign_keys(store);
    insert_attempt(store, &plan_id, &failed_attempt_id, "failed", None, None);
    insert_attempt(
        store,
        &plan_id,
        &completed_attempt_id,
        "completed",
        Some(&precursor_id),
        Some(&receipt_id),
    );

    assert_eq!(
        store
            .candidate_generation_execution_v1_state(&plan_id, &failed_attempt_id)
            .expect("failed state"),
        CandidateGenerationExecutionV1State::Failed
    );
    assert_eq!(
        store
            .candidate_generation_execution_v1_state(&plan_id, &completed_attempt_id)
            .expect("completed state"),
        CandidateGenerationExecutionV1State::Completed
    );
}

#[test]
fn unexpected_indexed_outcome_is_corrupt() {
    let fixture = StoreFixture::open("invalid-outcome");
    let store = &fixture.store;
    let plan_id = id("plan");
    let attempt_id = id("attempt");
    disable_foreign_keys(store);
    store
        .connection
        .execute_batch("PRAGMA ignore_check_constraints = ON;")
        .expect("permit corruption fixture");
    insert_attempt(store, &plan_id, &attempt_id, "unknown", None, None);

    assert!(matches!(
        store.candidate_generation_execution_v1_state(&plan_id, &attempt_id),
        Err(StoreError::CorruptRecord)
    ));
}

struct StoreFixture {
    store: ArtifactStateStore,
    _directory: tempfile::TempDir,
}

impl StoreFixture {
    fn open(label: &str) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join(format!("{label}.db"));
        let store = ArtifactStateStore::open(&path).expect("open current store");
        Self {
            store,
            _directory: directory,
        }
    }
}

fn disable_foreign_keys(store: &ArtifactStateStore) {
    store
        .connection
        .execute_batch("PRAGMA foreign_keys = OFF;")
        .expect("disable fixture foreign keys");
}

fn insert_attempt(
    store: &ArtifactStateStore,
    plan_id: &GenerationQualificationPlanId,
    attempt_id: &PlannedCandidateAttemptId,
    outcome: &str,
    precursor_id: Option<&str>,
    receipt_id: Option<&str>,
) {
    store
        .connection
        .execute(
            "INSERT INTO candidate_generation_attempt_records (
                 candidate_generation_attempt_record_id,
                 generation_qualification_plan_id,
                 planned_candidate_attempt_id,
                 outcome,
                 candidate_generation_attempt_precursor_id,
                 candidate_generation_receipt_id,
                 canonical_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                digest(attempt_id.digest().as_str()),
                plan_id.digest().as_str(),
                attempt_id.digest().as_str(),
                outcome,
                precursor_id,
                receipt_id,
                [0xff_u8].as_slice(),
            ],
        )
        .expect("insert terminal-state fixture");
}

fn id<T: DeserializeOwned>(label: &str) -> T {
    serde_json::from_value(serde_json::Value::String(digest(label))).expect("typed digest ID")
}

fn digest(label: &str) -> String {
    Digest::sha256(label.as_bytes()).as_str().to_owned()
}
