use super::*;
use crate::store::{
    candidate_generation_execution::tests::support,
    generation_qualification_preregistration::tests::support as prereg_support,
};

#[test]
fn complete_mixed_system_ledger_is_atomic_cold_readable_and_idempotent() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let target = support::completed_at(&fixture, 0);
    let baseline = support::completed_at(&fixture, 1);
    let path = directory.path().join("ledger.db");
    let mut store = support::prepared_store(&path, &fixture);
    for completed in [&target, &baseline] {
        support::checkpoint_precursor(&mut store, &fixture, &completed.precursor);
        store
            .transact_candidate_generation_execution_v1(
                support::completed_input(&fixture, completed),
                || Ok::<_, ()>(()),
                |_| Ok::<_, ()>(()),
            )
            .expect("real completed parent cohort");
    }
    let facts = [target.managed_input];
    let manifest = store
        .rederive_attempt_ledger_manifest_v1(support::read_input(&fixture), &facts)
        .expect("mixed target and baseline parents");
    let input = GenerationAttemptLedgerV1Input {
        preregistration: support::read_input(&fixture),
        managed_evidence_inputs: &facts,
        manifest: &manifest,
    };
    let before = store.connection.total_changes();
    assert_eq!(
        store
            .generation_attempt_ledger_v1(input)
            .expect("absent read"),
        None
    );
    assert_eq!(store.connection.total_changes(), before);
    let mut gates = 0;
    assert!(matches!(
        store.transact_generation_attempt_ledger_v1(input, || {
            gates += 1;
            if gates == 3 { Err(()) } else { Ok(()) }
        }),
        Err(GenerationAttemptLedgerV1TransactionError::Gate(()))
    ));
    assert_eq!(
        store
            .generation_attempt_ledger_v1(input)
            .expect("rolled back read"),
        None
    );
    assert_eq!(
        store
            .transact_generation_attempt_ledger_v1(input, || Ok::<_, ()>(()))
            .expect("published ledger"),
        WriteDisposition::Inserted
    );
    drop(store);
    let mut reopened = ArtifactStateStore::open(&path).expect("cold reopened store");
    assert_eq!(
        reopened
            .generation_attempt_ledger_v1(input)
            .expect("fresh reconstructed read"),
        Some(manifest.clone())
    );
    assert_eq!(
        reopened
            .transact_generation_attempt_ledger_v1(input, || Ok::<_, ()>(()))
            .expect("immutable replay"),
        WriteDisposition::AlreadyPresent
    );
    reopened
        .connection
        .execute(
            "UPDATE generation_attempt_ledger_manifests SET canonical_json = ?1",
            [b"{}".as_slice()],
        )
        .expect("corrupt fixture row");
    assert!(matches!(
        reopened.generation_attempt_ledger_v1(input),
        Err(StoreError::CorruptRecord)
    ));
    assert!(matches!(
        reopened.transact_generation_attempt_ledger_v1(input, || Ok::<_, ()>(())),
        Err(GenerationAttemptLedgerV1TransactionError::Store(
            StoreError::CorruptRecord
        ))
    ));
}

#[test]
fn missing_substituted_and_immutable_conflicting_ledgers_cannot_publish() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let fixture = prereg_support::fixture();
    let completed = support::completed(&fixture);
    let mut store = support::prepared_store(&directory.path().join("ledger.db"), &fixture);
    let skipped = store
        .rederive_attempt_ledger_manifest_v1(support::read_input(&fixture), &[])
        .expect("skipped ledger");
    let facts = [completed.managed_input.clone()];
    let input = GenerationAttemptLedgerV1Input {
        preregistration: support::read_input(&fixture),
        managed_evidence_inputs: &facts,
        manifest: &skipped,
    };
    assert!(
        store
            .transact_generation_attempt_ledger_v1(input, || Ok::<_, ()>(()))
            .is_err()
    );
    support::checkpoint_precursor(&mut store, &fixture, &completed.precursor);
    store
        .transact_candidate_generation_execution_v1(
            support::completed_input(&fixture, &completed),
            || Ok::<_, ()>(()),
            |_| Ok::<_, ()>(()),
        )
        .expect("completed parents");
    assert!(
        store
            .transact_generation_attempt_ledger_v1(input, || Ok::<_, ()>(()))
            .is_err()
    );
    let passed = store
        .rederive_attempt_ledger_manifest_v1(support::read_input(&fixture), &facts)
        .expect("Passed ledger");
    let input = GenerationAttemptLedgerV1Input {
        manifest: &passed,
        ..input
    };
    store.connection.execute(
        "INSERT INTO generation_attempt_ledger_manifests (generation_attempt_ledger_manifest_id, generation_system_id, generation_qualification_plan_id, generation_suite_manifest_id, evidence_item_count, status, canonical_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![skipped.attempt_ledger_manifest_id().digest().as_str(), skipped.generation_system_id().digest().as_str(), skipped.generation_qualification_plan_id().digest().as_str(), skipped.suite_manifest_id().digest().as_str(), 0, "skipped", serde_json::to_vec(&skipped).expect("canonical fixture")],
    ).expect("conflicting inert fixture");
    assert!(
        store
            .transact_generation_attempt_ledger_v1(input, || Ok::<_, ()>(()))
            .is_err()
    );
    assert_eq!(
        store
            .connection
            .query_row(
                "SELECT count(*) FROM generation_attempt_ledger_manifests",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("ledger count"),
        1
    );
}
