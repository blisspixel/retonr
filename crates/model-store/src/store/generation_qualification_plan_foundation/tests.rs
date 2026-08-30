use std::{sync::Arc, thread};

use rewrite_model::{
    GenerationQualificationPlanV1, GenerationQualificationPlanV1Input,
    GenerationRepetitionRecordV1, MAX_GENERATION_PLAN_JSON_BYTES, PlannedCandidateAttemptV1,
    PlannedCandidateAttemptV1Input, PlannedCandidateAttemptV1Relations,
};
use rewrite_types::Digest;
use rusqlite::Connection;
use tempfile::tempdir;

use super::*;
use crate::GenerationSystemFoundationV1Input;
use crate::store::generation_qualification_preregistration::tests::support::{self, Fixture};

fn input(fixture: &Fixture) -> GenerationQualificationPlanFoundationV1Input<'_> {
    GenerationQualificationPlanFoundationV1Input {
        clusters: &fixture.clusters,
        deterministic_case_contracts: &fixture.deterministic_case_contracts,
        cases: &fixture.cases,
        suite: &fixture.suite,
        repetitions: &fixture.repetitions,
        generation_systems: &fixture.systems,
        planned_attempts: &fixture.attempts,
        candidate_selection_policy: &fixture.selection_policy,
        plan: &fixture.plan,
    }
}

fn persist_system_foundations(store: &mut ArtifactStateStore, fixture: &Fixture) {
    store
        .put_artifact_set_manifest(&fixture.system.runtime_set)
        .expect("runtime artifact set");
    store
        .put_runtime_package_manifest(&fixture.system.runtime_package)
        .expect("runtime package");
    store
        .put_runtime_build_identity(&fixture.system.runtime_build)
        .expect("runtime build");
    store
        .put_effective_runtime_state(&fixture.system.runtime_state)
        .expect("runtime state");
    store
        .put_artifact_set_manifest(&fixture.system.model_set)
        .expect("model artifact set");
    store
        .put_model_package_manifest(&fixture.system.model_package)
        .expect("model package");
    for system in &fixture.systems {
        store
            .transact_generation_system_foundation_v1(
                GenerationSystemFoundationV1Input {
                    generation_system: system,
                    relations: fixture.system.relations(),
                },
                |_| Ok::<_, ()>(()),
            )
            .expect("generation-system foundation");
    }
}

#[test]
fn atomic_foundation_round_trips_cold_and_is_idempotent() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("plan-foundation.db");
    let fixture = support::fixture();
    let plan_id = fixture.plan.qualification_plan_id().clone();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    persist_system_foundations(&mut store, &fixture);

    let (validated, first) = store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |readback| {
            assert_eq!(readback.foundation().clusters(), fixture.clusters);
            assert_eq!(
                readback.foundation().deterministic_case_contracts(),
                fixture.deterministic_case_contracts
            );
            assert_eq!(readback.foundation().cases(), fixture.cases);
            assert_eq!(readback.foundation().suite(), &fixture.suite);
            assert_eq!(readback.foundation().repetitions(), fixture.repetitions);
            assert_eq!(readback.foundation().generation_systems(), fixture.systems);
            assert_eq!(readback.foundation().planned_attempts(), fixture.attempts);
            assert_eq!(
                readback.foundation().candidate_selection_policy(),
                &fixture.selection_policy
            );
            assert_eq!(readback.foundation().plan(), &fixture.plan);
            Ok::<_, ()>("validated")
        })
        .expect("insert plan foundation");
    assert_eq!(validated, "validated");
    assert_eq!(first.plan, WriteDisposition::Inserted);

    let ((), repeated) = store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| Ok::<_, ()>(()))
        .expect("repeat plan foundation");
    assert_eq!(repeated.plan, WriteDisposition::AlreadyPresent);
    drop(store);

    let reopened = ArtifactStateStore::open_existing_read_only(&path).expect("cold reopen");
    let stored = reopened
        .generation_qualification_plan_foundation_v1(&plan_id)
        .expect("cold read")
        .expect("plan foundation present");
    assert_eq!(stored.clusters(), fixture.clusters);
    assert_eq!(
        stored.deterministic_case_contracts(),
        fixture.deterministic_case_contracts
    );
    assert_eq!(stored.cases(), fixture.cases);
    assert_eq!(stored.suite(), &fixture.suite);
    assert_eq!(stored.repetitions(), fixture.repetitions);
    assert_eq!(stored.generation_systems(), fixture.systems);
    assert_eq!(stored.planned_attempts(), fixture.attempts);
    assert_eq!(
        stored.candidate_selection_policy(),
        &fixture.selection_policy
    );
    assert_eq!(stored.plan(), &fixture.plan);
}

