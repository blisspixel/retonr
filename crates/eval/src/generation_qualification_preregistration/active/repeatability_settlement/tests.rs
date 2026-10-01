//! Synthetic live adapters exercise real durable Passed closure publication.

use super::*;
use crate::{
    CompletePassedRepeatabilityRelations, VerifiedCandidateJudgeJoin,
    verify_complete_passed_repeatability_joins, verify_passed_repeatability_joins,
};
use rewrite_model::{
    GenerationAttemptLedgerManifestV1Relations, GenerationQualificationPhaseScopeV1,
    GenerationQualificationPhaseStatusV1, GenerationRepeatabilityResultRecordV1Relations,
    GenerationRepeatabilityTerminalStageV1,
};

pub(crate) fn exercise(
    active: &mut ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    repository: &mut GenerationQualificationPreregistrationRepository,
    join: VerifiedCandidateJudgeJoin<'_, '_, '_, '_>,
    database: &std::path::Path,
    failure: &str,
    cancellation: &CancellationToken,
    failure_control: &crate::verified_candidate_batch_set::tests::support::OfflineBatchFailureControl,
) {
    let (mut complete, result) = complete_fixture(active, join, cancellation);
    let connection = rusqlite::Connection::open(database).expect("fixture state");
    configure_failure(
        active,
        &connection,
        database,
        failure,
        cancellation,
        failure_control,
    );
    let stored =
        active.persist_complete_passed_repeatability(repository, &mut complete, cancellation);
    if !matches!(
        failure,
        "repeatability_none"
            | "repeatability_result_corruption"
            | "repeatability_manifest_corruption"
    ) {
        assert!(stored.is_err(), "{failure}");
        if failure == "repeatability_postcommit_cancel" {
            assert_eq!(
                stored,
                Err(ActiveGenerationQualificationRepeatabilitySettlementError::Cancelled)
            );
        }
        if failure == "repeatability_postcommit_finalization" {
            assert_eq!(stored, Err(ActiveGenerationQualificationRepeatabilitySettlementError::PrimaryAndFinalization));
        }
        assert_eq!(active.terminal, failure != "repeatability_unsettled");
        let committed_results = failure.starts_with("repeatability_postcommit_")
            || matches!(
                failure,
                "repeatability_manifest_abort" | "repeatability_manifest_tamper"
            );
        assert_eq!(
            row_count(
                &connection,
                "generation_repeatability_terminal_result_records"
            ),
            i64::from(committed_results)
        );
        assert_eq!(
            row_count(&connection, "generation_attempt_ledger_manifests"),
            i64::from(committed_results || failure == "repeatability_abort")
        );
        assert_eq!(
            row_count(&connection, "generation_repeatability_evidence_manifests"),
            i64::from(failure.starts_with("repeatability_postcommit_"))
        );
        if failure == "repeatability_manifest_abort" {
            connection
                .execute_batch("DROP TRIGGER abort_phase;")
                .expect("remove fixture abort");
            phase_store::exercise(active, &mut complete, database, cancellation);
        }
        return;
    }
    assert_eq!(
        stored.expect("real Passed result publication"),
        vec![(result.clone(), WriteDisposition::Inserted)]
    );
    let mut reopened = GenerationQualificationPreregistrationRepository::open(database)
        .expect("cold reopened repository");
    assert_eq!(
        active
            .persist_complete_passed_repeatability(&mut reopened, &mut complete, cancellation)
            .expect("exact live replay"),
        vec![(result, WriteDisposition::AlreadyPresent)]
    );
    assert_eq!(
        row_count(&connection, "generation_repeatability_evidence_manifests"),
        1
    );
    let corruption_sql = if failure == "repeatability_manifest_corruption" {
        "UPDATE generation_repeatability_evidence_manifests SET canonical_json = ?1"
    } else if failure == "repeatability_result_corruption" {
        "UPDATE generation_repeatability_terminal_result_records SET canonical_json = ?1"
    } else {
        "UPDATE generation_attempt_ledger_manifests SET canonical_json = ?1"
    };
    connection
        .execute(corruption_sql, [b"{}".as_slice()])
        .expect("corrupt ledger");
    assert!(
        active
            .persist_complete_passed_repeatability(&mut reopened, &mut complete, cancellation)
            .is_err()
    );
    assert!(active.terminal);
}

pub(crate) fn complete_fixture<'store, 'records, 'model, 'runtime>(
    active: &ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    mut join: VerifiedCandidateJudgeJoin<'store, 'records, 'model, 'runtime>,
    cancellation: &CancellationToken,
) -> (
    VerifiedCompletePassedRepeatabilityJoins<'store, 'records, 'model, 'runtime>,
    GenerationRepeatabilityResultRecordV1,
) {
    let foundation = active.prepared.plan_foundation().clone();
    let policy = active.operation_policy().clone();
    let target = foundation
        .generation_systems()
        .iter()
        .find(|system| system.generation_system_id() == policy.target_generation_system_id())
        .expect("target");
    let scope = GenerationQualificationPhaseScopeV1 {
        generation_system: target,
        qualification_plan: foundation.plan(),
        suite: foundation.suite(),
    };
    let ledger = active
        .attempt_ledger
        .as_ref()
        .expect("sealed target ledger");
    let result = join
        .with_settlement_view(cancellation, |view| {
            GenerationRepeatabilityResultRecordV1::new(
                GenerationRepeatabilityResultRecordV1Relations {
                    scope,
                    repetition: &foundation.repetitions()[0],
                    attempt_ledger: ledger.manifest(),
                    attempt_ledger_relations: GenerationAttemptLedgerManifestV1Relations {
                        scope,
                        phase_policy_digest: policy.attempt_ledger_policy_digest(),
                        planned_attempts: foundation.planned_attempts(),
                        attempt_records: ledger.target_attempt_records(),
                        status: GenerationQualificationPhaseStatusV1::Passed,
                    },
                    terminal_stage: GenerationRepeatabilityTerminalStageV1::Passed,
                    candidate_receipt_set: Some(view.input.candidate_a_receipt_set),
                    deterministic_evaluation: Some(view.input.deterministic_evaluation),
                    candidate_judge_join: Some(view.input.join),
                    terminal_evidence_digest: &rewrite_types::Digest::sha256(
                        b"synthetic complete closure",
                    ),
                },
            )
        })
        .expect("live projection")
        .expect("recursively checked result");
    let ordered = vec![result.clone()];
    let joins = verify_passed_repeatability_joins(
        target.generation_system_id(),
        &ordered,
        vec![join],
        cancellation,
    )
    .expect("live Passed joins");
    let complete = verify_complete_passed_repeatability_joins(
        CompletePassedRepeatabilityRelations {
            operation_policy: &policy,
            scope,
            planned_attempts: foundation.planned_attempts(),
            attempt_ledger_manifest: ledger.manifest(),
            preregistered_repetitions: foundation.repetitions(),
            ordered_results: &ordered,
        },
        joins,
        cancellation,
    )
    .expect("complete retained authority");
    (complete, result)
}