#[test]
fn missing_generation_system_rejects_without_partial_rows() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store =
        ArtifactStateStore::open(&directory.path().join("missing.db")).expect("open store");

    let result = store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(
            GenerationQualificationPlanFoundationV1TransactionError::Store(
                StoreError::MissingRecord
            )
        )
    ));
    assert_eq!(foundation_row_count(store.connection()), 0);
}

#[test]
fn callback_rejection_rolls_back_every_plan_row() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store =
        ArtifactStateStore::open(&directory.path().join("rollback.db")).expect("open store");
    persist_system_foundations(&mut store, &fixture);

    let result = store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| {
            Err::<(), _>("sensitive callback detail")
        });
    assert!(matches!(
        &result,
        Err(GenerationQualificationPlanFoundationV1TransactionError::Validation(_))
    ));
    assert_eq!(foundation_row_count(store.connection()), 0);
    let error = result.expect_err("callback must reject");
    assert_eq!(
        format!("{error:?}"),
        "GenerationQualificationPlanFoundationV1TransactionError::Validation"
    );
    assert!(!format!("{error}").contains("sensitive"));
}

#[test]
fn conflicting_cluster_rolls_back_every_new_plan_row() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store =
        ArtifactStateStore::open(&directory.path().join("conflict.db")).expect("open store");
    persist_system_foundations(&mut store, &fixture);
    store
        .connection()
        .execute(
            "INSERT INTO generation_cluster_records VALUES (?1, X'7B7D')",
            [fixture.clusters[0].cluster_id().digest().as_str()],
        )
        .expect("seed conflicting cluster");

    let result = store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(
            GenerationQualificationPlanFoundationV1TransactionError::Store(
                StoreError::ImmutableConflict
            )
        )
    ));
    assert_eq!(foundation_row_count(store.connection()), 1);
}

#[test]
fn retry_refuses_to_heal_an_existing_plan_with_an_association_gap() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store =
        ArtifactStateStore::open(&directory.path().join("gap-retry.db")).expect("open store");
    persist_system_foundations(&mut store, &fixture);
    store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| Ok::<_, ()>(()))
        .expect("store plan foundation");
    store
        .connection()
        .pragma_update(None, "foreign_keys", false)
        .expect("disable foreign keys for corruption fixture");
    store
        .connection()
        .execute(
            "DELETE FROM generation_qualification_plan_attempts WHERE plan_ordinal = 1",
            [],
        )
        .expect("remove plan association");

    let result = store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(
            GenerationQualificationPlanFoundationV1TransactionError::Store(
                StoreError::CorruptRecord
            )
        )
    ));
    let remaining: i64 = store
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM generation_qualification_plan_attempts",
            [],
            |row| row.get(0),
        )
        .expect("count remaining associations");
    assert_eq!(remaining, 1);
}