fn configure_failure(
    active: &mut ActiveGenerationQualificationOperation<'_, '_, '_, '_, '_>,
    connection: &rusqlite::Connection,
    database: &std::path::Path,
    failure: &str,
    cancellation: &CancellationToken,
    failure_control: &crate::verified_candidate_batch_set::tests::support::OfflineBatchFailureControl,
) {
    match failure {
        "repeatability_manifest_abort" => {
            connection.execute_batch("CREATE TRIGGER abort_phase BEFORE INSERT ON generation_repeatability_evidence_manifests BEGIN SELECT RAISE(ABORT, 'synthetic phase failure'); END;").expect("phase abort fixture");
        }
        "repeatability_manifest_tamper" => {
            connection.execute_batch("CREATE TRIGGER tamper_phase AFTER INSERT ON generation_repeatability_evidence_manifests BEGIN UPDATE generation_repeatability_evidence_manifests SET canonical_json = X'7b7d'; END;").expect("phase readback tamper fixture");
        }
        "repeatability_missing" => {
            connection
                .execute("DELETE FROM candidate_deterministic_evaluation_records", [])
                .expect("remove real parent");
        }
        "repeatability_corrupt_parent" => {
            connection
                .execute(
                    "UPDATE candidate_judge_response_aggregates SET canonical_json = ?1",
                    [b"{}".as_slice()],
                )
                .expect("corrupt real parent");
        }
        "repeatability_foreign" => {
            active.subject = crate::active_generation_qualification_subject::ActiveGenerationQualificationSubject::new();
        }
        "repeatability_unsettled" => {
            active.next_judge_settlement = 0;
        }
        "repeatability_wrong_execution" => {
            active.executed_judge_joins[0] = serde_json::from_str(&format!(
                "\"{}\"",
                rewrite_types::Digest::sha256(b"wrong consumed execution")
            ))
            .expect("foreign inert execution identity");
        }
        "repeatability_substituted_execution" => {
            active.executed_judge_joins.clear();
        }
        "repeatability_abort" => {
            connection.execute_batch("CREATE TRIGGER abort_repeatability BEFORE INSERT ON generation_repeatability_terminal_result_records BEGIN SELECT RAISE(ABORT, 'synthetic result failure'); END;").expect("abort fixture");
        }
        "repeatability_cancel" => cancellation.cancel(),
        _ => {}
    }
    if matches!(
        failure,
        "repeatability_postcommit_finalization" | "repeatability_postcommit_cancel"
    ) {
        let database = database.to_owned();
        let cancellation = cancellation.clone();
        let cancel = failure == "repeatability_postcommit_cancel";
        failure_control.fail_when(move || {
            let count = rusqlite::Connection::open(&database)
                .expect("finalizer fixture connection")
                .query_row(
                    "SELECT count(*) FROM generation_repeatability_terminal_result_records",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .expect("published result count");
            if count == 0 {
                return false;
            }
            if cancel {
                cancellation.cancel();
                false
            } else {
                true
            }
        });
    }
}

fn row_count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("fixture row count")
}

mod phase_store;

#[test]
fn terminal_gate_preserves_deadline_priority_and_independent_finalization() {
    use ActiveGenerationQualificationRepeatabilitySettlementError as Error;
    for primary in [Ok(7), Err(Error::OperationScope)] {
        assert_eq!(
            finish(primary, true, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineAndFinalization)
        );
        assert_eq!(
            finish(primary, true, Err(Error::Cancelled)),
            Err(Error::CancelledAndFinalization)
        );
        assert_eq!(
            finish(primary, false, Err(Error::DeadlineExceeded)),
            Err(Error::DeadlineExceeded)
        );
        assert_eq!(
            finish(primary, false, Err(Error::Cancelled)),
            Err(Error::Cancelled)
        );
    }
    assert_eq!(
        finish(Ok(7), true, Ok(())),
        Err(Error::MandatoryFinalization)
    );
    assert_eq!(
        finish::<u8>(Err(Error::Publication), true, Ok(())),
        Err(Error::PrimaryAndFinalization)
    );
    assert_eq!(finish(Ok(7), false, Ok(())), Ok(7));
    assert_eq!(
        map_preparation_error(GenerationQualificationPreparationError::Cancelled),
        Error::Cancelled
    );
    assert_eq!(
        map_preparation_error(GenerationQualificationPreparationError::DeadlineExceeded),
        Error::DeadlineExceeded
    );
}