#[test]
fn distinct_repetition_policies_can_share_one_suite_and_ordinal() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let mut store = ArtifactStateStore::open(&directory.path().join("repetition-policies.db"))
        .expect("open store");
    persist_system_foundations(&mut store, &fixture);
    store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| Ok::<_, ()>(()))
        .expect("store first plan");

    let repetitions = vec![
        GenerationRepetitionRecordV1::new(
            &fixture.suite,
            0,
            Digest::sha256(b"alternate repetition policy"),
        )
        .expect("alternate repetition"),
    ];
    let attempts = fixture
        .attempts
        .iter()
        .map(|attempt| {
            let system = fixture
                .systems
                .iter()
                .find(|system| system.generation_system_id() == attempt.generation_system_id())
                .expect("attempt system");
            PlannedCandidateAttemptV1::new(
                PlannedCandidateAttemptV1Relations {
                    suite: &fixture.suite,
                    case: &fixture.cases[0],
                    cluster: &fixture.clusters[0],
                    repetition: &repetitions[0],
                    generation_system: system,
                },
                PlannedCandidateAttemptV1Input {
                    attempt_ordinal: attempt.attempt_ordinal(),
                    declared_seed: attempt.declared_seed(),
                    grounded_request_digest: attempt.grounded_request_digest().clone(),
                    generation_request_binding_id: attempt.generation_request_binding_id().clone(),
                    candidate_output_contract_digest: attempt
                        .candidate_output_contract_digest()
                        .clone(),
                    output_ceilings: attempt.output_ceilings(),
                },
            )
            .expect("alternate planned attempt")
        })
        .collect::<Vec<_>>();
    let plan = GenerationQualificationPlanV1::new(
        &fixture.suite,
        &repetitions,
        &fixture.systems,
        &attempts,
        GenerationQualificationPlanV1Input {
            limits: fixture.plan.limits(),
            selection_policy_digest: fixture.plan.selection_policy_digest().clone(),
            failure_policy_digest: fixture.plan.failure_policy_digest().clone(),
        },
    )
    .expect("alternate plan");
    let alternate = GenerationQualificationPlanFoundationV1Input {
        clusters: &fixture.clusters,
        deterministic_case_contracts: &fixture.deterministic_case_contracts,
        cases: &fixture.cases,
        suite: &fixture.suite,
        repetitions: &repetitions,
        generation_systems: &fixture.systems,
        planned_attempts: &attempts,
        candidate_selection_policy: &fixture.selection_policy,
        plan: &plan,
    };
    store
        .transact_generation_qualification_plan_foundation_v1(alternate, |_| Ok::<_, ()>(()))
        .expect("store alternate plan");
    assert!(
        store
            .generation_qualification_plan_foundation_v1(fixture.plan.qualification_plan_id())
            .expect("read first plan")
            .is_some()
    );
    assert!(
        store
            .generation_qualification_plan_foundation_v1(plan.qualification_plan_id())
            .expect("read alternate plan")
            .is_some()
    );
}

#[test]
fn collection_bounds_fail_before_any_transaction_rows() {
    let directory = tempdir().expect("temporary directory");
    let fixture = support::fixture();
    let cases = vec![fixture.cases[0].clone(); 257];
    let contracts = vec![fixture.deterministic_case_contracts[0].clone(); 257];
    let mut store = ArtifactStateStore::open(&directory.path().join("collection-bound.db"))
        .expect("open store");
    let oversized = GenerationQualificationPlanFoundationV1Input {
        clusters: &fixture.clusters,
        deterministic_case_contracts: &contracts,
        cases: &cases,
        suite: &fixture.suite,
        repetitions: &fixture.repetitions,
        generation_systems: &fixture.systems,
        planned_attempts: &fixture.attempts,
        candidate_selection_policy: &fixture.selection_policy,
        plan: &fixture.plan,
    };
    assert!(matches!(
        store.transact_generation_qualification_plan_foundation_v1(oversized, |_| {
            Ok::<_, ()>(())
        }),
        Err(
            GenerationQualificationPlanFoundationV1TransactionError::Store(
                StoreError::InvalidGenerationQualificationPlan(_)
            )
        )
    ));
    assert_eq!(foundation_row_count(store.connection()), 0);
}

#[test]
fn cold_reads_reject_missing_gapped_and_mismatched_dependencies() {
    for (name, tamper) in [
        (
            "missing-case-order",
            "DELETE FROM generation_suite_cases WHERE semantic_ordinal = 0",
        ),
        (
            "gapped-attempt-order",
            "UPDATE generation_qualification_plan_attempts SET plan_ordinal = 7
             WHERE plan_ordinal = 1",
        ),
        (
            "mismatched-attempt-index",
            "UPDATE planned_candidate_attempts SET attempt_ordinal = 7
             WHERE attempt_ordinal = 1",
        ),
    ] {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join(format!("{name}.db"));
        let fixture = support::fixture();
        let plan_id = fixture.plan.qualification_plan_id().clone();
        let mut store = ArtifactStateStore::open(&path).expect("open store");
        persist_system_foundations(&mut store, &fixture);
        store
            .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| {
                Ok::<_, ()>(())
            })
            .expect("store plan foundation");
        store
            .connection()
            .pragma_update(None, "foreign_keys", false)
            .expect("disable foreign keys for corruption fixture");
        store
            .connection()
            .execute_batch(tamper)
            .expect("tamper plan foundation");
        drop(store);

        let reopened = ArtifactStateStore::open_existing_read_only(&path).expect("cold reopen");
        assert!(matches!(
            reopened.generation_qualification_plan_foundation_v1(&plan_id),
            Err(StoreError::CorruptRecord)
        ));
    }
}

#[test]
fn strict_storage_and_cold_blob_bounds_fail_closed() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("bounded.db");
    let fixture = support::fixture();
    let plan_id = fixture.plan.qualification_plan_id().clone();
    let mut store = ArtifactStateStore::open(&path).expect("open store");
    persist_system_foundations(&mut store, &fixture);
    store
        .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| Ok::<_, ()>(()))
        .expect("store plan foundation");
    assert!(
        store
            .connection()
            .execute(
                "UPDATE generation_qualification_plans
                 SET canonical_json = CAST('{}' AS TEXT)",
                [],
            )
            .is_err()
    );
    store
        .connection()
        .pragma_update(None, "ignore_check_constraints", true)
        .expect("disable check constraints for corruption fixture");
    store
        .connection()
        .execute(
            "UPDATE generation_qualification_plans SET canonical_json = zeroblob(?1)",
            [i64::try_from(MAX_GENERATION_PLAN_JSON_BYTES + 1).expect("bound fits i64")],
        )
        .expect("store oversized plan");
    drop(store);

    let reopened = ArtifactStateStore::open_existing_read_only(&path).expect("cold reopen");
    assert!(matches!(
        reopened.generation_qualification_plan_foundation_v1(&plan_id),
        Err(StoreError::CorruptRecord)
    ));
}

#[test]
fn concurrent_exact_writers_converge_without_duplicate_rows() {
    let directory = tempdir().expect("temporary directory");
    let path = Arc::new(directory.path().join("concurrent.db"));
    let fixture = support::fixture();
    let mut initial = ArtifactStateStore::open(path.as_ref()).expect("open initial store");
    persist_system_foundations(&mut initial, &fixture);
    drop(initial);

    let workers = (0..2)
        .map(|_| {
            let path = Arc::clone(&path);
            thread::spawn(move || {
                let fixture = support::fixture();
                let mut store = ArtifactStateStore::open_existing_writable_exact(path.as_ref())
                    .expect("open writer");
                store
                    .transact_generation_qualification_plan_foundation_v1(input(&fixture), |_| {
                        Ok::<_, ()>(())
                    })
                    .expect("concurrent transaction")
                    .1
                    .plan
            })
        })
        .collect::<Vec<_>>();
    let dispositions = workers
        .into_iter()
        .map(|worker| worker.join().expect("join writer"))
        .collect::<Vec<_>>();
    assert!(dispositions.contains(&WriteDisposition::Inserted));
    assert!(dispositions.contains(&WriteDisposition::AlreadyPresent));

    let store = ArtifactStateStore::open_existing_read_only(path.as_ref()).expect("cold reopen");
    assert_eq!(foundation_row_count(store.connection()), 15);
}

fn foundation_row_count(connection: &Connection) -> i64 {
    const TABLES: [&str; 12] = [
        "generation_cluster_records",
        "generation_deterministic_case_contracts",
        "generation_case_manifests",
        "generation_suite_manifests",
        "generation_suite_cases",
        "generation_repetition_records",
        "planned_candidate_attempts",
        "candidate_selection_policies",
        "generation_qualification_plans",
        "generation_qualification_plan_repetitions",
        "generation_qualification_plan_systems",
        "generation_qualification_plan_attempts",
    ];
    TABLES
        .iter()
        .map(|table| {
            connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("count plan-foundation rows")
        })
        .sum()
}
